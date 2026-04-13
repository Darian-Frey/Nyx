//! A rotary knob widget drawn with iced `Canvas`.

use iced::mouse;
use iced::widget::canvas::{self, Canvas, Frame, Path, Stroke};
use iced::{Element, Length, Point, Rectangle, Theme};

use crate::theme::NyxColors;

/// State for a rotary knob.
#[derive(Debug, Clone)]
pub struct KnobState {
    /// Current value in [0, 1].
    pub value: f32,
}

impl Default for KnobState {
    fn default() -> Self {
        Self {
            value: 0.5,
        }
    }
}

impl KnobState {
    pub fn new(value: f32) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
        }
    }
}

/// Messages produced by the knob.
#[derive(Debug, Clone, Copy)]
pub enum KnobMessage {
    Changed(f32),
}

/// A rotary knob widget.
pub struct Knob<'a> {
    state: &'a KnobState,
    size: f32,
}

impl<'a> Knob<'a> {
    pub fn new(state: &'a KnobState) -> Self {
        Self { state, size: 60.0 }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Render as an iced `Element`.
    pub fn view(self) -> Element<'a, KnobMessage> {
        Canvas::new(KnobCanvas {
            value: self.state.value,
        })
        .width(Length::Fixed(self.size))
        .height(Length::Fixed(self.size))
        .into()
    }
}

struct KnobCanvas {
    value: f32,
}

impl canvas::Program<KnobMessage> for KnobCanvas {
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
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let radius = bounds.width.min(bounds.height) / 2.0 - 4.0;

        // Track arc (270° sweep, starting from 135°)
        let start_angle = std::f32::consts::FRAC_PI_4 * 3.0; // 135°
        let sweep = std::f32::consts::FRAC_PI_2 * 3.0; // 270°

        // Background track
        let track = Path::circle(center, radius);
        frame.stroke(
            &track,
            Stroke::default()
                .with_color(NyxColors::TRACK)
                .with_width(4.0),
        );

        // Value indicator: a line from center toward the current angle
        let value_angle = start_angle + sweep * self.value;
        let tip = Point::new(
            center.x + value_angle.cos() * radius,
            center.y + value_angle.sin() * radius,
        );
        let indicator_start = Point::new(
            center.x + value_angle.cos() * (radius * 0.4),
            center.y + value_angle.sin() * (radius * 0.4),
        );
        let indicator = Path::line(indicator_start, tip);
        frame.stroke(
            &indicator,
            Stroke::default()
                .with_color(NyxColors::FILL)
                .with_width(3.0),
        );

        // Center dot
        let dot = Path::circle(center, 3.0);
        frame.fill(&dot, NyxColors::ACCENT);

        vec![frame.into_geometry()]
    }
}
