use nyx_iced::{
    KnobState, SliderState, XYPadState, NyxColors,
    OscilloscopeCanvas, SpectrumCanvas,
};
use nyx_iced::theme::lerp_color;
use iced::Color;

// ===================== Widget state tests =====================

#[test]
fn knob_state_default() {
    let state = KnobState::default();
    assert!((state.value - 0.5).abs() < f32::EPSILON);
}

#[test]
fn knob_state_clamps() {
    let state = KnobState::new(2.0);
    assert!((state.value - 1.0).abs() < f32::EPSILON);
    let state = KnobState::new(-1.0);
    assert!(state.value.abs() < f32::EPSILON);
}

#[test]
fn slider_state_default() {
    let state = SliderState::default();
    assert!((state.value - 0.5).abs() < f32::EPSILON);
}

#[test]
fn slider_state_clamps() {
    let state = SliderState::new(1.5);
    assert!((state.value - 1.0).abs() < f32::EPSILON);
}

#[test]
fn xypad_state_default() {
    let state = XYPadState::default();
    assert!((state.x - 0.5).abs() < f32::EPSILON);
    assert!((state.y - 0.5).abs() < f32::EPSILON);
}

#[test]
fn xypad_state_clamps() {
    let state = XYPadState::new(2.0, -1.0);
    assert!((state.x - 1.0).abs() < f32::EPSILON);
    assert!(state.y.abs() < f32::EPSILON);
}

// ===================== Theme tests =====================

#[test]
fn nyx_colors_are_valid() {
    // Just verify they're constructable and in range.
    let colors = [
        NyxColors::BG_DARK,
        NyxColors::BG_SURFACE,
        NyxColors::BORDER,
        NyxColors::TEXT,
        NyxColors::TEXT_DIM,
        NyxColors::ACCENT,
        NyxColors::WARM,
        NyxColors::WAVEFORM,
        NyxColors::SPECTRUM_LOW,
        NyxColors::SPECTRUM_HIGH,
        NyxColors::TRACK,
        NyxColors::FILL,
    ];
    for c in colors {
        assert!(c.r >= 0.0 && c.r <= 1.0);
        assert!(c.g >= 0.0 && c.g <= 1.0);
        assert!(c.b >= 0.0 && c.b <= 1.0);
    }
}

#[test]
fn lerp_color_endpoints() {
    let a = Color::from_rgb(0.0, 0.0, 0.0);
    let b = Color::from_rgb(1.0, 1.0, 1.0);

    let at_0 = lerp_color(a, b, 0.0);
    assert!(at_0.r.abs() < 1e-6);

    let at_1 = lerp_color(a, b, 1.0);
    assert!((at_1.r - 1.0).abs() < 1e-6);

    let mid = lerp_color(a, b, 0.5);
    assert!((mid.r - 0.5).abs() < 1e-6);
}

#[test]
fn lerp_color_clamps() {
    let a = Color::from_rgb(0.0, 0.0, 0.0);
    let b = Color::from_rgb(1.0, 1.0, 1.0);

    let over = lerp_color(a, b, 2.0);
    assert!((over.r - 1.0).abs() < 1e-6); // clamped to 1.0

    let under = lerp_color(a, b, -1.0);
    assert!(under.r.abs() < 1e-6); // clamped to 0.0
}

// ===================== Canvas constructors =====================

#[test]
fn oscilloscope_canvas_creates() {
    let canvas = OscilloscopeCanvas::new(1024);
    // Just verify it constructs without panicking.
    let _ = canvas;
}

#[test]
fn spectrum_canvas_creates() {
    let canvas = SpectrumCanvas::new(64);
    let _ = canvas;
}
