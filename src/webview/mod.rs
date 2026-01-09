mod events;
mod options;
mod script;

use std::borrow::Cow;
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

pub use events::*;
pub use options::*;
use saucer_sys::*;
pub use script::*;

use crate::icon::Icon;
use crate::macros::load_range;
use crate::macros::use_string;
use crate::navigation::Navigation;
use crate::permission::PermissionRequest;
use crate::policy::Policy;
use crate::scheme::Executor;
use crate::scheme::Request;
use crate::stash::Stash;
use crate::status::HandleStatus;
use crate::url::Url;
use crate::window::Window;

/// An unprotected raw webview handle.
struct RawWebview {
    inner: NonNull<saucer_webview>,
    drop_sender: Sender<Box<dyn FnOnce() + Send>>,
    host_tid: ThreadId,
    event_listener_data: RefCell<*mut EventListenerData>,
    scheme_handler_data: RefCell<*mut SchemeHandlerData>,
    schemes: Vec<Cow<'static, str>>,
    window: Window, // Keep the window alive
    _marker: PhantomData<saucer_webview>,
}

unsafe impl Send for RawWebview {}
unsafe impl Sync for RawWebview {}

struct RawWebviewCleanUp {
    inner: NonNull<saucer_webview>,
    schemes: Vec<Cow<'static, str>>,
    event_listener_data: *mut EventListenerData,
    scheme_handler_data: *mut SchemeHandlerData,
}

unsafe impl Send for RawWebviewCleanUp {}

impl Drop for RawWebview {
    fn drop(&mut self) {
        let cl = RawWebviewCleanUp {
            inner: self.inner,
            schemes: self.schemes.drain(..).collect(),
            event_listener_data: *self.event_listener_data.borrow_mut(), // Ensure exclusive access
            scheme_handler_data: *self.scheme_handler_data.borrow_mut(),
        };

        let col = move || unsafe {
            let _ = &cl;
            let ptr = cl.inner.as_ptr();

            for s in cl.schemes {
                use_string!(s: s.as_ref(); saucer_webview_remove_scheme(ptr, s));
            }

            saucer_webview_off_all(ptr, SAUCER_WEBVIEW_EVENT_PERMISSION);
            saucer_webview_off_all(ptr, SAUCER_WEBVIEW_EVENT_FULLSCREEN);
            saucer_webview_off_all(ptr, SAUCER_WEBVIEW_EVENT_DOM_READY);
            saucer_webview_off_all(ptr, SAUCER_WEBVIEW_EVENT_NAVIGATED);
            saucer_webview_off_all(ptr, SAUCER_WEBVIEW_EVENT_NAVIGATE);
            saucer_webview_off_all(ptr, SAUCER_WEBVIEW_EVENT_MESSAGE);
            saucer_webview_off_all(ptr, SAUCER_WEBVIEW_EVENT_REQUEST);
            saucer_webview_off_all(ptr, SAUCER_WEBVIEW_EVENT_FAVICON);
            saucer_webview_off_all(ptr, SAUCER_WEBVIEW_EVENT_TITLE);
            saucer_webview_off_all(ptr, SAUCER_WEBVIEW_EVENT_LOAD);

            // Technically, a webview may be freed after its corresponding window due to the
            // deferred posting, which may introduce broken states. However, such broken states will
            // only happen when the handle is being dropped, which is not visible to the safe world.
            saucer_webview_free(ptr);

            drop(Box::from_raw(cl.scheme_handler_data));
            drop(Box::from_raw(cl.event_listener_data));
        };

        if self.is_thread_safe() {
            col();
        } else {
            self.drop_sender.send(Box::new(col)).expect("failed to post webview destruction");
        }
    }
}

impl RawWebview {
    fn is_thread_safe(&self) -> bool { std::thread::current().id() == self.host_tid }
}

#[derive(Clone)]
pub struct Webview(Arc<RawWebview>);

