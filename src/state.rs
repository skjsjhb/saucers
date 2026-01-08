use saucer_sys::*;

/// An enum describing the load status of a web page.
pub enum LoadState {
    Started,
    Finished,
}

impl From<saucer_state> for LoadState {
    fn from(value: saucer_state) -> Self {
        match value {
            SAUCER_STATE_STARTED => Self::Started,
            SAUCER_STATE_FINISHED => Self::Finished,
            _ => unreachable!(),
        }
    }
}
