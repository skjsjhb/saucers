//! Application event cycle module.
//!
//! See [`App`] and [`AppManager`] for details.

mod events;
mod options;

use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::null_mut;
use std::ptr::NonNull;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;
use std::thread::JoinHandle;
use std::thread::ThreadId;
use std::time::Duration;

pub use events::*;
pub use options::*;
use saucer_sys::*;

use crate::macros::load_range;
use crate::policy::Policy;
use crate::screen::Screen;

/// An unprotected owned app handle.
struct RawApp {
    inner: NonNull<saucer_application>,
    drop_sender: Sender<Box<dyn FnOnce() + Send>>,
    host_tid: ThreadId,
    _marker: PhantomData<saucer_application>,
}

// SAFETY: App handles are thread-safe for dispatching, and dropping is handled by the collector.
// The event listener is only accessed on the event thread.
unsafe impl Send for RawApp {}
unsafe impl Sync for RawApp {}

struct RawAppCleanUp {
    inner: NonNull<saucer_application>,
}

unsafe impl Send for RawAppCleanUp {}

impl Drop for RawApp {
    fn drop(&mut self) {
        let cl = RawAppCleanUp { inner: self.inner };

        let col = move || unsafe {
            let _ = &cl;
            saucer_application_free(cl.inner.as_ptr());
        };

        if self.is_thread_safe() {
            col();
        } else {
            self.drop_sender.send(Box::new(col)).expect("failed to post app destruction");
        }
    }
}

impl RawApp {
    pub(crate) fn new(
        inner: NonNull<saucer_application>,
        drop_sender: Sender<Box<dyn FnOnce() + Send>>,
    ) -> Self {
        Self { inner, drop_sender, host_tid: std::thread::current().id(), _marker: PhantomData }
    }

    fn is_thread_safe(&self) -> bool { self.host_tid == std::thread::current().id() }

    pub(crate) fn as_ptr(&self) -> *mut saucer_application { self.inner.as_ptr() }
}

/// A struct that manages apps and collects all handles.
///
/// This struct never owns an app handle. Instead, it creates one on-demand when starting the event
/// loop, and then forgets it. Handles are collected when this struct is dropped.
pub struct AppManager {
    raw_opt: RawAppOptions,
    drop_sender: Option<Sender<Box<dyn FnOnce() + Send>>>,
    receiver: Receiver<Box<dyn FnOnce() + Send>>,
    _marker: PhantomData<saucer_application>,
}

impl Drop for AppManager {
    fn drop(&mut self) {
        drop(self.drop_sender.take()); // In case it's not consumed

        while let Ok(p) = self.receiver.recv() {
            // SAFETY: This struct is thread-local
            p();
        }
    }
}

impl AppManager {
    /// Constructs an app manager from the given options.
    pub fn new(opt: AppOptions) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Self {
            raw_opt: RawAppOptions::new(opt),
            drop_sender: Some(sender),
            receiver,
            _marker: PhantomData,
        }
    }

    /// Runs the app with specified the event handlers. Invokes the given callback once when
    /// entering the event loop.
    ///
    /// The thread that this method is called will be used as the event thread, that is, the thread
    /// that runs the event loop. Specifically, this method must only be called on the starting
    /// thread on macOS due to limitations of Cocoa.
    ///
    /// A mutable reference to [`FinishListener`] is provided to the callback, which can be used to
    /// set listeners that will be called once the event loop stops.
    ///
    /// In the C++ API, the callback is designed for holding windows and webviews so that they don't
    /// "escape" the app event cycle scope. It leverages async C++ for non-blocking app status
    /// check. As we currently have no plan to support async parts (which will more or less
    /// involve pulling in some crates for async), handles created in the callback will be
    /// dropped when it exits, which will normally destroy the windows. There are currently two
    /// workarounds:
    ///
    /// 1. Store the handle at a place which outlives the event loop lifecycle.
    /// 2. Move the handles into the finish callback and drop them there.
    pub fn run(
        mut self,
        start: impl FnOnce(App, &mut FinishListener) + 'static,
        event_listener: impl AppEventListener,
    ) -> crate::error::Result<()> {
        let mut ex = -1;

        // SAFETY: The options are kept valid until the app quits.
        let ptr = unsafe { saucer_application_new(self.raw_opt.as_ptr(), &raw mut ex) };

        let app = NonNull::new(ptr).ok_or(crate::error::Error::Saucer(ex))?;

        let sender = self.drop_sender.take().unwrap();
        let app = App(Arc::new(RawApp::new(app, sender)));

        // The listener is only dropped after the events are removed
        let evd = EventListenerData::new(&event_listener, app.downgrade());
        let data = Box::into_raw(Box::new(evd));

        unsafe {
            saucer_application_on(
                ptr,
                SAUCER_APPLICATION_EVENT_QUIT,
                ev_on_quit_tp as *mut c_void,
                true,
                data as *mut c_void,
            );
        }

        let cdata = RunCallbackData::new(start, app.clone()).into_raw();
        unsafe {
            saucer_application_run(ptr, Some(run_callback_tp), Some(finish_callback_tp), cdata)
        };

        let _ = unsafe { Box::from_raw(data) };

        drop(app); // Ensure the handle is kept to the very end

        Ok(())

        // All other handles will be collected when dropping self
    }
}

