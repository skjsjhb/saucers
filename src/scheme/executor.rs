use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::app::App;
use crate::capi::*;
use crate::scheme::Response;

/// Error types that can be used as the argument of [`Executor::reject`].
///
/// Currently, due to a missing CORS header, the enum variant used to reject the request is not accessible from the
/// frontend. It's advised to always resolve the request with a [`Response`], which is capable for sending more detailed
/// rejection message.
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

/// The executor object used to resolve or reject a request to a custom scheme.
///
/// An executor is passed as an argument to the scheme handler when a request comes. The handler can then
/// [`Executor::resolve_here`] or [`Executor::reject`] the request.
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

    /// Resolves with the given response immediately.
    ///
    /// To avoid data copying (as response payload can be large), this method uses a borrow-only approach to send the
    /// response. As the C API does not know about Rust data lifetimes, its original capability of resolving
    /// asynchronously can't be made safe, making this method only callable from the event thread. To resolve from other
    /// threads, use [`crate::webview::Webview::post`] and move/copy the payload optionally.
    ///
    /// # Panics
    ///
    /// Panics if not called on the event thread.
    pub fn resolve_here(self, res: &Response<'_>) {
        if !self.app.is_thread_safe() {
            panic!("Scheme executor can only be resolved on the event thread")
        }

        // This function internally forwards the data to the event thread and resolve there, so technically it's safe
        // to call it directly. However, the C library does not handle Rust lifetimes correctly, so it's up to us to
        // only call it on the event thread and provide another API for async resolving.
        let ptr = self.ptr.expect("Resolving a destroyed executor");
        unsafe { saucer_scheme_executor_resolve(ptr.as_ptr(), res.as_ptr()) }
    }

    /// Resolves with the given response as soon as possible. Potentially moves the response to another thread if
    /// needed.
    ///
    /// Unlike [`Self::resolve_here`], this method can be called on any thread. As a price, this method consumes the
    /// response, making it unreusable. Also, as the data may be moved to another thread, the lifetime of the given
    /// response is bounded to static.
    pub fn resolve(self, res: Response<'static>) {
        if self.app.is_thread_safe() {
            self.resolve_here(&res);
        } else {
            let app = self.app.clone();
            app.post(move |_| {
                self.resolve_here(&res);
            });
        }
    }

    /// Rejects with the given [`SchemeError`]. Unlike [`Self::resolve_here`], this method can be called on any thread.
    ///
    /// As described in [`SchemeError`], current implementation is missing a CORS header and the rejection message can't
    /// be identified by the frontend. Consider using [`Self::resolve_here`] with a status code instead.
    pub fn reject(self, ex: SchemeError) {
        unsafe { saucer_scheme_executor_reject(self.ptr.expect("Resolving a destroyed executor").as_ptr(), ex.into()) }
    }
}
