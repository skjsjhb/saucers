use std::ffi::c_char;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::capi::*;
use crate::macros::ctor;
use crate::stash::Stash;

/// Contains request details of a request to a custom scheme.
pub struct Request {
    ptr: NonNull<saucer_scheme_request>,
    _owns: PhantomData<saucer_scheme_request>
}

unsafe impl Send for Request {}
unsafe impl Sync for Request {}

impl Drop for Request {
    fn drop(&mut self) { unsafe { saucer_scheme_request_free(self.ptr.as_ptr()) } }
}

impl Request {
    pub(crate) fn from_ptr(ptr: *mut saucer_scheme_request) -> Self {
        Self {
            ptr: NonNull::new(ptr).expect("Invalid scheme request data"),
            _owns: PhantomData
        }
    }

    /// Gets the request headers.
    ///
    /// A copy of the headers is created each time this method is called. Consider reusing the headers instead of
    /// calling this method repetitively.
    pub fn headers(&self) -> Vec<(String, String)> {
        let mut keys: *mut *mut c_char = std::ptr::null_mut();
        let mut values: *mut *mut c_char = std::ptr::null_mut();
        let mut count = 0usize;
        unsafe {
            saucer_scheme_request_headers(
                self.ptr.as_ptr(),
                &mut keys as *mut *mut *mut c_char,
                &mut values as *mut *mut *mut c_char,
                &mut count as *mut usize
            );
        }

        if keys.is_null() || values.is_null() {
            return Vec::new();
        }

        let mut headers = Vec::with_capacity(count);
        for i in 0..count {
            let k = ctor!(free, *keys.add(i));
            let v = ctor!(free, *values.add(i));
            headers.push((k, v));
        }

        unsafe {
            saucer_memory_free(keys as *mut c_void);
            saucer_memory_free(values as *mut c_void);
        }

        headers
    }

    /// Gets the request URL.
    pub fn url(&self) -> String { ctor!(free, saucer_scheme_request_url(self.ptr.as_ptr())) }

    /// Gets the request method.
    pub fn method(&self) -> String { ctor!(free, saucer_scheme_request_method(self.ptr.as_ptr())) }

    /// Gets the request content.
    ///
    /// A copy of the body is created each time this method is called. Consider reusing the body instead of calling this
    /// method repetitively.
    pub fn content(&self) -> Stash<'static> {
        let ptr = unsafe { saucer_scheme_request_content(self.ptr.as_ptr()) };
        Stash::from_ptr(ptr)
    }
}
