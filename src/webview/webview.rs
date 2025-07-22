use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_char;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::ptr::null_mut;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::Weak;
use std::sync::mpsc::Sender;

use crate::app::App;
use crate::capi::*;
use crate::collector::Collect;
use crate::collector::UnsafeCollector;
use crate::embed::EmbedFile;
use crate::icon::Icon;
use crate::macros::rtoc;
use crate::prefs::Preferences;
use crate::scheme::Executor;
use crate::scheme::Request;
use crate::script::Script;
use crate::util::shot_str;
use crate::util::take_str;

pub(crate) struct WebviewPtr {
    ptr: NonNull<saucer_handle>,
    // Message handlers are only ever called on the event thread so `Rc` + `RefCell` is sufficient.
    message_handler: Option<*mut (WebviewRef, Rc<RefCell<Box<dyn FnMut(Webview, &str)>>>)>,
    // Unlike message handlers, scheme handlers may be called outside the event thread and must be locked.
    scheme_handlers: HashMap<String, *mut (WebviewRef, Arc<Mutex<Box<dyn FnMut(Webview, Request, Executor)>>>)>,
    _owns: PhantomData<saucer_handle>,
    _counter: Arc<()>,

    pub(in crate::webview) dyn_event_droppers: HashMap<(u32, u64), Box<dyn FnOnce() + 'static>>,

    // A pair of (checker, dropper), checker returns whether the dropper can be removed
    pub(in crate::webview) once_event_droppers: Vec<(Box<dyn FnMut() -> bool + 'static>, Box<dyn FnOnce() + 'static>)>
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

            for pt in self.scheme_handlers.into_values() {
                drop(Box::from_raw(pt));
            }

