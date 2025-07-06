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

#[derive(Eq, PartialEq)]
pub enum WebviewLoadState {
    Started,
    Finished
}

macro_rules! handle_evt {
    (webview, $sf:ident, $cfn:ident, $fun:ident -> $rtp:ty, $chn:expr) => {{
        handle_evt!($sf, $cfn, $fun -> $rtp, $chn, web_event_droppers, saucer_webview_on_with_arg);
    }};

    (window, $sf:ident, $cfn:ident, $fun:ident -> $rtp:ty, $chn:expr) => {{
        handle_evt!($sf, $cfn, $fun -> $rtp, $chn, window_event_droppers, saucer_window_on_with_arg);
    }};

    ($sf:ident, $cfn:ident, $fun:ident -> $rtp:ty, $chn:expr, $dm:ident, $capi:ident) => {{
        if !$sf.is_event_thread() {
            return None;
        }

        let bb = Box::new($fun) as Box<$rtp>;
        let rc = Rc::new(RefCell::new(bb));
        let wk = $sf.downgrade();
        let pair = (wk, rc);
        let ptr = Box::into_raw(Box::new(pair));
        let id = unsafe { $capi($sf.as_ptr(), $chn, $cfn as *mut c_void, ptr as *mut c_void) };

        let mut guard = $sf.0.write().unwrap();
        let key = ($chn, id);
        let dropper = Box::new(move || unsafe { drop(Box::from_raw(ptr)) });

        let old = guard.ptr.as_mut().unwrap().$dm.insert(key, dropper);

        // This is unlikely to happen as no two event handlers shall share the same ID.
        // This drop is reserved here in case.
        if let Some(old_dropper) = old {
            old_dropper();
        }

        return Some(id);
    }};
}

macro_rules! drop_evt {
    (webview, $sf:ident, $id:ident : $chn:expr) => {{ drop_evt!($sf, $id : $chn, web_event_droppers, saucer_webview_remove) }};

    (window, $sf:ident, $id:ident : $chn:expr) => {{ drop_evt!($sf, $id : $chn, window_event_droppers, saucer_window_remove) }};

    (webview, $sf:ident, * : $chn:expr) => {{ drop_evt!($sf, * : $chn, web_event_droppers, saucer_webview_clear) }};

    (window, $sf:ident, * : $chn:expr) => {{ drop_evt!($sf, * : $chn, window_event_droppers, saucer_window_clear) }};

    ($sf:ident, $id:ident : $chn:expr, $dm:ident, $capi:ident) => {{
        if !$sf.is_event_thread() {
            return;
        }

        unsafe { $capi($sf.as_ptr(), $chn, $id) }

        let mut guard = $sf.0.write().unwrap();
        let key = ($chn, $id);
        let old = guard.ptr.as_mut().unwrap().$dm.remove(&key);
        if let Some(dropper) = old {
            dropper();
        }
    }};

    ($sf:ident, * : $chn:expr, $dm:ident, $capi:ident) => {{
        if !$sf.is_event_thread() {
            return;
        }

        unsafe { $capi($sf.as_ptr(), $chn) }

        let mut guard = $sf.0.write().unwrap();
        for (_, dropper) in guard.ptr.as_mut().unwrap().$dm.drain() {
            dropper();
        }
    }};
}

macro_rules! handle_evt_once {
    (webview, $sf:ident, $cfn:ident, $fun:ident -> $rtp:ty, $chn:expr) => {{ handle_evt_once!($sf, $cfn, $fun -> $rtp, $chn, saucer_webview_on_with_arg) }};

    (window, $sf:ident, $cfn:ident, $fun:ident -> $rtp:ty, $chn:expr) => {{ handle_evt_once!($sf, $cfn, $fun -> $rtp, $chn, saucer_window_on_with_arg) }};

    ($sf:ident, $cfn:ident, $fun:ident -> $rtp:ty, $chn:expr, $capi:ident) => {{
        if !$sf.is_event_thread() {
            return;
        }

        let fn_ptr = $cfn as *mut c_void;
        let bb = Box::new($fun) as Box<$rtp>;
        let rc = Rc::new(RefCell::new(Some(bb)));
        let wk = $sf.downgrade();
        let pair = (wk, rc);
        let ptr = Box::into_raw(Box::new(pair));

        unsafe {
            $capi($sf.as_ptr(), $chn, fn_ptr, ptr as *mut c_void);
        }

        let mut guard = $sf.0.write().unwrap();

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

        // Tries to cleanup no-op droppers (managed handler has been executed)
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
    }};
}

