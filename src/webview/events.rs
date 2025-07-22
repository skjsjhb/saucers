//! Webview events module.
//!
//! See [`WebviewEvent`], [`Webview::on`] and related items for details.
use std::cell::RefCell;
use std::ffi::c_char;
use std::ffi::c_void;
use std::rc::Rc;

use crate::capi::*;
use crate::icon::Icon;
use crate::navigation::WebviewNavigation;
use crate::util::shot_str;
use crate::webview::Webview;
use crate::webview::WebviewRef;

/// Super trait for all webview events.
///
/// Methods of this trait shall be internal but are made public due to limitations of Rust. Implementations of this
/// trait acts as opaque types that are only used when specifying event in [`Webview::on`], [`Webview::once`],
/// [`Webview::off`] and [`Webview::clear`]. Fields of this trait are not part of the API.
///
/// # Safety
///
/// This trait is marked as unsafe because the event system rely on certain implementation details of the methods. Any
/// attempt to implement this trait may break such conventions and is considered unsafe.
pub unsafe trait WebviewEvent {
    /// Handler type of the event. When this event is used with [`Webview::on`], it accepts a boxed handler of this
    /// type.
    type Handler: ?Sized + 'static;

    /// One-time handler type of the event. When this event is used with [`Webview::once`], it accepts a boxed handler
    /// of this type.
    type OnceHandler: ?Sized + 'static;

    /// Internal method.
    unsafe fn register(ptr: *mut saucer_handle, raw: *mut c_void) -> u64;

    /// Internal method.
    unsafe fn register_once(ptr: *mut saucer_handle, raw: *mut c_void);

    /// Internal method.
    unsafe fn unregister(ptr: *mut saucer_handle, id: u64);

    /// Internal method.
    unsafe fn clear(ptr: *mut saucer_handle);

    /// Internal method.
    fn event_id() -> u32;
}

macro_rules! make_event {
    ($name:ident($($arg:ty),*) -> $rt:ty, $chn:ident, $tra:ident, $tro:ident, $reg:ident, $reo:ident, $rem:ident, $clear:ident, $offset:expr) => {
        // pub struct ${ concat($name, Event) };
        unsafe impl WebviewEvent for $name {
            type Handler = dyn (FnMut(Webview $(,$arg)*) -> $rt) + 'static;
            type OnceHandler = dyn (FnOnce(Webview $(,$arg)*) -> $rt) + 'static;

            unsafe fn register(ptr: *mut saucer_handle, raw: *mut c_void) -> u64 {
                unsafe {
                    $reg(ptr, $chn, $tra as *mut c_void, raw)
                }
            }

            unsafe fn register_once(ptr: *mut saucer_handle, raw: *mut c_void) {
                unsafe {
                    $reo(ptr, $chn, $tro as *mut c_void, raw)
                }
            }

            unsafe fn unregister(ptr: *mut saucer_handle, id: u64) {
                unsafe { $rem(ptr, $chn, id) }
            }

            unsafe fn clear(ptr: *mut saucer_handle) {
                unsafe { $clear(ptr, $chn) }
            }

            fn event_id() -> u32 { ($chn + $offset) as u32 }
        }
    };
}

macro_rules! make_webview_event {
    ($name:ident($($arg:ty),*) -> $rt:ty, $chn:ident, $tra:ident, $tro:ident) => {
        make_event!(
            $name($($arg),*) -> $rt,
            $chn,
            $tra,
            $tro,
            saucer_webview_on_with_arg,
            saucer_webview_once_with_arg,
            saucer_webview_remove,
            saucer_webview_clear,
            0
        );
    };
}

macro_rules! make_window_event {
    ($name:ident($($arg:ty),*) -> $rt:ty, $chn:ident, $tra:ident, $tro:ident) => {
        make_event!(
            $name($($arg),*)  -> $rt,
            $chn,
            $tra,
            $tro,
            saucer_window_on_with_arg,
            saucer_window_once_with_arg,
            saucer_window_remove,
            saucer_window_clear,
            256 // Must be greater than event count in webview
        );
    };
}

/// Fired when the DOM is ready.
pub struct DomReadyEvent;

/// Fired when the page is about to navigate.
///
/// Return `false` from the event handler to prevent it.
pub struct NavigateEvent;

/// Fired when the page navigates.
pub struct NavigatedEvent;

/// Fired when the page title changes.
pub struct TitleEvent;

/// Fired when the favicon changes.
pub struct FaviconEvent;

/// Fired when the page starts to load or has finished loading.
pub struct LoadEvent;

