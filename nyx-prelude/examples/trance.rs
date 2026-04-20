//! 90-second trance showpiece — built entirely from Nyx primitives.
//!
//! 138 BPM, A minor, Am–F–C–G chord loop every 16 bars.
//! Six sections across 52 bars (≈90 s):
//!
//! | Bars   | Section       | Elements                                            |
//! |--------|---------------|-----------------------------------------------------|
//! | 0–7    | INTRO         | Reverb pad + closed hats                            |
//! | 8–15   | BUILD         | + kick on 1, 16th bass, riser ramps in              |
//! | 16–31  | DROP          | Full mix: kick, snare, hats, bass, arp, lead, pad   |
//! | 32–39  | BREAKDOWN     | Pad + arp only, riser silent                        |
//! | 40–45  | FINAL BUILD   | Escalating kick + snare roll + noise riser          |
//! | 46–51  | FINAL DROP    | Full mix again, louder lead                         |
//!
//! Shows off: Clock + ClockState, section-driven mixing, inst::kick/
//! snare/hihat, ADSR envelopes, OscParam atomic pitch updates, inline
//! saw/sine oscillators for per-note state control, Freeverb stereo
//! reverb (genuine stereo via next_stereo), soft-clipping master bus.
//!
//! Run: cargo run -p nyx-prelude --example trance --release
//!
//! Use stereo headphones or speakers.

use nyx_prelude::*;

