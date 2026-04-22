//! Preset voice sanity tests — each preset should be triggerable (or
//! continuous as appropriate), produce audible output, stay bounded,
//! and not allocate after construction.

#[global_allocator]
static ALLOC: nyx_core::GuardedAllocator = nyx_core::GuardedAllocator;

use nyx_core::{AudioContext, DenyAllocGuard, Signal, render_to_buffer};
use nyx_seq::presets;

const SR: f32 = 44100.0;

fn rms(buf: &[f32]) -> f32 {
    (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt()
}

fn render_triggered<S: Signal>(mut sig: S, trigger: impl FnOnce(&mut S), secs: f32) -> Vec<f32> {
    trigger(&mut sig);
    render_to_buffer(&mut sig, secs, SR)
}

// ─── tb303 ────────────────────────────────────────────────────────────

#[test]
fn tb303_idle_is_silent() {
    let mut sig = presets::tb303(110.0);
    let buf = render_to_buffer(&mut sig, 0.1, SR);
    assert!(rms(&buf) < 1e-4, "tb303 before trigger should be silent");
}

#[test]
fn tb303_triggered_produces_sound_and_bounded() {
    let buf = render_triggered(presets::tb303(110.0), |s| s.trigger(), 0.5);
    let r = rms(&buf);
    assert!(r > 0.02, "tb303 should be audible after trigger, rms={r}");
    for &s in &buf {
        assert!(s.is_finite() && s.abs() <= 1.2, "tb303 out of bounds: {s}");
    }
}

#[test]
fn tb303_no_alloc() {
    let mut sig = presets::tb303(110.0);
    sig.trigger();
    let _guard = DenyAllocGuard::new();
    for tick in 0..1024 {
        let _ = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
    }
}

// ─── moog_bass ────────────────────────────────────────────────────────

#[test]
fn moog_bass_triggered_is_bounded() {
    let buf = render_triggered(presets::moog_bass(65.0), |s| s.trigger(), 0.5);
    let r = rms(&buf);
    assert!(r > 0.02, "moog_bass should be audible, rms={r}");
    for &s in &buf {
        assert!(s.is_finite() && s.abs() <= 1.2, "moog_bass out: {s}");
    }
}

#[test]
fn moog_bass_cutoff_override_changes_timbre() {
    // Gross RMS isn't a reliable proxy — closed-filter output sine-
    // ifies while open-filter tanh-squashes saw peaks, and RMS can go
    // either way. Check instead that the two cutoffs produce audibly
    // different waveforms: the open-filter output must contain more
    // high-frequency energy than the closed one. Measure via the
    // input minus a one-pole LP at 1.5 kHz (crude high-band residue).
    let mut dull = presets::moog_bass(110.0).cutoff(300.0);
    let mut bright = presets::moog_bass(110.0).cutoff(3000.0);
    dull.trigger();
    bright.trigger();
    let a = render_to_buffer(&mut dull, 0.3, SR);
    let b = render_to_buffer(&mut bright, 0.3, SR);

    let high_energy = |buf: &[f32]| -> f32 {
        let alpha = 1.0 - (-std::f32::consts::TAU * 1500.0 / SR).exp();
        let mut lp = 0.0_f32;
        let mut sum_sq = 0.0_f32;
        for &s in buf {
            lp += alpha * (s - lp);
            let hp = s - lp;
            sum_sq += hp * hp;
        }
        (sum_sq / buf.len() as f32).sqrt()
    };

    let hi_dull = high_energy(&a);
    let hi_bright = high_energy(&b);
    assert!(
        hi_bright > hi_dull * 1.5,
        "open filter should retain more HF energy: dull_hi={hi_dull} bright_hi={hi_bright}"
    );
}

// ─── supersaw ─────────────────────────────────────────────────────────

#[test]
fn supersaw_continuous_output() {
    let mut sig = presets::supersaw(440.0);
    let buf = render_to_buffer(&mut sig, 0.2, SR);
    let r = rms(&buf);
    assert!(
        (0.3..=1.2).contains(&r),
        "supersaw RMS should sit near unity (7 saws × 1/√7), got {r}"
    );
    for &s in &buf {
        assert!(s.is_finite() && s.abs() <= 2.0, "supersaw out: {s}");
    }
}

#[test]
fn supersaw_set_freq_changes_pitch() {
    // Render at 110 Hz and 880 Hz; their zero-crossing density must differ.
    let mut low = presets::supersaw(110.0);
    let mut high = presets::supersaw(880.0);
    let a = render_to_buffer(&mut low, 0.2, SR);
    let b = render_to_buffer(&mut high, 0.2, SR);
    let zc = |buf: &[f32]| {
        buf.windows(2)
            .filter(|w| w[0].signum() != w[1].signum())
            .count()
    };
    assert!(
        zc(&b) > zc(&a) * 4,
        "880 Hz should cross zero much more often than 110 Hz"
    );
}

#[test]
fn supersaw_no_alloc() {
    let mut sig = presets::supersaw(440.0);
    let _guard = DenyAllocGuard::new();
    for tick in 0..1024 {
        let _ = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
    }
}

// ─── prophet_pad ──────────────────────────────────────────────────────

#[test]
fn prophet_pad_triggered_swells() {
    // 400 ms attack means initial samples should be quieter than later.
    let mut pad = presets::prophet_pad(220.0);
    pad.trigger();
    let buf = render_to_buffer(&mut pad, 1.0, SR);
    let early = rms(&buf[..(SR * 0.05) as usize]);
    let late = rms(&buf[(SR * 0.5) as usize..]);
    assert!(
        late > early * 3.0,
        "prophet_pad should swell from quiet to loud; early={early} late={late}"
    );
}

// ─── dx7_bell ─────────────────────────────────────────────────────────

#[test]
fn dx7_bell_decays_over_time() {
    let mut bell = presets::dx7_bell(440.0);
    bell.trigger();
    let buf = render_to_buffer(&mut bell, 1.5, SR);
    let r_start = rms(&buf[..(SR * 0.1) as usize]);
    let r_end = rms(&buf[(SR * 1.3) as usize..]);
    assert!(
        r_start > r_end * 5.0,
        "bell should decay substantially: start={r_start} end={r_end}"
    );
    for &s in &buf {
        assert!(s.is_finite() && s.abs() <= 1.2, "bell out: {s}");
    }
}

// ─── noise_sweep ──────────────────────────────────────────────────────

#[test]
fn noise_sweep_produces_rising_content() {
    // Crude "rising" check: split buffer in half, compare low-frequency
    // energy. A sweep moving 200→4000 Hz should concentrate low
    // energy in the first half and high energy in the second.
    let mut sweep = presets::noise_sweep(0.5);
    sweep.trigger();
    let buf = render_to_buffer(&mut sweep, 0.5, SR);
    assert!(rms(&buf) > 0.005, "noise_sweep should be audible");

    let half = buf.len() / 2;
    // One-pole LP at 500 Hz splits low content.
    let alpha = 1.0 - (-std::f32::consts::TAU * 500.0 / SR).exp();
    let mut low_state = 0.0_f32;
    let mut low_first_sq = 0.0_f32;
    let mut low_second_sq = 0.0_f32;
    for (i, &s) in buf.iter().enumerate() {
        low_state += alpha * (s - low_state);
        if i < half {
            low_first_sq += low_state * low_state;
        } else {
            low_second_sq += low_state * low_state;
        }
    }
    assert!(
        low_first_sq > low_second_sq,
        "noise_sweep should start low-biased: first={low_first_sq} second={low_second_sq}"
    );
}

#[test]
fn noise_sweep_no_alloc() {
    let mut sig = presets::noise_sweep(0.3);
    sig.trigger();
    let _guard = DenyAllocGuard::new();
    for tick in 0..1024 {
        let _ = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
    }
}

// ─── juno_pad ─────────────────────────────────────────────────────────

#[test]
fn juno_pad_triggered_swells() {
    // 350 ms attack — second-half RMS should dwarf the first 50 ms.
    let mut pad = presets::juno_pad(220.0);
    pad.trigger();
    let buf = render_to_buffer(&mut pad, 1.0, SR);
    let early = rms(&buf[..(SR * 0.05) as usize]);
    let late = rms(&buf[(SR * 0.5) as usize..]);
    assert!(
        late > early * 3.0,
        "juno_pad should swell: early={early} late={late}"
    );
    for &s in &buf {
        assert!(s.is_finite() && s.abs() <= 1.3, "juno_pad out: {s}");
    }
}

#[test]
fn juno_pad_pwm_produces_movement() {
    // The LFO-swept pulse width means the spectrum shifts over time.
    // Compare two non-overlapping windows — their sample-by-sample
    // difference should be non-trivial even after the envelope has
    // plateaued.
    let mut pad = presets::juno_pad(220.0);
    pad.trigger();
    // Discard the attack.
    let _ = render_to_buffer(&mut pad, 0.5, SR);
    let a = render_to_buffer(&mut pad, 1.0, SR);
    let b = render_to_buffer(&mut pad, 1.0, SR);
    // Same pitch, same envelope plateau — but LFO has moved the
    // width, so the two second-long windows must not be identical.
    let diff = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0_f32, f32::max);
    assert!(
        diff > 0.02,
        "PWM LFO should produce waveform movement: max diff={diff}"
    );
}

