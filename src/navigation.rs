use std::marker::PhantomData;
use std::ptr::NonNull;

use saucer_sys::*;

use crate::macros::ffi_forward;
use crate::url::Url;

/// A navigation descriptor.
///
/// Because the navigation descriptor interacts with underlying request object
/// to fetch fields like [`Self::url`], it can't be safely owned / shared / sent
/// and is only safe when being "confined" inside the event handler. It's
/// associated lifetime indicates the validity.
pub struct Navigation<'a> {
    ptr: NonNull<saucer_navigation>,
    _marker: PhantomData<&'a ()>,
}

// !Send + !Sync as it may call thread-unsafe methods

impl Navigation<'_> {
    ffi_forward! {
        /// Checks whether the navigation requests a new window to be created.
        pub fn is_new_window(&Self) -> bool => saucer_navigation_new_window;
        /// Checks whether the navigation is initiated by a redirection.
        pub fn is_redirection(&Self) -> bool => saucer_navigation_redirection;
        /// Checks whether the navigation is initiated by user actions.
        pub fn is_user_initiated(&Self) -> bool => saucer_navigation_user_initiated;
    }

    /// SAFETY: The provided pointer must outlive the returned struct.
    pub(crate) unsafe fn from_ptr(ptr: *mut saucer_navigation) -> Self {
        Self {
            ptr: NonNull::new(ptr).expect("invalid navigation descriptor"),
            _marker: PhantomData,
        }
    }

    /// Gets the URL that's about to navigate to.
    pub fn url(&self) -> Url {
        let ptr = unsafe { saucer_navigation_url(self.ptr.as_ptr()) };
        unsafe { Url::from_ptr(ptr, -1) }.expect("navigation URL should be present")
    }

    fn as_ptr(&self) -> *mut saucer_navigation { self.ptr.as_ptr() }
}
