//! 90-second uplifting-trance track rendered to a WAV file.
//!
//! 138 BPM, A minor, Am–F–C–G (i–VI–III–VII), 52 bars ≈ 90.4 s.
//!
//! | Bars   | Section      | Notes                                             |
//! |--------|--------------|---------------------------------------------------|
//! | 0–7    | INTRO        | Reverbed pad + sparse offbeat closed hats         |
//! | 8–15   | BUILD        | + kick, rolling 16th bass, closed hats every 16th |
//! | 16–31  | DROP         | Full mix: 7-voice supersaw lead with filter sweep |
//! | 32–39  | BREAKDOWN    | Pad + arp only, no drums                          |
//! | 40–45  | FINAL BUILD  | Snare roll, noise riser climbs                    |
//! | 46–51  | FINAL DROP   | Full mix, louder lead, steeper filter sweep       |
//!
//! Refinements over `trance.rs` (live-playback version):
//!   - 7-voice supersaw (±6/±12/±18 cent detune stack)
//!   - Open hat on every 8th offbeat (steps 2, 6, 10, 14), not step 7
//!   - Sidechain pump on pad/bass/lead, keyed to kick hits
//!   - One-pole low-pass sweep that opens across each drop
//!   - Offline render to `target/trance.wav` — no audio hardware required
//!
//! Run: cargo run -p nyx-prelude --example trance_wav --release

use nyx_prelude::*;

const BPM: f32 = 138.0;
const SAMPLE_RATE: f32 = 44100.0;
const DURATION_SECS: f32 = 90.4;

#[derive(Copy, Clone, PartialEq, Eq)]
enum Section {
    Intro,
    Build,
    Drop,
    Breakdown,
    FinalBuild,
    FinalDrop,
}

fn section_for(bar: i32) -> Section {
    match bar {
        0..=7 => Section::Intro,
        8..=15 => Section::Build,
        16..=31 => Section::Drop,
        32..=39 => Section::Breakdown,
        40..=45 => Section::FinalBuild,
        _ => Section::FinalDrop,
    }
}

