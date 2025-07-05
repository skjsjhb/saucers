use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_char;
use std::ffi::c_void;
use std::ffi::CStr;
use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr::null_mut;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::mpmc::Sender;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::Weak;

use crate::app::App;
use crate::capi::*;
use crate::collector::Collect;
use crate::collector::UnsafeCollector;
use crate::embed::EmbedFile;
use crate::icon::Icon;
use crate::prefs::Preferences;
use crate::script::Script;

#[derive(Default)]
struct WebviewPtr {
    ptr: Option<NonNull<saucer_handle>>,
    message_handler: Option<*mut Rc<RefCell<Box<dyn FnMut(&str) -> bool>>>>,
    _owns: PhantomData<saucer_handle>,
    _counter: Arc<()>,

    web_event_droppers: HashMap<(SAUCER_WEB_EVENT, u64), Box<dyn FnOnce() + 'static>>,
    window_event_droppers: HashMap<(SAUCER_WINDOW_EVENT, u64), Box<dyn FnOnce() + 'static>>,

    // A pair of (checker, dropper), checker returns whether the dropper can be removed
    once_event_droppers: Vec<(Box<dyn FnMut() -> bool + 'static>, Box<dyn FnOnce() + 'static>)>
}

unsafe impl Send for WebviewPtr {}
unsafe impl Sync for WebviewPtr {}

impl WebviewPtr {
    fn as_ptr(&self) -> *mut saucer_handle { self.ptr.unwrap().as_ptr() }
}

impl Collect for WebviewPtr {
    fn collect(self: Box<Self>) {
        unsafe {
            saucer_free(self.ptr.unwrap().as_ptr());

            if let Some(ptr) = self.message_handler {
                drop(Box::from_raw(ptr));
            }

            for dropper in self.web_event_droppers.into_values() {
                dropper();
            }

            for dropper in self.window_event_droppers.into_values() {
                dropper();
            }

            for (_, dropper) in self.once_event_droppers {
                dropper();
            }
        }
    }
}

struct UnsafeWebview {
    ptr: Option<WebviewPtr>,
    collector: Option<Weak<UnsafeCollector>>,
    collector_tx: Sender<Box<dyn Collect>>,
    app: App
}

impl Drop for UnsafeWebview {
    fn drop(&mut self) {
        let bb = Box::new(self.ptr.take().unwrap());

        if self.app.is_thread_safe() {
            bb.collect();
            return;
        }

        self.collector_tx.send(bb).unwrap();

        // Unlike app, as each webview is associated with an app, we can safely just post the collector handle.
        let wk = self.collector.take().unwrap();
        self.app.post(move || {
            wk.upgrade()
                .expect("Collector dropped before webview is freed")
                .collect()
        });
    }
}

impl UnsafeWebview {
    fn new(pref: &Preferences) -> Option<Self> {
        let app = pref.get_app();
        if !app.is_thread_safe() {
            return None;
        }

        let collector = app
            .get_collector()
            .upgrade()
            .expect("Collector must not be dropped when creating webview");

        let ptr = unsafe { saucer_new(pref.as_ptr()) };
        let ptr = NonNull::new(ptr).expect("Failed to create webview");

        let mut wpt = WebviewPtr::default();
        wpt.ptr = Some(ptr);
        wpt._counter = collector.count();

        Some(Self {
            ptr: Some(wpt),
            collector: Some(Arc::downgrade(&collector)),
            collector_tx: collector.get_sender(),
            app
        })
    }

    fn as_ptr(&self) -> *mut saucer_handle { self.ptr.as_ref().unwrap().as_ptr() }

    fn remove_message_handler(&mut self) {
        if !self.app.is_thread_safe() {
            return;
        }

        unsafe { saucer_webview_on_message_with_arg(self.as_ptr(), None, null_mut()) }

        if let Some(ref mut wp) = self.ptr {
            if let Some(ptr) = wp.message_handler.take() {
                unsafe { drop(Box::from_raw(ptr)) }
            }
        }
    }

