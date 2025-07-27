//! Application event cycle module.
//!
//! See [`App`] for details.

use std::ffi::c_void;
use std::marker::PhantomData;
use std::mem::take;
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::Weak;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::thread::ThreadId;

use crate::capi::*;
use crate::collector::Collect;
use crate::collector::Collector;
use crate::collector::UnsafeCollector;
use crate::options::AppOptions;
use crate::webview::Webview;

struct ReplaceableAppPtr(RwLock<Option<NonNull<saucer_application>>>);

unsafe impl Send for ReplaceableAppPtr {}
unsafe impl Sync for ReplaceableAppPtr {}

struct AppPtr {
    ptr: Arc<ReplaceableAppPtr>,
    _owns: PhantomData<saucer_application>,
    _counter: Arc<()>
}

unsafe impl Send for AppPtr {}
unsafe impl Sync for AppPtr {}

impl AppPtr {
    fn as_ptr(&self) -> *mut saucer_application { self.ptr.0.read().unwrap().unwrap().as_ptr() }
}

impl Collect for AppPtr {
    fn collect(self: Box<Self>) {
        unsafe {
            let mut guard = self.ptr.0.write().unwrap();
            if let Some(ref ptr) = guard.take() {
                // Some posted function may be run during the destruction process.
                saucer_application_free(ptr.as_ptr())
            }
        }
    }
}

pub(crate) struct UnsafeApp {
    ptr: Option<AppPtr>,
    collector: Option<Weak<UnsafeCollector>>,
    collector_tx: Sender<Box<dyn Collect>>,
    stopped: AtomicBool,
    host_thread: ThreadId,
    webviews: Mutex<Vec<Webview>>,
    _opt: AppOptions
}

impl Drop for UnsafeApp {
    fn drop(&mut self) {
        let bb = Box::new(self.ptr.take().unwrap());

        if self.is_host_thread() {
            bb.collect();
            return;
        }

        self.collector_tx.send(bb).unwrap();

        // When dropping an app, it's pointless to notify the collector as the event loop has terminated. The handle
        // will be freed when the collector is dropped, which should happen shortly after the termination of the event
        // loop for a regular application.
    }
}

impl UnsafeApp {
    fn new(collector: Arc<UnsafeCollector>, mut opt: AppOptions) -> Self {
        let ptr = unsafe { saucer_application_init(opt.as_ptr()) };
        let ptr = AppPtr {
            ptr: Arc::new(ReplaceableAppPtr(RwLock::new(Some(
                NonNull::new(ptr).expect("Failed to create app")
            )))),
            _owns: PhantomData,
            _counter: collector.count()
        };

        Self {
            ptr: Some(ptr),
            collector: Some(Arc::downgrade(&collector)),
            collector_tx: collector.get_sender(),
            stopped: AtomicBool::new(false),
            host_thread: std::thread::current().id(),
            webviews: Mutex::default(),
            _opt: opt
        }
    }

    fn is_host_thread(&self) -> bool { std::thread::current().id() == self.host_thread }

    fn as_ptr(&self) -> *mut saucer_application { self.ptr.as_ref().unwrap().as_ptr() }

    fn post(&self, ar: AppRef, fun: impl FnOnce(App) + Send + 'static) {
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }

        let fna = move || {
            if let Some(a) = ar.upgrade() {
                fun(a);
            }
        };

        let bb = Box::new(fna) as Box<dyn FnOnce()>;
        let cpt = Box::into_raw(Box::new(bb));

        unsafe { saucer_application_post_with_arg(self.as_ptr(), Some(post_trampoline), cpt as *mut c_void) }
    }
}

/// The application handle.
///
/// An app handle manages event loops and other supportive structures (like thread pools). The handle is designed to be
/// sharable among threads, but certain features are restricted to the event thread, see method docs for details.
///
/// App handles are clonable, but cloning a handle does not clone the underlying event loop. A new app must be created
/// using the [`App::create`] or [`App::new`] constructor. Similarly, dropping an app handle does not destroy the app,
/// unless it's the last handle present in the process.
///
/// Capturing an app handle in various handlers can lead to circular references easily and will block the underlying
/// resources from being freed. Prefer using [`AppRef`] than directly capturing a handle.
#[derive(Clone)]
pub struct App(pub(crate) Arc<UnsafeApp>);

impl Drop for App {
    fn drop(&mut self) {
        let mut ws = self.0.webviews.lock().unwrap();
        if Arc::strong_count(&self.0) <= ws.len() + 1 {
            // The app only has interior references, clear the webviews.
            // As dropping the webview handle may result in dropping the app, we first take the value out, drop the
            // mutex guard, then clear each webview.
            let w = take(&mut *ws);
            drop(ws);
            drop(w);
        }
    }
}

