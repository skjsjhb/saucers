mod decoration;
mod edge;
mod events;

use std::cell::RefCell;
use std::ffi::c_char;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::null_mut;
use std::ptr::NonNull;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Weak;
use std::thread::ThreadId;

pub use decoration::*;
pub use events::*;
use saucer_sys::*;

use crate::app::App;
use crate::icon::Icon;
use crate::macros::load_range;
use crate::macros::use_string;
use crate::policy::Policy;
use crate::screen::Screen;
use crate::window::edge::WindowEdge;

/// An unprotected owned window handle.
struct RawWindow {
    inner: NonNull<saucer_window>,
    drop_sender: Sender<Box<dyn FnOnce() + Send>>,
    host_tid: ThreadId,
    event_listener_data: RefCell<*mut EventListenerData>, /* !Send, yet not visible on other
                                                           * threads */
    _marker: PhantomData<saucer_window>,
}

unsafe impl Send for RawWindow {}
unsafe impl Sync for RawWindow {}

struct RawWindowCleanUp {
    inner: NonNull<saucer_window>,
    event_listener_data: *mut EventListenerData,
}

unsafe impl Send for RawWindowCleanUp {}

impl Drop for RawWindow {
    fn drop(&mut self) {
        let cl = RawWindowCleanUp {
            inner: self.inner,
            event_listener_data: *self.event_listener_data.borrow(),
        };

        let col = move || unsafe {
            let _ = &cl;

            let ptr = cl.inner.as_ptr();

            saucer_window_free(ptr); // Events will be automatically cleaned

            // We can't be inside an event handler here as they're executed with a backed-up handle
            drop(Box::from_raw(cl.event_listener_data));
        };

        if self.is_thread_safe() {
            col();
        } else {
            self.drop_sender.send(Box::new(col)).expect("failed to post window destruction");
        }
    }
}

impl RawWindow {
    fn is_thread_safe(&self) -> bool { std::thread::current().id() == self.host_tid }
}

/// A window handle.
///
/// Window handles are shared and the underlying window is only closed after the last handle is
/// dropped. When created inside the app start callback, it should be consumed in a webview, or
/// moved into the [`crate::app::FinishListener`] so that it don't get closed immediately.
#[derive(Clone)]
pub struct Window(Arc<RawWindow>);

impl Window {
    /// Creates a new window using the given [`App`] and [`WindowEventListener`].
    ///
    /// This method must be called on the event thread, or it will panic.
    ///
    /// # Panics
    ///
    /// Panics if not called on the event thread.
    pub fn new(
        app: &App,
        event_listener: impl WindowEventListener + 'static,
    ) -> crate::error::Result<Self> {
        if !app.is_thread_safe() {
            panic!("windows must be created on the event thread");
        }

        let mut ex = -1;
        let ptr = unsafe { saucer_window_new(app.as_ptr(), &raw mut ex) };

        let wnd = NonNull::new(ptr).ok_or(crate::error::Error::Saucer(ex))?;

        let wnd = Self(Arc::new(RawWindow {
            inner: wnd,
            drop_sender: app.drop_sender(),
            host_tid: std::thread::current().id(),
            event_listener_data: RefCell::new(null_mut()),
            _marker: PhantomData,
        }));

        let data = EventListenerData::new(event_listener, wnd.downgrade());
        let data = Box::into_raw(Box::new(data));

        *wnd.0.event_listener_data.borrow_mut() = data;

        macro_rules! bind_event {
            ($ev:expr, $cb:expr) => {
                unsafe {
                    saucer_window_on(ptr, $ev, $cb as *mut c_void, true, data as *mut c_void)
                };
            };
        }

        bind_event!(SAUCER_WINDOW_EVENT_DECORATED, ev_on_decorated_tp);
        bind_event!(SAUCER_WINDOW_EVENT_MAXIMIZE, ev_on_maximize_tp);
        bind_event!(SAUCER_WINDOW_EVENT_MINIMIZE, ev_on_minimize_tp);
        bind_event!(SAUCER_WINDOW_EVENT_CLOSED, ev_on_closed_tp);
        bind_event!(SAUCER_WINDOW_EVENT_RESIZE, ev_on_resize_tp);
        bind_event!(SAUCER_WINDOW_EVENT_FOCUS, ev_on_focus_tp);
        bind_event!(SAUCER_WINDOW_EVENT_CLOSE, ev_on_close_tp);

        Ok(wnd)
    }

    /// Checks we're on the event thread.
    pub fn is_thread_safe(&self) -> bool { self.0.is_thread_safe() }

    /// Checks whether the window is visible.
    pub fn is_visible(&self) -> bool { unsafe { saucer_window_visible(self.as_ptr()) } }

    /// Checks whether the window is focused.
    pub fn is_focused(&self) -> bool { unsafe { saucer_window_focused(self.as_ptr()) } }

    /// Checks whether the window is maximized.
    pub fn is_maximized(&self) -> bool { unsafe { saucer_window_maximized(self.as_ptr()) } }

