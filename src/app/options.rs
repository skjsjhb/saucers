//! App options module.
//!
//! See [`AppOptions`] for details.
use std::ffi::c_char;
use std::ffi::c_int;
use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr::NonNull;

use saucer_sys::*;

use crate::macros::use_string;

/// Options for the application.
#[derive(Default)]
pub struct AppOptions {
    pub id: String,
    pub args: Vec<String>,
    pub quit_on_last_window_closed: bool,
}

impl AppOptions {
    /// Constructs options with its parts.
    pub fn new(id: String, args: Vec<String>, quit_on_last_window_closed: bool) -> Self {
        Self { id, args, quit_on_last_window_closed }
    }

    /// Constructs options with ID, leaving other fields as default.
    pub fn new_with_id(id: impl Into<String>) -> Self { Self::new(id.into(), Vec::new(), true) }

    /// Makes this options inherit [`std::env::args`] as its args.
    pub fn inherit_args(&mut self) { self.args = std::env::args().collect(); }
}

/// Helper struct for managing raw pointers to app options and args that must be kept valid.
pub(crate) struct RawAppOptions {
    inner: NonNull<saucer_application_options>,
    args: Vec<*mut c_char>,
    _marker: PhantomData<saucer_application_options>,
}

impl Drop for RawAppOptions {
    fn drop(&mut self) {
        // Claim the raw strings
        for pt in &self.args {
            // SAFETY: The pointers were created from Rust strings and won't be mutated.
            // It's declared as mutable, but aren't actually mutated in impls.
            let _ = unsafe { CString::from_raw(*pt) };
        }

        unsafe { saucer_application_options_free(self.inner.as_ptr()) };
    }
}

impl RawAppOptions {
    pub(crate) fn new(opt: AppOptions) -> Self {
        let inner = use_string!(
            id: opt.id;
            unsafe { saucer_application_options_new(id) }
        );

        let inner = NonNull::new(inner).unwrap();

        let argc = opt.args.len() as c_int;

        let mut args: Vec<*mut c_char> = opt
            .args
            .into_iter()
            .map(|s| CString::new(s).expect("FFI strings should not contain zeros").into_raw())
            .collect();

        let argv = args.as_mut_ptr();

        unsafe {
            saucer_application_options_set_argc(inner.as_ptr(), argc);
            // SAFETY: The provided argv will be kept valid until the app quits.
            saucer_application_options_set_argv(inner.as_ptr(), argv);
            saucer_application_options_set_quit_on_last_window_closed(
                inner.as_ptr(),
                opt.quit_on_last_window_closed,
            );
        }

        Self { inner, args, _marker: PhantomData }
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_application_options { self.inner.as_ptr() }
}
