use std::cell::RefCell;
use std::ffi::c_char;
use std::ffi::c_void;
use std::rc::Rc;

use crate::capi::*;
use crate::ctor;
use crate::icon::Icon;
use crate::navigation::WebviewNavigation;
use crate::webview::Webview;
use crate::webview::WebviewRef;

pub trait WebviewEvent {
    type Handler: ?Sized + 'static;
    type OnceHandler: ?Sized + 'static;

    unsafe fn register(ptr: *mut saucer_handle, raw: *mut c_void) -> u64;
    unsafe fn register_once(ptr: *mut saucer_handle, raw: *mut c_void);
    unsafe fn unregister(ptr: *mut saucer_handle, id: u64);
    unsafe fn clear(ptr: *mut saucer_handle);
    fn event_id() -> u32;
}

macro_rules! make_event {
    ($name:ident($($arg:ty),*) -> $rt:ty, $chn:ident, $tra:ident, $tro:ident, $reg:ident, $reo:ident, $rem:ident, $clear:ident, $offset:expr) => {
        // pub struct ${ concat($name, Event) };
        impl WebviewEvent for ${ concat($name, Event) } {
            type Handler = dyn FnMut(Webview $(,$arg)*) + 'static;
            type OnceHandler = dyn FnOnce(Webview $(,$arg)*) + 'static;

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

            fn event_id() -> u32 { $chn + $offset }
        }
    };
}

macro_rules! make_webview_event {
    ($name:ident($($arg:ty),*) -> $rt:ty, $chn:ident, $tra:ident) => {
        make_event!(
            $name($($arg),*) -> $rt,
            $ { concat(SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_, $chn) },
            $ { concat(on_, $tra, _trampoline) },
            $ { concat(once_, $tra, _trampoline) },
            saucer_webview_on_with_arg,
            saucer_webview_once_with_arg,
            saucer_webview_remove,
            saucer_webview_clear,
            0
        );
    };
}

macro_rules! make_window_event {
    ($name:ident($($arg:ty),*) -> $rt:ty, $chn:ident, $tra:ident) => {
        make_event!(
            $name($($arg),*)  -> $rt,
            $ { concat(SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_, $chn) },
            $ { concat(on_, $tra, _trampoline) },
            $ { concat(once_, $tra, _trampoline) },
            saucer_window_on_with_arg,
            saucer_window_once_with_arg,
            saucer_window_remove,
            saucer_window_clear,
            256 // Must be greater than event count in webview
        );
    };
}

// Some editors seem unable to expand macros with `macro_metavar_expr_concat` correctly.
// The event structs are extracted here for better DX.

pub struct DomReadyEvent;
pub struct NavigateEvent;
pub struct NavigatedEvent;
pub struct TitleEvent;
pub struct FaviconEvent;
pub struct LoadEvent;
pub struct DecoratedEvent;
pub struct MaximizeEvent;
pub struct MinimizeEvent;
pub struct ClosedEvent;
pub struct ResizeEvent;
pub struct FocusEvent;
pub struct CloseEvent;

make_webview_event!(DomReady() -> (), DOM_READY, dom_ready);
make_webview_event!(Navigate(WebviewNavigation) -> bool, NAVIGATE, navigate);
make_webview_event!(Navigated(&str) -> (), NAVIGATED, navigated);
make_webview_event!(Title(&str) -> (), TITLE, title);
make_webview_event!(Favicon(Icon) -> (), FAVICON, favicon);
make_webview_event!(Load(WebviewLoadState) -> (), LOAD, load);
make_window_event!(Decorated(bool) -> (), DECORATED, decorated);
make_window_event!(Maximize(bool) -> (), MAXIMIZE, maximize);
make_window_event!(Minimize(bool) -> (), MINIMIZE, minimize);
make_window_event!(Closed() -> (), CLOSED, closed);
make_window_event!(Resize(i32, i32) -> (), RESIZE, resize);
make_window_event!(Focus(bool) -> (), FOCUS, focus);
make_window_event!(Close() -> bool, CLOSE, close);

#[derive(Eq, PartialEq)]
pub enum WebviewLoadState {
    Started,
    Finished
}

impl Webview {
    pub fn on<T: WebviewEvent>(&self, _: T, handler: Box<T::Handler>) -> Option<u64> {
        if !self.is_event_thread() {
            return None;
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

        Some(id)
    }

    pub fn once<T: WebviewEvent>(&self, _: T, handler: Box<T::OnceHandler>) {
        if !self.is_event_thread() {
            return;
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

    pub fn off<T: WebviewEvent>(&self, _: T, id: u64) {
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

    pub fn clear<T: WebviewEvent>(&self, _: T) {
        if !self.is_event_thread() {
            return;
        }

        unsafe { T::clear(self.as_ptr()) }

        let mut guard = self.0.write().unwrap();
        for (_, dropper) in guard.ptr.as_mut().unwrap().dyn_event_droppers.drain() {
            dropper();
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
                Rc<RefCell<Option<Box<dyn FnOnce(Webview, WebviewNavigation) -> bool>>>>
            )
        )
    };
    let nav = WebviewNavigation::from_ptr(nav);
    let rt = do_once_trampoline!(bb; nav; true);
    let _ = Box::into_raw(bb);
    if rt {
        SAUCER_POLICY_SAUCER_POLICY_ALLOW
    } else {
        SAUCER_POLICY_SAUCER_POLICY_BLOCK
    }
}

extern "C" fn once_navigated_trampoline(_: *mut saucer_handle, arg: *mut c_void, url: *mut c_char) {
    let url = ctor!(url);
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Option<Box<dyn FnOnce(Webview, &str)>>>>)) };
    do_once_trampoline!(bb; &url);
    let _ = Box::into_raw(bb);
}

extern "C" fn once_title_trampoline(h: *mut saucer_handle, arg: *mut c_void, url: *mut c_char) {
    once_navigated_trampoline(h, arg, url);
}

extern "C" fn once_favicon_trampoline(_: *mut saucer_handle, arg: *mut c_void, icon: *mut saucer_icon) {
    let icon = Icon::from_ptr(icon);
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Option<Box<dyn FnOnce(Webview, Icon)>>>>)) };
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
                Rc<RefCell<Box<dyn FnMut(Webview, WebviewNavigation) -> bool>>>
            )
        )
    };
    let nav = WebviewNavigation::from_ptr(nav);
    let rt = do_mut_trampoline!(bb; nav; true);
    let _ = Box::into_raw(bb);
    if rt {
        SAUCER_POLICY_SAUCER_POLICY_ALLOW
    } else {
        SAUCER_POLICY_SAUCER_POLICY_BLOCK
    }
}

extern "C" fn on_navigated_trampoline(_: *mut saucer_handle, arg: *mut c_void, url: *mut c_char) {
    let url = ctor!(url);
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Box<dyn FnMut(Webview, &str)>>>)) };
    do_mut_trampoline!(bb; &url);
    let _ = Box::into_raw(bb);
}

extern "C" fn on_title_trampoline(h: *mut saucer_handle, arg: *mut c_void, url: *mut c_char) {
    on_navigated_trampoline(h, arg, url);
}

extern "C" fn on_favicon_trampoline(_: *mut saucer_handle, arg: *mut c_void, icon: *mut saucer_icon) {
    let icon = Icon::from_ptr(icon);
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Box<dyn FnMut(Webview, Icon)>>>)) };
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
