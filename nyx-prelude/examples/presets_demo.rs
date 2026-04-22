//! Quick audio tour of every voice in [`presets`] — renders a 20 s
//! WAV that plays each preset in turn so a new user can hear what
//! the library ships with.
//!
//! Schedule (seconds):
//!   0–3   tb303     — 4-note acid riff (E2, E2, G2, D2)
//!   3–6   moog_bass — root → fifth (A1, E2)
//!   6–9   supersaw  — C-major arpeggio (C4, E4, G4) with external envelope
//!   9–13  prophet_pad — F3 swell and release
//!   13–17 dx7_bell  — C5 → E5 → G5 → C6 arpeggio
//!   17–19 noise_sweep — 2-second riser
//!
//! Run: cargo run -p nyx-prelude --example presets_demo --release

use nyx_prelude::*;

#[derive(Clone, Copy)]
enum Evt {
    TbTrig(f32),
    MoogTrig(f32),
    MoogRel,
    SawTrig(f32),
    SawRel,
    PadTrig(f32),
    PadRel,
    BellTrig(f32),
}

fn main() {
    const SR: f32 = 44100.0;
    const DURATION: f32 = 20.0;

    // Per-preset voices — each holds its own state and envelope.
    let mut tb = presets::tb303(82.41);
    let mut moog = presets::moog_bass(55.0);
    let mut saw = presets::supersaw(261.63);
    // Supersaw has no intrinsic envelope — give it one externally.
    let mut saw_env = envelope::adsr(0.05, 0.30, 0.70, 0.40);
    let mut pad = presets::prophet_pad(174.61);
    let mut bell = presets::dx7_bell(523.25);
    let mut sweep = presets::noise_sweep(2.0);

    // (seconds, event). Consumed in order via a cursor.
    let schedule: [(f32, Evt); 18] = [
        (0.00, Evt::TbTrig(82.41)), // E2
        (0.75, Evt::TbTrig(82.41)),
        (1.50, Evt::TbTrig(98.00)),  // G2
        (2.25, Evt::TbTrig(73.42)),  // D2
        (3.00, Evt::MoogTrig(55.0)), // A1
        (4.40, Evt::MoogRel),
        (4.55, Evt::MoogTrig(82.41)), // E2
        (5.80, Evt::MoogRel),
        (6.00, Evt::SawTrig(261.63)), // C4
        (7.00, Evt::SawTrig(329.63)), // E4
        (8.00, Evt::SawTrig(392.00)), // G4
        (8.80, Evt::SawRel),
        (9.00, Evt::PadTrig(174.61)), // F3
        (11.50, Evt::PadRel),
        (13.00, Evt::BellTrig(523.25)),  // C5
        (14.00, Evt::BellTrig(659.25)),  // E5
        (15.00, Evt::BellTrig(783.99)),  // G5
        (16.00, Evt::BellTrig(1046.50)), // C6
    ];
    let mut cursor: usize = 0;

    // Extra one-off events we need to keep track of separately because
    // they fire after the sweep section starts.
    let mut sweep_fired = false;

    println!(
        "nyx: rendering presets_demo — 6 voices over {:.0} s...",
        DURATION
    );

    let signal = move |ctx: &AudioContext| {
        let t = ctx.tick as f32 / ctx.sample_rate;

        // Drain scheduled events whose time has arrived.
        while cursor < schedule.len() && schedule[cursor].0 <= t {
            match schedule[cursor].1 {
                Evt::TbTrig(f) => {
                    tb.set_freq(f);
                    tb.trigger();
                }
                Evt::MoogTrig(f) => {
                    moog.set_freq(f);
                    moog.trigger();
                }
                Evt::MoogRel => moog.release(),
                Evt::SawTrig(f) => {
                    saw.set_freq(f);
                    saw_env.trigger();
                }
                Evt::SawRel => saw_env.release(),
                Evt::PadTrig(f) => {
                    pad.set_freq(f);
                    pad.trigger();
                }
                Evt::PadRel => pad.release(),
                Evt::BellTrig(f) => {
                    bell.set_freq(f);
                    bell.trigger();
                }
            }
            cursor += 1;
        }
        if !sweep_fired && t >= 17.0 {
            sweep_fired = true;
            sweep.trigger();
        }

        // Every voice streams continuously so internal state stays
        // coherent; the presets with built-in envelopes self-silence
        // between notes, and supersaw is gated by `saw_env`.
        let tb_s = tb.next(ctx);
        let moog_s = moog.next(ctx);
        let saw_s = saw.next(ctx) * saw_env.next(ctx);
        let pad_s = pad.next(ctx);
        let bell_s = bell.next(ctx);
        let sweep_s = sweep.next(ctx);

        let mix = tb_s + moog_s + saw_s + pad_s + bell_s + sweep_s;
        (mix * 0.40).tanh()
    };

    let out = "target/presets_demo.wav";
    render_to_wav(signal, DURATION, SR, out).unwrap();
    println!(
        "nyx: wrote {} ({:.0} s, {} Hz, 16-bit mono)",
        out, DURATION, SR as i32
    );
}
