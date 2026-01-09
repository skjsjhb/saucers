use crate::policy::Policy;
use crate::window::Window;
use crate::window::WindowDecoration;

/// A trait containing window events.
///
/// Because the listener is stored inside the [`Window`] handle, capturing any handle directly will
/// form circular references and prevent them from dropping. It's advised to use the passed argument
/// or [`crate::window::WindowRef`] instead.
#[allow(unused)]
pub trait WindowEventListener {
    /// Fired when the window decoration status changes.
    fn on_decorated(&self, window: Window, decoration: WindowDecoration) {}

    /// Fired when the window maximization changes.
    fn on_maximize(&self, window: Window, maximized: bool) {}

    /// Fired when the window minimization changes.
    fn on_minimize(&self, window: Window, minimized: bool) {}

    /// Fired when the window has closed.
    fn on_closed(&self, window: Window) {}

    /// Fired when the window size changes.
    fn on_resize(&self, window: Window, width: u32, height: u32) {}

    /// Fired when the window is focused or blurred.
    fn on_focus(&self, window: Window, focused: bool) {}

    /// Fired when the window is about to close.
    fn on_close(&self, window: Window) -> Policy { Policy::Allow }
}
