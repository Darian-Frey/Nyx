//! Oscilloscope canvas widget consuming a `ScopeHandle`.

use iced::mouse;
use iced::widget::canvas::{self, Canvas, Frame, Path, Stroke};
use iced::{Element, Length, Point, Rectangle, Theme};

use crate::theme::NyxColors;
use nyx_core::ScopeHandle;

/// An oscilloscope canvas that renders waveform data from a `ScopeHandle`.
pub struct OscilloscopeCanvas {
    samples: Vec<f32>,
    width: f32,
    height: f32,
}

impl OscilloscopeCanvas {
    /// Create a new oscilloscope canvas.
    ///
    /// `buffer_size` determines how many samples to display at once.
    pub fn new(buffer_size: usize) -> Self {
        Self {
            samples: vec![0.0; buffer_size],
            width: 400.0,
            height: 200.0,
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

    /// Pull new samples from the scope handle. Call this in your `update`.
    ///
    /// Uses a sliding-window buffer: any samples available are appended to
    /// the tail while older samples shift left, so the display always shows
    /// the most recent `buffer_size` samples. Prevents the "half-filled
    /// scope" bug when the audio callback hasn't produced a full buffer's
    /// worth of samples between render frames.
    pub fn update(&mut self, handle: &mut ScopeHandle) {
        let buf_len = self.samples.len();
        let mut scratch = vec![0.0_f32; buf_len];
        let n = handle.read(&mut scratch);
        if n == 0 {
            return;
        }
        if n >= buf_len {
            self.samples.copy_from_slice(&scratch[n - buf_len..n]);
        } else {
            self.samples.rotate_left(n);
            self.samples[buf_len - n..].copy_from_slice(&scratch[..n]);
        }
    }

    /// Render as an iced `Element`.
    pub fn view(&self) -> Element<'_, ()> {
        Canvas::new(OscilloscopeProgram {
            samples: &self.samples,
        })
        .width(Length::Fixed(self.width))
        .height(Length::Fixed(self.height))
        .into()
    }
}

struct OscilloscopeProgram<'a> {
    samples: &'a [f32],
}

impl<'a> canvas::Program<()> for OscilloscopeProgram<'a> {
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

        // Center line
        let center_y = bounds.height / 2.0;
        let center_line = Path::line(
            Point::new(0.0, center_y),
            Point::new(bounds.width, center_y),
        );
        frame.stroke(
            &center_line,
            Stroke::default()
                .with_color(NyxColors::BORDER)
                .with_width(1.0),
        );

        // Waveform
        if self.samples.len() >= 2 {
            let step = bounds.width / (self.samples.len() - 1) as f32;
            let mut builder = canvas::path::Builder::new();
            for (i, &sample) in self.samples.iter().enumerate() {
                let x = i as f32 * step;
                let y = center_y - sample * (bounds.height / 2.0 - 2.0);
                if i == 0 {
                    builder.move_to(Point::new(x, y));
                } else {
                    builder.line_to(Point::new(x, y));
                }
            }
            let waveform = builder.build();
            frame.stroke(
                &waveform,
                Stroke::default()
                    .with_color(NyxColors::WAVEFORM)
                    .with_width(1.5),
            );
        }

        vec![frame.into_geometry()]
    }
}