impl Webview {
    pub fn new(
        opt: WebviewOptions,
        window: Window,
        event_listener: impl WebviewEventListener + 'static,
        scheme_handler: impl WebviewSchemeHandler + 'static,
        schemes: Vec<Cow<'static, str>>,
    ) -> crate::error::Result<Self> {
        if !window.is_thread_safe() {
            panic!("webviews must be created on the event thread");
        }

        let ds = window.drop_sender();
        let w = window.clone();
        let mut ex = -1;
        let opt = RawWebviewOptions::new(opt, window);
        let ptr = unsafe { saucer_webview_new(opt.as_ptr(), &raw mut ex) };
        let wv = NonNull::new(ptr).ok_or(crate::error::Error::Saucer(ex))?;

        let wv = Self(Arc::new(RawWebview {
            inner: wv,
            drop_sender: ds,
            host_tid: std::thread::current().id(),
            event_listener_data: RefCell::new(null_mut()),
            scheme_handler_data: RefCell::new(null_mut()),
            schemes,
            window: w,
            _marker: PhantomData,
        }));

        let data = EventListenerData::new(event_listener, wv.downgrade());
        let data = Box::into_raw(Box::new(data));

        let scheme_data = SchemeHandlerData::new(scheme_handler, wv.downgrade());
        let scheme_data = Box::into_raw(Box::new(scheme_data));

        *wv.0.event_listener_data.borrow_mut() = data;
        *wv.0.scheme_handler_data.borrow_mut() = scheme_data;

        for s in &wv.0.schemes {
            use_string!(s: s.as_ref(); unsafe {
               saucer_webview_handle_scheme(ptr, s, Some(handle_scheme_tp), scheme_data as *mut c_void)
            });
        }

        macro_rules! bind_event {
            ($ev:expr, $cb:expr) => {
                unsafe {
                    saucer_webview_on(ptr, $ev, $cb as *mut c_void, true, data as *mut c_void)
                };
            };
        }

        bind_event!(SAUCER_WEBVIEW_EVENT_PERMISSION, ev_on_permission_tp);
        bind_event!(SAUCER_WEBVIEW_EVENT_FULLSCREEN, ev_on_fullscreen_tp);
        bind_event!(SAUCER_WEBVIEW_EVENT_DOM_READY, ev_on_dom_ready_tp);
        bind_event!(SAUCER_WEBVIEW_EVENT_NAVIGATED, ev_on_navigated_tp);
        bind_event!(SAUCER_WEBVIEW_EVENT_NAVIGATE, ev_on_navigate_tp);
        bind_event!(SAUCER_WEBVIEW_EVENT_MESSAGE, ev_on_message_tp);
        bind_event!(SAUCER_WEBVIEW_EVENT_REQUEST, ev_on_request_tp);
        bind_event!(SAUCER_WEBVIEW_EVENT_FAVICON, ev_on_favicon_tp);
        bind_event!(SAUCER_WEBVIEW_EVENT_TITLE, ev_on_title_tp);
        bind_event!(SAUCER_WEBVIEW_EVENT_LOAD, ev_on_load_tp);

        Ok(wv)
    }

    pub fn url(&self) -> crate::error::Result<Url> {
        let mut ex = -1;
        let ptr = unsafe { saucer_webview_url(self.as_ptr(), &raw mut ex) };

        unsafe { Url::from_ptr(ptr, ex) }
    }

    pub fn favicon(&self) -> Icon {
        unsafe { Icon::from_ptr(saucer_webview_favicon(self.as_ptr())) }
    }

    pub fn page_title(&self) -> String {
        let buf = load_range!(ptr[size] = 0u8; {
            unsafe { saucer_webview_page_title(self.as_ptr(), ptr as *mut c_char, size) }
        });

        String::from_utf8_lossy(&buf).into_owned()
    }

    pub fn has_dev_tools(&self) -> bool { unsafe { saucer_webview_dev_tools(self.as_ptr()) } }

    pub fn has_context_menu(&self) -> bool { unsafe { saucer_webview_context_menu(self.as_ptr()) } }

    pub fn is_force_dark(&self) -> bool { unsafe { saucer_webview_force_dark(self.as_ptr()) } }

    pub fn background(&self) -> (u8, u8, u8, u8) {
        let mut r = 0;
        let mut g = 0;
        let mut b = 0;
        let mut a = 0;

        unsafe {
            saucer_webview_background(self.as_ptr(), &raw mut r, &raw mut g, &raw mut b, &raw mut a)
        }

        (r, g, b, a)
    }

    pub fn bounds(&self) -> (i32, i32, i32, i32) {
        let mut x = 0;
        let mut y = 0;
        let mut w = 0;
        let mut h = 0;

        unsafe {
            saucer_webview_bounds(self.as_ptr(), &raw mut x, &raw mut y, &raw mut w, &raw mut h)
        }

        (x, y, w, h)
    }

    pub fn set_url(&self, url: impl AsRef<Url>) {
        unsafe { saucer_webview_set_url(self.as_ptr(), url.as_ref().as_ptr()) } // Value copied
    }

    pub fn set_url_str(&self, url: impl Into<Vec<u8>>) {
        use_string!(url; unsafe { saucer_webview_set_url_str(self.as_ptr(), url) });
    }

    pub fn set_html(&self, html: impl Into<Vec<u8>>) {
        use_string!(html; unsafe { saucer_webview_set_html(self.as_ptr(), html) });
    }

