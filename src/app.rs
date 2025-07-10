use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::sync::mpmc::Sender;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::Weak;
use std::thread::ThreadId;

use crate::capi::*;
use crate::collector::Collect;
use crate::collector::Collector;
use crate::collector::UnsafeCollector;
use crate::options::AppOptions;

struct ReplaceableAppPtr(RwLock<Option<NonNull<saucer_application>>>);

unsafe impl Send for ReplaceableAppPtr {}
unsafe impl Sync for ReplaceableAppPtr {}

struct AppPtr {
    ptr: Arc<ReplaceableAppPtr>,
    posted_closures: RwLock<Vec<*mut Option<Box<dyn FnOnce()>>>>,
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
            let posted = self.posted_closures.write().unwrap();

            for cls in &*posted {
                drop(Box::from_raw(*cls));
            }

            drop(posted);

            let mut guard = self.ptr.0.write().unwrap();
            if let Some(ref ptr) = guard.take() {
                saucer_application_free(ptr.as_ptr())
            }
        }
    }
}

struct UnsafeApp {
    ptr: Option<AppPtr>,
    collector: Option<Weak<UnsafeCollector>>,
    collector_tx: Sender<Box<dyn Collect>>,
    host_thread: ThreadId,
    _opt: AppOptions
}

impl Drop for UnsafeApp {
    fn drop(&mut self) {
        let bb = Box::new(self.ptr.take().unwrap());

        if self.is_host_thread() {
            bb.collect();
            return;
        }

        let ptr = bb.ptr.clone();
        self.collector_tx.send(bb).unwrap();
        let wk = self.collector.take().unwrap();

        // It's possible that the collector free the pointer before posting, so a read lock is required.
        // The dropping process acquires a write lock, so if the `ptr` is `Some`, it must be valid.
        let guard = ptr.0.read().unwrap();
        if let Some(ref ptr) = *guard {
            Self::post_raw(ptr.as_ptr(), move || {
                // On some platform (e.g. GTK), the posted closure may still be executed even when the event loop has
                // already stopped, and the app has been dropped (so does the collector). At the time the closure is
                // being called, it's possible that the collector is no longer available.
                if let Some(cc) = wk.upgrade() {
                    cc.try_collect();
                }
            });
        }
    }
}

impl UnsafeApp {
    fn new(collector: Arc<UnsafeCollector>, mut opt: AppOptions) -> Self {
        let ptr = unsafe { saucer_application_init(opt.as_ptr()) };
        let ptr = AppPtr {
            ptr: Arc::new(ReplaceableAppPtr(RwLock::new(Some(
                NonNull::new(ptr).expect("Failed to create app")
            )))),
            posted_closures: RwLock::new(Vec::new()),
            _owns: PhantomData,
            _counter: collector.count()
        };

        Self {
            ptr: Some(ptr),
            collector: Some(Arc::downgrade(&collector)),
            collector_tx: collector.get_sender(),
            host_thread: std::thread::current().id(),
            _opt: opt
        }
    }

    fn is_host_thread(&self) -> bool { std::thread::current().id() == self.host_thread }

    fn as_ptr(&self) -> *mut saucer_application { self.ptr.as_ref().unwrap().as_ptr() }

    fn post_raw(ptr: *mut saucer_application, fun: impl FnOnce() + Send + 'static) {
        let bb = Box::new(fun) as Box<dyn FnOnce()>;
        let cpt = Box::into_raw(Box::new(bb)) as *mut c_void;
        unsafe { saucer_application_post_with_arg(ptr, Some(post_trampoline), cpt) }
    }

    fn post(&self, ar: AppRef, fun: impl FnOnce(App) + Send + 'static) {
        let fna = move || {
            if let Some(a) = ar.upgrade() {
                fun(a);
            }
        };

        let bb = Box::new(fna) as Box<dyn FnOnce()>;
        let cpt = Box::into_raw(Box::new(Some(bb)));

        self.save_closure_for_cleanup(cpt);

        unsafe { saucer_application_post_with_arg(self.as_ptr(), Some(post_trampoline), cpt as *mut c_void) }
    }