    fn replace_message_handler(&mut self, fun: impl FnMut(&str) -> bool + 'static) {
        if !self.app.is_thread_safe() {
            return;
        }

        self.remove_message_handler();

        let bb = Box::new(fun) as Box<dyn FnMut(&str) -> bool>;
        let ptr = Box::into_raw(Box::new(Rc::new(RefCell::new(bb))));
        unsafe { saucer_webview_on_message_with_arg(self.as_ptr(), Some(on_message_trampoline), ptr as *mut c_void) }
        self.ptr.as_mut().unwrap().message_handler = Some(ptr);
    }
}

macro_rules! ctor {
    (free, $ptr:expr) => {{
        ctor!($ptr, { saucer_memory_free($ptr as *mut c_void); })
    }};

    ($ptr:expr) => {{
        ctor!($ptr, {})
    }};

    ($ptr:expr, $drop:tt) => {{
        unsafe {
            if $ptr.is_null() {
                "".to_owned()
            } else {
                let st = CStr::from_ptr($ptr).to_str().expect("Invalid UTF-8 string").to_owned();
                $drop
                st
            }
        }
    }};
}

macro_rules! rtoc {
    ($($arg:expr => $ptr:ident),+ ; $ex: expr) => {{
        $(let $ptr = CString::new($arg.as_ref()).unwrap();)+
        unsafe { $ex }
    }};
}

extern "C" fn on_message_trampoline(msg: *const c_char, raw: *mut c_void) -> bool {
    let bb = unsafe { Box::from_raw(raw as *mut Rc<RefCell<Box<dyn FnMut(&str) -> bool>>>) };
    let rc = (*bb).clone();
    let mut fun = rc.borrow_mut();
    let _ = Box::into_raw(bb); // Avoid dropping the handler
    (*fun)(&ctor!(msg))
}

#[derive(Clone)]
pub struct Webview(Arc<RwLock<UnsafeWebview>>);

impl Webview {
    pub fn new(pref: &Preferences) -> Option<Self> { Some(Webview(Arc::new(RwLock::new(UnsafeWebview::new(pref)?)))) }

    /// Sets a handler for messages from the webview context.
    ///
    /// Only one handler can be set. Setting a new one replaces the previous one.
    ///
    /// This method must be called on the event thread, or it does nothing.
    pub fn on_message(&self, fun: impl FnMut(&str) -> bool + 'static) {
        self.0.write().unwrap().replace_message_handler(fun);
    }

    /// Removes the message handler, if any.
    ///
    /// This method must be called on the event thread, or it does nothing.
    pub fn off_message(&self) { self.0.write().unwrap().remove_message_handler(); }

    pub fn page_title(&self) -> String { ctor!(free, saucer_webview_page_title(self.as_ptr())) }
    pub fn dev_tools(&self) -> bool { unsafe { saucer_webview_dev_tools(self.as_ptr()) } }
    pub fn url(&self) -> String { ctor!(free, saucer_webview_url(self.as_ptr())) }
    pub fn context_menu(&self) -> bool { unsafe { saucer_webview_context_menu(self.as_ptr()) } }
    pub fn background(&self) -> (u8, u8, u8, u8) {
        let mut r = 0u8;
        let mut g = 0u8;
        let mut b = 0u8;
        let mut a = 0u8;
        unsafe {
            saucer_webview_background(
                self.as_ptr(),
                &mut r as *mut u8,
                &mut g as *mut u8,
                &mut b as *mut u8,
                &mut a as *mut u8
            );
        }
        (r, g, b, a)
    }
    pub fn force_dark_mode(&self) -> bool { unsafe { saucer_webview_force_dark_mode(self.as_ptr()) } }

