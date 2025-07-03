use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::Weak;

use crate::capi::*;
use crate::options::AppOptions;

/// The struct behind [`App`] which holds related resources.
struct ManagedApp {
    inner: NonNull<saucer_application>,
    _opt: AppOptions, // Unused, only for keeping values in the options alive
    has_guard: Cell<bool>, // Prevents multiple `run` and `run_once` from being fired
    _owns: PhantomData<saucer_application>,
}


// SAFETY: Though the pointer inside the managed app can be used to perform thread-unsafe operations, the struct itself
// is safe to be moved around, as long as its inner pointer is only accessed in a safe way, which `App` does exactly.
unsafe impl Send for ManagedApp {}

// SAFETY: Although `Cell` is not `Sync`, access to it is limited to the main thread only, where it was created.
// For the raw pointer, safe accesses can be performed from any thread, while unsafe accesses are guarded by `AppGuard`.
unsafe impl Sync for ManagedApp {}

impl Drop for ManagedApp {
    fn drop(&mut self) {
        unsafe {
            APP_REGISTRY.lock().unwrap().remove(&AppPtr(self.as_ptr()));
            saucer_application_free(self.as_ptr());
        }
    }
}

impl ManagedApp {
    fn new(mut opt: AppOptions) -> Self {
        // SAFETY: The application is eventually freed
        let ptr = unsafe {
            // Implementations using the args (e.g. Qt) does not store them
            // Thus the options object must stay managed on the Rust side
            // SAFETY: The options object is never shared (it's not cloneable and ownership is taken)
            // The options object is only dropped after app has been dropped
            saucer_application_init(opt.as_ptr())
        };

        Self {
            inner: NonNull::new(ptr).expect("Failed to create app"),
            _opt: opt,
            has_guard: Cell::new(false),
            _owns: PhantomData,
        }
    }

    /// This method deliberately discards the `mut` qualifier to provide interior mutability.
    /// Data racing is prevented in the C library (for shared access) and the inner lock (for exclusive access).
    /// [`App`] only provides safe operations, while unsafe operations are wrapped in [`AppGuard`], which provides
    /// guaranteed exclusive (via locking) and thread correctness.
    ///
    /// The saucer C-bindings lacks necessary qualifiers to ensure such read-only types.
    ///
    /// SAFETY: The returned pointer is used properly (see the comments in [`App`]).
    unsafe fn as_ptr(&self) -> *mut saucer_application { self.inner.as_ptr() }
}

#[derive(Eq, PartialEq, Copy, Clone, Hash)]
struct AppPtr(*mut saucer_application);

// SAFETY: This struct uses pointer by value and never dereferences or exposes it
unsafe impl Send for AppPtr {}
unsafe impl Sync for AppPtr {}

static APP_REGISTRY: LazyLock<Mutex<HashMap<AppPtr, Weak<ManagedApp>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// An app object for event handling.
///
/// This type only offers "safe" interfaces (safe as in thread-safe) in order to keep the [`Send`] and [`Sync`] marker
/// sound. To perform thread-specific operations (e.g. `post`), see [`AppGuard`].
#[derive(Clone)]
pub struct App(Arc<ManagedApp>);

// Implementation Notes (IMPORTANT!)
// Safety of the `app` module is largely guaranteed by the `App` struct, especially how it interacts with the inner
// `ManagedApp`. In particular, `ManagedApp` is `Send` and `Sync` because `App` rules out invalid accesses that may
// break such safety (e.g. misuse the pointer). That is to say, `ManagedApp` should behave like `!Send` and `!Sync`
// in the implementation block below.
impl App {
    /// Create a new app object with specified options.
    pub fn new(opt: AppOptions) -> Self {
        let ma = ManagedApp::new(opt);
        let ptr = AppPtr(unsafe { ma.as_ptr() }); // SAFETY: Taking pointer as value
        let ma = Arc::new(ma);
        let weak = Arc::downgrade(&ma);
        APP_REGISTRY.lock().unwrap().insert(ptr, weak);

        Self(ma)
    }

    /// Clones a shared-owning app object for the given pointer.
    ///
    /// This method does a lookup in an internal mapping table.
    /// If the given key does not exist, or its owner has been dropped, this method returns [`None`].
    fn from_ptr(ptr: *mut saucer_application) -> Option<Self> {
        let ma = APP_REGISTRY.lock().unwrap().get(&AppPtr(ptr))?.upgrade()?;
        Some(Self(ma))
    }

    pub(crate) unsafe fn as_ptr(&self) -> *mut saucer_application {
        unsafe { self.0.as_ptr() }
    }

    /// Gets the active app instance.
    pub fn active() -> Option<Self> {
        let ptr = unsafe {
            saucer_application_active()
        };
        Self::from_ptr(ptr)
    }

    /// Checks if the app is now being used on the thread it was created.
    pub fn is_thread_safe(&self) -> bool {
        unsafe {
            saucer_application_thread_safe(self.as_ptr())
        }
    }

