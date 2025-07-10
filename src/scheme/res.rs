use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::capi::*;
use crate::rtoc;
use crate::stash::Stash;

/// Contains response details to reply a request to a custom scheme.
///
/// The lifetime of a response is tied to the [`Stash`] it's created from. When the stash owns its data, the data is
/// copied to the response, leading to a static lifetime. When the stash borrows the data, the lifetime of this response
/// can't be longer than that borrowed data. Note that a response can outlive its stash since stashes are copied
/// internally, as long as it does not outlive the lifetime of the data referenced by that stash.
pub struct Response<'a> {
    ptr: NonNull<saucer_scheme_response>,
    _owns: PhantomData<(saucer_scheme_response, &'a ())>
}

unsafe impl Send for Response<'_> {}
unsafe impl Sync for Response<'_> {}

impl<'a> Drop for Response<'a> {
    fn drop(&mut self) { unsafe { saucer_scheme_response_free(self.ptr.as_ptr()) } }
}

impl<'a> Response<'a> {
    /// Creates a new response using the given [`Stash`] and MIME type. The stash can be dropped immediately after
    /// return, as the C API copies its content during construction. However, if the stash borrows the data, the
    /// response can't outlive the lifetime span that the stash borrows.
    ///
    /// As the given stash is always copied, it can be inefficient to use an owning stash as the data will be copied
    /// twice when sending the response. For large payloads, try to use a borrowing stash if possible.
    pub fn new(data: &Stash<'a>, mime: impl AsRef<str>) -> Self {
        // The C library copies the stash, so it's safe to drop the stash after this call, as long as the data inside
        // the stash lives at least as long as the given lifetime.
        let ptr = rtoc!(mime => s; saucer_scheme_response_new(data.as_ptr(), s.as_ptr()));

        Self {
            ptr: NonNull::new(ptr).expect("Invalid response data"),
            _owns: PhantomData
        }
    }

    /// Sets the response status code.
    pub fn set_status(&self, status: i32) { unsafe { saucer_scheme_response_set_status(self.ptr.as_ptr(), status) } }

    /// Adds a header to the response.
    pub fn set_header(&self, name: impl AsRef<str>, value: impl AsRef<str>) {
        rtoc!(name => k, value => v; saucer_scheme_response_add_header(self.ptr.as_ptr(), k.as_ptr(), v.as_ptr()));
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_scheme_response { self.ptr.as_ptr() }
}
