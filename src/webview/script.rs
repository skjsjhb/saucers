use saucer_sys::*;

/// The time that an injected script is executed.
pub enum ScriptTime {
    Creation,
    Ready,
}

impl From<ScriptTime> for saucer_script_time {
    fn from(value: ScriptTime) -> Self {
        match value {
            ScriptTime::Creation => SAUCER_SCRIPT_TIME_CREATION,
            ScriptTime::Ready => SAUCER_SCRIPT_TIME_READY,
        }
    }
}

pub struct ScriptId(usize);

impl ScriptId {
    pub(crate) fn from_usize(id: usize) -> Self { Self(id) }

    pub(crate) fn as_usize(&self) -> usize { self.0 }
}
