use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::Mutex;
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
    counter: Arc<Mutex<i32>>,
    options: Arc<AppOptions>,
    _no_send: PhantomData<*const ()>
}

impl Drop for App {
    fn drop(&mut self) {
        let mut counter = self.counter.try_lock().expect("App must not be dropped concurrently");
        *counter -= 1;

        if *counter > 0 {
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
        *self.counter.lock().unwrap() += 1;

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
            counter: Arc::new(Mutex::new(1)),
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
        let bb: Box<dyn FnOnce() + 'static> = Box::new(fun);
        let raw = Box::into_raw(Box::new(bb));
        unsafe {
            saucer_application_post_with_arg(ptr.as_ptr(), Some(post_trampoline), raw as *mut c_void);
        }
    }

    /// Runs the event loop (blocking).
    pub fn run(&self) {
        println!("Starting event loop");
        let (ptr, _guard) = self.get_ptr();
        unsafe {
            saucer_application_run(ptr.as_ptr());
        }
        println!("Leaving event loop")
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

    pub(crate) fn get_ptr(&self) -> (AppPtr, RwLockReadGuard<'_, Option<AppPtr>>) {
        let guard = self.inner.read().unwrap();
        let ptr = guard.expect("Owned app pointer should always be valid");
        (ptr, guard)
    }
}

pub struct AppHandle {
    inner: Arc<RwLock<Option<AppPtr>>>,
    counter: Arc<Mutex<i32>>,
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
        if let Some(ref ptr) = *guard {
            let bb: Box<dyn FnOnce() + Send + 'static> = Box::new(fun);
            let raw = Box::into_raw(Box::new(bb));
            unsafe {
                saucer_application_post_with_arg(ptr.as_ptr(), Some(post_trampoline), raw as *mut c_void);
            }
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
            *self.counter.lock().unwrap() += 1;
            App {
                inner: self.inner.clone(),
                counter: self.counter.clone(),
                options: self.options.clone(),
                _no_send: PhantomData
            }
        })
    }
}

extern "C" fn post_trampoline(raw: *mut c_void) {
    unsafe {
        let bb = Box::from_raw(raw as *mut Box<dyn FnOnce()>);
        bb();
    }
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use crate::options::AppOptions;

    #[test]
    fn test_app() {
        let app = App::new(AppOptions::new("saucer"));
        let app1 = app.clone();
        app.post(move || app1.quit());
        app.run();
        println!("After app run")
    }
}
