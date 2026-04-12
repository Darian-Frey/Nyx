pub mod clock;
pub mod envelope;
pub mod automation;
pub mod note;
pub mod scale;
pub mod chord;

pub use clock::{clock, Clock, ClockState};
pub use envelope::{adsr, Adsr, Stage};
pub use automation::{automation, Automation, AutomationExt, Follow};
pub use note::Note;
pub use scale::{Scale, ScaleMode};
pub use chord::{Chord, ChordType};
