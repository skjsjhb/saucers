//! Application event cycle module.
//!
//! See [`App`] and [`AppManager`] for details.

mod events;
mod options;

use std::ffi::c_void;
use std::panic::UnwindSafe;
use std::ptr::NonNull;
use std::ptr::null_mut;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::thread::ThreadId;
use std::time::Duration;

pub use events::*;
pub use options::*;
use saucer_sys::*;

use crate::cleanup::CleanUpHolder;
use crate::macros::ffi_forward;
use crate::macros::load_range;
use crate::policy::Policy;
use crate::screen::Screen;
use crate::util::ffi_callback;
use crate::webview::Webview;
use crate::window::Window;

/// An unprotected owned app handle.
struct RawApp {
    inner: NonNull<saucer_application>,
    /// Drop sender for other handles.
    drop_sender: Sender<CleanUpHolder>,
    /// Drop sender for the app itself.
    app_drop_sender: Sender<CleanUpHolder>,
    host_tid: ThreadId,
}

// SAFETY: App handles are thread-safe for dispatching, and dropping is handled
// by the collector. The event listener is only accessed on the event thread.
unsafe impl Send for RawApp {}
unsafe impl Sync for RawApp {}

impl Drop for RawApp {
    fn drop(&mut self) {
        // Send the cleanup pack even when we're on the event thread, ensuring it's
        // destroyed after other handles.
        self.app_drop_sender
            .send(CleanUpHolder::App { ptr: self.inner })
            .expect("failed to post app destruction");
    }
}

impl RawApp {
    pub(crate) fn new(
        inner: NonNull<saucer_application>,
        drop_sender: Sender<CleanUpHolder>,
        app_drop_sender: Sender<CleanUpHolder>,
    ) -> Self {
        Self {
            inner,
            drop_sender,
            app_drop_sender,
            host_tid: std::thread::current().id(),
        }
    }

    fn is_thread_safe(&self) -> bool { self.host_tid == std::thread::current().id() }

    pub(crate) fn as_ptr(&self) -> *mut saucer_application { self.inner.as_ptr() }
}

/// A struct that manages apps and collects all handles.
///
/// This struct never owns an app handle. Instead, it creates one on-demand when
/// starting the event loop, and collects them before exiting.
pub struct AppManager {
    raw_opt: RawAppOptions,
    drop_sender: Option<Sender<CleanUpHolder>>,
    receiver: Receiver<CleanUpHolder>,
    // App needs to be destroyed after all other handles, thus a dedicated channel
    app_drop_sender: Option<Sender<CleanUpHolder>>,
    app_receiver: Receiver<CleanUpHolder>,
}

impl AppManager {
    /// Attempts to collect and cleanup all handles, blocking if necessary.
    ///
    /// SAFETY: Must be called on the event thread.
    unsafe fn collect_handles(mut self) {
        drop(self.drop_sender.take());

        // As long as there is still one sender alive, this call would block, which
        // guarantees that no handles shall remain reachable after this loop.
        while let Ok(p) = self.receiver.recv() {
            unsafe { p.discard() };
        }

        // Now that all handles are destroyed, we can safely destroy the app.
        drop(self.app_drop_sender.take());
        while let Ok(p) = self.app_receiver.recv() {
            unsafe { p.discard() };
        }
    }

    /// Constructs an app manager from the given options.
    pub fn new(opt: AppOptions) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        let (app_sender, app_receiver) = std::sync::mpsc::channel();
        Self {
            raw_opt: RawAppOptions::new(opt),
            drop_sender: Some(sender),
            receiver,
            app_drop_sender: Some(app_sender),
            app_receiver,
        }
    }

    /// Runs the app with specified the event handlers. Invokes the given
    /// callback (the *start callback*) once when entering the event loop.
    ///
    /// The start callback may return a [`FinishRoutine`], which may specify a
    /// desired action when the event loop terminates. [`Webview`], [`Window`]
    /// and `(T,)` simply drop themselves, while closures will be invoked.
    ///
    /// The thread that this method is called will be used as the event thread,
    /// that is, the thread that runs the event loop. Specifically, this
    /// method must only be called on the starting thread on macOS due to
    /// limitations of Cocoa.
    ///
    /// In the C++ API, the callback is designed for holding windows and
    /// webviews so that they don't "escape" the app event cycle scope. It
    /// leverages async C++ for non-blocking app status check. As we
    /// currently have no plan to support async parts (which will more or less
    /// involve pulling in some crates for async), handles created in the
    /// callback will be dropped when it exits, which will normally destroy
    /// the windows. Handles can instead be kept alive by either:
    ///
    /// 1. Store the handles at a place which outlives the event loop lifecycle.
    /// 2. Return the handles as the finish routine, or move them into one.
    ///
    /// Any handle created in the start callback must be dropped no later than
    /// its finish routine. Failing to do so would lead to a deadlock, as this
    /// method tries to join all handles before returning.
    pub fn run<F>(
        mut self,
        start: impl FnOnce(App) -> F + UnwindSafe + 'static,
        event_listener: impl AppEventListener,
    ) -> crate::error::Result<()>
    where
        F: FinishRoutine + 'static,
    {
        #[cfg(target_os = "macos")]
        objc2::MainThreadMarker::new().expect("event loop must be started from the main thread");

        let mut ex = -1;

        // SAFETY: The options are kept valid until the app quits.
        let ptr = unsafe { saucer_application_new(self.raw_opt.as_ptr(), &raw mut ex) };

        let app = NonNull::new(ptr).ok_or(crate::error::Error::Saucer(ex))?;

        let sender = self.drop_sender.take().unwrap();
        let app_sender = self.app_drop_sender.take().unwrap();
        let app = App(Arc::new(RawApp::new(app, sender, app_sender)));

        // The listener is only dropped after the events are removed
        let data = Box::into_raw(Box::new(EventListenerData::new(
            &event_listener,
            app.downgrade(),
        )));

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
            // Callback data is freed in the finish callback
            saucer_application_run(ptr, Some(run_callback_tp), Some(finish_callback_tp), cdata)
        };

        drop(app); // Ensure the handle is kept to the very end to prevent immature frees

        unsafe { self.collect_handles() }; // SAFETY: On the event thread

        // App handles are invalid here, yet AppRef won't be able to upgrade anyway.
        unsafe { drop(Box::from_raw(data)) };

        Ok(())
    }
}