/// An application handle.
///
/// This handle manages a dedicated event loop and other resources (like event handlers). It's
/// designed to be operable on foreign threads, but comes with certain limitations. See method docs
/// for details.
///
/// An [`App`] cannot be constructed. Instead, it must be obtained from the callback of
/// [`AppManager::run`]. It can then be cloned and shared with other threads as needed.
///
/// Cloning this handle creates a shared reference to the same underlying event loop.
#[derive(Clone)]
pub struct App(Arc<RawApp>);

impl App {
    pub(crate) fn as_ptr(&self) -> *mut saucer_application { self.0.as_ptr() }

    /// Checks whether we're on the event thread.
    pub fn is_thread_safe(&self) -> bool { self.0.is_thread_safe() }

    /// Posts a callback to be invoked on the event thread, schedules it on the next event loop.
    pub fn post(&self, callback: impl FnOnce(App) + Send + 'static) {
        let data = PostCallbackData::new(callback, self.downgrade()).into_raw();
        unsafe { saucer_application_post(self.0.as_ptr(), Some(post_callback_tp), data) };
    }

    /// Like [`Self::post`], but automatically drops the callback after the given timeout if it's
    /// not invoked. Returns a [`JoinHandle`] that can be used to join the dropper thread, which
    /// returns whether the callback has been consumed by the dropper (i.e. was not invoked).
    ///
    /// This method spawns a thread that races with the event thread after the specified timeout, at
    /// which it will consume the callback. This will bring some overhead, but can be useful
    /// when guaranteed drop is needed.
    pub fn post_timeout(
        &self,
        callback: impl FnOnce(App) + Send + 'static,
        timeout: Duration,
    ) -> JoinHandle<bool> {
        let data = PostTimeoutCallbackData::new(callback, self.downgrade());
        let cb = data.clone_callback();
        let data = data.into_raw();

        unsafe { saucer_application_post(self.0.as_ptr(), Some(post_timeout_callback_tp), data) };

        std::thread::spawn(move || {
            std::thread::sleep(timeout);
            if let Ok(mut guard) = cb.try_lock() { guard.take().is_some() } else { false }
        })
    }

    /// Quits the app.
    pub fn quit(self) { unsafe { saucer_application_quit(self.as_ptr()) }; }

    /// Gets a list of screens available.
    pub fn screens(&self) -> Vec<Screen> {
        let data = load_range!(ptr[size] = null_mut(); {
            unsafe { saucer_application_screens(self.as_ptr(), ptr, size) };
        });

        data.into_iter().filter_map(|p| unsafe { Screen::from_raw(p) }).collect()
    }

    /// Gets a weak [`AppRef`].
    pub fn downgrade(&self) -> AppRef { AppRef(Arc::downgrade(&self.0)) }

    /// Clones a drop sender.
    pub(crate) fn drop_sender(&self) -> Sender<Box<dyn FnOnce() + Send>> {
        self.0.drop_sender.clone()
    }
}

/// A weak app handle.
///
/// This struct internally holds a weak reference to the app handle and does not prevent its
/// destruction. It's mainly designed for use in callbacks to prevent circular references.
#[derive(Clone)]
pub struct AppRef(Weak<RawApp>);

impl AppRef {
    /// Tries to upgrade to a strong handle.
    pub fn upgrade(&self) -> Option<App> { Some(App(self.0.upgrade()?)) }
}

/// A struct that holds a listener which can be invoked when the app quits.
#[derive(Default)]
pub struct FinishListener {
    inner: Option<Box<dyn FnOnce(App) + 'static>>,
}

