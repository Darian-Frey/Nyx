//! Nyx Midnight Theme: deep grays, neon accent, monospace aesthetic.

use iced::Color;

/// Nyx colour palette.
pub struct NyxColors;

impl NyxColors {
    /// Deep background.
    pub const BG_DARK: Color = Color::from_rgb(0.08, 0.08, 0.10);
    /// Panel / surface background.
    pub const BG_SURFACE: Color = Color::from_rgb(0.12, 0.12, 0.15);
    /// Subtle border / divider.
    pub const BORDER: Color = Color::from_rgb(0.20, 0.20, 0.25);
    /// Primary text.
    pub const TEXT: Color = Color::from_rgb(0.85, 0.85, 0.90);
    /// Dimmed / secondary text.
    pub const TEXT_DIM: Color = Color::from_rgb(0.50, 0.50, 0.55);
    /// Neon accent (cyan-ish).
    pub const ACCENT: Color = Color::from_rgb(0.0, 0.85, 0.95);
    /// Warm accent (for peaks, warnings).
    pub const WARM: Color = Color::from_rgb(1.0, 0.45, 0.25);
    /// Waveform / signal colour.
    pub const WAVEFORM: Color = Color::from_rgb(0.0, 0.85, 0.95);
    /// Spectrum gradient low.
    pub const SPECTRUM_LOW: Color = Color::from_rgb(0.0, 0.4, 0.8);
    /// Spectrum gradient high.
    pub const SPECTRUM_HIGH: Color = Color::from_rgb(1.0, 0.3, 0.5);
    /// Knob track / inactive area.
    pub const TRACK: Color = Color::from_rgb(0.18, 0.18, 0.22);
    /// Knob / slider fill (active).
    pub const FILL: Color = Color::from_rgb(0.0, 0.85, 0.95);
}

/// Linearly interpolate between two colours.
pub fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::from_rgba(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}
