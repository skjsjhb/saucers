use saucer_sys::*;

/// Window edge descriptor for specifying dragging operations.
pub enum WindowEdge {
    Top,
    Bottom,
    Left,
    Right,
    BottomLeft,
    BottomRight,
    TopLeft,
    TopRight,
}

impl From<WindowEdge> for saucer_window_edge {
    fn from(value: WindowEdge) -> Self {
        use WindowEdge::*;
        match value {
            Top => SAUCER_WINDOW_EDGE_TOP,
            Bottom => SAUCER_WINDOW_EDGE_BOTTOM,
            Left => SAUCER_WINDOW_EDGE_LEFT,
            Right => SAUCER_WINDOW_EDGE_RIGHT,
            BottomLeft => SAUCER_WINDOW_EDGE_BOTTOM_LEFT,
            BottomRight => SAUCER_WINDOW_EDGE_BOTTOM_RIGHT,
            TopLeft => SAUCER_WINDOW_EDGE_TOP_LEFT,
            TopRight => SAUCER_WINDOW_EDGE_TOP_RIGHT,
        }
    }
}
