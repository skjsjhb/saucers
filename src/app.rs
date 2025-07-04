use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::RwLockReadGuard;

use crate::capi::*;
use crate::options::AppOptions;

#[derive(Copy, Clone)]
pub(crate) struct AppPtr(NonNull<saucer_application>);

impl AppPtr {
    pub(crate) fn as_ptr(self) -> *mut saucer_application { self.0.as_ptr() }
}

/// SAFETY: Simply moving/sharing the pointer itself around is safe (that's what happens in the C++ library).
unsafe impl Send for AppPtr {}
unsafe impl Sync for AppPtr {}

pub struct App {
    inner: Arc<RwLock<Option<AppPtr>>>,
    counter: Arc<AtomicU32>,
    options: Arc<AppOptions>,
    _no_send: PhantomData<*const ()>
}

impl Drop for App {
    fn drop(&mut self) {
        // The atomic is only used on the event thread, so `Relaxed` is already sufficient.
        if self.counter.fetch_sub(1, Ordering::Relaxed) > 1 {
            return;
        }

        let mut ptr = self.inner.write().unwrap();
        let cpt = ptr
            .take()
            .expect("App pointer must not be taken outside the event thread");

        unsafe {
            // SAFETY: The DTOR is only called on the event thread as `App` can't be moved.
            saucer_application_free(cpt.as_ptr());
        }
    }
}

impl Clone for App {
    fn clone(&self) -> Self {
        self.counter.fetch_add(1, Ordering::Relaxed);

        Self {
            inner: self.inner.clone(),
            counter: self.counter.clone(),
            options: self.options.clone(),
            _no_send: PhantomData
        }
    }
}

impl App {
    /// Creates a new app with given options.
    pub fn new(mut opt: AppOptions) -> Self {
        let ptr = unsafe { saucer_application_init(opt.as_ptr()) };
        let ptr = NonNull::new(ptr).expect("Failed to create app");

        Self {
            inner: Arc::new(RwLock::new(Some(AppPtr(ptr)))),
            counter: Arc::new(AtomicU32::new(1)),
            options: Arc::new(opt),
            _no_send: PhantomData
        }
    }

    /// Creates a shared [`AppHandle`] to be used on other threads.
    pub fn make_handle(&self) -> AppHandle {
        AppHandle {
            inner: self.inner.clone(),
            counter: self.counter.clone(),
            options: self.options.clone()
        }
    }

    /// Schedules the closure to be called during the next message queue polling.
    pub fn post(&self, fun: impl FnOnce() + 'static) {
        let (ptr, _guard) = self.get_ptr();
        Self::post_raw(ptr, fun);
    }

    /// Posts a closure to be executed on a background thread and waits for it to return.
    pub fn pool_submit(&self, fun: impl FnOnce() + Send + 'static) {
        let (ptr, _guard) = self.get_ptr();
        Self::pool_submit_raw(ptr, fun);
    }

    /// Posts a closure to be executed on a background thread and returns immediately.
    pub fn pool_emplace(&self, fun: impl FnOnce() + Send + 'static) {
        let (ptr, _guard) = self.get_ptr();
        Self::pool_emplace_raw(ptr, fun);
    }

    /// Runs the event loop (blocking).
    pub fn run(&self) {
        let (ptr, _guard) = self.get_ptr();
        unsafe {
            saucer_application_run(ptr.as_ptr());
        }
    }

    /// Runs the event loop (non-blocking).
    pub fn run_once(&self) {
        let (ptr, _guard) = self.get_ptr();
        unsafe {
            saucer_application_run_once(ptr.as_ptr());
        }
    }

    /// Stops the event loop after this polling.
    pub fn quit(&self) {
        let (ptr, _guard) = self.get_ptr();
        unsafe {
            saucer_application_quit(ptr.as_ptr());
        }
    }

