#[global_allocator]
static ALLOC: nyx_core::GuardedAllocator = nyx_core::GuardedAllocator;

use nyx_core::{AudioContext, DenyAllocGuard, Signal};

fn ctx() -> AudioContext {
    AudioContext {
        sample_rate: 44100.0,
        tick: 0,
    }
}

struct PureSine {
    phase: f32,
    freq: f32,
}

impl Signal for PureSine {
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
fn pure_signal_does_not_allocate() {
    let mut sig = PureSine {
        phase: 0.0,
        freq: 440.0,
    };
    let c = ctx();
    let _guard = DenyAllocGuard::new();
    // Run 1024 samples — if any allocates, the guard panics.
    for _ in 0..1024 {
        sig.next(&c);
    }
}

#[test]
#[should_panic(expected = "no-alloc zone")]
fn boxed_allocation_panics_under_guard() {
    let _guard = DenyAllocGuard::new();
    // This Box::new forces a heap allocation — the guard should catch it.
    let _b = Box::new(42_u64);
}

#[test]
fn allocation_allowed_outside_guard() {
    // No guard active — allocation is fine.
    let _v: Vec<f32> = Vec::with_capacity(1024);
}

#[test]
fn guard_is_scoped() {
    {
        let _guard = DenyAllocGuard::new();
        // Inside guard — no alloc.
    }
    // Guard dropped — allocation should work again.
    let _v: Vec<f32> = Vec::with_capacity(64);
}