/// An application handle.
///
/// This handle manages a dedicated event loop and other resources (like event
/// handlers). It's designed to be operable on foreign threads, but comes with
/// certain limitations. See method docs for details.
///
/// An [`App`] cannot be constructed. Instead, it must be obtained from the
/// callback of [`AppManager::run`]. It can then be cloned and shared with other
/// threads as needed.
///
/// Cloning this handle creates a shared reference to the same underlying event
/// loop.
#[derive(Clone)]
pub struct App(Arc<RawApp>);

impl App {
    ffi_forward! {
        /// Quits the app.
        pub fn quit(Self) => saucer_application_quit;
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_application { self.0.as_ptr() }

    /// Checks whether we're on the event thread.
    pub fn is_thread_safe(&self) -> bool { self.0.is_thread_safe() }

    /// Posts a callback to be invoked on the event thread, schedules it on the
    /// next event loop.
    pub fn post(&self, callback: impl FnOnce(App) + Send + UnwindSafe + 'static) {
        let data = PostCallbackData::new(callback, self.downgrade()).into_raw();
        unsafe { saucer_application_post(self.0.as_ptr(), Some(post_callback_tp), data) };
    }

    /// Like [`Self::post`], but automatically drops the callback after the
    /// given timeout if it's not invoked. Returns a [`JoinHandle`] that can
    /// be used to join the dropper thread, which returns whether the
    /// callback has been consumed by the dropper (i.e. was not invoked).
    ///
    /// This method spawns a thread that races with the event thread after the
    /// specified timeout, at which it will consume the callback. This will
    /// bring some overhead, but can be useful when guaranteed drop is
    /// needed.
    pub fn post_timeout(
        &self,
        callback: impl FnOnce(App) + Send + UnwindSafe + 'static,
        timeout: Duration,
    ) -> JoinHandle<bool> {
        let data = PostTimeoutCallbackData::new(callback, self.downgrade());
        let cb = data.clone_callback();
        let data = data.into_raw();

        unsafe { saucer_application_post(self.0.as_ptr(), Some(post_timeout_callback_tp), data) };

        std::thread::spawn(move || {
            std::thread::sleep(timeout);
            if let Ok(mut guard) = cb.try_lock() {
                guard.take().is_some()
            } else {
                false
            }
        })
    }

    /// Gets a list of screens available.
    pub fn screens(&self) -> Vec<Screen> {
        let data = load_range!(ptr[size] = null_mut(); {
            unsafe { saucer_application_screens(self.as_ptr(), ptr, size) };
        });

        data.into_iter()
            .filter_map(|p| unsafe { Screen::from_raw(p) })
            .collect()
    }

    /// Gets a weak [`AppRef`].
    pub fn downgrade(&self) -> AppRef { AppRef(Arc::downgrade(&self.0)) }

    /// Clones a drop sender.
    pub(crate) fn drop_sender(&self) -> Sender<CleanUpHolder> { self.0.drop_sender.clone() }
}

/// A weak app handle.
///
/// This struct internally holds a weak reference to the app handle and does not
/// prevent its destruction. It's mainly designed for use in callbacks to
/// prevent circular references.
#[derive(Clone)]
pub struct AppRef(Weak<RawApp>);

impl AppRef {
    /// Tries to upgrade to a strong handle.
    pub fn upgrade(&self) -> Option<App> { Some(App(self.0.upgrade()?)) }
}

/// A one-shot routine invoked after the app event loop stops.
pub trait FinishRoutine: UnwindSafe {
    /// Runs this routine once after the event loop stops.
    ///
    /// [`Box`] receiver is used as this trait is subject to type erasing so it
    /// can be carried across FFI boundary.
    #[allow(unused)]
    fn on_finish(self: Box<Self>, app: App) {}
}

/// Returning a closure as [`FinishRoutine`] will invoke it upon stopping.
impl<F> FinishRoutine for F
where F: FnOnce(App) + UnwindSafe
{
    fn on_finish(self: Box<Self>, app: App) { self(app); }
}

// Capturing nothing from the start callback is almost never desired.
// impl FinishRoutine for () {}

/// Allows you to return a [`Webview`] directly from start callback to keep it.
impl FinishRoutine for Webview {}

/// Allows you to return a [`Webview`] directly from start callback to keep it.
impl FinishRoutine for Window {}

/// Allows you to keep an arbitrary value until the event loop quits.
impl<T: UnwindSafe> FinishRoutine for (T,) {}

type BoxedFinishRoutine = Box<dyn FinishRoutine + 'static>;
type BoxedRunCallback = Box<dyn FnOnce(App) -> BoxedFinishRoutine + UnwindSafe + 'static>;

struct RunCallbackData {
    callback: Option<BoxedRunCallback>,
    finish_routine: Option<BoxedFinishRoutine>,
    app: App, // This doesn't need to be weak as start callback will always be called
}

impl RunCallbackData {
    fn new<R>(cb: impl FnOnce(App) -> R + UnwindSafe + 'static, app: App) -> Self
    where R: FinishRoutine + 'static {
        Self {
            callback: Some(Box::new(move |app| Box::new(cb(app)))),
            finish_routine: None,
            app,
        }
    }