    pub fn set_dev_tools(&self, enabled: bool) { unsafe { saucer_webview_set_dev_tools(self.as_ptr(), enabled) } }
    pub fn set_context_menu(&self, enabled: bool) { unsafe { saucer_webview_set_context_menu(self.as_ptr(), enabled) } }
    pub fn set_force_dark_mode(&self, enabled: bool) {
        unsafe { saucer_webview_set_force_dark_mode(self.as_ptr(), enabled) }
    }
    pub fn set_background(&self, r: u8, g: u8, b: u8, a: u8) {
        unsafe { saucer_webview_set_background(self.as_ptr(), r, g, b, a) }
    }
    pub fn set_file(&self, file: impl AsRef<str>) {
        rtoc!(file => s ; saucer_webview_set_file(self.as_ptr(), s.as_ptr()));
    }
    pub fn set_url(&self, url: impl AsRef<str>) { rtoc!(url => s; saucer_webview_set_url(self.as_ptr(), s.as_ptr())) }

    pub fn back(&self) { unsafe { saucer_webview_back(self.as_ptr()) } }
    pub fn forward(&self) { unsafe { saucer_webview_forward(self.as_ptr()) } }
    pub fn reload(&self) { unsafe { saucer_webview_reload(self.as_ptr()) } }

    pub fn embed_file(&self, name: impl AsRef<str>, file: &EmbedFile, do_async: bool) {
        let launch = if do_async {
            SAUCER_LAUNCH_SAUCER_LAUNCH_ASYNC
        } else {
            SAUCER_LAUNCH_SAUCER_LAUNCH_SYNC
        };
        rtoc!(
            name => n;
            saucer_webview_embed_file(self.as_ptr(), n.as_ptr(), file.as_ptr(), launch) // Data copied internally in C
        );
    }

    pub fn serve(&self, file: impl AsRef<str>) { rtoc!(file => s; saucer_webview_serve(self.as_ptr(), s.as_ptr())) }

    pub fn clear_scripts(&self) { unsafe { saucer_webview_clear_scripts(self.as_ptr()) } }
    pub fn clear_embedded(&self) { unsafe { saucer_webview_clear_embedded(self.as_ptr()) } }

    pub fn inject(&self, script: &Script) { unsafe { saucer_webview_inject(self.as_ptr(), script.as_ptr()) } }
    pub fn execute(&self, code: impl AsRef<str>) { rtoc!(code => s; saucer_webview_execute(self.as_ptr(), s.as_ptr())) }

    pub fn visible(&self) -> bool { unsafe { saucer_window_visible(self.as_ptr()) } }
    pub fn focused(&self) -> bool { unsafe { saucer_window_focused(self.as_ptr()) } }
    pub fn minimized(&self) -> bool { unsafe { saucer_window_minimized(self.as_ptr()) } }
    pub fn maximized(&self) -> bool { unsafe { saucer_window_maximized(self.as_ptr()) } }
    pub fn resizable(&self) -> bool { unsafe { saucer_window_resizable(self.as_ptr()) } }
    pub fn decorations(&self) -> bool { unsafe { saucer_window_decorations(self.as_ptr()) } }
    pub fn always_on_top(&self) -> bool { unsafe { saucer_window_always_on_top(self.as_ptr()) } }
    pub fn click_through(&self) -> bool { unsafe { saucer_window_click_through(self.as_ptr()) } }

    pub fn title(&self) -> String { ctor!(free, saucer_window_title(self.as_ptr())) }

    pub fn size(&self) -> (i32, i32) {
        let mut w = 0;
        let mut h = 0;
        unsafe {
            saucer_window_size(self.as_ptr(), &mut w as *mut i32, &mut h as *mut i32);
        }
        (w, h)
    }

    pub fn max_size(&self) -> (i32, i32) {
        let mut w = 0;
        let mut h = 0;
        unsafe {
            saucer_window_max_size(self.as_ptr(), &mut w as *mut i32, &mut h as *mut i32);
        }
        (w, h)
    }

