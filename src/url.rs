use std::ffi::c_char;
use std::ffi::CString;
use std::fmt::Display;
use std::fmt::Formatter;
use std::marker::PhantomData;
use std::ptr::null_mut;
use std::ptr::NonNull;
use std::str::FromStr;

use saucer_sys::*;

use crate::macros::load_range;
use crate::macros::use_string;

/// A URL handle backed by an underlying native URL object.
pub struct Url {
    inner: NonNull<saucer_url>,
    _marker: PhantomData<saucer_url>,
}

unsafe impl Send for Url {}
unsafe impl Sync for Url {}

impl Drop for Url {
    fn drop(&mut self) { unsafe { saucer_url_free(self.inner.as_ptr()) } }
}

impl AsRef<Url> for Url {
    fn as_ref(&self) -> &Url { self }
}

impl Display for Url {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.content()) }
}

impl FromStr for Url {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> { Self::new_parse(s) }
}

impl Url {
    pub(crate) unsafe fn from_ptr(ptr: *mut saucer_url, ex: i32) -> crate::error::Result<Self> {
        if let Some(ptr) = NonNull::new(ptr) {
            Ok(Self { inner: ptr, _marker: PhantomData })
        } else {
            Err(crate::error::Error::Saucer(ex))
        }
    }

    /// Constructs a new URL from its parts.
    pub fn new(
        scheme: impl Into<Vec<u8>>,
        host: Option<impl Into<Vec<u8>>>,
        port: Option<usize>,
        path: impl Into<Vec<u8>>,
    ) -> Self {
        let port = match port.as_ref() {
            Some(p) => &raw const *p as *mut usize,
            None => null_mut(),
        };

        let host = host.map(|h| CString::new(h).expect("FFI strings should not contain zeros"));

        let host = match &host {
            Some(st) => st.as_ptr() as *mut c_char,
            None => null_mut(),
        };

        let ptr = use_string!(scheme, path; unsafe {
            saucer_url_new_opts(scheme, host, port, path)
        });

        Self { inner: NonNull::new(ptr).expect("invalid URL ptr"), _marker: PhantomData }
    }

    /// Creates a file URL using the given path.
    pub fn new_file(fp: impl Into<Vec<u8>>) -> crate::error::Result<Self> {
        let mut ex = -1;

        let ptr = use_string!(fp; unsafe {
           saucer_url_new_from(fp, &raw mut ex)
        });

        unsafe { Self::from_ptr(ptr, ex) }
    }

    /// Parses the given URL string.
    pub fn new_parse(url: impl Into<Vec<u8>>) -> crate::error::Result<Self> {
        let mut ex = -1;

        let ptr = use_string!(url; unsafe {
           saucer_url_new_parse(url, &raw mut ex)
        });

        unsafe { Self::from_ptr(ptr, ex) }
    }

    /// Gets the URL as a string.
    pub fn content(&self) -> String {
        let st = load_range!(ptr[size] = 0u8; {
            unsafe { saucer_url_string(self.as_ptr(), ptr as *mut c_char, size) };
        });

        String::from_utf8_lossy(&st).into_owned()
    }

    /// Gets the URL path.
    pub fn path(&self) -> String {
        let st = load_range!(ptr[size] = 0u8; {
            unsafe { saucer_url_path(self.as_ptr(), ptr as *mut c_char, size) };
        });

        String::from_utf8_lossy(&st).into_owned()
    }

    /// Gets the URL scheme.
    pub fn scheme(&self) -> String {
        let st = load_range!(ptr[size] = 0u8; {
            unsafe { saucer_url_scheme(self.as_ptr(), ptr as *mut c_char, size) };
        });

        String::from_utf8_lossy(&st).into_owned()
    }

    /// Gets the URL host. An empty string is returned if the URL has no host.
    pub fn host(&self) -> String {
        let st = load_range!(ptr[size] = 0u8; {
            unsafe { saucer_url_host(self.as_ptr(), ptr as *mut c_char, size) };
        });

        String::from_utf8_lossy(&st).into_owned()
    }

    /// Gets the URL user. An empty string is returned if the URL has no user.
    pub fn user(&self) -> String {
        let st = load_range!(ptr[size] = 0u8; {
            unsafe { saucer_url_user(self.as_ptr(), ptr as *mut c_char, size) };
        });

        String::from_utf8_lossy(&st).into_owned()
    }

    /// Gets the URL password. An empty string is returned if the URL has no password.
    pub fn password(&self) -> String {
        let st = load_range!(ptr[size] = 0u8; {
            unsafe { saucer_url_password(self.as_ptr(), ptr as *mut c_char, size) };
        });

        String::from_utf8_lossy(&st).into_owned()
    }

    /// Gets the URL port.
    pub fn port(&self) -> Option<usize> {
        let mut port = 0;
        let ok = unsafe { saucer_url_port(self.as_ptr(), &raw mut port) };
        ok.then_some(port)
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_url { self.inner.as_ptr() }
}
