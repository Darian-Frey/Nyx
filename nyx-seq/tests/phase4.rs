use nyx_core::{AudioContext, Signal, render_to_buffer};
use nyx_seq::clock::{self, Clock};
use nyx_seq::envelope::{self, Stage};
use nyx_seq::automation::{self, AutomationExt};

const SR: f32 = 44100.0;

fn ctx(tick: u64) -> AudioContext {
    AudioContext {
        sample_rate: SR,
        tick,
    }
}

// ===================== Clock tests =====================

#[test]
fn clock_starts_at_beat_zero() {
    let mut clk = clock::clock(120.0);
    let state = clk.tick(&ctx(0));
    assert!((state.beat - 0.0).abs() < 1e-6);
    assert!((state.bar - 0.0).abs() < 1e-6);
}

#[test]
fn clock_120bpm_reaches_beat_1_in_half_second() {
    // 120 BPM = 2 beats/sec → beat 1.0 at 0.5 seconds = 22050 samples.
    let mut clk = clock::clock(120.0);
    let mut state = clk.tick(&ctx(0));
    for tick in 1..22050 {
        state = clk.tick(&ctx(tick));
    }
    // Should be very close to beat 1.0.
    assert!(
        (state.beat - 1.0).abs() < 0.01,
        "expected beat ~1.0, got {}",
        state.beat
    );
}

#[test]
fn clock_bar_is_beat_div_4() {
    let mut clk = clock::clock(120.0);
    // Advance to ~4 beats (1 bar at 4/4).
    let samples_per_beat = (SR * 60.0 / 120.0) as u64;
    let mut state = clk.tick(&ctx(0));
    for tick in 1..(samples_per_beat * 4) {
        state = clk.tick(&ctx(tick));
    }
    assert!(
        (state.bar - 1.0).abs() < 0.01,
        "expected bar ~1.0, got {}",
        state.bar
    );
}

#[test]
fn clock_phase_in_beat_wraps() {
    let mut clk = clock::clock(120.0);
    let samples_per_beat = (SR * 60.0 / 120.0) as u64;
    let mut state = clk.tick(&ctx(0));
    // Advance 1.5 beats.
    for tick in 1..((samples_per_beat * 3) / 2) {
        state = clk.tick(&ctx(tick));
    }
    // phase_in_beat should be ~0.5
    assert!(
        (state.phase_in_beat - 0.5).abs() < 0.02,
        "expected phase_in_beat ~0.5, got {}",
        state.phase_in_beat
    );
}

#[test]
fn clock_custom_beats_per_bar() {
    let mut clk = clock::clock(120.0).beats_per_bar(3.0);
    let samples_per_beat = (SR * 60.0 / 120.0) as u64;
    let mut state = clk.tick(&ctx(0));
    // 3 beats = 1 bar in 3/4 time.
    for tick in 1..(samples_per_beat * 3) {
        state = clk.tick(&ctx(tick));
    }
    assert!(
        (state.bar - 1.0).abs() < 0.01,
        "expected bar ~1.0 in 3/4, got {}",
        state.bar
    );
}

#[test]
fn clock_modulated_bpm() {
    // BPM modulated by a signal that returns 240.0 constantly.
    let bpm_signal = |_ctx: &AudioContext| 240.0_f32;
    let mut clk = clock::clock(bpm_signal);
    // At 240 BPM = 4 beats/sec → beat 1 at 0.25 seconds = 11025 samples.
    let mut state = clk.tick(&ctx(0));
    for tick in 1..11025 {
        state = clk.tick(&ctx(tick));
    }
    assert!(
        (state.beat - 1.0).abs() < 0.01,
        "expected beat ~1.0 at 240BPM, got {}",
        state.beat
    );
}

#[test]
fn clock_reset() {
    let mut clk = clock::clock(120.0);
    for tick in 0..10000 {
        clk.tick(&ctx(tick));
    }
    clk.reset();
    let state = clk.tick(&ctx(10000));
    assert!((state.beat - 0.0).abs() < 0.01);
}

// ===================== Quantisation tests =====================

#[test]
fn snap_quarter_note() {
    assert!((Clock::<nyx_core::param::ConstSignal>::snap(1.3, 1.0) - 1.0).abs() < 1e-6);
    assert!((Clock::<nyx_core::param::ConstSignal>::snap(1.6, 1.0) - 2.0).abs() < 1e-6);
}

#[test]
fn snap_sixteenth_note() {
    assert!((Clock::<nyx_core::param::ConstSignal>::snap(0.13, 0.25) - 0.25).abs() < 1e-6);
    assert!((Clock::<nyx_core::param::ConstSignal>::snap(0.37, 0.25) - 0.25).abs() < 1e-6);
    assert!((Clock::<nyx_core::param::ConstSignal>::snap(0.38, 0.25) - 0.5).abs() < 1e-6);
}

#[test]
fn snap_zero_grid_returns_input() {
    assert!((Clock::<nyx_core::param::ConstSignal>::snap(1.7, 0.0) - 1.7).abs() < 1e-6);
}

// ===================== ADSR tests =====================

#[test]
fn adsr_starts_idle() {
    let env = envelope::adsr(0.01, 0.05, 0.7, 0.1);
    assert_eq!(env.stage(), Stage::Idle);
    assert!(env.is_idle());
}

