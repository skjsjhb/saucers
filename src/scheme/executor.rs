use std::marker::PhantomData;
use std::ptr::NonNull;

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
/// [`Executor::resolve`], [`Executor::resolve`] or [`Executor::reject`] the request.
pub struct Executor {
    ptr: Option<NonNull<saucer_scheme_executor>>,
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
    pub(crate) fn from_ptr(ptr: *mut saucer_scheme_executor) -> Self {
        Self {
            ptr: Some(NonNull::new(ptr).expect("Invalid scheme executor")),
            _owns: PhantomData
        }
    }

    /// Resolves with the given response.
    ///
    /// The request content is copied internally, making it droppable once this method returns. However, saucer may
    /// delay resolving the request to arbitrary time (in order to post it to the event thread). Thus, the response
    /// must not borrow data of a non-static lifetime.
    pub fn resolve(self, res: &Response<'static>) {
        let ptr = self.ptr.expect("Resolving a destroyed executor");

        // SAFETY: The response data is copied before returning.
        // The stash is also copied, making a borrowed stash unsafe, unless it's of static lifetime.
        unsafe { saucer_scheme_executor_resolve(ptr.as_ptr(), res.as_ptr()) }
    }

    /// Rejects with the given [`SchemeError`]. Unlike [`Self::resolve`], this method can be called on any thread.
    ///
    /// As described in [`SchemeError`], current implementation is missing a CORS header and the rejection message can't
    /// be identified by the frontend. Consider using [`Self::resolve`] with a status code instead.
    pub fn reject(self, ex: SchemeError) {
        unsafe { saucer_scheme_executor_reject(self.ptr.expect("Resolving a destroyed executor").as_ptr(), ex.into()) }
    }
}
