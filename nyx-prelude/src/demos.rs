//! Reusable signal builders for the Nyx demo tracks.
//!
//! Each function returns `impl Signal + Send + 'static` so it can feed
//! either the offline WAV renderer (`render_to_wav`) or a live engine
//! (`play`, the WASM `NyxDemo`, etc). The signal closures are pure DSP —
//! no I/O, no allocation after construction — so they are safe to run
//! on the audio thread.
//!
//! Keep track definitions here so the examples and the browser demo
//! stay in sync: one source of truth for the musical content.

use nyx_core::osc_input::{OscParam, OscParamWriter};
use nyx_core::{AudioContext, Signal, SignalExt};
use nyx_seq::{Note, clock, envelope, inst};

/// 90-second Tron: Legacy-style electro-orchestral cue.
///
/// 120 BPM, D natural minor, 45 bars ≈ 90 s.
/// Progression: Dm – B♭ – F – C (i – VI – III – VII), substituting
/// Dm – Gm – B♭ – A (i – iv – VI – V♯) at the climax for the harmonic-
/// minor V colour that Daft Punk use for cadences.
///
/// Structure:
/// | Bars   | Section | Elements                                        |
/// |--------|---------|-------------------------------------------------|
/// | 0–7    | Intro   | String pad drone, timpani on bars 4 and 8       |
/// | 8–15   | Motif   | Filtered lead enters over strings, no drums    |
/// | 16–17  | Build   | Snare roll, noise riser, final timpani         |
/// | 18–33  | Main    | Full groove: kick, bass ostinato, strings, lead |
/// | 34–39  | Climax  | Harmonic-minor V substitution, strings +1 oct   |
/// | 40–44  | Decay   | Drums drop, filter closes, reverb tail          |
pub fn tron() -> impl Signal + 'static {
    const BPM: f32 = 120.0;

    #[derive(Copy, Clone, PartialEq, Eq)]
    enum Section {
        Intro,
        Motif,
        Build,
        Main,
        Climax,
        Decay,
    }
    fn section_for(bar: i32) -> Section {
        match bar {
            0..=7 => Section::Intro,
            8..=15 => Section::Motif,
            16..=17 => Section::Build,
            18..=33 => Section::Main,
            34..=39 => Section::Climax,
            _ => Section::Decay,
        }
    }

    let mut clk = clock::clock(BPM);
    let mut kick = inst::kick();
    let mut snare = inst::snare();
    let mut hat_c = inst::hihat(false);
    let mut hat_o = inst::hihat(true);

    let base_roots: [u8; 4] = [38, 34, 41, 36]; // D2, B♭1, F2, C2
    let climax_roots: [u8; 4] = [38, 31, 34, 33]; // D2, G1, B♭1, A1

    let base_intervals: [[u8; 4]; 4] = [[0, 3, 7, 12], [0, 4, 7, 12], [0, 4, 7, 12], [0, 4, 7, 12]];
    let climax_intervals: [[u8; 4]; 4] = [
        [0, 3, 7, 12], // Dm
        [0, 3, 7, 12], // Gm
        [0, 4, 7, 12], // B♭
        [0, 4, 7, 12], // A (major — raised 3rd gives C♯)
    ];

    // String ensemble: 4 chord voices × 3 detuned saws each.
    let mut string_phases: [[f32; 3]; 4] = [
        [0.00, 0.33, 0.66],
        [0.11, 0.44, 0.77],
        [0.22, 0.55, 0.88],
        [0.05, 0.38, 0.71],
    ];
    let string_detunes: [f32; 3] = [0.9960, 1.0000, 1.0040]; // ±7 cents
    let string_freqs: [OscParam; 4] = [
        OscParam::new(146.83),
        OscParam::new(174.61),
        OscParam::new(220.00),
        OscParam::new(293.66),
    ];
    let string_writers: [OscParamWriter; 4] = [
        string_freqs[0].writer(),
        string_freqs[1].writer(),
        string_freqs[2].writer(),
        string_freqs[3].writer(),
    ];
    let mut string_lp_state = 0.0_f32;
    let mut string_hp_state = 0.0_f32;

    let pad_params: [OscParam; 4] = [
        OscParam::new(293.66),
        OscParam::new(349.23),
        OscParam::new(440.00),
        OscParam::new(587.33),
    ];
    let pad_writers: [OscParamWriter; 4] = [
        pad_params[0].writer(),
        pad_params[1].writer(),
        pad_params[2].writer(),
        pad_params[3].writer(),
    ];
    let pad_voices = nyx_core::osc::sine(pad_params[0].signal(25.0))
        .add(nyx_core::osc::sine(pad_params[1].signal(25.0)))
        .add(nyx_core::osc::sine(pad_params[2].signal(25.0)))
        .add(nyx_core::osc::sine(pad_params[3].signal(25.0)))
        .amp(0.06);
    let mut pad = pad_voices
        .freeverb()
        .room_size(0.92)
        .damping(0.50)
        .width(1.0)
        .wet(0.65);

    let mut lead_phases: [f32; 3] = [0.00, 0.31, 0.63];
    let lead_detunes: [f32; 3] = [0.9942, 1.0000, 1.0058];
    let mut lead_sub_phase = 0.0_f32;
    let mut lead_freq = 440.0_f32;
    let mut lead_env = envelope::adsr(0.020, 0.30, 0.55, 0.35);
    let mut lead_filter_env = envelope::adsr(0.080, 0.60, 0.00, 0.20);
    let mut lead_lp_state = 0.0_f32;

    let base_motif: [[u8; 8]; 4] = [
        [69, 69, 69, 69, 65, 65, 62, 62],
        [65, 65, 65, 65, 62, 62, 58, 58],
        [72, 72, 72, 72, 69, 69, 65, 65],
        [67, 67, 67, 67, 64, 64, 60, 60],
    ];
    let climax_motif: [[u8; 8]; 4] = [
        [81, 81, 81, 81, 77, 77, 74, 74],
        [79, 79, 79, 79, 74, 74, 70, 70],
        [77, 77, 77, 77, 74, 74, 70, 70],
        [76, 76, 76, 76, 73, 73, 69, 69],
    ];

    let mut bass_phase = 0.0_f32;
    let mut bass_sub_phase = 0.0_f32;
    let mut bass_freq = 73.42_f32;
    let mut bass_env = envelope::adsr(0.001, 0.09, 0.0, 0.04);
    let bass_oct_pattern: [i32; 16] = [0, -1, 0, -1, 0, -1, 0, -1, 0, -1, 12, -1, 0, -1, 12, -1];

    let mut timp_phase = 0.0_f32;
    let mut timp_samples_since: f32 = 1.0e9;
    let mut timp_env = envelope::adsr(0.005, 0.50, 0.0, 0.10);

    let mut samples_since_kick: f32 = 1.0e9;
    let mut noise_state: u32 = 0xCAFEF00D;

    let mut last_16th: i32 = -1;
    let mut last_chord_idx: i32 = -1;
    let mut last_section: Option<Section> = None;

    move |ctx: &AudioContext| {
        let state = clk.tick(ctx);
        let beat = state.beat;
        let bar = (beat as i32) / 4;
        let sixteenth = (beat * 4.0) as i32;
        let step = sixteenth.rem_euclid(16) as usize;
        let eighth = step / 2;
        let sec = section_for(bar);
        let in_climax = sec == Section::Climax;

        let roots = if in_climax {
            &climax_roots
        } else {
            &base_roots
        };
        let intervals = if in_climax {
            &climax_intervals
        } else {
            &base_intervals
        };
        let motif = if in_climax {
            &climax_motif
        } else {
            &base_motif
        };

        let chord_idx = (bar.rem_euclid(4)) as usize;
        let chord_changed = chord_idx as i32 != last_chord_idx;
        let section_changed = Some(sec) != last_section;
        if chord_changed || section_changed {
            last_chord_idx = chord_idx as i32;
            last_section = Some(sec);
            let root = roots[chord_idx];
            let ivl = intervals[chord_idx];
            for i in 0..4 {
                let n = Note::from_midi(root + 24 + ivl[i]);
                string_writers[i].set(n.to_freq());
            }
            for i in 0..4 {
                let n = Note::from_midi(root + 36 + ivl[i]);
                pad_writers[i].set(n.to_freq());
            }
            bass_freq = Note::from_midi(root).to_freq();
        }

        if sixteenth != last_16th {
            last_16th = sixteenth;

            let drums_on = matches!(sec, Section::Main | Section::Climax);
            if drums_on && step.is_multiple_of(4) {
                kick.trigger();
                samples_since_kick = 0.0;
            }
            if drums_on && (step == 4 || step == 12) {
                snare.trigger();
            }
            if sec == Section::Build {
                let fast = bar == 17 && step >= 8;
                if fast || step.is_multiple_of(2) {
                    snare.trigger();
                }
            }
            if drums_on {
                if step == 14 {
                    hat_o.trigger();
                } else if step.is_multiple_of(2) {
                    hat_c.trigger();
                }
            }

            if step == 0 && (bar == 3 || bar == 7 || bar == 15) {
                timp_phase = 0.0;
                timp_samples_since = 0.0;
                timp_env.trigger();
            }

            let bass_on = matches!(sec, Section::Main | Section::Climax);
            if bass_on {
                let offset = bass_oct_pattern[step];
                if offset >= 0 {
                    let midi = (roots[chord_idx] as i32 + offset) as u8;
                    bass_freq = Note::from_midi(midi).to_freq();
                    bass_phase = 0.0;
                    bass_sub_phase = 0.0;
                    bass_env.trigger();
                }
            }

            let lead_on = matches!(sec, Section::Motif | Section::Main | Section::Climax);
            if lead_on && step.is_multiple_of(2) {
                let midi = motif[chord_idx][eighth];
                lead_freq = Note::from_midi(midi).to_freq();
                lead_env.trigger();
                lead_filter_env.trigger();
            }
        }

        samples_since_kick += 1.0;
        timp_samples_since += 1.0;

        let mut string_sum = 0.0_f32;
        for voice in 0..4 {
            let f = string_freqs[voice].get();
            for (i, &det) in string_detunes.iter().enumerate() {
                string_sum += 2.0 * string_phases[voice][i] - 1.0;
                string_phases[voice][i] += f * det / ctx.sample_rate;
                string_phases[voice][i] -= string_phases[voice][i].floor();
            }
        }
        string_sum /= 12.0;
        let lp_cut = 3000.0;
        let lp_a = 1.0 - (-std::f32::consts::TAU * lp_cut / ctx.sample_rate).exp();
        string_lp_state += lp_a * (string_sum - string_lp_state);
        let hp_cut = 180.0;
        let hp_a = 1.0 - (-std::f32::consts::TAU * hp_cut / ctx.sample_rate).exp();
        string_hp_state += hp_a * (string_lp_state - string_hp_state);
        let strings_raw = string_lp_state - string_hp_state;

        let string_amp = match sec {
            Section::Intro => 0.28 * (0.25 + 0.75 * (bar as f32 / 8.0).min(1.0)),
            Section::Motif => 0.42,
            Section::Build => 0.40,
            Section::Main => 0.34,
            Section::Climax => 0.52,
            Section::Decay => {
                let t = (bar - 40) as f32 / 5.0 + state.phase_in_bar / 5.0;
                0.34 * (1.0 - t.clamp(0.0, 1.0))
            }
        };
        let strings = strings_raw * string_amp;

        let pad_sample = pad.next(ctx);
        let pad_amp = match sec {
            Section::Intro => 0.9,
            Section::Motif => 0.8,
            Section::Build => 0.7,
            Section::Main => 0.55,
            Section::Climax => 0.80,
            Section::Decay => {
                let t = (bar - 40) as f32 / 5.0 + state.phase_in_bar / 5.0;
                0.55 * (1.0 - t.clamp(0.0, 1.0))
            }
        };
        let pad_out = pad_sample * pad_amp;

        let k = kick.next(ctx) * 1.05;
        let s = snare.next(ctx) * 0.70;
        let hats = hat_c.next(ctx) * 0.32 + hat_o.next(ctx) * 0.26;

        let timp_t = (timp_samples_since / (0.15 * ctx.sample_rate)).min(1.0);
        let timp_freq = 70.0 + (48.0 - 70.0) * timp_t;
        let timp_sample = (timp_phase * std::f32::consts::TAU).sin();
        timp_phase += timp_freq / ctx.sample_rate;
        timp_phase -= timp_phase.floor();
        let timp_out = timp_sample * timp_env.next(ctx) * 0.55;

        let bass_saw = 2.0 * bass_phase - 1.0;
        bass_phase += bass_freq / ctx.sample_rate;
        bass_phase -= bass_phase.floor();
        let bass_sub = (bass_sub_phase * std::f32::consts::TAU).sin();
        bass_sub_phase += (bass_freq * 0.5) / ctx.sample_rate;
        bass_sub_phase -= bass_sub_phase.floor();
        let bass_env_val = bass_env.next(ctx);
        let bass_out = ((bass_saw * 1.3).tanh() * 0.55 + bass_sub * 0.35) * bass_env_val * 0.58;

        let mut lead_raw = 0.0_f32;
        for (i, &det) in lead_detunes.iter().enumerate() {
            lead_raw += 2.0 * lead_phases[i] - 1.0;
            lead_phases[i] += lead_freq * det / ctx.sample_rate;
            lead_phases[i] -= lead_phases[i].floor();
        }
        lead_raw /= 3.0;
        let sub_raw = if lead_sub_phase < 0.5 { 1.0 } else { -1.0 };
        lead_sub_phase += (lead_freq * 0.5) / ctx.sample_rate;
        lead_sub_phase -= lead_sub_phase.floor();
        let lead_mixed = lead_raw * 0.75 + sub_raw * 0.20;
        let filt_env = lead_filter_env.next(ctx);
        let motif_openness = match sec {
            Section::Motif => {
                let t = (bar - 8) as f32 / 8.0 + state.phase_in_bar / 8.0;
                t.clamp(0.0, 1.0)
            }
            Section::Main | Section::Climax => 1.0,
            _ => 0.0,
        };
        let cutoff = 400.0 + 800.0 * motif_openness + filt_env * 2700.0;
        let a = 1.0 - (-std::f32::consts::TAU * cutoff / ctx.sample_rate).exp();
        lead_lp_state += a * (lead_mixed - lead_lp_state);
        let lead_amp = match sec {
            Section::Motif => 0.30,
            Section::Main => 0.38,
            Section::Climax => 0.48,
            _ => 0.0,
        };
        let lead_out = lead_lp_state.tanh() * lead_env.next(ctx) * lead_amp;

        let mut x = noise_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        noise_state = x;
        let noise = (x as f32 / u32::MAX as f32) * 2.0 - 1.0;
        let riser_amp = if sec == Section::Build {
            let t = (bar - 16) as f32 / 2.0 + state.phase_in_bar / 2.0;
            t.clamp(0.0, 1.0).powi(2) * 0.22
        } else {
            0.0
        };
        let riser = noise * riser_amp;

        let pump_samples = 0.20 * ctx.sample_rate;
        let pump_t = (samples_since_kick / pump_samples).min(1.0);
        let pump = 0.72 + 0.28 * pump_t * pump_t;
        let pump_active = matches!(sec, Section::Main | Section::Climax);
        let p = if pump_active { pump } else { 1.0 };

        let mix = match sec {
            Section::Intro => strings + pad_out * 0.55 + timp_out,
            Section::Motif => strings + pad_out * 0.50 + lead_out,
            Section::Build => strings + pad_out * 0.45 + timp_out + s * 0.65 + riser,
            Section::Main => {
                strings * p + pad_out * 0.40 * p + k + s + hats + bass_out * p + lead_out * p
            }
            Section::Climax => {
                strings * p + pad_out * 0.55 * p + k + s + hats + bass_out * p + lead_out * p
            }
            Section::Decay => strings + pad_out * 0.60,
        };

        (mix * 0.48).tanh()
    }
}