            for dropper in self.dyn_event_droppers.into_values() {
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
        self.app.post(move |_| {
            if let Some(cc) = wk.upgrade() {
                cc.try_collect();
            }
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
                scheme_handlers: HashMap::new(),
                _owns: PhantomData,
                _counter: collector.count(),
                dyn_event_droppers: HashMap::new(),
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

    fn replace_message_handler(&mut self, wk: WebviewRef, fun: impl FnMut(Webview, &str) + 'static) {
        if !self.app.is_thread_safe() {
            panic!("Message handlers cannot be altered outside the event thread.");
        }

        self.remove_message_handler();

        let bb = Box::new(fun) as Box<dyn FnMut(Webview, &str)>;
        let rc = Rc::new(RefCell::new(bb));
        let pair = (wk, rc);
        let ptr = Box::into_raw(Box::new(pair));
        unsafe { saucer_webview_on_message_with_arg(self.as_ptr(), Some(on_message_trampoline), ptr as *mut c_void) }
        self.ptr.as_mut().unwrap().message_handler = Some(ptr);
    }

    fn remove_scheme_handler(&mut self, name: impl AsRef<str>) {
        if !self.app.is_thread_safe() {
            return;
        }

        rtoc!(name => n; saucer_webview_remove_scheme(self.as_ptr(), n.as_ptr()));

        if let Some(ptr) = self.ptr.as_mut().unwrap().scheme_handlers.remove(name.as_ref()) {
            unsafe { drop(Box::from_raw(ptr)) }
        }
    }

    fn replace_scheme_handler(
        &mut self,
        wk: WebviewRef,
        name: impl AsRef<str>,
        handler: impl FnMut(Webview, Request, Executor) + 'static,
        is_async: bool
    ) {
        if !self.app.is_thread_safe() {
            panic!("Scheme handlers cannot be altered outside the event thread.");
        }

        self.remove_scheme_handler(name.as_ref());

        let arc = Arc::new(Mutex::new(
            Box::new(handler) as Box<dyn FnMut(Webview, Request, Executor)>
        ));
        let pair = (wk, arc);
        let ptr = Box::into_raw(Box::new(pair));
        let lp = if is_async {
            SAUCER_LAUNCH_SAUCER_LAUNCH_ASYNC
        } else {
            SAUCER_LAUNCH_SAUCER_LAUNCH_SYNC
        };

        let name_s = name.as_ref().to_owned();

        rtoc!(name => n ; saucer_webview_handle_scheme_with_arg(self.as_ptr(), n.as_ptr(), Some(scheme_trampoline), ptr as *mut c_void, lp ));

        self.ptr.as_mut().unwrap().scheme_handlers.insert(name_s, ptr);
    }
}

extern "C" fn on_message_trampoline(msg: *const c_char, raw: *mut c_void) -> bool {
    let bb = unsafe { Box::from_raw(raw as *mut (WebviewRef, Rc<RefCell<Box<dyn FnMut(Webview, &str)>>>)) };
    let rc = (*bb).1.clone();
    if let Some(w) = bb.0.upgrade() {
        rc.borrow_mut()(w, &shot_str(msg).unwrap());
    }
    let _ = Box::into_raw(bb); // Avoid dropping the handler

    // The C bindings is loaded as a module.
    // For other modules, returning false here allows the message to be passed on to the next module.
    // We're on the main program and there isn't a module chained after this for handling, so simply return true here.
    true
}

extern "C" fn scheme_trampoline(
    _: *mut saucer_handle,
    req: *mut saucer_scheme_request,
    exec: *mut saucer_scheme_executor,
    raw: *mut c_void
) {
    let bb = unsafe { Box::from_raw(raw as *mut (WebviewRef, Arc<Mutex<Box<dyn FnMut(Webview, Request, Executor)>>>)) };
    let arc = (*bb).1.clone();
    if let Some(w) = bb.0.upgrade() {
        // Request data is copied, so it can be passed by value
        let req = Request::from_ptr(req);
        let exec = Executor::from_ptr(exec);
        arc.lock().unwrap()(w, req, exec);
    }
    let _ = Box::into_raw(bb);
}

/// Describes the window edge to be resized.
pub enum WindowEdge {
    Top,
    Bottom,
    Left,
    Right
}

impl From<WindowEdge> for SAUCER_WINDOW_EDGE {
    fn from(e: WindowEdge) -> Self {
        match e {
            WindowEdge::Top => SAUCER_WINDOW_EDGE_SAUCER_WINDOW_EDGE_TOP,
            WindowEdge::Bottom => SAUCER_WINDOW_EDGE_SAUCER_WINDOW_EDGE_BOTTOM,
            WindowEdge::Left => SAUCER_WINDOW_EDGE_SAUCER_WINDOW_EDGE_LEFT,
            WindowEdge::Right => SAUCER_WINDOW_EDGE_SAUCER_WINDOW_EDGE_RIGHT
        }
    }
}

/// The webview handle.
///
/// A webview handle manages the window and (possibly) the browser process behind it. Like [`App`], the handle is
/// designed to be sharable among threads, but certain features are restricted to the event thread, see method docs for
/// details.
///
/// Like [`App`], webview handles are clonable, but cloning a handle does not clone the underlying webview window. A new
/// webview must be created using the [`Webview::new`] constructor. Similarly, dropping a webview handle does not destroy
/// the window or its web contents, unless it's the last handle present in the process.
///
/// Note that the webview window is destroyed when the last handle referring to it is dropped:
///
/// ```no_run
/// use saucers::app::App;
/// use saucers::prefs::Preferences;
/// use saucers::webview::Webview;
///
/// fn early_destroyed(app: App) {
///     let w = Webview::new(&Preferences::new(&app));
///     // Oh, no! The webview is dropped here and the window is destroyed!
///     // drop(w);
/// }
/// ```
///
/// It's up to the user to keep at least one handle during the expected lifetime of the webview.
///
/// Like [`App`], capturing a webview handle in various handlers can lead to circular references easily and will block
/// the underlying resources from being freed. It's advised to use [`Weak`] to prevent directly capturing a handle.
///
/// # A [`Webview`] Is an [`App`]
///
/// A webview handle internally holds an [`App`] handle and should be seen as an equivalent to the [`App`] handle,
/// meaning that whenever considering the usage of [`App`] handles, all [`Webview`] handles must also be taken into
/// account. For example:
///
/// ```should_panic
/// use saucers::app::App;
/// use saucers::options::AppOptions;
/// use saucers::prefs::Preferences;
/// use saucers::webview::Webview;
///
/// let (cc, app) = App::new(AppOptions::new("app_id"));
/// let w = Webview::new(&Preferences::new(&app));
///
/// // Wait, this drops the handle bound to `app`, but not the one stored in the webview!
/// drop(app);
///
/// // Oh, no! The app is not fully dropped yet as the webview is still holding it!
/// // The call below will panic:
/// drop(cc);
/// ```
///
/// This is especially crucial for calling [`crate::collector::Collector::collect_now`], as a deadlock can be easily
/// formed if extra care is not taken:
///
/// ```no_run
/// use saucers::app::App;
/// use saucers::options::AppOptions;
/// use saucers::prefs::Preferences;
/// use saucers::webview::Webview;
///
/// let (cc, app) = App::new(AppOptions::new("app_id"));
/// let w = Webview::new(&Preferences::new(&app));
///
/// // Like above, there is still another app handle in the webview
/// drop(app);
///
/// // Oh, no! The collector waits for the webview to get dropped, which happens after this!
/// // The following call will form a deadlock and never return:
/// cc.collect_now();
///
/// // Only at here can the last handle of the app get dropped, but it can't be reached
/// // drop(w);
/// ```
#[derive(Clone)]
pub struct Webview(pub(crate) Arc<RwLock<UnsafeWebview>>);

impl Webview {
    /// Creates a new webview window using the given [`Preferences`].
    ///
    /// This method must be called on the event thread of the [`App`] referenced by the [`Preferences`], or it is a
    /// no-op and [`None`] is returned.
    ///
    /// The newly created webview handle stores a clone of the app handle internally. This can cause issues with
    /// dropping. See [`Webview`] for details.
    pub fn new(pref: &Preferences) -> Option<Self> { Some(Webview(Arc::new(RwLock::new(UnsafeWebview::new(pref)?)))) }

    /// Sets a handler for messages from the webview context. Only one handler can be set at a time. Setting a new one
    /// will replace the previous one. This method must be called on the event thread.
    ///
    /// The provided closure is dropped when being replaced, or removed via [`Self::off_message`]. If it's the last
    /// active message handler when the webview is destroyed, it's dropped at least not later than the
    /// [`crate::collector::Collector`] referenced by the app of this webview.
    ///
    /// # Keep Message Content Unique
    ///
    /// Saucer internally uses the same message channel to send and receive certain internal events, e.g. `dom_loaded`
    /// for triggering the [`crate::webview::events::DomReadyEvent`]. Allowing arbitrary messages to be sent over this
    /// channel may accidentally trigger these handlers. Consider prefixing messages to avoid such conflicts.
    ///
    /// # Don't Capture Handles
    ///
    /// Like [`App::post`], capturing any handles in the message handler may result in circular references and prevent
    /// them from being dropped correctly. Either use the passed argument directly without capturing, or consider
    /// wrapping them with [`Weak`] if other handles are needed. Alternatively, use [`Self::off_message`] to remove the
    /// handler manually.
    ///
    /// # Panics
    ///
    /// Panics if not called on the event thread.
    pub fn on_message(&self, fun: impl FnMut(Webview, &str) + 'static) {
        self.0.write().unwrap().replace_message_handler(self.downgrade(), fun);
    }

    /// Removes the previously set message handler, if any.
    ///
    /// This method must be called on the event thread, or it does nothing.
    pub fn off_message(&self) { self.0.write().unwrap().remove_message_handler(); }

    /// Gets the current title of the HTML page in the webview window. Not to be confused with the window title.
    pub fn page_title(&self) -> String { take_str(unsafe { saucer_webview_page_title(self.as_ptr()) }).unwrap() }

    /// Checks whether DevTools is opened.
    pub fn dev_tools(&self) -> bool { unsafe { saucer_webview_dev_tools(self.as_ptr()) } }

    /// Gets the URL of the current page.
    pub fn url(&self) -> String { take_str(unsafe { saucer_webview_url(self.as_ptr()) }).unwrap() }

    /// Gets whether context menu is now enabled.
    pub fn context_menu(&self) -> bool { unsafe { saucer_webview_context_menu(self.as_ptr()) } }

    /// Gets the background color.
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

    /// Checks whether dark mode is being enforced now.
    pub fn force_dark_mode(&self) -> bool { unsafe { saucer_webview_force_dark_mode(self.as_ptr()) } }

    /// Sets whether the DevTools is opened.
    pub fn set_dev_tools(&self, enabled: bool) { unsafe { saucer_webview_set_dev_tools(self.as_ptr(), enabled) } }

    /// Sets whether the context menu is enabled.
    pub fn set_context_menu(&self, enabled: bool) { unsafe { saucer_webview_set_context_menu(self.as_ptr(), enabled) } }

    /// Sets whether to enforce dark mode to be enabled.
    pub fn set_force_dark_mode(&self, enabled: bool) {
        unsafe { saucer_webview_set_force_dark_mode(self.as_ptr(), enabled) }
    }

    /// Sets the background color.
    pub fn set_background(&self, r: u8, g: u8, b: u8, a: u8) {
        unsafe { saucer_webview_set_background(self.as_ptr(), r, g, b, a) }
    }

    /// Converts the given path to a `file://` URL, then navigates to it.
    pub fn set_file(&self, file: impl AsRef<str>) {
        rtoc!(file => s ; saucer_webview_set_file(self.as_ptr(), s.as_ptr()));
    }

    /// Navigates to the given URL.
    ///
    /// Avoid navigating to a URL without DOM content (e.g. `about:blank`) as some saucer internal scripts rely on the
    /// presence of the DOM to function. For an empty page, consider using an empty data URL like `data:text/html,`.
    pub fn set_url(&self, url: impl AsRef<str>) { rtoc!(url => s; saucer_webview_set_url(self.as_ptr(), s.as_ptr())) }

    /// Navigates back.
    pub fn back(&self) { unsafe { saucer_webview_back(self.as_ptr()) } }

    /// Navigates forward.
    pub fn forward(&self) { unsafe { saucer_webview_forward(self.as_ptr()) } }

    /// Reloads the current page.
    pub fn reload(&self) { unsafe { saucer_webview_reload(self.as_ptr()) } }

    /// A quick way of making an [`EmbedFile`] accessible in the web content.
    ///
    /// This method updates an internal scheme handler to return the content of `file` when a URL associated with `name`
    /// is being requested. For now, the URL is `saucer://embedded/{name}`, but this pattern is not documented and
    /// should not be relied on.
    ///
    /// This method is designed for embedding static content. Once a file is embedded, subsequent calls to this method
    /// with the same name will not change the content. To embed dynamic content, use [`Self::handle_scheme`] instead.
    ///
    /// The `is_async` flag sets the launch policy of the internal scheme handler. The launch policy is fixed and can't
    /// be changed after the first call to [`Self::embed_file`], until [`Self::clear_embedded`] is called.
    pub fn embed_file(&self, name: impl AsRef<str>, file: &EmbedFile, is_async: bool) {
        let launch = if is_async {
            SAUCER_LAUNCH_SAUCER_LAUNCH_ASYNC
        } else {
            SAUCER_LAUNCH_SAUCER_LAUNCH_SYNC
        };
        rtoc!(
            name => n;
            // The embedded file and its stash are copied, so both handles are free to be dropped, as long as the data
            // lives for static lifetime.
            saucer_webview_embed_file(self.as_ptr(), n.as_ptr(), file.as_ptr(), launch)
        );
    }

    /// Navigates to the URL of a previously embedded file named `name`.
    ///
    /// This method sets the URL to `saucer://embedded/{name}` for now, but the exact pattern of the URL is subject to
    /// change. See[`Self::embed_file`] for details.
    pub fn serve(&self, name: impl AsRef<str>) { rtoc!(name => s; saucer_webview_serve(self.as_ptr(), s.as_ptr())) }

    /// Removes all injected scripts except those marked as permanent.
    pub fn clear_scripts(&self) { unsafe { saucer_webview_clear_scripts(self.as_ptr()) } }

    /// Removes all embedded files and unregisters the internal scheme handler.
    ///
    /// In particular, this method is not functionally equivalent to calling [`Self::clear_embedded_file`] for each
    /// embedded file. Apart from clearing them, this method also removes the internal scheme handler, making the launch
    /// policy (specified by `is_async`) of the next [`Self::embed_file`] to have effect.
    pub fn clear_embedded(&self) { unsafe { saucer_webview_clear_embedded(self.as_ptr()) } }

    /// Removes a previously embedded file named `name`.
    pub fn clear_embedded_file(&self, name: impl AsRef<str>) {
        rtoc!(name => s; saucer_webview_clear_embedded_file(self.as_ptr(), s.as_ptr()))
    }

    /// Schedules a script to be executed when a document is loaded. The load time and injected frames are controlled by
    /// the script object. The script is executed in the main script world.
    ///
    /// Injecting arbitrary script can cause **SEVERE SECURITY RISK**! Make sure to read the docs of [`Script`] to
    /// understand the features and limitations of injected scripts before start using this method.
    ///
    /// Once injected, a script will stay attached to the webview until it's cleared via [`Self::clear_scripts`]. If
    /// a script is defined as permanent, then there is no way to remove it.
    ///
    /// The script code is copied before attached for each invocation, making transferring large payload inefficient.
    /// Consider other methods for such use cases.
    pub fn inject(&self, script: &Script) { unsafe { saucer_webview_inject(self.as_ptr(), script.as_ptr()) } }

    /// Executes the given code in the main script world. Returns immediately without waiting.
    ///
    /// This method delays the script to be executed when DOM is loaded if posted earlier that this happens.
    ///
    /// This method executes the code without any sanitizing, making it have the same security concerns as
    /// [`Self::inject`] and [`Script`]. In short, executing arbitrary code is of **SEVERE RISK**! See the docs above
    /// for details.
    pub fn execute(&self, code: impl AsRef<str>) { rtoc!(code => s; saucer_webview_execute(self.as_ptr(), s.as_ptr())) }

    /// Sets a scheme handler for the scheme named `name`. The scheme handler will be executed entirely on the event
    /// thread. As this method can only be called on the event thread too, it eliminated the [`Send`] bound of the
    /// handler in [`Self::handle_scheme_async`].
    ///
    /// A scheme must first be registered using [`crate::scheme::register_scheme`] before it can be handled. Only one
    /// scheme handler can be set for a given scheme name. Setting a new handler will replace the previous one.
    ///
    /// Like [`Self::on_message`], the provided closure is dropped when being replaced, or removed via
    /// [`Self::remove_scheme`]. If it's the last active handler of a scheme when the webview is destroyed, it's dropped
    /// at least not later than the [`crate::collector::Collector`] referenced by the app of this webview.
    ///
    /// # Don't Capture Handles
    ///
    /// Like [`Self::on_message`], capturing handles in handlers added by this method may interfere the correct
    /// dropping behavior and should be avoided. See the docs there for details.
    ///
    /// # Panics
    ///
    /// Panics if not called on the event thread.
    pub fn handle_scheme(&self, name: impl AsRef<str>, fun: impl FnMut(Webview, Request, Executor) + 'static) {
        self.0
            .write()
            .unwrap()
            .replace_scheme_handler(self.downgrade(), name, fun, false);
    }

    /// Like [`Self::handle_scheme`], but executes the scheme handler in the background thread pool.
    ///
    /// Limitations and caveats of [`Self::handle_scheme`] also apply to this method. Make sure to check the docs there.
    ///
    /// # Panics
    ///
    /// Panics if not called on the event thread.
    pub fn handle_scheme_async(
        &self,
        name: impl AsRef<str>,
        fun: impl FnMut(Webview, Request, Executor) + Send + 'static
    ) {
        self.0
            .write()
            .unwrap()
            .replace_scheme_handler(self.downgrade(), name, fun, true);
    }

    /// Removes the current handler of the scheme named `name`, if any.
    ///
    /// Removing a handler does not unregister the scheme. Another handler can be set for the scheme later and  function
    /// properly.
    ///
    /// This method must be called on the event thread, or it does nothing.
    pub fn remove_scheme(&self, name: impl AsRef<str>) { self.0.write().unwrap().remove_scheme_handler(name) }

    /// Checks whether the window is visible. Not to be confused with [`Self::minimized`].
    pub fn visible(&self) -> bool { unsafe { saucer_window_visible(self.as_ptr()) } }

    /// Checks whether the window is focused.
    pub fn focused(&self) -> bool { unsafe { saucer_window_focused(self.as_ptr()) } }

    /// Checks whether the window is minimized.
    pub fn minimized(&self) -> bool { unsafe { saucer_window_minimized(self.as_ptr()) } }

    /// Checks whether the window is maximized.
    pub fn maximized(&self) -> bool { unsafe { saucer_window_maximized(self.as_ptr()) } }

    /// Checks whether the window is resizable.
    pub fn resizable(&self) -> bool { unsafe { saucer_window_resizable(self.as_ptr()) } }

    /// Checks whether the window has decorations (i.e. not frameless).
    pub fn decorations(&self) -> bool { unsafe { saucer_window_decorations(self.as_ptr()) } }

    /// Checks whether the window is set to be always on the top.
    pub fn always_on_top(&self) -> bool { unsafe { saucer_window_always_on_top(self.as_ptr()) } }

    /// Checks whether the window can be clicked through.
    pub fn click_through(&self) -> bool { unsafe { saucer_window_click_through(self.as_ptr()) } }

    /// Gets the title of the window.
    pub fn title(&self) -> String { take_str(unsafe { saucer_window_title(self.as_ptr()) }).unwrap() }

    /// Gets the size of the window.
    pub fn size(&self) -> (i32, i32) {
        let mut w = 0;
        let mut h = 0;
        unsafe {
            saucer_window_size(self.as_ptr(), &mut w as *mut i32, &mut h as *mut i32);
        }
        (w, h)
    }

    /// Gets the maximum size of the window.
    pub fn max_size(&self) -> (i32, i32) {
        let mut w = 0;
        let mut h = 0;
        unsafe {
            saucer_window_max_size(self.as_ptr(), &mut w as *mut i32, &mut h as *mut i32);
        }
        (w, h)
    }

    /// Gets the minimum size of the window.
    pub fn min_size(&self) -> (i32, i32) {
        let mut w = 0;
        let mut h = 0;
        unsafe {
            saucer_window_min_size(self.as_ptr(), &mut w as *mut i32, &mut h as *mut i32);
        }
        (w, h)
    }

    /// Hides the window.
    pub fn hide(&self) { unsafe { saucer_window_hide(self.as_ptr()) } }

    /// Show the window.
    pub fn show(&self) { unsafe { saucer_window_show(self.as_ptr()) } }

    /// Closes the window. Not to be confused with [`Self::hide`]. Once a window is closed, it's essentially destroyed
    /// and cannot be reopened.
    ///
    /// When the last webview window of an [`App`] is closed (even not dropped), either by this method or by user
    /// actions, an implicit quit message is dispatched on the event thread as if [`App::quit`] were called, making the
    /// event loop terminate. To prevent such behavior, either create a hidden empty webview window, or replace the
    /// closing behavior with hiding. See [`crate::webview::events::CloseEvent`] for details.
    pub fn close(&self) { unsafe { saucer_window_close(self.as_ptr()) } }

    /// Focuses the window.
    pub fn focus(&self) { unsafe { saucer_window_focus(self.as_ptr()) } }

    /// Starts dragging the window at the current cursor position.
    ///
    /// A window can always be dragged using native control widgets. This method is primarily intended to be used to
    /// implement dragging using HTML elements.
    pub fn start_drag(&self) { unsafe { saucer_window_start_drag(self.as_ptr()) } }

    /// Starts resizing the given edge of the window at the current cursor position.
    ///
    /// A window can be resized as long as it's set to be resizable. This method is primarily intended to be used to
    /// implement resizing using HTML elements.
    pub fn start_resize(&self, edge: WindowEdge) { unsafe { saucer_window_start_resize(self.as_ptr(), edge.into()) } }

    /// Sets whether the window is minimized.
    pub fn set_minimized(&self, b: bool) { unsafe { saucer_window_set_minimized(self.as_ptr(), b) } }

    /// Sets whether the window is maximized.
    pub fn set_maximized(&self, b: bool) { unsafe { saucer_window_set_maximized(self.as_ptr(), b) } }

    /// Sets whether the window is resizable.
    pub fn set_resizable(&self, b: bool) { unsafe { saucer_window_set_resizable(self.as_ptr(), b) } }

    /// Sets whether the window has decorations (frame).
    pub fn set_decorations(&self, b: bool) { unsafe { saucer_window_set_decorations(self.as_ptr(), b) } }

    /// Sets whether the window is always on the top.
    pub fn set_always_on_top(&self, b: bool) { unsafe { saucer_window_set_always_on_top(self.as_ptr(), b) } }

    /// Sets whether the window can be clicked through.
    pub fn set_click_through(&self, b: bool) { unsafe { saucer_window_set_click_through(self.as_ptr(), b) } }

    /// Sets the window icon.
    pub fn set_icon(&self, icon: &Icon) { unsafe { saucer_window_set_icon(self.as_ptr(), icon.as_ptr()) } }

    /// Sets the window title.
    pub fn set_title(&self, title: impl AsRef<str>) {
        rtoc!(title => s; saucer_window_set_title(self.as_ptr(), s.as_ptr()))
    }

    /// Sets the window size.
    pub fn set_size(&self, w: i32, h: i32) { unsafe { saucer_window_set_size(self.as_ptr(), w, h) } }

    /// Sets the maximum size of the window.
    pub fn set_max_size(&self, w: i32, h: i32) { unsafe { saucer_window_set_max_size(self.as_ptr(), w, h) } }

    /// Sets the minimum size of the window.
    pub fn set_min_size(&self, w: i32, h: i32) { unsafe { saucer_window_set_min_size(self.as_ptr(), w, h) } }

    /// Like [`App::post`], but adds the webview handle as the second argument to eliminate the need of manual
    /// capturing.
    ///
    /// Limitations and caveats of [`App::post`] also apply to this method. See the docs there for details.
    pub fn post(&self, fun: impl FnOnce(App, Webview) + Send + 'static) {
        let wk = self.downgrade();
        self.0.read().unwrap().app.post(move |a| {
            if let Some(w) = wk.upgrade() {
                fun(a, w)
            }
        });
    }

    /// Clones a handle of the [`App`] referenced by this webview window.
    pub fn app(&self) -> App { self.0.read().unwrap().app.clone() }

    pub(crate) fn as_ptr(&self) -> *mut saucer_handle { self.0.read().unwrap().as_ptr() }

    pub(crate) fn is_event_thread(&self) -> bool { self.0.read().unwrap().app.is_thread_safe() }

    pub(crate) fn downgrade(&self) -> WebviewRef { WebviewRef(Arc::downgrade(&self.0)) }
}

pub(crate) struct WebviewRef(pub(crate) Weak<RwLock<UnsafeWebview>>);

impl WebviewRef {
    pub(crate) fn upgrade(&self) -> Option<Webview> { Some(Webview(self.0.upgrade()?)) }
}
