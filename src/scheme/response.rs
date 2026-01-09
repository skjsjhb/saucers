use std::marker::PhantomData;
use std::ptr::NonNull;

use saucer_sys::*;

use crate::macros::use_string;
use crate::stash::Stash;

/// Contains response details to reply a request to a custom scheme.
pub struct Response<'a> {
    ptr: NonNull<saucer_scheme_response>,
    _marker: PhantomData<(saucer_scheme_response, &'a ())>,
}

unsafe impl Send for Response<'_> {}
// !Sync as the stash is !Sync, and there are methods than can observe it via the pointer.

impl<'a> Drop for Response<'a> {
    fn drop(&mut self) { unsafe { saucer_scheme_response_free(self.ptr.as_ptr()) } }
}

impl<'a> Response<'a> {
    /// Creates a new response from the given [`Stash`] and MIME type. Also appends a `Content-Type`
    /// header with the specified MIME type.
    ///
    /// Although the stash handle is consumed, this method will always copy the stash, thus
    /// providing a large owned stash can be inefficient.
    pub fn new(data: Stash<'a>, mime: impl Into<Vec<u8>>) -> Self {
        // Stash is copied
        let ptr = use_string!(mime; unsafe {
           saucer_scheme_response_new(data.as_ptr(), mime)
        });

        Self { ptr: NonNull::new(ptr).expect("invalid response data"), _marker: PhantomData }
    }

    /// Sets the response status code.
    pub fn set_status(&mut self, status: i32) {
        unsafe { saucer_scheme_response_set_status(self.as_ptr(), status) }
    }

    /// Adds a header to the response.
    pub fn add_header(&mut self, name: impl Into<Vec<u8>>, value: impl Into<Vec<u8>>) {
        use_string!(name, value; unsafe {
           saucer_scheme_response_append_header(self.as_ptr(), name, value)
        });
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_scheme_response { self.ptr.as_ptr() }
}
