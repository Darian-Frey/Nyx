//! Horizontal and vertical slider widgets drawn with iced `Canvas`.

use iced::mouse;
use iced::widget::canvas::{self, Canvas, Event, Frame, Path, Stroke};
use iced::{Element, Length, Point, Rectangle, Theme};

use crate::theme::NyxColors;

/// State for a slider.
#[derive(Debug, Clone)]
pub struct SliderState {
    /// Current value in [0, 1].
    pub value: f32,
}

impl Default for SliderState {
    fn default() -> Self {
        Self { value: 0.5 }
    }
}

impl SliderState {
    pub fn new(value: f32) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
        }
    }
}

/// Messages produced by sliders.
#[derive(Debug, Clone, Copy)]
pub enum SliderMessage {
    Changed(f32),
}

/// Per-instance interaction state.
#[derive(Default)]
pub struct SliderInteraction {
    dragging: bool,
}

// ─── Horizontal Slider ──────────────────────────────────────────────

pub struct HSlider<'a> {
    state: &'a SliderState,
    width: f32,
    height: f32,
}

impl<'a> HSlider<'a> {
    pub fn new(state: &'a SliderState) -> Self {
        Self {
            state,
            width: 200.0,
            height: 24.0,
        }
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    pub fn view(self) -> Element<'a, SliderMessage> {
        Canvas::new(HSliderCanvas {
            value: self.state.value,
        })
        .width(Length::Fixed(self.width))
        .height(Length::Fixed(self.height))
        .into()
    }
}

struct HSliderCanvas {
    value: f32,
}

impl canvas::Program<SliderMessage> for HSliderCanvas {
    type State = SliderInteraction;

    fn update(
        &self,
        state: &mut Self::State,
        event: Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<SliderMessage>) {
        let Some(pos) = cursor.position_in(bounds) else {
            if let Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) = event {
                state.dragging = false;
            }
            return (canvas::event::Status::Ignored, None);
        };

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                state.dragging = true;
                let pad = 4.0;
                let new_value = ((pos.x - pad) / (bounds.width - 2.0 * pad)).clamp(0.0, 1.0);
                (
                    canvas::event::Status::Captured,
                    Some(SliderMessage::Changed(new_value)),
                )
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
                (canvas::event::Status::Captured, None)
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if state.dragging => {
                let pad = 4.0;
                let new_value = ((pos.x - pad) / (bounds.width - 2.0 * pad)).clamp(0.0, 1.0);
                (
                    canvas::event::Status::Captured,
                    Some(SliderMessage::Changed(new_value)),
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
        let y = bounds.height / 2.0;
        let pad = 4.0;

        let track = Path::line(Point::new(pad, y), Point::new(bounds.width - pad, y));
        frame.stroke(&track, Stroke::default().with_color(NyxColors::TRACK).with_width(4.0));

        let fill_x = pad + self.value * (bounds.width - 2.0 * pad);
        let fill = Path::line(Point::new(pad, y), Point::new(fill_x, y));
        frame.stroke(&fill, Stroke::default().with_color(NyxColors::FILL).with_width(4.0));

        let thumb = Path::circle(Point::new(fill_x, y), 6.0);
        frame.fill(&thumb, NyxColors::ACCENT);

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
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

// ─── Vertical Slider ────────────────────────────────────────────────

pub struct VSlider<'a> {
    state: &'a SliderState,
    width: f32,
    height: f32,
}

impl<'a> VSlider<'a> {
    pub fn new(state: &'a SliderState) -> Self {
        Self {
            state,
            width: 24.0,
            height: 200.0,
        }
    }

    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }

    pub fn view(self) -> Element<'a, SliderMessage> {
        Canvas::new(VSliderCanvas {
            value: self.state.value,
        })
        .width(Length::Fixed(self.width))
        .height(Length::Fixed(self.height))
        .into()
    }
}

struct VSliderCanvas {
    value: f32,
}

impl canvas::Program<SliderMessage> for VSliderCanvas {
    type State = SliderInteraction;

    fn update(
        &self,
        state: &mut Self::State,
        event: Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<SliderMessage>) {
        let Some(pos) = cursor.position_in(bounds) else {
            if let Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) = event {
                state.dragging = false;
            }
            return (canvas::event::Status::Ignored, None);
        };

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                state.dragging = true;
                let pad = 4.0;
                // Inverted: top = 1.0, bottom = 0.0
                let new_value =
                    (1.0 - (pos.y - pad) / (bounds.height - 2.0 * pad)).clamp(0.0, 1.0);
                (
                    canvas::event::Status::Captured,
                    Some(SliderMessage::Changed(new_value)),
                )
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
                (canvas::event::Status::Captured, None)
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if state.dragging => {
                let pad = 4.0;
                let new_value =
                    (1.0 - (pos.y - pad) / (bounds.height - 2.0 * pad)).clamp(0.0, 1.0);
                (
                    canvas::event::Status::Captured,
                    Some(SliderMessage::Changed(new_value)),
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
        let x = bounds.width / 2.0;
        let pad = 4.0;

        let track = Path::line(Point::new(x, pad), Point::new(x, bounds.height - pad));
        frame.stroke(&track, Stroke::default().with_color(NyxColors::TRACK).with_width(4.0));

        let fill_y = bounds.height - pad - self.value * (bounds.height - 2.0 * pad);
        let fill = Path::line(Point::new(x, bounds.height - pad), Point::new(x, fill_y));
        frame.stroke(&fill, Stroke::default().with_color(NyxColors::FILL).with_width(4.0));

        let thumb = Path::circle(Point::new(x, fill_y), 6.0);
        frame.fill(&thumb, NyxColors::ACCENT);

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
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}