/// Fired when the decoration status of the window changes.
pub struct DecoratedEvent;

/// Fired when the window is maximized or unmaximized.
pub struct MaximizeEvent;

/// Fired when the window is minimized or unminimized.
pub struct MinimizeEvent;

/// Fired when the window is closed.
pub struct ClosedEvent;

/// Fired when the window size changes.
pub struct ResizeEvent;

/// Fired when the window is focused or loses focus.
pub struct FocusEvent;

/// Fired when the window is about to close.
///
/// Return `false` from the event handler to prevent it.
pub struct CloseEvent;

// Some editors seem unable to expand macros with `macro_metavar_expr_concat` correctly.
// Use the full name can provide potentially better DX for users.
make_webview_event!(DomReadyEvent() -> (), SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_DOM_READY, on_dom_ready_trampoline, once_dom_ready_trampoline);
make_webview_event!(NavigateEvent(&WebviewNavigation) -> bool, SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_NAVIGATE, on_navigate_trampoline, once_navigate_trampoline);
make_webview_event!(NavigatedEvent(&str) -> (), SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_NAVIGATED, on_navigated_trampoline, once_navigated_trampoline);
make_webview_event!(TitleEvent(&str) -> (), SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_TITLE, on_title_trampoline, once_title_trampoline);
make_webview_event!(FaviconEvent(&Icon) -> (), SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_FAVICON, on_favicon_trampoline, once_favicon_trampoline);
make_webview_event!(LoadEvent(WebviewLoadState) -> (), SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_LOAD, on_load_trampoline, once_load_trampoline);
make_window_event!(DecoratedEvent(bool) -> (), SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_DECORATED, on_decorated_trampoline, once_decorated_trampoline);
make_window_event!(MaximizeEvent(bool) -> (), SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_MAXIMIZE, on_maximize_trampoline, once_maximize_trampoline);
make_window_event!(MinimizeEvent(bool) -> (), SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_MINIMIZE, on_minimize_trampoline, once_minimize_trampoline);
make_window_event!(ClosedEvent() -> (), SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_CLOSED, on_closed_trampoline, once_closed_trampoline);
make_window_event!(ResizeEvent(i32, i32) -> (), SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_RESIZE, on_resize_trampoline, once_resize_trampoline);
make_window_event!(FocusEvent(bool) -> (), SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_FOCUS, on_focus_trampoline, once_focus_trampoline);
make_window_event!(CloseEvent() -> bool, SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_CLOSE, on_close_trampoline, once_close_trampoline);

#[derive(Eq, PartialEq)]
pub enum WebviewLoadState {
    Started,
    Finished
}

impl Webview {
    /// Adds an event handler for the event represented by `T`. The arguments and the return value are specified by the
    /// associated `Handler` type of `T`. Both the event handler and this method will (can) only be called on the event
    /// thread.
    ///
    /// Returns a unique ID in the scope of this event, which can later be used to unregister the handler.
    ///
    /// Like [`Webview::on_message`], event handlers are dropped when being removed via [`Self::off`]. If an event
    /// handler is still active when the webview is dropped, it's dropped at least not later than the
    /// [`crate::collector::Collector`] referenced by the app of this webview.
    ///
    /// # Don't Capture Handles
    ///
    /// Like [`Self::on_message`], capturing handles in handlers added by this method may interfere the correct
    /// dropping behavior and should be avoided. See the docs there for details.
    ///
    /// # Panics
    ///
    /// Panics if not called on the event thread.
    pub fn on<T: WebviewEvent>(&self, handler: Box<T::Handler>) -> u64 {
        if !self.is_event_thread() {
            panic!("Event handlers must be added on the event thread.")
        }

        // Repeatable handlers are fully managed by the webview handle.
        // The closure is dropped when the handler is removed.
        // It's unlikely (maybe impossible?) that a handler is re-borrowed during invocation, but just in case, we wrap
        // it in an `RefCell` to help to find errors.
        let rc = Rc::new(RefCell::new(handler));
        let wk = self.downgrade();
        let pair = (wk, rc);
        let ptr = Box::into_raw(Box::new(pair));
        let id = unsafe { T::register(self.as_ptr(), ptr as *mut c_void) };

        let mut guard = self.0.write().unwrap();
        let key = (T::event_id(), id);
        let dropper = Box::new(move || unsafe { drop(Box::from_raw(ptr)) });

        let old = guard.ptr.as_mut().unwrap().dyn_event_droppers.insert(key, dropper);

        // This is unlikely to happen as no two event handlers shall share the same ID.
        // This drop is reserved here in case.
        if let Some(old_dropper) = old {
            old_dropper();
        }

        id
    }

