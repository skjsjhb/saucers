use crate::policy::Policy;
use crate::window::Window;
use crate::window::WindowDecoration;

pub trait WindowEventListener {
    fn on_decorated(&self, _window: Window, _decoration: WindowDecoration) {}
    fn on_maximize(&self, _window: Window, _maximized: bool) {}
    fn on_minimize(&self, _window: Window, _minimized: bool) {}
    fn on_closed(&self, _window: Window) {}
    fn on_resize(&self, _window: Window, _width: u32, _height: u32) {}
    fn on_focus(&self, _window: Window, _focused: bool) {}
    fn on_close(&self, _window: Window) -> Policy { Policy::Allow }
}
