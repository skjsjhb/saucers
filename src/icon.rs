//! Native icon module.
//!
//! See [`Icon`] for details.
use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::ptr::null_mut;

use crate::capi::*;
use crate::stash::Stash;

/// A handler for native icon data. This handle can be used to exchange icon data with webview.
pub struct Icon {
    ptr: NonNull<saucer_icon>,
    _owns: PhantomData<saucer_icon>
}

unsafe impl Send for Icon {}
unsafe impl Sync for Icon {}

impl Drop for Icon {
    fn drop(&mut self) { unsafe { saucer_icon_free(self.ptr.as_ptr()) } }
}

impl Icon {
    pub(crate) fn from_ptr(ptr: *mut saucer_icon) -> Self {
        Self {
            ptr: NonNull::new(ptr).expect("Invalid icon data"),
            _owns: PhantomData
        }
    }

    /// Loads an icon from the given file.
    pub fn from_file(fp: impl AsRef<str>) -> Option<Icon> {
        let mut ptr: *mut saucer_icon = null_mut();
        let cst = CString::new(fp.as_ref()).unwrap();
        unsafe {
            saucer_icon_from_file(&mut ptr as *mut *mut saucer_icon, cst.as_ptr());
        }
        if ptr.is_null() { None } else { Some(Icon::from_ptr(ptr)) }
    }

    /// Loads an icon from the given [`Stash`].
    pub fn from_data(stash: &Stash<'_>) -> Option<Icon> {
        let mut ptr: *mut saucer_icon = null_mut();
        unsafe {
            // Data copied internally in C
            saucer_icon_from_data(&mut ptr as *mut *mut saucer_icon, stash.as_ptr());
        }
        if ptr.is_null() { None } else { Some(Icon::from_ptr(ptr)) }
    }

    /// Checks whether the icon is empty.
    pub fn is_empty(&self) -> bool { unsafe { saucer_icon_empty(self.ptr.as_ptr()) } }

    /// Copies and returns the icon content.
    pub fn data(&self) -> Stash<'static> {
        let ptr = unsafe { saucer_icon_data(self.ptr.as_ptr()) };

        // Icon data is copied before returned
        Stash::from_ptr(ptr)
    }

    /// Saves the icon to the specified file.
    pub fn save(&self, fp: impl AsRef<str>) {
        let cst = CString::new(fp.as_ref()).unwrap();
        unsafe { saucer_icon_save(self.ptr.as_ptr(), cst.as_ptr()) }
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_icon { self.ptr.as_ptr() }
}