    pub fn min_size(&self) -> (i32, i32) {
        let mut w = 0;
        let mut h = 0;
        unsafe {
            saucer_window_min_size(self.as_ptr(), &mut w as *mut i32, &mut h as *mut i32);
        }
        (w, h)
    }

    pub fn hide(&self) { unsafe { saucer_window_hide(self.as_ptr()) } }
    pub fn show(&self) { unsafe { saucer_window_show(self.as_ptr()) } }
    pub fn close(&self) { unsafe { saucer_window_close(self.as_ptr()) } }
    pub fn focus(&self) { unsafe { saucer_window_focus(self.as_ptr()) } }

    pub fn set_minimized(&self, b: bool) { unsafe { saucer_window_set_minimized(self.as_ptr(), b) } }
    pub fn set_maximized(&self, b: bool) { unsafe { saucer_window_set_maximized(self.as_ptr(), b) } }
    pub fn set_resizable(&self, b: bool) { unsafe { saucer_window_set_resizable(self.as_ptr(), b) } }
    pub fn set_decorations(&self, b: bool) { unsafe { saucer_window_set_decorations(self.as_ptr(), b) } }
    pub fn set_always_on_top(&self, b: bool) { unsafe { saucer_window_set_always_on_top(self.as_ptr(), b) } }
    pub fn set_click_through(&self, b: bool) { unsafe { saucer_window_set_click_through(self.as_ptr(), b) } }

    pub fn set_title(&self, title: impl AsRef<str>) {
        rtoc!(title => s; saucer_window_set_title(self.as_ptr(), s.as_ptr()))
    }

    pub fn set_size(&self, w: i32, h: i32) { unsafe { saucer_window_set_size(self.as_ptr(), w, h) } }
    pub fn set_max_size(&self, w: i32, h: i32) { unsafe { saucer_window_set_max_size(self.as_ptr(), w, h) } }
    pub fn set_min_size(&self, w: i32, h: i32) { unsafe { saucer_window_set_min_size(self.as_ptr(), w, h) } }

    fn as_ptr(&self) -> *mut saucer_handle { self.0.read().unwrap().as_ptr() }

    fn is_event_thread(&self) -> bool { self.0.read().unwrap().app.is_thread_safe() }
}

// --- Event Handling ---

pub struct WebviewNavigation {
    ptr: NonNull<saucer_navigation>,
    _owns: PhantomData<saucer_navigation>
}

unsafe impl Send for WebviewNavigation {}
unsafe impl Sync for WebviewNavigation {}

impl Drop for WebviewNavigation {
    fn drop(&mut self) { unsafe { saucer_navigation_free(self.ptr.as_ptr()) } }
}

impl WebviewNavigation {
    unsafe fn from_ptr(ptr: *mut saucer_navigation) -> Self {
        Self {
            ptr: NonNull::new(ptr).expect("Invalid navigation descriptor"),
            _owns: PhantomData
        }
    }

    pub fn is_new_window(&self) -> bool { unsafe { saucer_navigation_new_window(self.ptr.as_ptr()) } }

    pub fn is_redirection(&self) -> bool { unsafe { saucer_navigation_redirection(self.ptr.as_ptr()) } }

    pub fn is_user_initiated(&self) -> bool { unsafe { saucer_navigation_user_initiated(self.ptr.as_ptr()) } }

