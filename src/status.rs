use saucer_sys::*;

/// A status returned by handler, describing whether a event has been handled.
pub enum HandleStatus {
    Handled,
    Unhandled,
}

impl From<HandleStatus> for saucer_status {
    fn from(value: HandleStatus) -> Self {
        match value {
            HandleStatus::Handled => SAUCER_STATE_HANDLED,
            HandleStatus::Unhandled => SAUCER_STATE_UNHANDLED,
        }
    }
}