    pub fn set_dev_tools(&self, enabled: bool) {
        unsafe { saucer_webview_set_dev_tools(self.as_ptr(), enabled) }
    }

    pub fn set_context_menu(&self, enabled: bool) {
        unsafe { saucer_webview_set_context_menu(self.as_ptr(), enabled) }
    }

    pub fn set_force_dark(&self, enabled: bool) {
        unsafe { saucer_webview_set_force_dark(self.as_ptr(), enabled) }
    }

    pub fn set_background(&self, r: u8, g: u8, b: u8, a: u8) {
        unsafe { saucer_webview_set_background(self.as_ptr(), r, g, b, a) }
    }

    pub fn reset_bounds(&self) { unsafe { saucer_webview_reset_bounds(self.as_ptr()) } }

    pub fn set_bounds(&self, x: i32, y: i32, w: i32, h: i32) {
        unsafe { saucer_webview_set_bounds(self.as_ptr(), x, y, w, h) }
    }

    pub fn back(&self) { unsafe { saucer_webview_back(self.as_ptr()) } }

    pub fn forward(&self) { unsafe { saucer_webview_forward(self.as_ptr()) } }

    pub fn reload(&self) { unsafe { saucer_webview_reload(self.as_ptr()) } }

    /// Navigates to the embedded content specified by the path.
    pub fn serve(&self, path: impl Into<Vec<u8>>) {
        use_string!(path; unsafe { saucer_webview_serve(self.as_ptr(), path) });
    }

    pub fn embed(
        &self,
        path: impl Into<Vec<u8>>,
        content: Stash<'static>,
        mime: impl Into<Vec<u8>>,
    ) {
        use_string!(path, mime; unsafe {
            saucer_webview_embed(self.as_ptr(), path, content.as_ptr(), mime) // Value copied, yet the stash is !Sync
        });
    }

    pub fn unembed_all(&self) { unsafe { saucer_webview_unembed_all(self.as_ptr()) } }

    pub fn unembed(&self, path: impl Into<Vec<u8>>) {
        use_string!(path; unsafe { saucer_webview_unembed(self.as_ptr(), path) });
    }

    pub fn execute(&self, js: impl Into<Vec<u8>>) {
        use_string!(js; unsafe { saucer_webview_execute(self.as_ptr(), js) });
    }

    pub fn inject(
        &self,
        js: impl Into<Vec<u8>>,
        script_time: ScriptTime,
        no_frames: bool,
        clearable: bool,
    ) -> ScriptId {
        let u = use_string!(js; unsafe {
            saucer_webview_inject(self.as_ptr(), js, script_time.into(), no_frames, clearable)
        });

        ScriptId::from_usize(u)
    }

    pub fn uninject_all(&self) { unsafe { saucer_webview_uninject_all(self.as_ptr()) } }

    pub fn uninject(&self, id: ScriptId) {
        unsafe { saucer_webview_uninject(self.as_ptr(), id.as_usize()) }
    }

    pub fn window(&self) -> Window { self.0.window.clone() }

    pub fn downgrade(&self) -> WebviewRef { WebviewRef(Arc::downgrade(&self.0)) }

    pub(crate) fn as_ptr(&self) -> *mut saucer_webview { self.0.inner.as_ptr() }
}

#[derive(Clone)]
pub struct WebviewRef(Weak<RawWebview>);

impl WebviewRef {
    /// Tries to upgrade to a strong handle.
    pub fn upgrade(&self) -> Option<Webview> { Some(Webview(self.0.upgrade()?)) }
}

struct SchemeHandlerData {
    handler: Box<dyn WebviewSchemeHandler + 'static>,
    webview: WebviewRef,
}

impl SchemeHandlerData {
    fn new(handler: impl WebviewSchemeHandler + 'static, webview: WebviewRef) -> Self {
        Self { handler: Box::new(handler), webview }
    }
}

struct EventListenerData {
    listener: Box<dyn WebviewEventListener + 'static>,
    webview: WebviewRef,
}

impl EventListenerData {
    fn new(listener: impl WebviewEventListener + 'static, webview: WebviewRef) -> Self {
        Self { listener: Box::new(listener), webview }
    }
}

extern "C" fn ev_on_permission_tp(
    _: *mut saucer_webview,
    req: *mut saucer_permission_request,
    data: *mut c_void,
) -> saucer_status {
    let req = unsafe { PermissionRequest::from_ptr(saucer_permission_request_copy(req)) };
    let data = unsafe { &*(data as *const EventListenerData) };

    let ret = if let Some(w) = data.webview.upgrade() {
        data.listener.on_permission(w.clone(), req)
    } else {
        HandleStatus::Unhandled
    };

    ret.into()
}