    /// Like [`Self::on`], but the event handler is only fired once. This allows the event handler to loosen the
    /// [`FnMut`] bound to [`FnOnce`]. Other usages, limitations and caveats remain the same as [`Self::on`].
    ///
    /// Unlike [`Self::on`], once a handler is attached using this method, there is no way to cancel it. However, the
    /// event handler will still be properly dropped like uncleared event handlers added by [`Self::on`], even if it has
    /// never been called.
    ///
    /// # Panics
    ///
    /// Panics if not called on the event thread.
    pub fn once<T: WebviewEvent>(&self, handler: Box<T::OnceHandler>) {
        if !self.is_event_thread() {
            panic!("Event handlers must be added on the event thread.")
        }

        // Unlike repeatable handlers, one-time handler may only be called once, so it must be taken by value.
        // This make cleanup tricky as it's not fully managed by the webview handle, but may be taken during invocation.
        // We use an `Option` here, allowing the trampoline to take out the handler and consume it when fired.
        // The cleanup code remains the same (`from_raw`).
        let rc = Rc::new(RefCell::new(Some(handler)));
        let wk = self.downgrade();
        let pair = (wk, rc);
        let ptr = Box::into_raw(Box::new(pair));

        unsafe { T::register_once(self.as_ptr(), ptr as *mut c_void) }

        let mut guard = self.0.write().unwrap();

        // Returns true if the dropper can be removed
        let checker = Box::new(move || {
            let bb = unsafe { Box::from_raw(ptr) };
            let rt = if let Ok(opt) = bb.1.try_borrow() {
                opt.is_none() // The once handler has been executed
            } else {
                false
            };
            let _ = Box::into_raw(bb);
            rt
        });

        let dropper = Box::new(move || unsafe { drop(Box::from_raw(ptr)) });

        let v = &mut guard.ptr.as_mut().unwrap().once_event_droppers;

        v.push((checker, dropper));

        // Tries to clean up no-op droppers (managed handler has been executed)
        let mut i = 0;

        // Ignore the newly added pair
        while i < v.len() - 1 {
            if v[i].0() {
                let (_, dropper) = v.swap_remove(i);
                // The pointer needs to be collected even if the handler has already been dropped
                dropper();
            } else {
                i += 1;
            }
        }
    }

    /// Removes a previously added event handler for event type `T` with the given ID.
    ///
    /// This method must be called on the event thread, or it does nothing.
    pub fn off<T: WebviewEvent>(&self, id: u64) {
        if !self.is_event_thread() {
            return;
        }

        unsafe { T::unregister(self.as_ptr(), id) }

        let mut guard = self.0.write().unwrap();
        let key = (T::event_id(), id);
        let old = guard.ptr.as_mut().unwrap().dyn_event_droppers.remove(&key);
        if let Some(dropper) = old {
            dropper();
        }
    }

    /// Removes all handlers of the event type `T`.
    ///
    /// This method must be called on the event thread, or it does nothing.
    pub fn clear<T: WebviewEvent>(&self) {
        if !self.is_event_thread() {
            return;
        }

        unsafe { T::clear(self.as_ptr()) }

        let mut guard = self.0.write().unwrap();

        let mm = &mut guard.ptr.as_mut().unwrap().dyn_event_droppers;

        let mut removal = Vec::new();

        for (k, _) in &*mm {
            if k.0 == T::event_id() {
                removal.push(*k);
            }
        }

        for k in removal {
            if let Some(dropper) = mm.remove(&k) {
                dropper();
            }
        }
    }
}

macro_rules! do_once_trampoline {
    ($bb:expr; $($arg:expr),*; $dv:expr) => {{
        if let Some(fun) = $bb.1.borrow_mut().take() {
            if let Some(w) = $bb.0.upgrade() {
                fun(w $(,$arg)*)
            } else {
                $dv
            }
        } else {
            $dv
        }
    }};

    ($bb:expr; $($arg:expr),*) => {{
        if let Some(fun) = $bb.1.borrow_mut().take() {
            if let Some(w) = $bb.0.upgrade() {
                fun(w $(,$arg)*);
            }
        }
    }};
}

