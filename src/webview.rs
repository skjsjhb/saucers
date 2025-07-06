use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_char;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::ptr::null_mut;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::Weak;
use std::sync::mpmc::Sender;

use crate::app::App;
use crate::capi::*;
use crate::collector::Collect;
use crate::collector::UnsafeCollector;
use crate::ctor;
use crate::embed::EmbedFile;
use crate::prefs::Preferences;
use crate::rtoc;
use crate::script::Script;

pub(crate) struct WebviewPtr {
    ptr: NonNull<saucer_handle>,
    message_handler: Option<*mut Rc<RefCell<Box<dyn FnMut(&str) -> bool>>>>,
    _owns: PhantomData<saucer_handle>,
    _counter: Arc<()>,

    pub(crate) web_event_droppers: HashMap<(SAUCER_WEB_EVENT, u64), Box<dyn FnOnce() + 'static>>,
    pub(crate) window_event_droppers: HashMap<(SAUCER_WINDOW_EVENT, u64), Box<dyn FnOnce() + 'static>>,

    // A pair of (checker, dropper), checker returns whether the dropper can be removed
    pub(crate) once_event_droppers: Vec<(Box<dyn FnMut() -> bool + 'static>, Box<dyn FnOnce() + 'static>)>
}

unsafe impl Send for WebviewPtr {}
unsafe impl Sync for WebviewPtr {}

impl WebviewPtr {
    fn as_ptr(&self) -> *mut saucer_handle { self.ptr.as_ptr() }
}

impl Collect for WebviewPtr {
    fn collect(self: Box<Self>) {
        unsafe {
            saucer_free(self.ptr.as_ptr());

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

pub(crate) struct UnsafeWebview {
    pub(crate) ptr: Option<WebviewPtr>,
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

        Some(Self {
            ptr: Some(WebviewPtr {
                ptr,
                message_handler: None,
                _owns: PhantomData,
                _counter: collector.count(),
                web_event_droppers: HashMap::new(),
                window_event_droppers: HashMap::new(),
                once_event_droppers: Vec::new()
            }),
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

extern "C" fn on_message_trampoline(msg: *const c_char, raw: *mut c_void) -> bool {
    let bb = unsafe { Box::from_raw(raw as *mut Rc<RefCell<Box<dyn FnMut(&str) -> bool>>>) };
    let rc = (*bb).clone();
    let mut fun = rc.borrow_mut();
    let _ = Box::into_raw(bb); // Avoid dropping the handler
    (*fun)(&ctor!(msg))
}

#[derive(Clone)]
pub struct Webview(pub(crate) Arc<RwLock<UnsafeWebview>>);

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

    pub(crate) fn as_ptr(&self) -> *mut saucer_handle { self.0.read().unwrap().as_ptr() }

    pub(crate) fn is_event_thread(&self) -> bool { self.0.read().unwrap().app.is_thread_safe() }
}
