//! A rotary knob widget drawn with iced `Canvas`.

use iced::mouse;
use iced::widget::canvas::{self, Canvas, Event, Frame, Path, Stroke};
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
        Self { value: 0.5 }
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

/// Per-instance interaction state tracked by the canvas.
#[derive(Default)]
pub struct KnobInteraction {
    dragging: bool,
    last_y: f32,
}

impl canvas::Program<KnobMessage> for KnobCanvas {
    type State = KnobInteraction;

    fn update(
        &self,
        state: &mut Self::State,
        event: Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<KnobMessage>) {
        let Some(pos) = cursor.position_in(bounds) else {
            return (canvas::event::Status::Ignored, None);
        };

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                state.dragging = true;
                state.last_y = pos.y;
                (canvas::event::Status::Captured, None)
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
                (canvas::event::Status::Captured, None)
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if state.dragging => {
                let dy = state.last_y - pos.y;
                state.last_y = pos.y;
                // Sensitivity: full drag over the widget height = 0→1
                let sensitivity = 1.0 / bounds.height;
                let new_value = (self.value + dy * sensitivity).clamp(0.0, 1.0);
                (
                    canvas::event::Status::Captured,
                    Some(KnobMessage::Changed(new_value)),
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
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let radius = bounds.width.min(bounds.height) / 2.0 - 4.0;

        let start_angle = std::f32::consts::FRAC_PI_4 * 3.0;
        let sweep = std::f32::consts::FRAC_PI_2 * 3.0;

        // Background track
        let track = Path::circle(center, radius);
        frame.stroke(
            &track,
            Stroke::default()
                .with_color(NyxColors::TRACK)
                .with_width(4.0),
        );

        // Value indicator line
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

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.dragging {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::canvas::Program;

    const BOUNDS: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
    };

    fn cursor_at(x: f32, y: f32) -> mouse::Cursor {
        mouse::Cursor::Available(Point::new(x, y))
    }

    fn press_left() -> Event {
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
    }

    fn release_left() -> Event {
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
    }

    fn move_to(x: f32, y: f32) -> Event {
        Event::Mouse(mouse::Event::CursorMoved {
            position: Point::new(x, y),
        })
    }

    #[test]
    fn press_outside_bounds_ignored() {
        let canvas = KnobCanvas { value: 0.5 };
        let mut state = KnobInteraction::default();
        let (status, msg) =
            canvas.update(&mut state, press_left(), BOUNDS, cursor_at(200.0, 200.0));
        assert!(matches!(status, canvas::event::Status::Ignored));
        assert!(msg.is_none());
    }

    #[test]
    fn press_inside_starts_drag() {
        let canvas = KnobCanvas { value: 0.5 };
        let mut state = KnobInteraction::default();
        let (status, msg) = canvas.update(&mut state, press_left(), BOUNDS, cursor_at(50.0, 50.0));
        assert!(matches!(status, canvas::event::Status::Captured));
        assert!(msg.is_none()); // press alone doesn't emit a change
    }

    #[test]
    fn drag_up_increases_value() {
        let canvas = KnobCanvas { value: 0.5 };
        let mut state = KnobInteraction::default();

        // Press at y=60
        canvas.update(&mut state, press_left(), BOUNDS, cursor_at(50.0, 60.0));

        // Drag up to y=40 (dy = 20, upward)
        let (status, msg) = canvas.update(
            &mut state,
            move_to(50.0, 40.0),
            BOUNDS,
            cursor_at(50.0, 40.0),
        );

        assert!(matches!(status, canvas::event::Status::Captured));
        match msg {
            Some(KnobMessage::Changed(v)) => {
                assert!(v > 0.5, "dragging up should increase value, got {v}");
            }
            None => panic!("expected a Changed message on drag"),
        }
    }

    #[test]
    fn drag_down_decreases_value() {
        let canvas = KnobCanvas { value: 0.5 };
        let mut state = KnobInteraction::default();

        canvas.update(&mut state, press_left(), BOUNDS, cursor_at(50.0, 40.0));
        let (_, msg) = canvas.update(
            &mut state,
            move_to(50.0, 60.0),
            BOUNDS,
            cursor_at(50.0, 60.0),
        );

        match msg {
            Some(KnobMessage::Changed(v)) => {
                assert!(v < 0.5, "dragging down should decrease value, got {v}");
            }
            None => panic!("expected Changed"),
        }
    }

    #[test]
    fn drag_without_press_ignored() {
        let canvas = KnobCanvas { value: 0.5 };
        let mut state = KnobInteraction::default();
        // No press first — just move
        let (_, msg) = canvas.update(
            &mut state,
            move_to(50.0, 40.0),
            BOUNDS,
            cursor_at(50.0, 40.0),
        );
        assert!(msg.is_none());
    }

    #[test]
    fn release_stops_dragging() {
        let canvas = KnobCanvas { value: 0.5 };
        let mut state = KnobInteraction::default();

        canvas.update(&mut state, press_left(), BOUNDS, cursor_at(50.0, 50.0));
        assert!(state.dragging);

        canvas.update(&mut state, release_left(), BOUNDS, cursor_at(50.0, 50.0));
        assert!(!state.dragging);
    }

    #[test]
    fn value_clamps_to_range() {
        // Start at the top of range, drag up hard — should clamp to 1.0.
        let canvas = KnobCanvas { value: 0.95 };
        let mut state = KnobInteraction::default();

        canvas.update(&mut state, press_left(), BOUNDS, cursor_at(50.0, 90.0));
        let (_, msg) = canvas.update(
            &mut state,
            move_to(50.0, 10.0),
            BOUNDS,
            cursor_at(50.0, 10.0),
        );

        if let Some(KnobMessage::Changed(v)) = msg {
            assert!(
                (0.0..=1.0).contains(&v),
                "value must stay in [0, 1], got {v}"
            );
        }
    }

    #[test]
    fn mouse_interaction_changes_with_state() {
        let canvas = KnobCanvas { value: 0.5 };
        let mut state = KnobInteraction::default();

        // Not dragging, not over bounds → default
        let i = canvas.mouse_interaction(&state, BOUNDS, cursor_at(200.0, 200.0));
        assert!(matches!(
            i,
            mouse::Interaction::None | mouse::Interaction::Idle
        ));

        // Not dragging, over bounds → Grab
        let i = canvas.mouse_interaction(&state, BOUNDS, cursor_at(50.0, 50.0));
        assert!(matches!(i, mouse::Interaction::Grab));

        // Dragging → Grabbing
        state.dragging = true;
        let i = canvas.mouse_interaction(&state, BOUNDS, cursor_at(50.0, 50.0));
        assert!(matches!(i, mouse::Interaction::Grabbing));
    }
}