    /// Checks whether the window is minimized.
    pub fn is_minimized(&self) -> bool { unsafe { saucer_window_minimized(self.as_ptr()) } }

    /// Checks whether the window is resizable.
    pub fn is_resizable(&self) -> bool { unsafe { saucer_window_resizable(self.as_ptr()) } }

    /// Checks whether the window is fullscreen.
    pub fn is_fullscreen(&self) -> bool { unsafe { saucer_window_fullscreen(self.as_ptr()) } }

    /// Checks whether the window is always on top.
    pub fn is_always_on_top(&self) -> bool { unsafe { saucer_window_always_on_top(self.as_ptr()) } }

    /// Checks whether the window is click-through.
    pub fn is_click_through(&self) -> bool { unsafe { saucer_window_click_through(self.as_ptr()) } }

    /// Gets the window title.
    pub fn title(&self) -> String {
        let st = load_range!(ptr[size] = 0u8; {
            unsafe { saucer_window_title(self.as_ptr(), ptr as *mut c_char, size) };
        });

        String::from_utf8_lossy(&st).into_owned()
    }

    /// Gets the window background color.
    pub fn background(&self) -> (u8, u8, u8, u8) {
        let mut r = 0;
        let mut g = 0;
        let mut b = 0;
        let mut a = 0;
        unsafe {
            saucer_window_background(self.as_ptr(), &raw mut r, &raw mut g, &raw mut b, &raw mut a)
        };
        (r, g, b, a)
    }

    /// Gets the window decoration status.
    pub fn decorations(&self) -> WindowDecoration {
        unsafe { saucer_window_decorations(self.as_ptr()) as saucer_window_decoration }.into()
    }

    /// Gets the window size.
    pub fn size(&self) -> (i32, i32) {
        let mut x = 0;
        let mut y = 0;
        unsafe { saucer_window_size(self.as_ptr(), &raw mut x, &raw mut y) };

        (x, y)
    }

    /// Gets the window maximum size.
    pub fn max_size(&self) -> (i32, i32) {
        let mut x = 0;
        let mut y = 0;
        unsafe { saucer_window_max_size(self.as_ptr(), &raw mut x, &raw mut y) };
        (x, y)
    }

    /// Gets the window minimum size.
    pub fn min_size(&self) -> (i32, i32) {
        let mut x = 0;
        let mut y = 0;
        unsafe { saucer_window_min_size(self.as_ptr(), &raw mut x, &raw mut y) };
        (x, y)
    }

    /// Gets the window position.
    pub fn position(&self) -> (i32, i32) {
        let mut x = 0;
        let mut y = 0;
        unsafe { saucer_window_position(self.as_ptr(), &raw mut x, &raw mut y) };
        (x, y)
    }

    /// Gets the screen this window is on. Returns [`None`] if the screen can't be determined.
    pub fn screen(&self) -> Option<Screen> {
        unsafe { Screen::from_raw(saucer_window_screen(self.as_ptr())) }
    }

    /// Hides the window.
    pub fn hide(&self) { unsafe { saucer_window_hide(self.as_ptr()) } }

    /// Shows the window.
    pub fn show(&self) { unsafe { saucer_window_show(self.as_ptr()) } }

    /// Closes the window.
    pub fn close(&self) { unsafe { saucer_window_close(self.as_ptr()) } }

    /// Focuses the window.
    pub fn focus(&self) { unsafe { saucer_window_focus(self.as_ptr()) } }

    /// Starts a drag operation.
    pub fn start_drag(&self) { unsafe { saucer_window_start_drag(self.as_ptr()) } }

    /// Starts a resize operation on the given edge.
    pub fn start_resize(&self, edge: WindowEdge) {
        unsafe { saucer_window_start_resize(self.as_ptr(), edge.into()) }
    }

    /// Toggles window maximization.
    pub fn set_maximized(&self, maximized: bool) {
        unsafe { saucer_window_set_maximized(self.as_ptr(), maximized) }
    }

    /// Toggles window minimization.
    pub fn set_minimized(&self, minimized: bool) {
        unsafe { saucer_window_set_minimized(self.as_ptr(), minimized) }
    }

    /// Toggles window resizability.
    pub fn set_resizable(&self, resizable: bool) {
        unsafe { saucer_window_set_resizable(self.as_ptr(), resizable) }
    }

    /// Toggles window fullscreen.
    pub fn set_fullscreen(&self, fullscreen: bool) {
        unsafe { saucer_window_set_fullscreen(self.as_ptr(), fullscreen) }
    }

    /// Sets whether the window is always on top.
    pub fn set_always_on_top(&self, always_on_top: bool) {
        unsafe { saucer_window_set_always_on_top(self.as_ptr(), always_on_top) }
    }

    /// Sets whether the window is click-through.
    pub fn set_click_through(&self, click_through: bool) {
        unsafe { saucer_window_set_click_through(self.as_ptr(), click_through) }
    }

