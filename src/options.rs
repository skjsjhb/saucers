use std::ffi::CString;
use std::ffi::c_char;
use std::ffi::c_int;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::ptr::null_mut;

use crate::capi::*;

/// Options for the application.
pub struct AppOptions {
    inner: NonNull<saucer_options>,
    args: Option<(*mut *mut c_char, usize, usize)>,
    _owns: PhantomData<saucer_options>
}

unsafe impl Send for AppOptions {}
unsafe impl Sync for AppOptions {}

impl Drop for AppOptions {
    fn drop(&mut self) {
        unsafe {
            let this = self.inner.as_ptr();

            // The options object should no longer use the inner args, but just in case
            saucer_options_set_argc(this, 0);
            saucer_options_set_argv(this, null_mut());

            if let Some(ax) = self.args.take() {
                drop_raw_args(ax);
            }

            saucer_options_free(this);
        }
    }
}

impl AppOptions {
    /// Creates a new options pack with specified ID.
    pub fn new(id: &str) -> Self {
        let cst = CString::new(id).unwrap();

        let ptr = unsafe {
            // SAFETY: Value is freed when dropping
            saucer_options_new(cst.as_ptr()) // Value copied in C
        };
        Self {
            inner: NonNull::new(ptr).expect("Failed to create options"),
            args: None,
            _owns: PhantomData
        }
    }

    /// Sets arguments passed to the app.
    pub fn set_args(&mut self, args: impl IntoIterator<Item = impl AsRef<str>>) {
        let mut v: Vec<*mut c_char> = args
            .into_iter()
            .map(|a| CString::new(a.as_ref()).unwrap().into_raw())
            .collect();

        v.push(null_mut()); // Terminating nullptr

        let raw_vec = v.into_raw_parts();

        unsafe {
            let this = self.inner.as_ptr();

            saucer_options_set_argc(this, raw_vec.1 as c_int);

            // SAFETY: The string array is disassembled, remains unchanged and lives longer than the C ref
            saucer_options_set_argv(this, raw_vec.0); // Value borrowed in C
        }

        if let Some(ax) = self.args.replace(raw_vec) {
            drop_raw_args(ax);
        }
    }

    /// Sets number of threads used for async dispatching.
    pub fn set_threads(&mut self, th: usize) {
        unsafe {
            saucer_options_set_threads(self.inner.as_ptr(), th);
        }
    }

    /// Gets the inner pointer to the C value.
    ///
    /// SAFETY: The retrieved pointer must not be mutated when an `application` is using it.
    /// Options are currently only read in implementations, so it's safe to get multiple copies of the pointer and share
    /// them among `application`s (as for now), yet mutation must be exclusive.
    pub(crate) fn as_ptr(&mut self) -> *mut saucer_options { self.inner.as_ptr() }
}

/// Reassemble a vector containing raw pointers to [`CString`]s and drop them.
fn drop_raw_args((ptr, len, cap): (*mut *mut c_char, usize, usize)) {
    // SAFETY: The tuple was disassembled from a vector and has never been mutated
    let mut v = unsafe { Vec::from_raw_parts(ptr, len, cap) };

    v.pop(); // Drops the terminator

    for a in v {
        unsafe {
            drop(CString::from_raw(a));
        }
    }
}