extern "C" fn ev_on_fullscreen_tp(
    _: *mut saucer_webview,
    is_fullscreen: bool,
    data: *mut c_void,
) -> saucer_policy {
    let data = unsafe { &*(data as *const EventListenerData) };

    let ret = if let Some(w) = data.webview.upgrade() {
        data.listener.on_fullscreen(w.clone(), is_fullscreen)
    } else {
        Policy::Allow
    };

    ret.into()
}

extern "C" fn ev_on_dom_ready_tp(_: *mut saucer_webview, data: *mut c_void) {
    let data = unsafe { &*(data as *const EventListenerData) };

    if let Some(w) = data.webview.upgrade() {
        data.listener.on_dom_ready(w.clone());
    }
}

extern "C" fn ev_on_navigated_tp(_: *mut saucer_webview, url: *mut saucer_url, data: *mut c_void) {
    let url = unsafe {
        Url::from_ptr(saucer_url_copy(url), -1).expect("navigation target URL should exist")
    };
    let data = unsafe { &*(data as *const EventListenerData) };

    if let Some(w) = data.webview.upgrade() {
        data.listener.on_navigated(w.clone(), url);
    }
}

extern "C" fn ev_on_navigate_tp(
    _: *mut saucer_webview,
    nav: *mut saucer_navigation,
    data: *mut c_void,
) -> saucer_policy {
    let nav = unsafe { Navigation::from_ptr(nav) }; // SAFETY: It can't be moved out

    let data = unsafe { &*(data as *const EventListenerData) };

    let ret = if let Some(w) = data.webview.upgrade() {
        data.listener.on_navigate(w.clone(), &nav)
    } else {
        Policy::Allow
    };

    let _ = nav; // Ensure it's not moved
    ret.into()
}

extern "C" fn ev_on_message_tp(
    _: *mut saucer_webview,
    msg: *mut c_char,
    size: usize,
    data: *mut c_void,
) -> saucer_status {
    let s = unsafe { std::slice::from_raw_parts_mut(msg as *mut u8, size) };
    let s = String::from_utf8_lossy(s);

    let data = unsafe { &*(data as *const EventListenerData) };

    let ret = if let Some(w) = data.webview.upgrade() {
        data.listener.on_message(w.clone(), s)
    } else {
        HandleStatus::Unhandled
    };

    ret.into()
}

extern "C" fn ev_on_request_tp(_: *mut saucer_webview, req: *mut saucer_url, data: *mut c_void) {
    let url = unsafe { Url::from_ptr(saucer_url_copy(req), -1).expect("request URL should exist") };
    let data = unsafe { &*(data as *const EventListenerData) };

    if let Some(w) = data.webview.upgrade() {
        data.listener.on_request(w.clone(), url);
    }
}

extern "C" fn ev_on_favicon_tp(
    _: *mut saucer_webview,
    favicon: *mut saucer_icon,
    data: *mut c_void,
) {
    let icon = unsafe { Icon::from_ptr(saucer_icon_copy(favicon)) };
    let data = unsafe { &*(data as *const EventListenerData) };

    if let Some(w) = data.webview.upgrade() {
        data.listener.on_favicon(w.clone(), icon);
    }
}

extern "C" fn ev_on_title_tp(
    _: *mut saucer_webview,
    title: *mut c_char,
    size: usize,
    data: *mut c_void,
) {
    let s = unsafe { std::slice::from_raw_parts_mut(title as *mut u8, size) };
    let s = String::from_utf8_lossy(s).into_owned();

    let data = unsafe { &*(data as *const EventListenerData) };

    if let Some(w) = data.webview.upgrade() {
        data.listener.on_title(w.clone(), s);
    }
}

extern "C" fn ev_on_load_tp(_: *mut saucer_webview, state: saucer_state, data: *mut c_void) {
    let data = unsafe { &*(data as *const EventListenerData) };

    if let Some(w) = data.webview.upgrade() {
        data.listener.on_load(w.clone(), state.into());
    }
}

extern "C" fn handle_scheme_tp(
    req: *mut saucer_scheme_request,
    exc: *mut saucer_scheme_executor,
    data: *mut c_void,
) {
    let data = unsafe { &*(data as *mut SchemeHandlerData) };

    // Both the request and the executor are borrowed (via auto conversion in C++)

    let req = unsafe { Request::from_ptr(saucer_scheme_request_copy(req)) };
    let exc = unsafe { Executor::from_ptr(saucer_scheme_executor_copy(exc)) };

    if let Some(w) = data.webview.upgrade() {
        data.handler.handle_scheme(w.clone(), req, exc)
    }
}
