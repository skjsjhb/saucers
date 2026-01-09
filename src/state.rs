use saucer_sys::*;

/// The load state of a web page. Used to distinguish stages in
/// [`crate::webview::WebviewEventListener::on_load`].
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
