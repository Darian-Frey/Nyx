//! XY Pad widget: a 2D control surface drawn with iced `Canvas`.

use iced::mouse;
use iced::widget::canvas::{self, Canvas, Event, Frame, Path, Stroke};
use iced::{Element, Length, Point, Rectangle, Theme};

use crate::theme::NyxColors;

/// State for an XY pad.
#[derive(Debug, Clone)]
pub struct XYPadState {
    /// X value in [0, 1].
    pub x: f32,
    /// Y value in [0, 1].
    pub y: f32,
}

impl Default for XYPadState {
    fn default() -> Self {
        Self { x: 0.5, y: 0.5 }
    }
}

impl XYPadState {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x: x.clamp(0.0, 1.0),
            y: y.clamp(0.0, 1.0),
        }
    }
}

/// Messages produced by the XY pad.
#[derive(Debug, Clone, Copy)]
pub enum XYPadMessage {
    Changed { x: f32, y: f32 },
}

/// Per-instance interaction state.
#[derive(Default)]
pub struct XYPadInteraction {
    dragging: bool,
}

/// An XY pad widget.
pub struct XYPad<'a> {
    state: &'a XYPadState,
    size: f32,
}

impl<'a> XYPad<'a> {
    pub fn new(state: &'a XYPadState) -> Self {
        Self { state, size: 200.0 }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn view(self) -> Element<'a, XYPadMessage> {
        Canvas::new(XYPadCanvas {
            x: self.state.x,
            y: self.state.y,
        })
        .width(Length::Fixed(self.size))
        .height(Length::Fixed(self.size))
        .into()
    }
}

struct XYPadCanvas {
    x: f32,
    y: f32,
}

impl canvas::Program<XYPadMessage> for XYPadCanvas {
    type State = XYPadInteraction;

    fn update(
        &self,
        state: &mut Self::State,
        event: Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<XYPadMessage>) {
        let Some(pos) = cursor.position_in(bounds) else {
            if let Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) = event {
                state.dragging = false;
            }
            return (canvas::event::Status::Ignored, None);
        };

        let compute_xy = |pos: Point| {
            let pad = 2.0;
            let x = ((pos.x - pad) / (bounds.width - 2.0 * pad)).clamp(0.0, 1.0);
            let y = (1.0 - (pos.y - pad) / (bounds.height - 2.0 * pad)).clamp(0.0, 1.0);
            (x, y)
        };

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                state.dragging = true;
                let (x, y) = compute_xy(pos);
                (
                    canvas::event::Status::Captured,
                    Some(XYPadMessage::Changed { x, y }),
                )
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
                (canvas::event::Status::Captured, None)
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if state.dragging => {
                let (x, y) = compute_xy(pos);
                (
                    canvas::event::Status::Captured,
                    Some(XYPadMessage::Changed { x, y }),
                )
            }
            _ => (canvas::event::Status::Ignored, None),
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let pad = 2.0;

        // Background
        let bg = Path::rectangle(Point::ORIGIN, bounds.size());
        frame.fill(&bg, NyxColors::BG_SURFACE);
        frame.stroke(&bg, Stroke::default().with_color(NyxColors::BORDER).with_width(1.0));

        // Crosshair
        let px = pad + self.x * (bounds.width - 2.0 * pad);
        let py = bounds.height - pad - self.y * (bounds.height - 2.0 * pad);

        let crosshair_color = iced::Color::from_rgba(
            NyxColors::ACCENT.r,
            NyxColors::ACCENT.g,
            NyxColors::ACCENT.b,
            0.3,
        );
        let stroke = Stroke::default().with_color(crosshair_color).with_width(1.0);

        let h_line = Path::line(Point::new(pad, py), Point::new(bounds.width - pad, py));
        let v_line = Path::line(Point::new(px, pad), Point::new(px, bounds.height - pad));
        frame.stroke(&h_line, stroke);
        frame.stroke(&v_line, stroke);

        // Cursor dot
        let dot = Path::circle(Point::new(px, py), 6.0);
        frame.fill(&dot, NyxColors::ACCENT);

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.dragging {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}