macro_rules! do_mut_trampoline {
    ($bb:expr; $($arg:expr),*; $dv:expr) => {{
        let rc = (*$bb).1.clone();
        if let Some(w) = $bb.0.upgrade() {
            rc.borrow_mut()(w $(,$arg)*)
        } else {
            $dv
        }
    }};

    ($bb:expr; $($arg:expr),*) => {{
        let rc = (*$bb).1.clone();
        if let Some(w) = $bb.0.upgrade() {
            rc.borrow_mut()(w $(,$arg)*);
        }
    }};
}

extern "C" fn once_dom_ready_trampoline(_: *mut saucer_handle, arg: *mut c_void) {
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Option<Box<dyn FnOnce(Webview)>>>>)) };
    do_once_trampoline!(bb;);
    let _ = Box::into_raw(bb);
}

extern "C" fn once_navigate_trampoline(
    _: *mut saucer_handle,
    arg: *mut c_void,
    nav: *mut saucer_navigation
) -> SAUCER_POLICY {
    let bb = unsafe {
        Box::from_raw(
            arg as *mut (
                WebviewRef,
                Rc<RefCell<Option<Box<dyn FnOnce(Webview, &WebviewNavigation) -> bool>>>>
            )
        )
    };
    // The navigation handle is cloned, but not its data
    // Make a borrow here so it's dropped at the end of the trampoline
    // And make sure that no one can move it out
    let nav = unsafe { &WebviewNavigation::from_ptr(nav) };
    let rt = do_once_trampoline!(bb; nav; true);
    let _ = Box::into_raw(bb);
    if rt {
        SAUCER_POLICY_SAUCER_POLICY_ALLOW
    } else {
        SAUCER_POLICY_SAUCER_POLICY_BLOCK
    }
}

extern "C" fn once_navigated_trampoline(_: *mut saucer_handle, arg: *mut c_void, url: *mut c_char) {
    let url = shot_str(url).unwrap();
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Option<Box<dyn FnOnce(Webview, &str)>>>>)) };
    do_once_trampoline!(bb; &url);
    let _ = Box::into_raw(bb);
}

extern "C" fn once_title_trampoline(h: *mut saucer_handle, arg: *mut c_void, url: *mut c_char) {
    once_navigated_trampoline(h, arg, url);
}

extern "C" fn once_favicon_trampoline(_: *mut saucer_handle, arg: *mut c_void, icon: *mut saucer_icon) {
    // This icon is borrowed
    // Make a reference here so no one can move it out
    let icon = unsafe { &Icon::from_ptr(icon) };
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Option<Box<dyn FnOnce(Webview, &Icon)>>>>)) };
    do_once_trampoline!(bb; icon);
    let _ = Box::into_raw(bb);
}

extern "C" fn once_load_trampoline(_: *mut saucer_handle, arg: *mut c_void, state: SAUCER_STATE) {
    let bb = unsafe {
        Box::from_raw(
            arg as *mut (
                WebviewRef,
                Rc<RefCell<Option<Box<dyn FnOnce(Webview, WebviewLoadState)>>>>
            )
        )
    };

    let state = if state == SAUCER_STATE_SAUCER_STATE_STARTED {
        WebviewLoadState::Started
    } else {
        WebviewLoadState::Finished
    };

    do_once_trampoline!(bb; state);

    let _ = Box::into_raw(bb);
}

extern "C" fn once_decorated_trampoline(_: *mut saucer_handle, arg: *mut c_void, b: bool) {
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Option<Box<dyn FnOnce(Webview, bool)>>>>)) };
    do_once_trampoline!(bb; b);
    let _ = Box::into_raw(bb);
}

extern "C" fn once_maximize_trampoline(h: *mut saucer_handle, arg: *mut c_void, b: bool) {
    once_decorated_trampoline(h, arg, b);
}

extern "C" fn once_minimize_trampoline(h: *mut saucer_handle, arg: *mut c_void, b: bool) {
    once_decorated_trampoline(h, arg, b);
}

extern "C" fn once_closed_trampoline(h: *mut saucer_handle, arg: *mut c_void) { once_dom_ready_trampoline(h, arg); }

extern "C" fn once_resize_trampoline(_: *mut saucer_handle, arg: *mut c_void, w: i32, h: i32) {
    let bb =
        unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Option<Box<dyn FnOnce(Webview, i32, i32)>>>>)) };
    do_once_trampoline!(bb; w, h);
    let _ = Box::into_raw(bb);
}

extern "C" fn once_focus_trampoline(h: *mut saucer_handle, arg: *mut c_void, b: bool) {
    once_decorated_trampoline(h, arg, b);
}