    fn save_closure_for_cleanup(&self, cls: *mut Option<Box<dyn FnOnce()>>) {
        let mut guard = self.ptr.as_ref().unwrap().posted_closures.write().unwrap();
        guard.push(cls);

        guard.retain(|ptr| unsafe {
            let b = Box::from_raw(*ptr);

            if b.is_some() {
                let _ = Box::into_raw(b);
                true
            } else {
                false
            }
        });
    }
}

/// The application handle.
///
/// An app handle manages event loops and other supportive structures (like thread pools). The handle is designed to be
/// sharable among threads, but certain features are restricted to the event thread, see method docs for details.
///
/// App handles are clonable, but cloning a handle does not clone the underlying event loop. A new app must be created
/// using the [`App::new`] constructor. Similarly, dropping an app handle does not destroy the app, unless it's the
/// last handle present in the process.
///
/// Capturing an app handle in various handlers can lead to circular references easily and will block the underlying
/// resources from being freed. It's advised to use [`Weak`] to prevent directly capturing a handle.
#[derive(Clone)]
pub struct App(Arc<UnsafeApp>);

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
    pub fn new(collector: &Collector, opt: AppOptions) -> Self {
        Self(Arc::new(UnsafeApp::new(collector.get_inner(), opt)))
    }

    /// Schedules the closure to be called on the event thread. The closure is scheduled to be processed after all other
    /// event messages (e.g. window events) that has been scheduled before calling this method.
    ///
    /// This method is useful for executing certain operations that must happen on the event thread. Saucer internally
    /// uses posting to make many methods usable outside the event thread. However, this method makes no guarantee on
    /// when the closure will be executed, or even whether it will end up being executed. Critical operations may not
    /// rely on the assumption of the execution time of the posted closure.
    ///
    /// # Don't Capture Handles
    ///
    /// The closure and the [`App`] handle passed to it will be dropped once it finishes execution. If the closure has
    /// not been executed when the app is dropped, it's dropped at least not later than the [`Collector`] of the app is
    /// dropped. However, capturing any handles in the closure, including [`App`] and other handles that rely on it, may
    /// create circular references and block such drop from happening, thus it's **highly discouraged** to capture any
    /// handles in the provided closure without using [`Weak`].
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
    /// use saucers::collector::Collector;
    /// use saucers::options::AppOptions;
    /// let cc = Collector::new();
    /// let app = App::new(&cc, AppOptions::new("saucer"));
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
    /// # Panics
    ///
    /// Panics if not called on the event thread.
    pub fn run(&self) {
        if !self.is_thread_safe() {
            panic!("Event loop must only be executed on the even thread.");
        }
        unsafe { saucer_application_run(self.as_ptr()) }
    }

    /// Polls and processes at most one event in the event queue. Makes no attempt to wait for one when the queue is
    /// empty.
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
    /// Like [`Self::post`], this method makes no guarantee on when the closure will be called, or whether it will end
    /// up being executed. However, unlike [`Self::post`], the closure is **NOT** guaranteed to be dropped when it fails
    /// to be executed (e.g. the thread pool is full before dropping the app), means that content captured by the
    /// closure might be leaked forever. Given the description above, it's **absolutely discouraged** to capture any
    /// handle inside the closure without [`Weak`], unless for very specific valid use case.
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
    /// Beyond that, as it's impossible to control when the thread is started or terminated, it's almost **NEVER** a
    /// good idea to capture any handles in it, **even using [`Weak`]**.
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

    pub(crate) fn as_ptr(&self) -> *mut saucer_application { self.0.as_ptr() }

    pub(crate) fn get_collector(&self) -> Weak<UnsafeCollector> { self.0.collector.as_ref().unwrap().clone() }

    fn downgrade(&self) -> AppRef { AppRef(Arc::downgrade(&self.0)) }
}

struct AppRef(Weak<UnsafeApp>);

impl AppRef {
    fn upgrade(&self) -> Option<App> { Some(App(self.0.upgrade()?)) }
}

extern "C" fn post_trampoline(raw: *mut c_void) {
    unsafe {
        let mut bb = Box::from_raw(raw as *mut Option<Box<dyn FnOnce()>>);
        if let Some(f) = bb.take() {
            f();
        }
        let _ = Box::into_raw(bb);
    }
}

extern "C" fn submit_trampoline(raw: *mut c_void) {
    unsafe {
        let bb = Box::from_raw(raw as *mut Box<dyn FnOnce()>);
        bb();
    }
}
