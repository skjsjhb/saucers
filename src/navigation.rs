//! Webview navigation descriptor module.
//!
//! See [`Navigation`] for details.
use std::marker::PhantomData;
use std::ptr::NonNull;

use saucer_sys::*;

use crate::url::Url;

/// A navigation descriptor.
pub struct Navigation<'a> {
    ptr: NonNull<saucer_navigation>,
    _marker: PhantomData<&'a saucer_navigation>,
}

unsafe impl Send for Navigation<'_> {}
unsafe impl Sync for Navigation<'_> {}

impl Navigation<'_> {
    /// SAFETY: The provided pointer must outlive the returned struct.
    pub(crate) unsafe fn from_ptr(ptr: *mut saucer_navigation) -> Self {
        Self {
            ptr: NonNull::new(ptr).expect("invalid navigation descriptor"),
            _marker: PhantomData,
        }
    }

    /// Checks whether the navigation requests a new window to be created.
    pub fn is_new_window(&self) -> bool {
        unsafe { saucer_navigation_new_window(self.ptr.as_ptr()) }
    }

    /// Checks whether the navigation is initiated by a redirection.
    pub fn is_redirection(&self) -> bool {
        unsafe { saucer_navigation_redirection(self.ptr.as_ptr()) }
    }

    /// Checks whether the navigation is initiated by user actions.
    pub fn is_user_initiated(&self) -> bool {
        unsafe { saucer_navigation_user_initiated(self.ptr.as_ptr()) }
    }

    /// Gets the URL that's about to navigate to.
    pub fn url(&self) -> Url {
        let ptr = unsafe { saucer_navigation_url(self.ptr.as_ptr()) };
        unsafe { Url::from_ptr(ptr, -1) }.expect("navigation URL should be present")
    }
}
