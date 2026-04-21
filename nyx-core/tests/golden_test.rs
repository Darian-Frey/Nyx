use nyx_core::golden::{GoldenTest, assert_golden};
use nyx_core::{AudioContext, Signal};

struct TestSine {
    phase: f32,
    freq: f32,
}

impl Signal for TestSine {
    fn next(&mut self, ctx: &AudioContext) -> f32 {
        let sample = (self.phase * std::f32::consts::TAU).sin();
        self.phase += self.freq / ctx.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        sample
    }
}

#[test]
fn golden_sine_440() {
    let mut sig = TestSine {
        phase: 0.0,
        freq: 440.0,
    };
    assert_golden(
        &mut sig,
        &GoldenTest {
            name: "sine_440",
            duration_secs: 0.01, // short — just enough to verify
            sample_rate: 44100.0,
            tolerance: 1e-6,
        },
    );
}

#[test]
fn golden_silence() {
    let mut sig = |_ctx: &AudioContext| 0.0_f32;
    assert_golden(
        &mut sig,
        &GoldenTest {
            name: "silence",
            duration_secs: 0.01,
            sample_rate: 44100.0,
            tolerance: 0.0,
        },
    );
}
