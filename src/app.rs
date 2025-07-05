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

struct AppPtr {
    ptr: Arc<RwLock<Option<NonNull<saucer_application>>>>,
    _owns: PhantomData<saucer_application>,
    _counter: Arc<()>
}

unsafe impl Send for AppPtr {}
unsafe impl Sync for AppPtr {}

impl AppPtr {
    fn as_ptr(&self) -> *mut saucer_application { self.ptr.read().unwrap().unwrap().as_ptr() }
}

impl Collect for AppPtr {
    fn collect(self: Box<Self>) {
        unsafe {
            let mut guard = self.ptr.write().unwrap();
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
        let guard = ptr.read().unwrap();
        if let Some(ref ptr) = *guard {
            Self::post_raw(ptr.as_ptr(), move || {
                wk.upgrade().expect("Collector dropped before app is freed").collect()
            });
        }
    }
}

impl UnsafeApp {
    fn new(collector: Arc<UnsafeCollector>, mut opt: AppOptions) -> Self {
        let ptr = unsafe { saucer_application_init(opt.as_ptr()) };
        let ptr = Arc::new(RwLock::new(Some(NonNull::new(ptr).expect("Failed to create app"))));
        let ptr = AppPtr {
            ptr,
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
        unsafe { saucer_application_post_with_arg(ptr, Some(c_call_trampoline), cpt) }
    }

    fn post(&self, fun: impl FnOnce() + Send + 'static) { Self::post_raw(self.as_ptr(), fun) }
}

#[derive(Clone)]
pub struct App(Arc<UnsafeApp>);

impl App {
    pub fn new(collector: &Collector, opt: AppOptions) -> Self {
        Self(Arc::new(UnsafeApp::new(collector.get_inner(), opt)))
    }

    pub fn post(&self, fun: impl FnOnce() + Send + 'static) { self.0.post(fun); }

    pub fn is_thread_safe(&self) -> bool { self.0.is_host_thread() }

    /// Runs the event loop (blocking).
    ///
    /// This method must be called on the event thread, or it does nothing.
    pub fn run(&self) {
        if !self.is_thread_safe() {
            return;
        }
        unsafe { saucer_application_run(self.0.as_ptr()) }
    }

    /// Runs the event loop (non-blocking).
    ///
    /// This method must be called on the event thread, or it does nothing.
    pub fn run_once(&self) {
        if !self.is_thread_safe() {
            return;
        }
        unsafe { saucer_application_run_once(self.0.as_ptr()) }
    }

    pub fn pool_submit(&self, fun: impl FnOnce() + Send + 'static) {
        let bb = Box::new(fun) as Box<dyn FnOnce()>;
        let ptr = Box::into_raw(Box::new(bb)) as *mut c_void;
        unsafe { saucer_application_pool_submit_with_arg(self.0.as_ptr(), Some(c_call_trampoline), ptr) }
    }

    pub fn pool_emplace(&self, fun: impl FnOnce() + Send + 'static) {
        let bb = Box::new(fun) as Box<dyn FnOnce()>;
        let ptr = Box::into_raw(Box::new(bb)) as *mut c_void;
        unsafe { saucer_application_pool_emplace_with_arg(self.0.as_ptr(), Some(c_call_trampoline), ptr) }
    }

    pub fn quit(&self) {
        if self.is_thread_safe() {
            unsafe { saucer_application_quit(self.0.as_ptr()) }
        } else {
            let this = self.clone();
            self.post(move || this.quit());
        }
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_application { self.0.as_ptr() }

    pub(crate) fn get_collector(&self) -> Weak<UnsafeCollector> { self.0.collector.as_ref().unwrap().clone() }
}

extern "C" fn c_call_trampoline(raw: *mut c_void) {
    unsafe {
        let bb = Box::from_raw(raw as *mut Box<dyn FnOnce()>);
        bb();
    }
}