    fn post_raw(ptr: AppPtr, fun: impl FnOnce() + 'static) {
        let bb: Box<dyn FnOnce() + 'static> = Box::new(fun);
        let raw = Box::into_raw(Box::new(bb));
        unsafe {
            saucer_application_post_with_arg(ptr.as_ptr(), Some(c_call_trampoline), raw as *mut c_void);
        }
    }

    fn pool_submit_raw(ptr: AppPtr, fun: impl FnOnce() + Send + 'static) {
        let bb: Box<dyn FnOnce() + Send + 'static> = Box::new(fun);
        let raw = Box::into_raw(Box::new(bb));
        unsafe {
            saucer_application_pool_submit_with_arg(ptr.as_ptr(), Some(c_call_trampoline), raw as *mut c_void);
        }
    }

    fn pool_emplace_raw(ptr: AppPtr, fun: impl FnOnce() + Send + 'static) {
        let bb: Box<dyn FnOnce() + Send + 'static> = Box::new(fun);
        let raw = Box::into_raw(Box::new(bb));
        unsafe {
            saucer_application_pool_emplace_with_arg(ptr.as_ptr(), Some(c_call_trampoline), raw as *mut c_void);
        }
    }

    pub(crate) fn get_ptr(&self) -> (AppPtr, RwLockReadGuard<'_, Option<AppPtr>>) {
        let guard = self.inner.read().unwrap();
        let ptr = guard.expect("Owned app pointer should always be valid");
        (ptr, guard)
    }
}

#[derive(Clone)]
pub struct AppHandle {
    inner: Arc<RwLock<Option<AppPtr>>>,
    counter: Arc<AtomicU32>,
    options: Arc<AppOptions>
}

impl AppHandle {
    /// Checks whether this method is being called on the event thread.
    ///
    /// Returns `false` if the app has been dropped.
    pub fn is_thread_safe(&self) -> bool {
        let guard = self.inner.read().unwrap();
        if let Some(ref ptr) = *guard {
            unsafe { saucer_application_thread_safe(ptr.as_ptr()) }
        } else {
            false
        }
    }

    /// Posts a closure to be executed on the event thread.
    /// If called on the event thread, schedules it to be called during the next message queue polling.
    ///
    /// Does nothing if the app has been dropped.
    pub fn post(&self, fun: impl FnOnce() + Send + 'static) {
        let guard = self.inner.read().unwrap();
        if let Some(ptr) = *guard {
            App::post_raw(ptr, fun);
        }
    }

    /// Posts a closure to be executed on a background thread and waits for it to return.
    ///
    /// Does nothing if the app has been dropped.
    pub fn pool_submit(&self, fun: impl FnOnce() + Send + 'static) {
        let guard = self.inner.read().unwrap();
        if let Some(ptr) = *guard {
            App::pool_submit_raw(ptr, fun);
        }
    }

    /// Posts a closure to be executed on a background thread and returns immediately.
    ///
    /// Does nothing if the app has been dropped.
    pub fn pool_emplace(&self, fun: impl FnOnce() + Send + 'static) {
        let guard = self.inner.read().unwrap();
        if let Some(ptr) = *guard {
            App::pool_emplace_raw(ptr, fun);
        }
    }

    /// Tries to upgrade this handle to [`App`] if called on the event thread.
    ///
    /// When called from other threads, or the app has been dropped, returns [`None`].
    pub fn upgrade(&self) -> Option<App> {
        if !self.is_thread_safe() {
            return None;
        }

        self.inner.read().unwrap().map(|_| {
            // This is only called on the event thread (checked above).
            self.counter.fetch_add(1, Ordering::Relaxed);
            App {
                inner: self.inner.clone(),
                counter: self.counter.clone(),
                options: self.options.clone(),
                _no_send: PhantomData
            }
        })
    }
}

extern "C" fn c_call_trampoline(raw: *mut c_void) {
    unsafe {
        let bb = Box::from_raw(raw as *mut Box<dyn FnOnce()>);
        bb();
    }
}