    /// Submits a closure to be executed in the background thread pool and waits for it to finish.
    pub fn pool_submit(&self, exec: impl FnOnce() + Send + 'static) {
        unsafe {
            // Two-level boxing to avoid casting around with trait objects
            // The cast to boxed trait object must be done explicitly to avoid being inferred as static types
            // SAFETY: The leaked pointer will be collected and dropped, if the posted function is eventually called
            let dy = Box::new(exec) as Box<dyn FnOnce() + Send + 'static>;
            let arg = Box::into_raw(Box::new(dy)) as *mut c_void;
            saucer_application_pool_submit_with_arg(self.as_ptr(), Some(closure_trampoline), arg);
        }
    }

    /// Emplaces a closure to be executed in the background thread pool and return immediately.
    pub fn pool_emplace(&self, exec: impl FnOnce() + Send + 'static) {
        unsafe {
            let dy = Box::new(exec) as Box<dyn FnOnce() + Send + 'static>;
            let arg = Box::into_raw(Box::new(dy)) as *mut c_void;
            saucer_application_pool_emplace_with_arg(self.as_ptr(), Some(closure_trampoline), arg);
        }
    }

    /// Posts a closure to be executed on the event thread.
    pub fn post(&self, exec: impl FnOnce() + Send + 'static) {
        unsafe {
            let dy = Box::new(exec) as Box<dyn FnOnce() + Send + 'static>;
            let arg = Box::into_raw(Box::new(dy)) as *mut c_void;
            saucer_application_post_with_arg(self.as_ptr(), Some(closure_trampoline), arg);
        }
    }

    /// Tries to obtain a lock object to perform thread-specific and/or exclusive operations.
    pub fn require_main(&self) -> Option<AppGuard<'_>> {
        if self.is_thread_safe() && !self.0.has_guard.get() {
            self.0.has_guard.set(true);
            Some(
                AppGuard {
                    has_guard_ref: &self.0.has_guard,
                    // SAFETY: The guard (which uses this pointer) cannot live longer than the owner
                    // The exported pointer can only be used on the same thread as the `App` which creates it
                    // The guard above guarantees exclusive access
                    ptr: unsafe { self.as_ptr() },
                }
            )
        } else {
            None
        }
    }

    /// Requests the app to quit.
    pub fn quit(&self) {
        unsafe {
            saucer_application_quit(self.as_ptr());
        }
    }
}

pub struct AppGuard<'a> {
    has_guard_ref: &'a Cell<bool>,
    ptr: *mut saucer_application,
}

impl Drop for AppGuard<'_> {
    fn drop(&mut self) {
        self.has_guard_ref.set(false)
    }
}

impl<'a> AppGuard<'a> {
    /// Runs the app (blocking).
    ///
    /// This method may only be called on the thread that it's created on.
    pub fn run(&self) {
        unsafe {
            // For some platforms (e.g. GTK), `run` calls should be exclusive, thus an exclusive lock is acquired
            saucer_application_run(self.ptr);
        }
    }

    /// Runs the app (non-blocking).
    ///
    /// This method may only be called on the thread that it's created on.
    pub fn run_once(&self) {
        unsafe {
            saucer_application_run_once(self.ptr);
        }
    }
}

/// Helper function for reconstructing Rust [`FnOnce`] and invoking them.
extern "C" fn closure_trampoline(arg: *mut c_void) {
    unsafe {
        // SAFETY: The arg pointer was previously leaked with the same type
        // No external functions are able to call this function in a safe way, thus it can't receive unexpected argument
        let closure = Box::from_raw(arg as *mut Box<dyn FnOnce() + Send + 'static>);
        closure();
    }
}


#[cfg(test)]
mod tests {
    use crate::app::App;
    use crate::options::AppOptions;
    use std::sync::{Arc, RwLock};

    #[test]
    fn test_app() {
        let mut opt = AppOptions::new("saucer");
        opt.set_threads(1);

        let app = App::new(opt);
        let app1 = app.clone();
        let app2 = app.clone();
        let app3 = app.clone();

        let th = std::thread::spawn(move || {
            assert!(app2.require_main().is_none(), "Trying to upgrade on another thread should fail");
            drop(app2);
        });

        let rf = Arc::new(RwLock::new(false));

        app.pool_submit({
            let rf = rf.clone();
            move || {
                *rf.write().unwrap() = true;
            }
        });

        assert!(*rf.read().unwrap(), "Submitted background task should be executed");

        let (tx2, rx2) = std::sync::mpsc::channel();

        app.pool_emplace(move || {
            tx2.send(1).unwrap();
        });

        assert_eq!(rx2.recv().unwrap(), 1, "Emplaced background task should be executed");

        let (tx1, rx1) = std::sync::mpsc::channel();
        app.post(move || {
            tx1.send(1).unwrap();
            assert!(app1.require_main().is_none(), "Trying to upgrade on the same thread should fail");
            app1.quit();
            drop(app1);
        });

        app.require_main().unwrap().run_once();
        assert!(app.require_main().is_some(), "Main thread should be able to upgrade multiple times");

        app.require_main().unwrap().run();

        assert_eq!(rx1.recv().unwrap(), 1, "Posted closure should have been called");

        let pt = unsafe { app.as_ptr() };

        drop(app);

        assert!(App::from_ptr(pt).is_some(), "App should not be dropped when there still exists handles");

        th.join().unwrap(); // Ensure the handle in another thread has been dropped
        drop(app3);

        assert!(App::from_ptr(pt).is_none(), "App should be dropped when there are no handles");
    }
}