fn main() {
    let mut clk = clock::clock(BPM);
    let mut kick = inst::kick();
    let mut snare = inst::snare();
    let mut hat_c = inst::hihat(false);
    let mut hat_o = inst::hihat(true);

    // Am – F – C – G: MIDI roots A2, F2, C2, G2.
    let chord_roots: [u8; 4] = [33, 29, 24, 31];
    // (root, 3rd, 5th, octave) intervals — Am is minor, F/C/G are major.
    let chord_intervals: [[u8; 4]; 4] = [
        [0, 3, 7, 12],
        [0, 4, 7, 12],
        [0, 4, 7, 12],
        [0, 4, 7, 12],
    ];

    let pad_params: [OscParam; 4] = [
        OscParam::new(220.0),
        OscParam::new(261.63),
        OscParam::new(329.63),
        OscParam::new(440.0),
    ];
    let pad_writers: [OscParamWriter; 4] = [
        pad_params[0].writer(),
        pad_params[1].writer(),
        pad_params[2].writer(),
        pad_params[3].writer(),
    ];
    let pad_voices = osc::sine(pad_params[0].signal(30.0))
        .add(osc::sine(pad_params[1].signal(30.0)))
        .add(osc::sine(pad_params[2].signal(30.0)))
        .add(osc::sine(pad_params[3].signal(30.0)))
        .amp(0.10);
    let mut pad = pad_voices
        .freeverb()
        .room_size(0.88)
        .damping(0.55)
        .width(1.0)
        .wet(0.55);

    // 7-voice supersaw: phases offset so they don't all start aligned,
    // detunes at ±6, ±12, ±18 cents around the fundamental.
    let mut lead_phases: [f32; 7] = [0.00, 0.13, 0.29, 0.41, 0.57, 0.73, 0.89];
    let lead_detunes: [f32; 7] = [
        0.98965, 0.99309, 0.99654, 1.00000, 1.00347, 1.00695, 1.01048,
    ];
    let mut lead_freq = 440.0_f32;
    let mut lead_env = envelope::adsr(0.012, 0.22, 0.50, 0.28);
    let lead_melody: [u8; 8] = [76, 72, 69, 67, 64, 67, 72, 76];
    let mut lead_lp_state = 0.0_f32;

    let mut arp_phase = 0.0_f32;
    let mut arp_freq = 440.0_f32;
    let mut arp_env = envelope::adsr(0.001, 0.09, 0.0, 0.04);

    let mut bass_phase = 0.0_f32;
    let mut bass_freq = 110.0_f32;
    let mut bass_env = envelope::adsr(0.001, 0.07, 0.0, 0.03);
    // Rolling offbeats: silent on kick steps (0, 4, 8, 12), active elsewhere.
    let bass_pattern: [bool; 16] = [
        false, true, true, true, false, true, true, true, false, true, true, true, false, true,
        true, true,
    ];

    let mut samples_since_kick: f32 = 1.0e9;
    let mut noise_state: u32 = 0xDEAD_BEEF;

    let mut last_16th: i32 = -1;
    let mut last_chord_idx: usize = usize::MAX;

    println!(
        "nyx: rendering trance — {} BPM, A minor, 52 bars ({:.1} s)...",
        BPM as i32, DURATION_SECS
    );

    let signal = move |ctx: &AudioContext| {
        let state = clk.tick(ctx);
        let beat = state.beat;
        let bar = (beat as i32) / 4;
        let sixteenth = (beat * 4.0) as i32;
        let step = sixteenth.rem_euclid(16) as usize;
        let sec = section_for(bar);

        // Chord change every 4 bars.
        let chord_idx = ((bar / 4).rem_euclid(4)) as usize;
        if chord_idx != last_chord_idx {
            last_chord_idx = chord_idx;
            let root_midi = chord_roots[chord_idx];
            let intervals = chord_intervals[chord_idx];
            for i in 0..4 {
                let n = Note::from_midi(root_midi + 24 + intervals[i]);
                pad_writers[i].set(n.to_freq());
            }
            bass_freq = Note::from_midi(root_midi).to_freq();
        }

        if sixteenth != last_16th {
            last_16th = sixteenth;

            let has_kick = matches!(
                sec,
                Section::Build | Section::Drop | Section::FinalBuild | Section::FinalDrop
            );
            if has_kick && step.is_multiple_of(4) {
                kick.trigger();
                samples_since_kick = 0.0;
            }

            if matches!(sec, Section::Drop | Section::FinalDrop) && (step == 4 || step == 12) {
                snare.trigger();
            }
            if sec == Section::FinalBuild && bar >= 44 {
                snare.trigger();
            }

            // Offbeat open hat (8th upbeats); closed hat elsewhere.
            let is_offbeat = matches!(step, 2 | 6 | 10 | 14);
            match sec {
                Section::Intro => {
                    if is_offbeat {
                        hat_c.trigger();
                    }
                }
                Section::Build | Section::FinalBuild => hat_c.trigger(),
                Section::Drop | Section::FinalDrop => {
                    if is_offbeat {
                        hat_o.trigger();
                    } else {
                        hat_c.trigger();
                    }
                }
                Section::Breakdown => {}
            }

            if bass_pattern[step]
                && matches!(
                    sec,
                    Section::Build | Section::Drop | Section::FinalBuild | Section::FinalDrop
                )
            {
                bass_phase = 0.0;
                bass_env.trigger();
            }

            // 16th-note arp cycling chord tones, octave rises mid-bar.
            if matches!(sec, Section::Drop | Section::Breakdown | Section::FinalDrop) {
                let intervals = chord_intervals[chord_idx];
                let note_idx = step % 4;
                let root = chord_roots[chord_idx];
                let oct_shift = if step < 8 { 36 } else { 48 };
                let note = Note::from_midi((root + oct_shift).min(108) + intervals[note_idx]);
                arp_freq = note.to_freq();
                arp_phase = 0.0;
                arp_env.trigger();
            }

            // Lead: anthem melody, retriggers on every beat in drops.
            if matches!(sec, Section::Drop | Section::FinalDrop) && step.is_multiple_of(4) {
                let lead_step = ((sixteenth / 4).rem_euclid(8)) as usize;
                lead_freq = Note::from_midi(lead_melody[lead_step]).to_freq();
                lead_env.trigger();
            }
        }

        samples_since_kick += 1.0;

        let k = kick.next(ctx);
        let s = snare.next(ctx);
        let hats = hat_c.next(ctx) * 0.38 + hat_o.next(ctx) * 0.28;

        let pad_sample = pad.next(ctx);

        let arp_saw = 2.0 * arp_phase - 1.0;
        arp_phase += arp_freq / ctx.sample_rate;
        arp_phase -= arp_phase.floor();
        let arp_out = arp_saw.tanh() * arp_env.next(ctx) * 0.26;

        // Supersaw stack.
        let mut lead_raw = 0.0_f32;
        for (i, &det) in lead_detunes.iter().enumerate() {
            lead_raw += 2.0 * lead_phases[i] - 1.0;
            lead_phases[i] += lead_freq * det / ctx.sample_rate;
            lead_phases[i] -= lead_phases[i].floor();
        }
        lead_raw /= 7.0;

        // Section-dependent low-pass cutoff that opens across the drop.
        let sweep = match sec {
            Section::Drop => {
                let t = ((bar - 16) as f32 + state.phase_in_bar) / 16.0;
                1200.0 + t.clamp(0.0, 1.0) * 6000.0
            }
            Section::FinalDrop => {
                let t = ((bar - 46) as f32 + state.phase_in_bar) / 6.0;
                2500.0 + t.clamp(0.0, 1.0) * 6500.0
            }
            _ => 4000.0,
        };
        // One-pole LP (no coefficient smoothing needed at this rate of change).
        let alpha = 1.0 - (-std::f32::consts::TAU * sweep / ctx.sample_rate).exp();
        lead_lp_state += alpha * (lead_raw - lead_lp_state);
        let lead_out = lead_lp_state.tanh() * lead_env.next(ctx) * 0.24;

        let bass_saw = 2.0 * bass_phase - 1.0;
        bass_phase += bass_freq / ctx.sample_rate;
        bass_phase -= bass_phase.floor();
        let bass_out = (bass_saw * 1.4).tanh() * bass_env.next(ctx) * 0.52;

        let mut x = noise_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        noise_state = x;
        let noise = (x as f32 / u32::MAX as f32) * 2.0 - 1.0;
        let riser_amp = match sec {
            Section::Build => {
                let p = (bar - 8) as f32 / 8.0 + state.phase_in_bar / 8.0;
                p.clamp(0.0, 1.0) * 0.18
            }
            Section::FinalBuild => {
                let p = (bar - 40) as f32 / 6.0 + state.phase_in_bar / 6.0;
                p.clamp(0.0, 1.0) * 0.32
            }
            _ => 0.0,
        };
        let riser = noise * riser_amp;

        // Sidechain pump: 0.35 at kick impact, recovers to 1.0 over 180 ms (quadratic release).
        let pump_samples = 0.18 * ctx.sample_rate;
        let pump_t = (samples_since_kick / pump_samples).min(1.0);
        let pump = 0.35 + 0.65 * pump_t * pump_t;
        let pump_active = matches!(
            sec,
            Section::Drop | Section::FinalDrop | Section::Build | Section::FinalBuild
        );
        let p = if pump_active { pump } else { 1.0 };

        let mix = match sec {
            Section::Intro => pad_sample * 0.90 + hats * 0.30,
            Section::Build => {
                pad_sample * 0.60 * p + k * 0.85 + bass_out * 0.55 * p + hats * 0.55 + riser
            }
            Section::Drop => {
                pad_sample * 0.30 * p + k + s + hats + bass_out * p + arp_out + lead_out * p
            }
            Section::Breakdown => pad_sample * 1.00 + arp_out * 0.45 + hats * 0.15,
            Section::FinalBuild => {
                pad_sample * 0.40 * p + k + s * 0.9 + hats * 0.7 + bass_out * 0.8 * p + riser
            }
            Section::FinalDrop => {
                pad_sample * 0.25 * p
                    + k
                    + s
                    + hats
                    + bass_out * 1.1 * p
                    + arp_out * 1.1
                    + lead_out * 1.2 * p
            }
        };

        (mix * 0.48).tanh()
    };

    let out = "target/trance.wav";
    render_to_wav(signal, DURATION_SECS, SAMPLE_RATE, out).unwrap();
    println!(
        "nyx: wrote {} ({:.1} s, {} Hz, 16-bit mono)",
        out, DURATION_SECS, SAMPLE_RATE as i32
    );
}
