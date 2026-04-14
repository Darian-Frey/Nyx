//! Nannou oscilloscope — live waveform visualiser.
//!
//! Plays a detuned saw bass with wobble, draws the waveform via Nannou.
//!
//! Run: cargo run -p nyx-examples --example nannou_scope --release

use nannou::prelude::*;
use nyx_prelude::*;

const SCOPE_SAMPLES: usize = 2048;

struct Model {
    _engine: Engine,
    scope: ScopeHandle,
    buffer: Vec<f32>,
}

fn main() {
    nannou::app(model).update(update).run();
}

fn model(app: &App) -> Model {
    app.new_window().size(800, 400).view(view).build().unwrap();

    // Wobbling saw bass — good visual variety on the scope.
    let lfo = osc::sine(1.5).amp(800.0).offset(1000.0);
    let sig = osc::saw(55.0)
        .add(osc::saw(55.3).amp(0.6))
        .lowpass(lfo, 3.0)
        .soft_clip(1.5)
        .amp(0.3);

    let (sig, scope) = sig.scope(SCOPE_SAMPLES * 4);
    let engine = Engine::play(sig).expect("failed to open audio device");

    Model {
        _engine: engine,
        scope,
        buffer: vec![0.0; SCOPE_SAMPLES],
    }
}

fn update(_app: &App, model: &mut Model, _update: Update) {
    // Pull all available samples. At ~60 fps with 44.1 kHz audio we get
    // ~735 new samples per frame, so we shift the window left and append
    // the new samples to keep a rolling view of the most recent audio.
    let mut scratch = [0.0_f32; SCOPE_SAMPLES];
    let n = model.scope.read(&mut scratch);
    if n == 0 {
        return;
    }
    let buf_len = model.buffer.len();
    if n >= buf_len {
        model.buffer.copy_from_slice(&scratch[n - buf_len..n]);
    } else {
        model.buffer.rotate_left(n);
        model.buffer[buf_len - n..].copy_from_slice(&scratch[..n]);
    }
}

fn view(app: &App, model: &Model, frame: Frame) {
    let draw = app.draw();
    let win = app.window_rect();
    draw.background().color(rgb(0.08, 0.08, 0.10));

    // Draw the waveform as a connected polyline.
    let h = win.h() * 0.4;
    let w = win.w();
    let points = model.buffer.iter().enumerate().map(|(i, &s)| {
        let x = win.left() + (i as f32 / model.buffer.len() as f32) * w;
        let y = s * h;
        pt2(x, y)
    });

    draw.polyline()
        .weight(1.5)
        .points_colored(points.map(|p| (p, rgb(0.0, 0.85, 0.95))));

    draw.to_frame(app, &frame).unwrap();
}