#[test]
fn adsr_idle_outputs_zero() {
    let mut env = envelope::adsr(0.01, 0.05, 0.7, 0.1);
    assert!(env.next(&ctx(0)).abs() < 1e-6);
}

#[test]
fn adsr_attack_reaches_one() {
    let attack_secs = 0.01;
    let mut env = envelope::adsr(attack_secs, 0.05, 0.7, 0.1);
    env.trigger();

    let attack_samples = (attack_secs * SR) as usize;
    let mut val = 0.0;
    for tick in 0..=attack_samples {
        val = env.next(&ctx(tick as u64));
    }
    assert!(
        (val - 1.0).abs() < 0.05,
        "attack should reach ~1.0, got {val}"
    );
}

#[test]
fn adsr_decay_reaches_sustain() {
    let mut env = envelope::adsr(0.001, 0.01, 0.5, 0.1);
    env.trigger();

    // Run through attack + decay.
    let total = ((0.001 + 0.01) * SR) as usize + 100; // extra margin
    let mut val = 0.0;
    for tick in 0..total {
        val = env.next(&ctx(tick as u64));
    }
    assert!(
        (val - 0.5).abs() < 0.05,
        "should settle at sustain 0.5, got {val}"
    );
    assert_eq!(env.stage(), Stage::Sustain);
}

#[test]
fn adsr_release_reaches_zero() {
    let mut env = envelope::adsr(0.001, 0.001, 0.5, 0.01);
    env.trigger();

    // Run through attack + decay to sustain.
    for tick in 0..500 {
        env.next(&ctx(tick));
    }
    assert_eq!(env.stage(), Stage::Sustain);

    env.release();

    // Run through release.
    let release_samples = (0.01 * SR) as usize + 100;
    let mut val = 0.0;
    for tick in 500..(500 + release_samples as u64) {
        val = env.next(&ctx(tick));
    }
    assert!(
        val.abs() < 0.05,
        "release should reach ~0.0, got {val}"
    );
    assert_eq!(env.stage(), Stage::Idle);
}

#[test]
fn adsr_retrigger_during_release() {
    let mut env = envelope::adsr(0.001, 0.001, 0.8, 0.05);
    env.trigger();

    // Run to sustain.
    for tick in 0..500 {
        env.next(&ctx(tick));
    }
    env.release();

    // Partially through release.
    for tick in 500..700 {
        env.next(&ctx(tick));
    }

    // Re-trigger — should restart attack from current level.
    env.trigger();
    assert_eq!(env.stage(), Stage::Attack);
}

#[test]
fn adsr_instant_attack() {
    let mut env = envelope::adsr(0.0, 0.01, 0.5, 0.1);
    env.trigger();
    let val = env.next(&ctx(0));
    assert!(
        (val - 1.0).abs() < 1e-6,
        "instant attack should hit 1.0, got {val}"
    );
}

#[test]
fn adsr_output_in_range() {
    let mut env = envelope::adsr(0.01, 0.05, 0.7, 0.1);
    env.trigger();
    for tick in 0..10000 {
        let val = env.next(&ctx(tick));
        assert!(
            val >= -1e-6 && val <= 1.0 + 1e-6,
            "ADSR sample {tick} out of range: {val}"
        );
    }
}

// ===================== Automation tests =====================

#[test]
fn automation_linear_ramp() {
    let mut sig = automation::automation(|t| (t / 2.0).min(1.0));
    // At t=0 → 0.0
    assert!(sig.next(&ctx(0)).abs() < 1e-6);
    // At t=1s (tick=44100) → 0.5
    let val = sig.next(&ctx(44100));
    assert!((val - 0.5).abs() < 0.01, "expected ~0.5 at t=1s, got {val}");
}

#[test]
fn automation_as_modulator() {
    // Use automation to modulate frequency: ramp from 220 to 440 over 1 second.
    let freq_ramp = automation::automation(|t| 220.0 + 220.0 * (t / 1.0).min(1.0));
    let mut sig = nyx_core::osc::sine(freq_ramp);
    let buf = render_to_buffer(&mut sig, 0.01, SR);
    assert!(!buf.is_empty());
}

#[test]
fn follow_multiplies_by_automation() {
    // Constant signal * ramp = ramp
    let mut sig = (|_ctx: &AudioContext| 1.0_f32).follow(|t| t);
    // At t=0 → 0.0
    assert!(sig.next(&ctx(0)).abs() < 1e-6);
    // At t=1s → 1.0
    let val = sig.next(&ctx(44100));
    assert!((val - 1.0).abs() < 0.01, "expected ~1.0 at t=1s, got {val}");
}

#[test]
fn follow_with_oscillator() {
    // Sine fading in over 0.5 seconds.
    let mut sig = nyx_core::osc::sine(440.0).follow(|t| (t / 0.5).min(1.0));
    // First sample should be near zero (sine starts at 0 AND fade starts at 0).
    let first = sig.next(&ctx(0));
    assert!(first.abs() < 1e-6);
    // After 0.5 seconds, fade is at 1.0, so output is full sine amplitude.
    let buf = render_to_buffer(&mut sig, 0.5, SR);
    let last_rms: f32 = {
        let tail = &buf[buf.len() - 1000..];
        (tail.iter().map(|s| s * s).sum::<f32>() / tail.len() as f32).sqrt()
    };
    assert!(
        last_rms > 0.5,
        "after fade-in, RMS should be substantial, got {last_rms}"
    );
}