    pub fn url(&self) -> String { ctor!(free, saucer_navigation_url(self.ptr.as_ptr())) }
}

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
        let ptr = Box::into_raw(Box::new(Rc::new(RefCell::new(bb))));
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
        let ptr = Box::into_raw(Box::new(Rc::new(RefCell::new(Some(bb)))));

        unsafe {
            $capi($sf.as_ptr(), $chn, fn_ptr, ptr as *mut c_void);
        }

        let mut guard = $sf.0.write().unwrap();

        // Returns true if the dropper can be removed
        let checker = Box::new(move || {
            let bb = unsafe { Box::from_raw(ptr) };
            let rt = if let Ok(opt) = bb.try_borrow() {
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
    pub fn once_dom_ready(&self, fun: impl FnOnce() + 'static) {
        handle_evt_once!(
            webview,
            self,
            once_dom_ready_trampoline,
            fun -> dyn FnOnce() + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_DOM_READY
        )
    }

    pub fn once_navigate(&self, fun: impl FnOnce(WebviewNavigation) -> bool + 'static) {
        handle_evt_once!(
            webview,
            self,
            once_navigate_trampoline,
            fun -> dyn FnOnce(WebviewNavigation) -> bool + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_NAVIGATE
        )
    }

    pub fn once_navigated(&self, fun: impl FnOnce(&str) + 'static) {
        handle_evt_once!(
            webview,
            self,
            once_navigated_trampoline,
            fun -> dyn FnOnce(&str) + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_NAVIGATED
        )
    }

    pub fn once_title(&self, fun: impl FnOnce(&str) + 'static) {
        handle_evt_once!(
            webview,
            self,
            once_title_trampoline,
            fun -> dyn FnOnce(&str) + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_TITLE
        )
    }

    pub fn once_favicon(&self, fun: impl FnOnce(Icon) + 'static) {
        handle_evt_once!(
            webview,
            self,
            once_favicon_trampoline,
            fun -> dyn FnOnce(Icon) + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_FAVICON
        )
    }

    pub fn once_load(&self, fun: impl FnOnce(WebviewLoadState) + 'static) {
        handle_evt_once!(
            webview,
            self,
            once_load_trampoline,
            fun -> dyn FnOnce(WebviewLoadState) + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_LOAD
        )
    }

    pub fn once_decorated(&self, fun: impl FnOnce(bool) + 'static) {
        handle_evt_once!(
            webview,
            self,
            once_decorated_trampoline,
            fun -> dyn FnOnce(bool) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_DECORATED
        )
    }

    pub fn once_maximize(&self, fun: impl FnOnce(bool) + 'static) {
        handle_evt_once!(
            window,
            self,
            once_maximize_trampoline,
            fun -> dyn FnOnce(bool) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_MAXIMIZE
        )
    }

    pub fn once_minimize(&self, fun: impl FnOnce(bool) + 'static) {
        handle_evt_once!(
            window,
            self,
            once_minimize_trampoline,
            fun -> dyn FnOnce(bool) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_MINIMIZE
        )
    }

    pub fn once_closed(&self, fun: impl FnOnce() + 'static) {
        handle_evt_once!(
            window,
            self,
            once_closed_trampoline,
            fun -> dyn FnOnce() + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_CLOSED
        )
    }

    pub fn once_resize(&self, fun: impl FnOnce(i32, i32) + 'static) {
        handle_evt_once!(
            window,
            self,
            once_resize_trampoline,
            fun -> dyn FnOnce(i32, i32) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_RESIZE
        )
    }

    pub fn once_focus(&self, fun: impl FnOnce(bool) + 'static) {
        handle_evt_once!(
            window,
            self,
            once_focus_trampoline,
            fun -> dyn FnOnce(bool) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_FOCUS
        )
    }

    pub fn once_close(&self, fun: impl FnOnce() -> bool + 'static) {
        handle_evt_once!(
            window,
            self,
            once_close_trampoline,
            fun -> dyn FnOnce() -> bool + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_CLOSE
        )
    }

    pub fn on_dom_ready(&self, fun: impl FnMut() + 'static) -> Option<u64> {
        handle_evt!(
            webview,
            self,
            on_dom_ready_trampoline,
            fun -> dyn FnMut() + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_DOM_READY
        )
    }

    pub fn on_navigate(&self, fun: impl FnMut(WebviewNavigation) -> bool + 'static) -> Option<u64> {
        handle_evt!(
            webview,
            self,
            on_navigate_trampoline,
            fun -> dyn FnMut(WebviewNavigation) -> bool + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_NAVIGATE
        )
    }

    pub fn on_navigated(&self, fun: impl FnMut(&str) + 'static) -> Option<u64> {
        handle_evt!(
            webview,
            self,
            on_navigated_trampoline,
            fun -> dyn FnMut(&str) + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_NAVIGATED
        )
    }

    pub fn on_title(&self, fun: impl FnMut(&str) + 'static) -> Option<u64> {
        handle_evt!(
            webview,
            self,
            on_title_trampoline,
            fun -> dyn FnMut(&str) + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_TITLE
        )
    }

    pub fn on_favicon(&self, fun: impl FnMut(Icon) + 'static) -> Option<u64> {
        handle_evt!(
            webview,
            self,
            on_favicon_trampoline,
            fun -> dyn FnMut(Icon) + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_FAVICON
        )
    }

    pub fn on_load(&self, fun: impl FnMut(WebviewLoadState) + 'static) -> Option<u64> {
        handle_evt!(
            webview,
            self,
            on_load_trampoline,
            fun -> dyn FnMut(WebviewLoadState) + 'static,
            SAUCER_WEB_EVENT_SAUCER_WEB_EVENT_LOAD
        )
    }

    pub fn on_decorated(&self, fun: impl FnMut(bool) + 'static) -> Option<u64> {
        handle_evt!(
            window,
            self,
            on_decorated_trampoline,
            fun -> dyn FnMut(bool) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_DECORATED
        )
    }

    pub fn on_maximize(&self, fun: impl FnMut(bool) + 'static) -> Option<u64> {
        handle_evt!(
            window,
            self,
            on_maximize_trampoline,
            fun -> dyn FnMut(bool) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_MAXIMIZE
        )
    }

    pub fn on_minimize(&self, fun: impl FnMut(bool) + 'static) -> Option<u64> {
        handle_evt!(
            window,
            self,
            on_minimize_trampoline,
            fun -> dyn FnMut(bool) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_MINIMIZE
        )
    }

    pub fn on_closed(&self, fun: impl FnMut() + 'static) -> Option<u64> {
        handle_evt!(
            window,
            self,
            on_closed_trampoline,
            fun -> dyn FnMut() + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_CLOSED
        )
    }

    pub fn on_resize(&self, fun: impl FnMut(i32, i32) + 'static) -> Option<u64> {
        handle_evt!(
            window,
            self,
            on_resize_trampoline ,
            fun -> dyn FnMut(i32, i32) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_RESIZE
        )
    }

    pub fn on_focus(&self, fun: impl FnMut(bool) + 'static) -> Option<u64> {
        handle_evt!(
            window,
            self,
            on_focus_trampoline ,
            fun -> dyn FnMut(bool) + 'static,
            SAUCER_WINDOW_EVENT_SAUCER_WINDOW_EVENT_FOCUS
        )
    }

    pub fn on_close(&self, fun: impl FnMut() -> bool + 'static) -> Option<u64> {
        handle_evt!(
            window,
            self,
            on_close_trampoline,
            fun -> dyn FnMut() -> bool + 'static,
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
    let bb = unsafe { Box::from_raw(arg as *mut Rc<RefCell<Option<Box<dyn FnOnce()>>>>) };
    if let Some(fun) = bb.borrow_mut().take() {
        fun();
    }
    let _ = Box::into_raw(bb);
}

extern "C" fn once_navigate_trampoline(_: *mut saucer_handle, arg: *mut c_void, nav: *mut saucer_navigation) -> bool {
    let bb = unsafe { Box::from_raw(arg as *mut Rc<RefCell<Option<Box<dyn FnOnce(WebviewNavigation) -> bool>>>>) };
    let rt = if let Some(fun) = bb.borrow_mut().take() {
        let nav = unsafe { WebviewNavigation::from_ptr(nav) };
        fun(nav)
    } else {
        false
    };
    let _ = Box::into_raw(bb);
    rt
}

extern "C" fn once_navigated_trampoline(_: *mut saucer_handle, arg: *mut c_void, url: *mut c_char) {
    let url = ctor!(url);
    let bb = unsafe { Box::from_raw(arg as *mut Rc<RefCell<Option<Box<dyn FnOnce(&str)>>>>) };
    if let Some(fun) = bb.borrow_mut().take() {
        fun(&url);
    }
    let _ = Box::into_raw(bb);
}

extern "C" fn once_title_trampoline(h: *mut saucer_handle, arg: *mut c_void, url: *mut c_char) {
    once_navigated_trampoline(h, arg, url);
}

extern "C" fn once_favicon_trampoline(_: *mut saucer_handle, arg: *mut c_void, icon: *mut saucer_icon) {
    let icon = Icon::from_ptr(icon);
    let bb = unsafe { Box::from_raw(arg as *mut Rc<RefCell<Option<Box<dyn FnOnce(Icon)>>>>) };
    if let Some(fun) = bb.borrow_mut().take() {
        fun(icon);
    }
    let _ = Box::into_raw(bb);
}

extern "C" fn once_load_trampoline(_: *mut saucer_handle, arg: *mut c_void, state: SAUCER_STATE) {
    let bb = unsafe { Box::from_raw(arg as *mut Rc<RefCell<Option<Box<dyn FnOnce(WebviewLoadState)>>>>) };

    let state = if state == SAUCER_STATE_SAUCER_STATE_STARTED {
        WebviewLoadState::Started
    } else {
        WebviewLoadState::Finished
    };

    if let Some(fun) = bb.borrow_mut().take() {
        fun(state);
    }

    let _ = Box::into_raw(bb);
}

extern "C" fn once_decorated_trampoline(_: *mut saucer_handle, arg: *mut c_void, b: bool) {
    let bb = unsafe { Box::from_raw(arg as *mut Rc<RefCell<Option<Box<dyn FnOnce(bool)>>>>) };
    if let Some(fun) = bb.borrow_mut().take() {
        fun(b);
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
    let bb = unsafe { Box::from_raw(arg as *mut Rc<RefCell<Option<Box<dyn FnOnce(i32, i32)>>>>) };
    if let Some(fun) = bb.borrow_mut().take() {
        fun(w, h);
    }
    let _ = Box::into_raw(bb);
}

extern "C" fn once_focus_trampoline(h: *mut saucer_handle, arg: *mut c_void, b: bool) {
    once_decorated_trampoline(h, arg, b);
}

extern "C" fn once_close_trampoline(_: *mut saucer_handle, arg: *mut c_void) -> SAUCER_POLICY {
    let bb = unsafe { Box::from_raw(arg as *mut Rc<RefCell<Option<Box<dyn FnOnce() -> bool>>>>) };
    let rt = if let Some(fun) = bb.borrow_mut().take() {
        if fun() {
            SAUCER_POLICY_SAUCER_POLICY_ALLOW
        } else {
            SAUCER_POLICY_SAUCER_POLICY_BLOCK
        }
    } else {
        SAUCER_POLICY_SAUCER_POLICY_ALLOW
    };
    let _ = Box::into_raw(bb);
    rt
}

extern "C" fn on_dom_ready_trampoline(_: *mut saucer_handle, arg: *mut c_void) {
    let bb = unsafe { Box::from_raw(arg as *mut Rc<RefCell<Box<dyn FnMut()>>>) };
    let rc = (*bb).clone();
    let _ = Box::into_raw(bb);
    rc.borrow_mut()();
}

extern "C" fn on_navigate_trampoline(_: *mut saucer_handle, arg: *mut c_void, nav: *mut saucer_navigation) -> bool {
    let bb = unsafe { Box::from_raw(arg as *mut Rc<RefCell<Box<dyn FnMut(WebviewNavigation) -> bool>>>) };
    let rc = (*bb).clone();
    let _ = Box::into_raw(bb);
    let nav = unsafe { WebviewNavigation::from_ptr(nav) };
    rc.borrow_mut()(nav)
}

extern "C" fn on_navigated_trampoline(_: *mut saucer_handle, arg: *mut c_void, url: *mut c_char) {
    let url = ctor!(url);
    let bb = unsafe { Box::from_raw(arg as *mut Rc<RefCell<Box<dyn FnMut(&str)>>>) };
    let rc = (*bb).clone();
    let _ = Box::into_raw(bb);
    rc.borrow_mut()(&url);
}

extern "C" fn on_title_trampoline(h: *mut saucer_handle, arg: *mut c_void, url: *mut c_char) {
    on_navigated_trampoline(h, arg, url);
}

extern "C" fn on_favicon_trampoline(_: *mut saucer_handle, arg: *mut c_void, icon: *mut saucer_icon) {
    let icon = Icon::from_ptr(icon);
    let bb = unsafe { Box::from_raw(arg as *mut Rc<RefCell<Box<dyn FnMut(Icon)>>>) };
    let rc = (*bb).clone();
    let _ = Box::into_raw(bb);
    rc.borrow_mut()(icon);
}

extern "C" fn on_load_trampoline(_: *mut saucer_handle, arg: *mut c_void, state: SAUCER_STATE) {
    let bb = unsafe { Box::from_raw(arg as *mut Rc<RefCell<Box<dyn FnMut(WebviewLoadState)>>>) };
    let rc = (*bb).clone();
    let _ = Box::into_raw(bb);

    let state = if state == SAUCER_STATE_SAUCER_STATE_STARTED {
        WebviewLoadState::Started
    } else {
        WebviewLoadState::Finished
    };

    rc.borrow_mut()(state);
}

extern "C" fn on_decorated_trampoline(_: *mut saucer_handle, arg: *mut c_void, b: bool) {
    let bb = unsafe { Box::from_raw(arg as *mut Rc<RefCell<Box<dyn FnMut(bool)>>>) };
    let rc = (*bb).clone();
    let _ = Box::into_raw(bb);
    rc.borrow_mut()(b);
}

extern "C" fn on_maximize_trampoline(h: *mut saucer_handle, arg: *mut c_void, b: bool) {
    on_decorated_trampoline(h, arg, b);
}

extern "C" fn on_minimize_trampoline(h: *mut saucer_handle, arg: *mut c_void, b: bool) {
    on_decorated_trampoline(h, arg, b);
}

extern "C" fn on_closed_trampoline(h: *mut saucer_handle, arg: *mut c_void) { on_dom_ready_trampoline(h, arg); }

extern "C" fn on_resize_trampoline(_: *mut saucer_handle, arg: *mut c_void, w: i32, h: i32) {
    let bb = unsafe { Box::from_raw(arg as *mut Rc<RefCell<Box<dyn FnMut(i32, i32)>>>) };
    let rc = (*bb).clone();
    let _ = Box::into_raw(bb);
    rc.borrow_mut()(w, h);
}

extern "C" fn on_focus_trampoline(h: *mut saucer_handle, arg: *mut c_void, b: bool) {
    on_decorated_trampoline(h, arg, b);
}

extern "C" fn on_close_trampoline(_: *mut saucer_handle, arg: *mut c_void) -> SAUCER_POLICY {
    let bb = unsafe { Box::from_raw(arg as *mut Rc<RefCell<Box<dyn FnMut() -> bool>>>) };
    let rc = (*bb).clone();
    let _ = Box::into_raw(bb);
    let rr = rc.borrow_mut()();

    if rr {
        SAUCER_POLICY_SAUCER_POLICY_ALLOW
    } else {
        SAUCER_POLICY_SAUCER_POLICY_BLOCK
    }
}