impl Webview {
    pub fn once_dom_ready(&self, fun: impl FnOnce(Webview) + 'static) {
        handle_evt_once!(
            webview,
            self,
            once_dom_ready_trampoline,
            fun -> dyn FnOnce(Webview) + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_DOM_READY
        )
    }

    pub fn once_navigate(&self, fun: impl FnOnce(Webview, WebviewNavigation) -> bool + 'static) {
        handle_evt_once!(
            webview,
            self,
            once_navigate_trampoline,
            fun -> dyn FnOnce(Webview, WebviewNavigation) -> bool + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_NAVIGATE
        )
    }

    pub fn once_navigated(&self, fun: impl FnOnce(Webview, &str) + 'static) {
        handle_evt_once!(
            webview,
            self,
            once_navigated_trampoline,
            fun -> dyn FnOnce(Webview, &str) + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_NAVIGATED
        )
    }

    pub fn once_title(&self, fun: impl FnOnce(Webview, &str) + 'static) {
        handle_evt_once!(
            webview,
            self,
            once_title_trampoline,
            fun -> dyn FnOnce(Webview, &str) + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_TITLE
        )
    }

    pub fn once_favicon(&self, fun: impl FnOnce(Webview, Icon) + 'static) {
        handle_evt_once!(
            webview,
            self,
            once_favicon_trampoline,
            fun -> dyn FnOnce(Webview, Icon) + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_FAVICON
        )
    }

    pub fn once_load(&self, fun: impl FnOnce(Webview, WebviewLoadState) + 'static) {
        handle_evt_once!(
            webview,
            self,
            once_load_trampoline,
            fun -> dyn FnOnce(Webview, WebviewLoadState) + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_LOAD
        )
    }

    pub fn once_decorated(&self, fun: impl FnOnce(Webview, bool) + 'static) {
        handle_evt_once!(
            webview,
            self,
            once_decorated_trampoline,
            fun -> dyn FnOnce(Webview, bool) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_DECORATED
        )
    }

    pub fn once_maximize(&self, fun: impl FnOnce(Webview, bool) + 'static) {
        handle_evt_once!(
            window,
            self,
            once_maximize_trampoline,
            fun -> dyn FnOnce(Webview, bool) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_MAXIMIZE
        )
    }

    pub fn once_minimize(&self, fun: impl FnOnce(Webview, bool) + 'static) {
        handle_evt_once!(
            window,
            self,
            once_minimize_trampoline,
            fun -> dyn FnOnce(Webview, bool) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_MINIMIZE
        )
    }

    pub fn once_closed(&self, fun: impl FnOnce(Webview) + 'static) {
        handle_evt_once!(
            window,
            self,
            once_closed_trampoline,
            fun -> dyn FnOnce(Webview) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_CLOSED
        )
    }

    pub fn once_resize(&self, fun: impl FnOnce(Webview, i32, i32) + 'static) {
        handle_evt_once!(
            window,
            self,
            once_resize_trampoline,
            fun -> dyn FnOnce(Webview, i32, i32) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_RESIZE
        )
    }

    pub fn once_focus(&self, fun: impl FnOnce(Webview, bool) + 'static) {
        handle_evt_once!(
            window,
            self,
            once_focus_trampoline,
            fun -> dyn FnOnce(Webview, bool) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_FOCUS
        )
    }

    pub fn once_close(&self, fun: impl FnOnce(Webview) -> bool + 'static) {
        handle_evt_once!(
            window,
            self,
            once_close_trampoline,
            fun -> dyn FnOnce(Webview) -> bool + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_CLOSE
        )
    }

    pub fn on_dom_ready(&self, fun: impl FnMut(Webview) + 'static) -> Option<u64> {
        handle_evt!(
            webview,
            self,
            on_dom_ready_trampoline,
            fun -> dyn FnMut(Webview) + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_DOM_READY
        )
    }