#[test]
fn juno_pad_no_alloc() {
    let mut sig = presets::juno_pad(220.0);
    sig.trigger();
    let _guard = DenyAllocGuard::new();
    for tick in 0..1024 {
        let _ = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
    }
}

// ─── handpan ──────────────────────────────────────────────────────────

#[test]
fn handpan_triggered_rings_and_decays() {
    let mut pan = presets::handpan(261.63); // C4
    pan.trigger();
    let buf = render_to_buffer(&mut pan, 2.0, SR);
    let r_start = rms(&buf[..(SR * 0.1) as usize]);
    let r_end = rms(&buf[(SR * 1.7) as usize..]);
    assert!(
        r_start > 0.05,
        "handpan should be audible after trigger, rms={r_start}"
    );
    assert!(
        r_start > r_end * 3.0,
        "handpan should decay substantially: start={r_start} end={r_end}"
    );
    for &s in &buf {
        assert!(s.is_finite() && s.abs() <= 1.2, "handpan out: {s}");
    }
}

#[test]
fn handpan_untriggered_is_silent() {
    let mut pan = presets::handpan(261.63);
    let buf = render_to_buffer(&mut pan, 0.1, SR);
    // damps start at 0 → no output even though oscillators advance.
    assert!(rms(&buf) < 1e-5, "handpan pre-trigger must be silent");
}

