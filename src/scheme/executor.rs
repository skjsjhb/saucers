use std::marker::PhantomData;
use std::ptr::NonNull;

use saucer_sys::*;

use crate::scheme::Response;

/// Error types that can be used as the argument of [`Executor::reject`].
pub enum SchemeError {
    NotFound,
    Invalid,
    Denied,
    Failed,
}

impl From<SchemeError> for saucer_scheme_error {
    fn from(value: SchemeError) -> Self {
        match value {
            SchemeError::NotFound => SAUCER_SCHEME_ERROR_NOT_FOUND,
            SchemeError::Invalid => SAUCER_SCHEME_ERROR_INVALID,
            SchemeError::Denied => SAUCER_SCHEME_ERROR_DENIED,
            SchemeError::Failed => SAUCER_SCHEME_ERROR_FAILED,
        }
    }
}

/// The executor object used to resolve or reject a request to a custom scheme.
///
/// An executor is passed as an argument to the scheme handler when a request comes. The handler can
/// then [`Executor::accept`] or [`Executor::reject`] the request.
pub struct Executor {
    ptr: NonNull<saucer_scheme_executor>,
    // TODO: Hold a webview handle to prevent destruction
    _marker: PhantomData<saucer_scheme_executor>,
}

unsafe impl Send for Executor {}
unsafe impl Sync for Executor {}

impl Drop for Executor {
    fn drop(&mut self) { unsafe { saucer_scheme_executor_free(self.ptr.as_ptr()) } }
}

impl Executor {
    /// SAFETY: The pointer must be valid and the returned handle must be dropped before the
    /// webview quits.
    pub(crate) unsafe fn from_ptr(ptr: *mut saucer_scheme_executor) -> Self {
        Self { ptr: NonNull::new(ptr).expect("invalid scheme executor"), _marker: PhantomData }
    }

    /// Resolves with the given response.
    ///
    /// The response is consumed, yet it's unclear when it will be polled, thus it's 'static.
    pub fn accept(self, res: Response<'static>) {
        // The inner stash is copied for unbound usage, thus 'static
        unsafe { saucer_scheme_executor_accept(self.ptr.as_ptr(), res.as_ptr()) }
    }

    /// Rejects with the given [`SchemeError`].
    pub fn reject(self, ex: SchemeError) {
        unsafe { saucer_scheme_executor_reject(self.ptr.as_ptr(), ex.into()) }
    }
}
