//! Webview navigation descriptor module.
//!
//! See [`WebviewNavigation`] for details.
use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::capi::*;
use crate::util::take_str;

/// Contains details about a navigation action.
pub struct WebviewNavigation {
    ptr: NonNull<saucer_navigation>,
    _owns: PhantomData<saucer_navigation>
}

unsafe impl Send for WebviewNavigation {}

unsafe impl Sync for WebviewNavigation {}

impl Drop for WebviewNavigation {
    fn drop(&mut self) { unsafe { saucer_navigation_free(self.ptr.as_ptr()) } }
}

impl WebviewNavigation {
    /// SAFETY: This struct has no lifetime specifier and a navigation handle does not own its data. Instances created
    /// using this method must be dropped before the handle is invalidated.
    pub(crate) unsafe fn from_ptr(ptr: *mut saucer_navigation) -> Self {
        Self {
            ptr: NonNull::new(ptr).expect("Invalid navigation descriptor"),
            _owns: PhantomData
        }
    }

    /// Checks whether the navigation requests a new window to be created.
    pub fn is_new_window(&self) -> bool { unsafe { saucer_navigation_new_window(self.ptr.as_ptr()) } }

    /// Checks whether the navigation is initiated by a redirection.
    pub fn is_redirection(&self) -> bool { unsafe { saucer_navigation_redirection(self.ptr.as_ptr()) } }

    /// Checks whether the navigation is initiated by user actions.
    pub fn is_user_initiated(&self) -> bool { unsafe { saucer_navigation_user_initiated(self.ptr.as_ptr()) } }

    /// Gets the URL that's about to navigate to.
    pub fn url(&self) -> String { unsafe { take_str(saucer_navigation_url(self.ptr.as_ptr())).unwrap() } }
}