extern "C" fn once_close_trampoline(_: *mut saucer_handle, arg: *mut c_void) -> SAUCER_POLICY {
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Option<Box<dyn FnOnce(Webview) -> bool>>>>)) };
    let rt = do_once_trampoline!(bb;;false);
    let _ = Box::into_raw(bb);
    if rt {
        SAUCER_POLICY_SAUCER_POLICY_ALLOW
    } else {
        SAUCER_POLICY_SAUCER_POLICY_BLOCK
    }
}

extern "C" fn on_dom_ready_trampoline(_: *mut saucer_handle, arg: *mut c_void) {
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Box<dyn FnMut(Webview)>>>)) };
    do_mut_trampoline!(bb;);
    let _ = Box::into_raw(bb);
}

extern "C" fn on_navigate_trampoline(
    _: *mut saucer_handle,
    arg: *mut c_void,
    nav: *mut saucer_navigation
) -> SAUCER_POLICY {
    let bb = unsafe {
        Box::from_raw(
            arg as *mut (
                WebviewRef,
                Rc<RefCell<Box<dyn FnMut(Webview, &WebviewNavigation) -> bool>>>
            )
        )
    };
    let nav = unsafe { &WebviewNavigation::from_ptr(nav) };
    let rt = do_mut_trampoline!(bb; nav; true);
    let _ = Box::into_raw(bb);
    if rt {
        SAUCER_POLICY_SAUCER_POLICY_ALLOW
    } else {
        SAUCER_POLICY_SAUCER_POLICY_BLOCK
    }
}

extern "C" fn on_navigated_trampoline(_: *mut saucer_handle, arg: *mut c_void, url: *mut c_char) {
    let url = shot_str(url).unwrap();
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Box<dyn FnMut(Webview, &str)>>>)) };
    do_mut_trampoline!(bb; &url);
    let _ = Box::into_raw(bb);
}

extern "C" fn on_title_trampoline(h: *mut saucer_handle, arg: *mut c_void, url: *mut c_char) {
    on_navigated_trampoline(h, arg, url);
}

extern "C" fn on_favicon_trampoline(_: *mut saucer_handle, arg: *mut c_void, icon: *mut saucer_icon) {
    let icon = unsafe { &Icon::from_ptr(icon) };
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Box<dyn FnMut(Webview, &Icon)>>>)) };
    do_mut_trampoline!(bb; icon);
    let _ = Box::into_raw(bb);
}

extern "C" fn on_load_trampoline(_: *mut saucer_handle, arg: *mut c_void, state: SAUCER_STATE) {
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Box<dyn FnMut(Webview, WebviewLoadState)>>>)) };

    let state = if state == SAUCER_STATE_SAUCER_STATE_STARTED {
        WebviewLoadState::Started
    } else {
        WebviewLoadState::Finished
    };

    do_mut_trampoline!(bb; state);

    let _ = Box::into_raw(bb);
}

extern "C" fn on_decorated_trampoline(_: *mut saucer_handle, arg: *mut c_void, b: bool) {
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Box<dyn FnMut(Webview, bool)>>>)) };
    do_mut_trampoline!(bb; b);
    let _ = Box::into_raw(bb);
}

extern "C" fn on_maximize_trampoline(h: *mut saucer_handle, arg: *mut c_void, b: bool) {
    on_decorated_trampoline(h, arg, b);
}

extern "C" fn on_minimize_trampoline(h: *mut saucer_handle, arg: *mut c_void, b: bool) {
    on_decorated_trampoline(h, arg, b);
}

extern "C" fn on_closed_trampoline(h: *mut saucer_handle, arg: *mut c_void) { on_dom_ready_trampoline(h, arg); }

extern "C" fn on_resize_trampoline(_: *mut saucer_handle, arg: *mut c_void, w: i32, h: i32) {
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Box<dyn FnMut(Webview, i32, i32)>>>)) };
    do_mut_trampoline!(bb; w, h);
    let _ = Box::into_raw(bb);
}

extern "C" fn on_focus_trampoline(h: *mut saucer_handle, arg: *mut c_void, b: bool) {
    on_decorated_trampoline(h, arg, b);
}

extern "C" fn on_close_trampoline(_: *mut saucer_handle, arg: *mut c_void) -> SAUCER_POLICY {
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Box<dyn FnMut(Webview) -> bool>>>)) };
    let rt = do_mut_trampoline!(bb;;true);
    let _ = Box::into_raw(bb);
    if rt {
        SAUCER_POLICY_SAUCER_POLICY_ALLOW
    } else {
        SAUCER_POLICY_SAUCER_POLICY_BLOCK
    }
}
