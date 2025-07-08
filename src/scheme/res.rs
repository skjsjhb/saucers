use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::capi::*;
use crate::rtoc;
use crate::stash::Stash;

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
    pub fn new(data: &Stash<'a>, mime: impl AsRef<str>) -> Self {
        // The C library copies the stash, so it's safe to drop the stash after this call, as long as the data inside
        // the stash lives at least as long as the given lifetime.
        let ptr = rtoc!(mime => s; saucer_scheme_response_new(data.as_ptr(), s.as_ptr()));

        Self {
            ptr: NonNull::new(ptr).expect("Invalid response data"),
            _owns: PhantomData
        }
    }

    pub fn set_status(&self, status: i32) { unsafe { saucer_scheme_response_set_status(self.ptr.as_ptr(), status) } }

    pub fn set_header(&self, name: impl AsRef<str>, value: impl AsRef<str>) {
        rtoc!(name => k, value => v; saucer_scheme_response_add_header(self.ptr.as_ptr(), k.as_ptr(), v.as_ptr()));
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_scheme_response { self.ptr.as_ptr() }
}
