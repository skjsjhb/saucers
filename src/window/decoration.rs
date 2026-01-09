use saucer_sys::*;

/// Describes the decoration status of a window.
pub enum WindowDecoration {
    None,
    Partial,
    Full,
}

impl From<saucer_window_decoration> for WindowDecoration {
    fn from(value: saucer_window_decoration) -> Self {
        match value {
            SAUCER_WINDOW_DECORATION_NONE => Self::None,
            SAUCER_WINDOW_DECORATION_PARTIAL => Self::Partial,
            SAUCER_WINDOW_DECORATION_FULL => Self::Full,
            _ => unreachable!(),
        }
    }
}

impl From<WindowDecoration> for saucer_window_decoration {
    fn from(value: WindowDecoration) -> Self {
        use WindowDecoration::*;
        match value {
            None => SAUCER_WINDOW_DECORATION_NONE,
            Partial => SAUCER_WINDOW_DECORATION_PARTIAL,
            Full => SAUCER_WINDOW_DECORATION_FULL,
        }
    }
}