impl App {
    /// Creates a new app and returns a handle for it, using the given options.
    ///
    /// The created app will rely on the given collector to clean its resources, thus the collector must live longer
    /// than any app handles cloned from the return value.
    ///
    /// The thread where the app is created (i.e. this method is called) will be used as the **event thread**. Certain
    /// operations are only allowed on the event thread and will panic if not doing so, including [`Self::run`],
    /// [`Self::run_once`].
    ///
    /// # Notes for macOS
    ///
    /// Like most GUI applications, the application event loop can only be run on the first thread of the process, thus
    /// the [`App`] and its [`Collector`] must also be created on this thread.
    pub fn create(collector: &Collector, opt: AppOptions) -> Self {
        Self(Arc::new(UnsafeApp::new(collector.get_inner(), opt)))
    }

    /// Like [`Self::create`], but creates a [`Collector`] internally and returns it with the created app.
    ///
    /// Usages of the returned collector should follow the same rules as one would use a separated collector. See the
    /// docs there for details.
    pub fn new(opt: AppOptions) -> (Collector, Self) {
        let cc = Collector::new();
        let s = Self::create(&cc, opt);
        (cc, s)
    }

    /// Schedules the closure to be called on the event thread if applicable. The closure is scheduled to be processed
    /// after all other event messages (e.g. window events) that has been scheduled before calling this method.
    ///
    /// The app will decide whether a closure can be accepted. It simply ignores the call if it believes that the event
    /// loop does not seem likely to start again, e.g. after a successful return from [`Self::run`]. If a closure is
    /// accepted, the app tries the best effort to run it, but still makes no guarantee on when the closure will be
    /// executed, or even whether it will end up being executed. Critical operations should not rely on the assumption
    /// of the execution time of the posted closure.
    ///
    /// Once a closure is accepted, it will be dropped once it finishes execution. If a closure is accepted but failed
    /// to be scheduled, then its content is leaked. Chances of such cases are reduced to a minimum, but can't be
    /// completely ruled out. Thus, one should not rely on the drop behavior of the posted closures.
    ///
    /// # Don't Capture Handles
    ///
    /// Capturing any handles in the closure, including [`App`] and other handles that rely on it, may create circular
    /// references and block other handles from being dropped correctly. Always prefer using [`AppRef`] to share handles
    /// across closures.
    ///
    /// # Performance Concerns
    ///
    /// **Don't block the event thread.** This is the golden rule for your app to keep responsive. Under no
    /// circumstances should any closure on the event thread, especially posted by this method, block the event thread
    /// for long-running operations.
    ///
    /// # Example
    ///
    /// ```
    /// use saucers::app::App;
    /// use saucers::options::AppOptions;
    /// let (cc, app) = App::new(AppOptions::new("saucer"));
    ///
    /// let _ = std::thread::spawn({
    ///     let app = app.clone();
    ///     move || {
    ///         app.post(|app| {
    ///             // We're now on the event thread!
    ///             assert!(app.is_thread_safe());
    ///         })
    ///     }
    /// })
    /// .join();
    ///
    /// app.run_once();
    /// ```
    pub fn post(&self, fun: impl FnOnce(App) + Send + 'static) { self.0.post(self.downgrade(), fun); }

    /// Checks whether the current thread is the event thread.
    pub fn is_thread_safe(&self) -> bool { self.0.is_host_thread() }

    /// Runs the event loop. Polls and processes events from the event queue and blocks to wait for new ones, until a
    /// quit message is received.
    ///
    /// Once this method returns, further attempts to run the event loop may not behave consistently across platforms,
    /// see also [`Self::quit`].
    ///
    /// The app stops accepting new [`Self::post`]ed closures once this method returns. If the platform supports
    /// restarting the event loop, then new closures can only be accepted again after the next call to this method
    /// happens.
    ///
    /// # Panics
    ///
    /// Panics if not called on the event thread.
    pub fn run(&self) {
        if !self.is_thread_safe() {
            panic!("Event loop must only be executed on the even thread.");
        }

        self.0.stopped.store(false, Ordering::SeqCst);

        unsafe { saucer_application_run(self.as_ptr()) }

        self.0.stopped.store(true, Ordering::SeqCst);

        // Polls possible messages that's added during the quit process (e.g. posted closures).
        // This should clean up all closures posted before the app ends.
        self.run_once();
    }

    /// Polls and processes at most one event in the event queue. Makes no attempt to wait for one when the queue is
    /// empty.
    ///
    /// Unlike [`Self::run`], this method does not affect the status of accepting [`Self::post`]ed closures. Given that
    /// it's non-blocking, this method should not be used as the way to run the app. Rather, it shall be considered as a
    /// one-shot hint to clear the events.
    ///
    /// # Panics
    ///
    /// Panics if not called on the event thread.
    pub fn run_once(&self) {
        if !self.is_thread_safe() {
            panic!("Event loop must only be executed on the even thread.");
        }
        unsafe { saucer_application_run_once(self.as_ptr()) }
    }

    /// Runs a closure in the background thread pool and waits until it finishes.
    ///
    /// Like [`Self::post`], this method makes no guarantee on when the closure will be called. However, unlike
    /// [`Self::post`], this method does not reject the closure based on the status of the thread pool, means that
    /// content captured by the closure might be leaked forever, sometimes even likely. Such possibility are not guarded
    /// or reduced, even in attempts, by this method.
    ///
    /// # Deprecation Notes
    ///
    /// This method is marked as deprecated due to the above limitations. Saucer API also marks it for removal in the
    /// next release. There are few valid use case of this method, comparing to manually create a thread in Rust, which
    /// can be far safer.
    #[deprecated = "Use Rust threads or async runtimes instead."]
    pub fn pool_submit(&self, fun: impl FnOnce() + Send + 'static) {
        let bb = Box::new(fun) as Box<dyn FnOnce()>;
        let ptr = Box::into_raw(Box::new(bb)) as *mut c_void;
        unsafe { saucer_application_pool_submit_with_arg(self.as_ptr(), Some(submit_trampoline), ptr) }
    }

    /// Runs a closure in the background thread pool and returns immediately.
    ///
    /// All caveats of [`Self::pool_submit`], including the drop limitations of the closure, also apply to this method.
    ///
    /// # Deprecation Notes
    ///
    /// Deprecated out of the same reason as [`Self::pool_submit`].
    #[deprecated = "Use Rust threads or async runtimes instead."]
    pub fn pool_emplace(&self, fun: impl FnOnce() + Send + 'static) {
        let bb = Box::new(fun) as Box<dyn FnOnce()>;
        let ptr = Box::into_raw(Box::new(bb)) as *mut c_void;
        unsafe { saucer_application_pool_emplace_with_arg(self.as_ptr(), Some(submit_trampoline), ptr) }
    }

    /// Requests the app to quit. Apart from a panic, this is the only way to interrupt a running event loop started by
    /// [`Self::run`].
    ///
    /// Calling this method multiple times may enqueue multiple quit messages and interrupts the following up calls to
    /// [`Self::run`] and [`Self::run_once`], if any. This is a platform-specific behavior and should not be relied on.
    pub fn quit(&self) {
        if self.is_thread_safe() {
            unsafe { saucer_application_quit(self.as_ptr()) }
        } else {
            self.post(|app| unsafe { saucer_application_quit(app.as_ptr()) });
        }
    }

    pub(crate) fn add_webview(&self, w: Webview) { self.0.webviews.lock().unwrap().push(w); }

    pub(crate) unsafe fn unref_webview(&self, w: &Webview) {
        self.0.webviews.lock().unwrap().retain(|wi| wi.as_ptr() != w.as_ptr());
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_application { self.0.as_ptr() }

    pub(crate) fn get_collector(&self) -> Weak<UnsafeCollector> { self.0.collector.as_ref().unwrap().clone() }

    /// Gets a weak [`AppRef`] of this app.
    pub fn downgrade(&self) -> AppRef { AppRef(Arc::downgrade(&self.0)) }
}

/// A weak handle of [`App`].
///
/// This struct internally holds a weak reference to the app handle and does not prevent its destruction. This weak
/// handle must be [`AppRef::upgrade`]ed manually to access the original handle.
#[derive(Clone)]
pub struct AppRef(Weak<UnsafeApp>);

impl AppRef {
    /// Tries to upgrade and get the original handle.
    ///
    /// Once upgraded, the returned [`App`] is considered an equivalent to those created via the constructors or
    /// [`App::clone`] and must be counted when considering dropping.
    pub fn upgrade(&self) -> Option<App> { Some(App(self.0.upgrade()?)) }
}

extern "C" fn post_trampoline(raw: *mut c_void) {
    unsafe {
        let bb = Box::from_raw(raw as *mut Box<dyn FnOnce()>);
        bb();
    }
}

extern "C" fn submit_trampoline(raw: *mut c_void) {
    unsafe {
        let bb = Box::from_raw(raw as *mut Box<dyn FnOnce()>);
        bb();
    }
}