#[test]
fn handpan_no_alloc() {
    let mut sig = presets::handpan(261.63);
    sig.trigger();
    let _guard = DenyAllocGuard::new();
    for tick in 0..1024 {
        let _ = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
    }
}

// ─── chime ────────────────────────────────────────────────────────────

#[test]
fn chime_triggered_rings_longer_than_handpan() {
    // Longer partial τs → more energy at 2 s than a handpan of the
    // same fundamental. Crude but effective sanity check on the
    // partial-table distinction.
    let mut pan = presets::handpan(440.0);
    pan.trigger();
    let pan_tail = rms(&render_to_buffer(&mut pan, 2.0, SR)[(SR * 1.5) as usize..]);

    let mut chime = presets::chime(440.0);
    chime.trigger();
    let chime_tail = rms(&render_to_buffer(&mut chime, 2.0, SR)[(SR * 1.5) as usize..]);

    assert!(
        chime_tail > pan_tail,
        "chime should ring longer than handpan: chime={chime_tail} pan={pan_tail}"
    );
}

#[test]
fn chime_no_alloc() {
    let mut sig = presets::chime(440.0);
    sig.trigger();
    let _guard = DenyAllocGuard::new();
    for tick in 0..1024 {
        let _ = sig.next(&AudioContext {
            sample_rate: SR,
            tick,
        });
    }
}