const BPM: f32 = 138.0;

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
    // ─── Clock and drums ────────────────────────────────────────────
    let mut clk = clock::clock(BPM);
    let mut kick = inst::kick();
    let mut snare = inst::snare();
    let mut hat_c = inst::hihat(false);
    let mut hat_o = inst::hihat(true);

    // ─── Chord progression in A minor: Am – F – C – G ───────────────
    // Root MIDI notes (used as A2, F2, C2, G2 for bass; shifted up for pad/arp).
    let chord_roots: [u8; 4] = [33, 29, 24, 31]; // A2, F2, C2, G2
    // Intervals above root for (root, third, fifth, octave) in each chord.
    //   Am = minor (0, 3, 7, 12)
    //   F  = major (0, 4, 7, 12)
    //   C  = major (0, 4, 7, 12)
    //   G  = major (0, 4, 7, 12)
    let chord_intervals: [[u8; 4]; 4] = [
        [0, 3, 7, 12], // Am
        [0, 4, 7, 12], // F
        [0, 4, 7, 12], // C
        [0, 4, 7, 12], // G
    ];

    // ─── Pad: 4 sine voices, one per chord note, through reverb ─────
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
        .amp(0.12);
    let mut pad = pad_voices
        .freeverb()
        .room_size(0.88)
        .damping(0.55)
        .width(1.0)
        .wet(0.55);

    // ─── Pluck arp (inline saw + envelope for phase control) ────────
    let mut arp_phase = 0.0_f32;
    let mut arp_freq = 440.0_f32;
    let mut arp_env = envelope::adsr(0.001, 0.10, 0.0, 0.04);

    // ─── Supersaw lead (3 detuned saws) ─────────────────────────────
    let mut lead_phases: [f32; 3] = [0.0, 0.17, 0.33];
    let mut lead_freq = 440.0_f32;
    let lead_detunes: [f32; 3] = [1.0, 1.006, 0.994];
    let mut lead_env = envelope::adsr(0.015, 0.25, 0.55, 0.22);
    // Anthemic lead melody in A minor pentatonic, 8 notes per phrase.
    //   E5 - C5 - A4 - G4 - E4 - G4 - C5 - E5
    let lead_melody: [u8; 8] = [76, 72, 69, 67, 64, 67, 72, 76];

    // ─── Rolling 16th-note bass (inline saw + envelope) ─────────────
    let mut bass_phase = 0.0_f32;
    let mut bass_freq = 110.0_f32;
    let mut bass_env = envelope::adsr(0.001, 0.08, 0.0, 0.04);
    // Off-beat trance bass pattern: silent on beats (0, 4, 8, 12), hit on everything else.
    let bass_pattern: [bool; 16] = [
        false, true, true, true,
        false, true, true, true,
        false, true, true, true,
        false, true, true, true,
    ];

    // ─── Noise riser state ──────────────────────────────────────────
    let mut noise_state: u32 = 0xDEAD_BEEF;

    // ─── Tracking ───────────────────────────────────────────────────
    let mut last_16th: i32 = -1;
    let mut last_chord_idx: usize = usize::MAX;

    println!(
        "nyx trance — {} BPM, A minor, Am-F-C-G — 90 s ({} bars)",
        BPM as i32, 52
    );

    let signal = move |ctx: &AudioContext| {
        let state = clk.tick(ctx);
        let beat = state.beat;
        let bar = (beat as i32) / 4;
        let sixteenth = (beat * 4.0) as i32;
        let step = sixteenth.rem_euclid(16) as usize;
        let sec = section_for(bar);

        // Chord changes every 4 bars.
        let chord_idx = ((bar / 4).rem_euclid(4)) as usize;
        if chord_idx != last_chord_idx {
            last_chord_idx = chord_idx;
            let root_midi = chord_roots[chord_idx];
            let intervals = chord_intervals[chord_idx];
            // Pad voices: shifted up 2 octaves from bass root.
            for i in 0..4 {
                let n = Note::from_midi(root_midi + 24 + intervals[i]);
                pad_writers[i].set(n.to_freq());
            }
            // Bass at the chord root (already in the bass octave).
            bass_freq = Note::from_midi(root_midi).to_freq();
        }

        // 16th-note event dispatch.
        if sixteenth != last_16th {
            last_16th = sixteenth;

            // KICK — on every beat (steps 0, 4, 8, 12) from Build onward.
            if step.is_multiple_of(4)
                && matches!(
                    sec,
                    Section::Build | Section::Drop | Section::FinalBuild | Section::FinalDrop
                )
            {
                kick.trigger();
            }

            // SNARE — backbeat on steps 4 and 12 in Drops.
            // Snare roll: every 16th in the last two bars of FinalBuild.
            if matches!(sec, Section::Drop | Section::FinalDrop) && (step == 4 || step == 12) {
                snare.trigger();
            }
            if sec == Section::FinalBuild && bar >= 44 {
                snare.trigger();
            }

            // HI-HATS — offbeats (odd 16ths) with open hat at step 7.
            if step % 2 == 1 {
                match sec {
                    Section::Intro => {
                        if step == 7 || step == 15 {
                            hat_c.trigger();
                        }
                    }
                    Section::Build | Section::FinalBuild => {
                        hat_c.trigger();
                    }
                    Section::Drop | Section::FinalDrop => {
                        if step == 7 {
                            hat_o.trigger();
                        } else {
                            hat_c.trigger();
                        }
                    }
                    _ => {}
                }
            }

            // BASS — 16th-note pattern during energetic sections.
            if bass_pattern[step]
                && matches!(
                    sec,
                    Section::Build | Section::Drop | Section::FinalBuild | Section::FinalDrop
                )
            {
                bass_phase = 0.0;
                bass_env.trigger();
            }

            // ARP — every 16th, cycling through current chord tones.
            // Octave rises through the bar for upward motion.
            if matches!(
                sec,
                Section::Drop | Section::Breakdown | Section::FinalDrop
            ) {
                let intervals = chord_intervals[chord_idx];
                let note_idx = step % 4;
                let root = chord_roots[chord_idx];
                let octave_shift = if step < 8 { 36 } else { 48 }; // +3 or +4 octaves
                let arp_note =
                    Note::from_midi((root + octave_shift).min(108) + intervals[note_idx]);
                arp_freq = arp_note.to_freq();
                arp_phase = 0.0;
                arp_env.trigger();
            }

            // LEAD — on every beat in Drops. 8-note melody, loops every 2 bars.
            if matches!(sec, Section::Drop | Section::FinalDrop) && step.is_multiple_of(4) {
                let lead_step = ((sixteenth / 4).rem_euclid(8)) as usize;
                lead_freq = Note::from_midi(lead_melody[lead_step]).to_freq();
                lead_env.trigger();
            }
        }

        // ─── Per-sample voice rendering ────────────────────────────

        // Drums
        let k = kick.next(ctx);
        let s = snare.next(ctx);
        let hats = hat_c.next(ctx) * 0.40 + hat_o.next(ctx) * 0.30;

        // Pad (pre-built with reverb). Uses mono-safe .next().
        let pad_sample = pad.next(ctx);

        // Arp
        let arp_saw = 2.0 * arp_phase - 1.0;
        arp_phase += arp_freq / ctx.sample_rate;
        arp_phase -= arp_phase.floor();
        let arp_out = arp_saw.tanh() * arp_env.next(ctx) * 0.28;

        // Supersaw lead
        let mut lead_sum = 0.0_f32;
        for (i, &det) in lead_detunes.iter().enumerate() {
            lead_sum += 2.0 * lead_phases[i] - 1.0;
            lead_phases[i] += lead_freq * det / ctx.sample_rate;
            lead_phases[i] -= lead_phases[i].floor();
        }
        let lead_out = (lead_sum / 3.0).tanh() * lead_env.next(ctx) * 0.22;

        // Bass
        let bass_saw = 2.0 * bass_phase - 1.0;
        bass_phase += bass_freq / ctx.sample_rate;
        bass_phase -= bass_phase.floor();
        let bass_out = (bass_saw * 1.5).tanh() * bass_env.next(ctx) * 0.55;

        // Riser noise (only during builds)
        let mut x = noise_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        noise_state = x;
        let noise = (x as f32 / u32::MAX as f32) * 2.0 - 1.0;
        let riser_amp = match sec {
            Section::Build => {
                // Ramp 0 → 0.2 over 8 bars
                let progress = (bar - 8) as f32 / 8.0 + state.phase_in_bar / 8.0;
                progress.clamp(0.0, 1.0) * 0.20
            }
            Section::FinalBuild => {
                // Steeper ramp 0 → 0.35 over 6 bars
                let progress = (bar - 40) as f32 / 6.0 + state.phase_in_bar / 6.0;
                progress.clamp(0.0, 1.0) * 0.35
            }
            _ => 0.0,
        };
        let riser = noise * riser_amp;

        // ─── Section mix ───────────────────────────────────────────
        let mix = match sec {
            Section::Intro => pad_sample * 0.90 + hats * 0.35,
            Section::Build => {
                pad_sample * 0.70 + k * 0.85 + bass_out * 0.55 + hats * 0.60 + riser
            }
            Section::Drop => {
                pad_sample * 0.30
                    + k
                    + s
                    + hats
                    + bass_out
                    + arp_out
                    + lead_out
            }
            Section::Breakdown => pad_sample * 1.0 + arp_out * 0.45 + hats * 0.15,
            Section::FinalBuild => {
                pad_sample * 0.40 + k + s * 0.9 + hats * 0.7 + bass_out * 0.8 + riser
            }
            Section::FinalDrop => {
                pad_sample * 0.25
                    + k
                    + s
                    + hats
                    + bass_out * 1.1
                    + arp_out * 1.1
                    + lead_out * 1.2
            }
        };

        // Master bus: soft-clip for gentle limiting.
        (mix * 0.5).tanh()
    };

    play(signal).unwrap();
}
