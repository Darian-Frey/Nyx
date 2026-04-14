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

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::canvas::Program;

    const H_BOUNDS: Rectangle = Rectangle { x: 0.0, y: 0.0, width: 200.0, height: 24.0 };
    const V_BOUNDS: Rectangle = Rectangle { x: 0.0, y: 0.0, width: 24.0, height: 200.0 };

    fn cursor_at(x: f32, y: f32) -> mouse::Cursor {
        mouse::Cursor::Available(Point::new(x, y))
    }

    fn press() -> Event {
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
    }

    fn release() -> Event {
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
    }

    fn move_to(x: f32, y: f32) -> Event {
        Event::Mouse(mouse::Event::CursorMoved { position: Point::new(x, y) })
    }

    // ─── HSlider ────────────────────────────────────────────────────

    #[test]
    fn hslider_click_at_left_gives_zero() {
        let canvas = HSliderCanvas { value: 0.5 };
        let mut state = SliderInteraction::default();
        // Click at x=4 (pad=4), should give value 0
        let (_, msg) = canvas.update(&mut state, press(), H_BOUNDS, cursor_at(4.0, 12.0));
        if let Some(SliderMessage::Changed(v)) = msg {
            assert!(v.abs() < 0.01, "expected ~0, got {v}");
        } else {
            panic!("expected Changed");
        }
    }

    #[test]
    fn hslider_click_at_right_gives_one() {
        let canvas = HSliderCanvas { value: 0.5 };
        let mut state = SliderInteraction::default();
        // Click at x=196 (bounds.width - pad), should give value 1
        let (_, msg) = canvas.update(&mut state, press(), H_BOUNDS, cursor_at(196.0, 12.0));
        if let Some(SliderMessage::Changed(v)) = msg {
            assert!((v - 1.0).abs() < 0.01, "expected ~1, got {v}");
        } else {
            panic!("expected Changed");
        }
    }

    #[test]
    fn hslider_click_at_middle_gives_half() {
        let canvas = HSliderCanvas { value: 0.0 };
        let mut state = SliderInteraction::default();
        let (_, msg) = canvas.update(&mut state, press(), H_BOUNDS, cursor_at(100.0, 12.0));
        if let Some(SliderMessage::Changed(v)) = msg {
            assert!((v - 0.5).abs() < 0.02, "expected ~0.5, got {v}");
        } else {
            panic!("expected Changed");
        }
    }

    #[test]
    fn hslider_drag_updates_continuously() {
        let canvas = HSliderCanvas { value: 0.0 };
        let mut state = SliderInteraction::default();

        canvas.update(&mut state, press(), H_BOUNDS, cursor_at(50.0, 12.0));
        let (_, msg) = canvas.update(&mut state, move_to(150.0, 12.0), H_BOUNDS, cursor_at(150.0, 12.0));
        if let Some(SliderMessage::Changed(v)) = msg {
            assert!(v > 0.6, "drag to right should increase value, got {v}");
        } else {
            panic!("expected Changed during drag");
        }
    }

    #[test]
    fn hslider_release_stops_drag() {
        let canvas = HSliderCanvas { value: 0.5 };
        let mut state = SliderInteraction::default();
        canvas.update(&mut state, press(), H_BOUNDS, cursor_at(50.0, 12.0));
        assert!(state.dragging);
        canvas.update(&mut state, release(), H_BOUNDS, cursor_at(50.0, 12.0));
        assert!(!state.dragging);
    }

    #[test]
    fn hslider_drag_without_press_ignored() {
        let canvas = HSliderCanvas { value: 0.5 };
        let mut state = SliderInteraction::default();
        let (_, msg) = canvas.update(&mut state, move_to(50.0, 12.0), H_BOUNDS, cursor_at(50.0, 12.0));
        assert!(msg.is_none());
    }

    #[test]
    fn hslider_press_outside_bounds_ignored() {
        let canvas = HSliderCanvas { value: 0.5 };
        let mut state = SliderInteraction::default();
        let (status, msg) = canvas.update(&mut state, press(), H_BOUNDS, cursor_at(300.0, 300.0));
        assert!(matches!(status, canvas::event::Status::Ignored));
        assert!(msg.is_none());
    }

    // ─── VSlider ────────────────────────────────────────────────────

    #[test]
    fn vslider_click_at_bottom_gives_zero() {
        let canvas = VSliderCanvas { value: 0.5 };
        let mut state = SliderInteraction::default();
        // Click at y=196 (bottom - pad), should give value 0
        let (_, msg) = canvas.update(&mut state, press(), V_BOUNDS, cursor_at(12.0, 196.0));
        if let Some(SliderMessage::Changed(v)) = msg {
            assert!(v.abs() < 0.02, "expected ~0 at bottom, got {v}");
        } else {
            panic!("expected Changed");
        }
    }

    #[test]
    fn vslider_click_at_top_gives_one() {
        let canvas = VSliderCanvas { value: 0.5 };
        let mut state = SliderInteraction::default();
        let (_, msg) = canvas.update(&mut state, press(), V_BOUNDS, cursor_at(12.0, 4.0));
        if let Some(SliderMessage::Changed(v)) = msg {
            assert!((v - 1.0).abs() < 0.02, "expected ~1 at top, got {v}");
        } else {
            panic!("expected Changed");
        }
    }

    #[test]
    fn vslider_click_at_middle_gives_half() {
        let canvas = VSliderCanvas { value: 0.0 };
        let mut state = SliderInteraction::default();
        let (_, msg) = canvas.update(&mut state, press(), V_BOUNDS, cursor_at(12.0, 100.0));
        if let Some(SliderMessage::Changed(v)) = msg {
            assert!((v - 0.5).abs() < 0.02, "expected ~0.5, got {v}");
        } else {
            panic!("expected Changed");
        }
    }

    #[test]
    fn vslider_drag_up_increases() {
        let canvas = VSliderCanvas { value: 0.0 };
        let mut state = SliderInteraction::default();
        canvas.update(&mut state, press(), V_BOUNDS, cursor_at(12.0, 150.0));
        let (_, msg) = canvas.update(&mut state, move_to(12.0, 50.0), V_BOUNDS, cursor_at(12.0, 50.0));
        if let Some(SliderMessage::Changed(v)) = msg {
            assert!(v > 0.6, "drag up should increase, got {v}");
        } else {
            panic!("expected Changed");
        }
    }
}