    pub fn on_navigate(&self, fun: impl FnMut(Webview, WebviewNavigation) -> bool + 'static) -> Option<u64> {
        handle_evt!(
            webview,
            self,
            on_navigate_trampoline,
            fun -> dyn FnMut(Webview, WebviewNavigation) -> bool + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_NAVIGATE
        )
    }

    pub fn on_navigated(&self, fun: impl FnMut(Webview, &str) + 'static) -> Option<u64> {
        handle_evt!(
            webview,
            self,
            on_navigated_trampoline,
            fun -> dyn FnMut(Webview, &str) + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_NAVIGATED
        )
    }

    pub fn on_title(&self, fun: impl FnMut(Webview, &str) + 'static) -> Option<u64> {
        handle_evt!(
            webview,
            self,
            on_title_trampoline,
            fun -> dyn FnMut(Webview, &str) + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_TITLE
        )
    }

    pub fn on_favicon(&self, fun: impl FnMut(Webview, Icon) + 'static) -> Option<u64> {
        handle_evt!(
            webview,
            self,
            on_favicon_trampoline,
            fun -> dyn FnMut(Webview, Icon) + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_FAVICON
        )
    }

    pub fn on_load(&self, fun: impl FnMut(Webview, WebviewLoadState) + 'static) -> Option<u64> {
        handle_evt!(
            webview,
            self,
            on_load_trampoline,
            fun -> dyn FnMut(Webview, WebviewLoadState) + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_LOAD
        )
    }

    pub fn on_decorated(&self, fun: impl FnMut(Webview, bool) + 'static) -> Option<u64> {
        handle_evt!(
            window,
            self,
            on_decorated_trampoline,
            fun -> dyn FnMut(Webview, bool) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_DECORATED
        )
    }

    pub fn on_maximize(&self, fun: impl FnMut(Webview, bool) + 'static) -> Option<u64> {
        handle_evt!(
            window,
            self,
            on_maximize_trampoline,
            fun -> dyn FnMut(Webview, bool) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_MAXIMIZE
        )
    }

    pub fn on_minimize(&self, fun: impl FnMut(Webview, bool) + 'static) -> Option<u64> {
        handle_evt!(
            window,
            self,
            on_minimize_trampoline,
            fun -> dyn FnMut(Webview, bool) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_MINIMIZE
        )
    }

    pub fn on_closed(&self, fun: impl FnMut(Webview) + 'static) -> Option<u64> {
        handle_evt!(
            window,
            self,
            on_closed_trampoline,
            fun -> dyn FnMut(Webview) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_CLOSED
        )
    }

    pub fn on_resize(&self, fun: impl FnMut(Webview, i32, i32) + 'static) -> Option<u64> {
        handle_evt!(
            window,
            self,
            on_resize_trampoline ,
            fun -> dyn FnMut(Webview, i32, i32) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_RESIZE
        )
    }

    pub fn on_focus(&self, fun: impl FnMut(Webview, bool) + 'static) -> Option<u64> {
        handle_evt!(
            window,
            self,
            on_focus_trampoline ,
            fun -> dyn FnMut(Webview, bool) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_FOCUS
        )
    }

    pub fn on_close(&self, fun: impl FnMut(Webview) -> bool + 'static) -> Option<u64> {
        handle_evt!(
            window,
            self,
            on_close_trampoline,
            fun -> dyn FnMut(Webview) -> bool + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_CLOSE
        )
    }

    pub fn off_dom_ready(&self, id: u64) { drop_evt!(webview, self, id : SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_DOM_READY) }

    pub fn off_navigate(&self, id: u64) { drop_evt!(webview, self, id : SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_NAVIGATE) }

    pub fn off_navigated(&self, id: u64) { drop_evt!(webview, self, id : SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_NAVIGATED) }

    pub fn off_title(&self, id: u64) { drop_evt!(webview, self, id : SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_TITLE) }

    pub fn off_favicon(&self, id: u64) { drop_evt!(webview, self, id : SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_FAVICON) }

    pub fn off_load(&self, id: u64) { drop_evt!(webview, self, id : SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_LOAD) }

    pub fn off_decorated(&self, id: u64) {
        drop_evt!(window, self, id : SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_DECORATED)
    }

    pub fn off_maximize(&self, id: u64) {
        drop_evt!(window, self, id : SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_MAXIMIZE)
    }