    fn into_raw(self) -> *mut c_void { Box::into_raw(Box::new(self)) as *mut c_void }
}

extern "C" fn run_callback_tp(_: *mut saucer_application, data: *mut c_void) {
    // SAFETY: The method is invoked only once.
    let mut data = unsafe { Box::from_raw(data as *mut RunCallbackData) };
    if let Some(start) = data.callback.take() {
        let app = data.app.clone();
        data.finish_routine = ffi_callback(None, move || Some(start(app)));
    }
    let _ = Box::into_raw(data); // It will be used in the finish callback
}

extern "C" fn finish_callback_tp(_: *mut saucer_application, data: *mut c_void) {
    ffi_callback((), || {
        // SAFETY: The method will not be invoked before the run callback returns,
        // making it safe to reclaim the ownership of the user data.
        let data = unsafe { Box::from_raw(data as *mut RunCallbackData) };

        if let Some(routine) = data.finish_routine {
            routine.on_finish(data.app);
        }
    });
}

type BoxedPostCallback = Box<dyn FnOnce(App) + Send + UnwindSafe + 'static>;

struct PostCallbackData {
    callback: BoxedPostCallback,
    app: AppRef,
}

impl PostCallbackData {
    fn new(cb: impl FnOnce(App) + Send + UnwindSafe + 'static, app: AppRef) -> Self {
        Self {
            callback: Box::new(cb),
            app,
        }
    }

    fn into_raw(self) -> *mut c_void { Box::into_raw(Box::new(self)) as *mut c_void }
}

extern "C" fn post_callback_tp(data: *mut c_void) {
    ffi_callback((), || {
        // SAFETY: The method is invoked only once.
        let data = unsafe { Box::from_raw(data as *mut PostCallbackData) };
        if let Some(app) = data.app.upgrade() {
            // Clone is not needed like webviews, as app is guaranteed to be valid when the
            // event loop is running
            (data.callback)(app);
        }
    });
}

struct PostTimeoutCallbackData {
    callback: Arc<Mutex<Option<BoxedPostCallback>>>,
    app: AppRef,
}

impl PostTimeoutCallbackData {
    fn new(cb: impl FnOnce(App) + Send + UnwindSafe + 'static, app: AppRef) -> Self {
        Self {
            callback: Arc::new(Mutex::new(Some(Box::new(cb)))),
            app,
        }
    }

    fn clone_callback(&self) -> Arc<Mutex<Option<BoxedPostCallback>>> { self.callback.clone() }

    fn into_raw(self) -> *mut c_void { Box::into_raw(Box::new(self)) as *mut c_void }
}

extern "C" fn post_timeout_callback_tp(data: *mut c_void) {
    ffi_callback((), || {
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
    });
}

struct EventListenerData<'a> {
    listener: &'a dyn AppEventListener,
    app: AppRef,
}

impl<'a> EventListenerData<'a> {
    fn new(listener: &'a dyn AppEventListener, app: AppRef) -> Self { Self { listener, app } }
}

extern "C" fn ev_on_quit_tp(_: *mut saucer_application, data: *mut c_void) -> saucer_policy {
    // SAFETY: The borrow inside the data is guaranteed to be valid as long as the
    // app runs.
    let data = unsafe { &*(data as *const EventListenerData) };
    ffi_callback(Policy::Allow.into(), || {
        if let Some(app) = data.app.upgrade() {
            data.listener.on_quit(app).into()
        } else {
            Policy::Allow.into()
        }
    })
}