    /// Sets the window icon.
    pub fn set_icon(&self, icon: impl AsRef<Icon>) {
        unsafe { saucer_window_set_icon(self.as_ptr(), icon.as_ref().as_ptr()) }
    }

    /// Sets the window title.
    pub fn set_title(&self, title: impl Into<Vec<u8>>) {
        use_string!(
            t: title;
            unsafe { saucer_window_set_title(self.as_ptr(), t) }
        )
    }

    /// Sets the window background color.
    pub fn set_background(&self, color: (u8, u8, u8, u8)) {
        unsafe { saucer_window_set_background(self.as_ptr(), color.0, color.1, color.2, color.3) }
    }

    /// Sets the window decoration status.
    pub fn set_decorations(&self, dec: WindowDecoration) {
        unsafe { saucer_window_set_decorations(self.as_ptr(), dec.into()) }
    }

    /// Sets the window size.
    pub fn set_size(&self, size: (i32, i32)) {
        unsafe { saucer_window_set_size(self.as_ptr(), size.0, size.1) }
    }

    /// Sets the window maximum size.
    pub fn set_max_size(&self, size: (i32, i32)) {
        unsafe { saucer_window_set_max_size(self.as_ptr(), size.0, size.1) }
    }

    /// Sets the window minimum size.
    pub fn set_min_size(&self, size: (i32, i32)) {
        unsafe { saucer_window_set_min_size(self.as_ptr(), size.0, size.1) }
    }

    /// Sets the window position.
    pub fn set_position(&self, pos: (i32, i32)) {
        unsafe { saucer_window_set_position(self.as_ptr(), pos.0, pos.1) }
    }

    /// Gets a weak [`WindowRef`].
    pub fn downgrade(&self) -> WindowRef { WindowRef(Arc::downgrade(&self.0)) }

    pub(crate) fn as_ptr(&self) -> *mut saucer_window { self.0.inner.as_ptr() }

    pub(crate) fn drop_sender(&self) -> Sender<Box<dyn FnOnce() + Send>> {
        self.0.drop_sender.clone()
    }
}

/// A weak window handle.
///
/// Like [`crate::app::AppRef`], this handle does not prevent deallocation and can be used in
/// various listeners.
#[derive(Clone)]
pub struct WindowRef(Weak<RawWindow>);

impl WindowRef {
    /// Tries to upgrade to a strong handle.
    pub fn upgrade(&self) -> Option<Window> { Some(Window(self.0.upgrade()?)) }
}

struct EventListenerData {
    listener: Box<dyn WindowEventListener + 'static>,
    window: WindowRef,
}

impl EventListenerData {
    fn new(listener: impl WindowEventListener + 'static, window: WindowRef) -> Self {
        Self { listener: Box::new(listener), window }
    }
}

unsafe extern "C" fn ev_on_decorated_tp(
    _: *mut saucer_window,
    dec: saucer_window_decoration,
    data: *mut c_void,
) {
    let data = unsafe { &*(data as *const EventListenerData) };
    if let Some(wnd) = data.window.upgrade() {
        data.listener.on_decorated(wnd.clone(), dec.into()); // Clone to avoid dropping in the handler
    }
}

extern "C" fn ev_on_maximize_tp(_: *mut saucer_window, maximized: bool, data: *mut c_void) {
    let data = unsafe { &*(data as *const EventListenerData) };
    if let Some(wnd) = data.window.upgrade() {
        data.listener.on_maximize(wnd.clone(), maximized);
    }
}

extern "C" fn ev_on_minimize_tp(_: *mut saucer_window, minimized: bool, data: *mut c_void) {
    let data = unsafe { &*(data as *const EventListenerData) };
    if let Some(wnd) = data.window.upgrade() {
        data.listener.on_minimize(wnd.clone(), minimized);
    }
}

extern "C" fn ev_on_closed_tp(_: *mut saucer_window, data: *mut c_void) {
    let data = unsafe { &*(data as *const EventListenerData) };
    if let Some(wnd) = data.window.upgrade() {
        data.listener.on_closed(wnd.clone());
    }
}

extern "C" fn ev_on_resize_tp(_: *mut saucer_window, width: u32, height: u32, data: *mut c_void) {
    let data = unsafe { &*(data as *const EventListenerData) };
    if let Some(wnd) = data.window.upgrade() {
        data.listener.on_resize(wnd.clone(), width, height);
    }
}

extern "C" fn ev_on_focus_tp(_: *mut saucer_window, focused: bool, data: *mut c_void) {
    let data = unsafe { &*(data as *const EventListenerData) };
    if let Some(wnd) = data.window.upgrade() {
        data.listener.on_focus(wnd.clone(), focused);
    }
}

extern "C" fn ev_on_close_tp(_: *mut saucer_window, data: *mut c_void) -> saucer_policy {
    let data = unsafe { &*(data as *const EventListenerData) };
    if let Some(wnd) = data.window.upgrade() {
        data.listener.on_close(wnd.clone()).into()
    } else {
        Policy::Allow.into()
    }
}