impl FinishListener {
    /// Sets the finish callback. Replaces it if already set.
    pub fn set(&mut self, listener: impl FnOnce(App) + 'static) {
        self.inner = Some(Box::new(listener));
    }
}

type BoxedRunCallback = Box<dyn FnOnce(App, &mut FinishListener) + 'static>;

struct RunCallbackData {
    callback: Option<BoxedRunCallback>,
    finish_listener: FinishListener,
    app: App, // This doesn't need to be weak as start routine will always be called
}

impl RunCallbackData {
    fn new(cb: impl FnOnce(App, &mut FinishListener) + 'static, app: App) -> Self {
        Self { callback: Some(Box::new(cb)), finish_listener: FinishListener::default(), app }
    }

    fn into_raw(self) -> *mut c_void { Box::into_raw(Box::new(self)) as *mut c_void }
}

extern "C" fn run_callback_tp(_: *mut saucer_application, data: *mut c_void) {
    // SAFETY: The method is invoked only once.
    let mut data = unsafe { Box::from_raw(data as *mut RunCallbackData) };
    let start = data.callback.take().expect("start callback should be present");
    start(data.app.clone(), &mut data.finish_listener);
    let _ = Box::into_raw(data); // It will be used in the finish callback
}

extern "C" fn finish_callback_tp(_: *mut saucer_application, data: *mut c_void) {
    // SAFETY: The method will not be invoked before the run callback returns, making it safe to
    // reclaim the ownership of the user data.
    let data = unsafe { Box::from_raw(data as *mut RunCallbackData) };

    if let Some(cb) = data.finish_listener.inner {
        cb(data.app);
    }
}

type BoxedPostCallback = Box<dyn FnOnce(App) + Send + 'static>;

struct PostCallbackData {
    callback: BoxedPostCallback,
    app: AppRef,
}

impl PostCallbackData {
    fn new(cb: impl FnOnce(App) + Send + 'static, app: AppRef) -> Self {
        Self { callback: Box::new(cb), app }
    }

    fn into_raw(self) -> *mut c_void { Box::into_raw(Box::new(self)) as *mut c_void }
}

extern "C" fn post_callback_tp(data: *mut c_void) {
    // SAFETY: The method is invoked only once.
    let data = unsafe { Box::from_raw(data as *mut PostCallbackData) };
    if let Some(app) = data.app.upgrade() {
        // Clone is not needed like webviews, as app is guaranteed to be valid when the event loop
        // is running
        (data.callback)(app);
    }
}

struct PostTimeoutCallbackData {
    callback: Arc<Mutex<Option<BoxedPostCallback>>>,
    app: AppRef,
}

impl PostTimeoutCallbackData {
    fn new(cb: impl FnOnce(App) + Send + 'static, app: AppRef) -> Self {
        Self { callback: Arc::new(Mutex::new(Some(Box::new(cb)))), app }
    }

    fn clone_callback(&self) -> Arc<Mutex<Option<BoxedPostCallback>>> { self.callback.clone() }

    fn into_raw(self) -> *mut c_void { Box::into_raw(Box::new(self)) as *mut c_void }
}

extern "C" fn post_timeout_callback_tp(data: *mut c_void) {
    // SAFETY: The method is invoked only once.
    let data = unsafe { Box::from_raw(data as *mut PostTimeoutCallbackData) };
    let cb = {
        let Ok(mut guard) = data.callback.try_lock() else {
            return; // The dropper has acquired the lock, give up
        };

        match guard.take() {
            Some(cb) => cb,
            None => return,
        }
    };

    if let Some(app) = data.app.upgrade() {
        cb(app);
    }
}

struct EventListenerData<'a> {
    listener: &'a dyn AppEventListener,
    app: AppRef,
}

impl<'a> EventListenerData<'a> {
    fn new(listener: &'a dyn AppEventListener, app: AppRef) -> Self { Self { listener, app } }
}

extern "C" fn ev_on_quit_tp(_: *mut saucer_application, data: *mut c_void) -> saucer_policy {
    // SAFETY: The borrow inside the data is guaranteed to be valid as long as the app runs.
    let data = unsafe { &*(data as *const EventListenerData) };
    if let Some(app) = data.app.upgrade() {
        data.listener.on_quit(app).into()
    } else {
        Policy::Allow.into()
    }
}
