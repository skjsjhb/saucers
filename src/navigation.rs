use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::capi::*;
use crate::ctor;

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
    pub(crate) fn from_ptr(ptr: *mut saucer_navigation) -> Self {
        Self {
            ptr: NonNull::new(ptr).expect("Invalid navigation descriptor"),
            _owns: PhantomData
        }
    }

    pub fn is_new_window(&self) -> bool { unsafe { saucer_navigation_new_window(self.ptr.as_ptr()) } }

    pub fn is_redirection(&self) -> bool { unsafe { saucer_navigation_redirection(self.ptr.as_ptr()) } }

    pub fn is_user_initiated(&self) -> bool { unsafe { saucer_navigation_user_initiated(self.ptr.as_ptr()) } }

    pub fn url(&self) -> String { ctor!(free, saucer_navigation_url(self.ptr.as_ptr())) }
}
