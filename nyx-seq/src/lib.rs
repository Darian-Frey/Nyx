pub mod clock;
pub mod envelope;
pub mod automation;

pub use clock::{clock, Clock, ClockState};
pub use envelope::{adsr, Adsr, Stage};
pub use automation::{automation, Automation, AutomationExt, Follow};
