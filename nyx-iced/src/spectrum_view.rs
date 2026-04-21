//! Spectrum analyser canvas widget consuming a `SpectrumHandle`.

use iced::mouse;
use iced::widget::canvas::{self, Canvas, Frame, Path};
use iced::{Element, Length, Point, Rectangle, Size, Theme};

use crate::theme::{NyxColors, lerp_color};
use nyx_core::{FreqBin, SpectrumHandle};

/// A spectrum analyser canvas that renders FFT magnitude bins.
pub struct SpectrumCanvas {
    bins: Vec<FreqBin>,
    width: f32,
    height: f32,
    /// Number of bars to display (bins are grouped).
    bar_count: usize,
}

impl SpectrumCanvas {
    /// Create a new spectrum canvas.
    pub fn new(bar_count: usize) -> Self {
        Self {
            bins: Vec::new(),
            width: 400.0,
            height: 200.0,
            bar_count,
        }
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }

    /// Pull new spectrum data from the handle. Call this in your `update`.
    pub fn update(&mut self, handle: &SpectrumHandle) {
        self.bins = handle.snapshot();
    }

    /// Render as an iced `Element`.
    pub fn view(&self) -> Element<'_, ()> {
        Canvas::new(SpectrumProgram {
            bins: &self.bins,
            bar_count: self.bar_count,
        })
        .width(Length::Fixed(self.width))
        .height(Length::Fixed(self.height))
        .into()
    }
}

struct SpectrumProgram<'a> {
    bins: &'a [FreqBin],
    bar_count: usize,
}

impl<'a> canvas::Program<()> for SpectrumProgram<'a> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        // Background
        let bg = Path::rectangle(Point::ORIGIN, bounds.size());
        frame.fill(&bg, NyxColors::BG_DARK);

        if self.bins.is_empty() || self.bar_count == 0 {
            return vec![frame.into_geometry()];
        }

        let bar_count = self.bar_count.min(self.bins.len());
        let bar_width = bounds.width / bar_count as f32;
        let bins_per_bar = self.bins.len() / bar_count;
        let gap = 1.0_f32;

        // Find max magnitude for normalisation.
        let max_mag = self
            .bins
            .iter()
            .map(|b| b.magnitude)
            .fold(0.0_f32, f32::max)
            .max(1e-10);

        for i in 0..bar_count {
            let start = i * bins_per_bar;
            let end = (start + bins_per_bar).min(self.bins.len());
            let avg_mag: f32 = self.bins[start..end]
                .iter()
                .map(|b| b.magnitude)
                .sum::<f32>()
                / (end - start) as f32;

            let normalized = (avg_mag / max_mag).clamp(0.0, 1.0);
            let bar_height = normalized * (bounds.height - 4.0);
            let x = i as f32 * bar_width + gap / 2.0;
            let y = bounds.height - bar_height;

            let color = lerp_color(
                NyxColors::SPECTRUM_LOW,
                NyxColors::SPECTRUM_HIGH,
                normalized,
            );
            let bar = Path::rectangle(
                Point::new(x, y),
                Size::new((bar_width - gap).max(1.0), bar_height),
            );
            frame.fill(&bar, color);
        }

        vec![frame.into_geometry()]
    }
}
