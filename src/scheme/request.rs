use std::ffi::c_char;
use std::marker::PhantomData;
use std::ptr::NonNull;

use saucer_sys::*;

use crate::macros::load_range;
use crate::stash::Stash;
use crate::url::Url;
use crate::util::inflate_strings;

/// Contains request details of a request to a custom scheme.
pub struct Request {
    ptr: NonNull<saucer_scheme_request>,
    _marker: PhantomData<saucer_scheme_request>,
}

// !Send + !Sync as we can't guarantee the safety of reading headers & body.

impl Drop for Request {
    fn drop(&mut self) { unsafe { saucer_scheme_request_free(self.ptr.as_ptr()) } }
}

impl Request {
    /// SAFETY: The pointer must be valid, and the returned handle must be dropped before leaving
    /// the request callback.
    pub(crate) unsafe fn from_ptr(ptr: *mut saucer_scheme_request) -> Self {
        Self { ptr: NonNull::new(ptr).expect("invalid scheme request ptr"), _marker: PhantomData }
    }

    /// Gets the request headers.
    ///
    /// A copy of the headers is created each time this method is called. Consider reusing the
    /// headers instead of calling this method repetitively.
    pub fn headers(&self) -> Vec<(String, String)> {
        let mut buf = load_range!(ptr[size] = 0u8; {
            unsafe { saucer_scheme_request_headers(self.ptr.as_ptr(), ptr as *mut c_char, size) }
        });

        buf.push(0);

        let entries = inflate_strings(&buf);

        entries
            .into_iter()
            .filter_map(|s| s.split_once(":").map(|(k, v)| (k.to_owned(), v.to_owned())))
            .collect()
    }

    /// Gets the request URL.
    pub fn url(&self) -> Url {
        let ptr = unsafe { saucer_scheme_request_url(self.ptr.as_ptr()) };
        unsafe { Url::from_ptr(ptr, -1) }.expect("request URL should be present")
    }

    /// Gets the request method.
    pub fn method(&self) -> String {
        let buf = load_range!(ptr[size] = 0u8; {
           unsafe { saucer_scheme_request_method(self.ptr.as_ptr(), ptr as *mut c_char, size) }
        });

        String::from_utf8_lossy(&buf).into_owned()
    }

    /// Gets the request content.
    ///
    /// A copy of the body is created each time this method is called. Consider reusing the body
    /// instead of calling this method repetitively.
    pub fn content(&self) -> Stash<'static> {
        let ptr = unsafe { saucer_scheme_request_content(self.ptr.as_ptr()) };
        Stash::from_ptr(ptr)
    }
}
