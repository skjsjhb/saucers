use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::app::App;
use crate::capi::*;
use crate::scheme::Response;

pub enum SchemeError {
    NotFound,
    Invalid,
    Aborted,
    Denied,
    Failed
}

impl From<SchemeError> for SAUCER_SCHEME_ERROR {
    fn from(value: SchemeError) -> Self {
        match value {
            SchemeError::NotFound => SAUCER_SCHEME_ERROR_SAUCER_REQUEST_ERROR_NOT_FOUND,
            SchemeError::Invalid => SAUCER_SCHEME_ERROR_SAUCER_REQUEST_ERROR_INVALID,
            SchemeError::Aborted => SAUCER_SCHEME_ERROR_SAUCER_REQUEST_ERROR_ABORTED,
            SchemeError::Denied => SAUCER_SCHEME_ERROR_SAUCER_REQUEST_ERROR_DENIED,
            SchemeError::Failed => SAUCER_SCHEME_ERROR_SAUCER_REQUEST_ERROR_FAILED
        }
    }
}

pub struct Executor {
    ptr: Option<NonNull<saucer_scheme_executor>>,
    app: App,
    _owns: PhantomData<saucer_scheme_executor>
}

unsafe impl Send for Executor {}
unsafe impl Sync for Executor {}

impl Drop for Executor {
    fn drop(&mut self) {
        if let Some(ptr) = self.ptr.take() {
            unsafe { saucer_scheme_executor_free(ptr.as_ptr()) }
        }
    }
}

impl Executor {
    pub(crate) fn from_ptr(ptr: *mut saucer_scheme_executor, app: App) -> Self {
        Self {
            ptr: Some(NonNull::new(ptr).expect("Invalid scheme executor")),
            app,
            _owns: PhantomData
        }
    }

    pub fn resolve_here(self, res: &Response<'_>) {
        if !self.app.is_thread_safe() {
            panic!("Scheme executor can only be resolved asynchronously on the event thread")
        }

        // This function internally forwards the data to the event thread and resolve there, so technically it's safe
        // to call it directly. However, the C library does not handle Rust lifetimes correctly, so it's up to us to
        // only call it on the event thread and provide another method for async resolving.
        let ptr = self.ptr.expect("Resolving a destroyed executor");
        unsafe { saucer_scheme_executor_resolve(ptr.as_ptr(), res.as_ptr()) }
    }

    pub fn resolve(mut self, res: Response<'static>) {
        if self.app.is_thread_safe() {
            self.resolve_here(&res);
        } else {
            let ptr = self.ptr.take().expect("Resolving a destroyed executor").as_ptr() as usize;
            self.app.post(move |_| {
                let ptr = ptr as *mut saucer_scheme_executor;
                unsafe {
                    saucer_scheme_executor_resolve(ptr, res.as_ptr());
                    saucer_scheme_executor_free(ptr);
                }
            });
        }
    }

    pub fn reject(self, ex: SchemeError) {
        unsafe { saucer_scheme_executor_reject(self.ptr.expect("Resolving a destroyed executor").as_ptr(), ex.into()) }
    }
}