    pub fn off_minimize(&self, id: u64) {
        drop_evt!(window, self, id : SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_MINIMIZE)
    }

    pub fn off_closed(&self, id: u64) { drop_evt!(window, self, id : SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_CLOSED) }

    pub fn off_resize(&self, id: u64) { drop_evt!(window, self, id : SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_RESIZE) }

    pub fn off_focus(&self, id: u64) { drop_evt!(window, self, id : SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_FOCUS) }

    pub fn off_close(&self, id: u64) { drop_evt!(window, self, id : SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_CLOSE) }

    pub fn clear_dom_ready(&self) {
        drop_evt!(webview, self, * : SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_DOM_READY);
    }

    pub fn clear_navigate(&self) {
        drop_evt!(webview, self, * : SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_NAVIGATE);
    }

    pub fn clear_navigated(&self) {
        drop_evt!(webview, self, * : SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_NAVIGATED);
    }

    pub fn clear_title(&self) {
        drop_evt!(webview, self, * : SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_TITLE);
    }

    pub fn clear_favicon(&self) {
        drop_evt!(webview, self, * : SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_FAVICON);
    }

    pub fn clear_load(&self) {
        drop_evt!(webview, self, * : SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_LOAD);
    }

    pub fn clear_decorated(&self) {
        drop_evt!(window, self, * : SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_DECORATED);
    }

    pub fn clear_maximize(&self) {
        drop_evt!(window, self, * : SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_MAXIMIZE);
    }

    pub fn clear_minimize(&self) {
        drop_evt!(window, self, * : SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_MINIMIZE);
    }

    pub fn clear_closed(&self) {
        drop_evt!(window, self, * : SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_CLOSED);
    }

    pub fn clear_resize(&self) {
        drop_evt!(window, self, * : SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_RESIZE);
    }

    pub fn clear_focus(&self) {
        drop_evt!(window, self, * : SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_FOCUS);
    }

    pub fn clear_close(&self) {
        drop_evt!(window, self, * : SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_CLOSE);
    }
}

extern "C" fn once_dom_ready_trampoline(_: *mut saucer_handle, arg: *mut c_void) {
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Option<Box<dyn FnOnce(Webview)>>>>)) };
    if let Some(fun) = bb.1.borrow_mut().take() {
        if let Some(w) = bb.0.upgrade() {
            fun(w);
        }
    }
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
    let rt = if let Some(fun) = bb.1.borrow_mut().take() {
        if let Some(w) = bb.0.upgrade() {
            let nav = WebviewNavigation::from_ptr(nav);
            fun(w, nav)
        } else {
            false
        }
    } else {
        false
    };
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
    if let Some(fun) = bb.1.borrow_mut().take() {
        if let Some(w) = bb.0.upgrade() {
            fun(w, &url);
        }
    }
    let _ = Box::into_raw(bb);
}

extern "C" fn once_title_trampoline(h: *mut saucer_handle, arg: *mut c_void, url: *mut c_char) {
    once_navigated_trampoline(h, arg, url);
}

extern "C" fn once_favicon_trampoline(_: *mut saucer_handle, arg: *mut c_void, icon: *mut saucer_icon) {
    let icon = Icon::from_ptr(icon);
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Option<Box<dyn FnOnce(Webview, Icon)>>>>)) };
    if let Some(fun) = bb.1.borrow_mut().take() {
        if let Some(w) = bb.0.upgrade() {
            fun(w, icon);
        }
    }
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

    if let Some(fun) = bb.1.borrow_mut().take() {
        if let Some(w) = bb.0.upgrade() {
            fun(w, state);
        }
    }

    let _ = Box::into_raw(bb);
}

extern "C" fn once_decorated_trampoline(_: *mut saucer_handle, arg: *mut c_void, b: bool) {
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Option<Box<dyn FnOnce(Webview, bool)>>>>)) };
    if let Some(fun) = bb.1.borrow_mut().take() {
        if let Some(w) = bb.0.upgrade() {
            fun(w, b);
        }
    }
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
    if let Some(fun) = bb.1.borrow_mut().take() {
        if let Some(wv) = bb.0.upgrade() {
            fun(wv, w, h);
        }
    }
    let _ = Box::into_raw(bb);
}

extern "C" fn once_focus_trampoline(h: *mut saucer_handle, arg: *mut c_void, b: bool) {
    once_decorated_trampoline(h, arg, b);
}

extern "C" fn once_close_trampoline(_: *mut saucer_handle, arg: *mut c_void) -> SAUCER_POLICY {
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Option<Box<dyn FnOnce(Webview) -> bool>>>>)) };
    let rt = if let Some(fun) = bb.1.borrow_mut().take() {
        if let Some(w) = bb.0.upgrade() {
            if fun(w) {
                SAUCER_POLICY_SAUCER_POLICY_ALLOW
            } else {
                SAUCER_POLICY_SAUCER_POLICY_BLOCK
            }
        } else {
            SAUCER_POLICY_SAUCER_POLICY_ALLOW
        }
    } else {
        SAUCER_POLICY_SAUCER_POLICY_ALLOW
    };
    let _ = Box::into_raw(bb);
    rt
}

extern "C" fn on_dom_ready_trampoline(_: *mut saucer_handle, arg: *mut c_void) {
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Box<dyn FnMut(Webview)>>>)) };
    let rc = (*bb).1.clone();
    if let Some(w) = bb.0.upgrade() {
        rc.borrow_mut()(w);
    }
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
    let rc = (*bb).1.clone();
    let nav = WebviewNavigation::from_ptr(nav);
    let rt = if let Some(w) = bb.0.upgrade() {
        rc.borrow_mut()(w, nav)
    } else {
        true
    };
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
    let rc = (*bb).1.clone();
    if let Some(w) = bb.0.upgrade() {
        rc.borrow_mut()(w, &url);
    }
    let _ = Box::into_raw(bb);
}

extern "C" fn on_title_trampoline(h: *mut saucer_handle, arg: *mut c_void, url: *mut c_char) {
    on_navigated_trampoline(h, arg, url);
}

extern "C" fn on_favicon_trampoline(_: *mut saucer_handle, arg: *mut c_void, icon: *mut saucer_icon) {
    let icon = Icon::from_ptr(icon);
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Box<dyn FnMut(Webview, Icon)>>>)) };
    let rc = (*bb).1.clone();
    if let Some(w) = bb.0.upgrade() {
        rc.borrow_mut()(w, icon);
    }
    let _ = Box::into_raw(bb);
}

extern "C" fn on_load_trampoline(_: *mut saucer_handle, arg: *mut c_void, state: SAUCER_STATE) {
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Box<dyn FnMut(Webview, WebviewLoadState)>>>)) };
    let rc = (*bb).1.clone();

    let state = if state == SAUCER_STATE_SAUCER_STATE_STARTED {
        WebviewLoadState::Started
    } else {
        WebviewLoadState::Finished
    };

    if let Some(w) = bb.0.upgrade() {
        rc.borrow_mut()(w, state);
    }

    let _ = Box::into_raw(bb);
}

extern "C" fn on_decorated_trampoline(_: *mut saucer_handle, arg: *mut c_void, b: bool) {
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Box<dyn FnMut(Webview, bool)>>>)) };
    let rc = (*bb).1.clone();
    if let Some(w) = bb.0.upgrade() {
        rc.borrow_mut()(w, b);
    }
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
    let rc = (*bb).1.clone();
    if let Some(wv) = bb.0.upgrade() {
        rc.borrow_mut()(wv, w, h);
    }
    let _ = Box::into_raw(bb);
}

extern "C" fn on_focus_trampoline(h: *mut saucer_handle, arg: *mut c_void, b: bool) {
    on_decorated_trampoline(h, arg, b);
}

extern "C" fn on_close_trampoline(_: *mut saucer_handle, arg: *mut c_void) -> SAUCER_POLICY {
    let bb = unsafe { Box::from_raw(arg as *mut (WebviewRef, Rc<RefCell<Box<dyn FnMut(Webview) -> bool>>>)) };
    let rc = (*bb).1.clone();
    let rt = if let Some(w) = bb.0.upgrade() {
        rc.borrow_mut()(w)
    } else {
        true
    };

    let _ = Box::into_raw(bb);
    if rt {
        SAUCER_POLICY_SAUCER_POLICY_ALLOW
    } else {
        SAUCER_POLICY_SAUCER_POLICY_BLOCK
    }
}
