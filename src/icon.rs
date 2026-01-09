//! Native icon module.
//!
//! See [`Icon`] for details.
use std::marker::PhantomData;
use std::ptr::NonNull;

use saucer_sys::*;

use crate::macros::use_string;
use crate::stash::Stash;

/// A native icon.
pub struct Icon {
    ptr: NonNull<saucer_icon>,
    _marker: PhantomData<saucer_icon>,
}

unsafe impl Send for Icon {}
unsafe impl Sync for Icon {}

impl Drop for Icon {
    fn drop(&mut self) { unsafe { saucer_icon_free(self.ptr.as_ptr()) } }
}

impl Clone for Icon {
    fn clone(&self) -> Self {
        let ptr = unsafe { saucer_icon_copy(self.as_ptr()) };
        Self {
            ptr: NonNull::new(ptr).expect("copied icon should be non-null"),
            _marker: PhantomData,
        }
    }
}

impl AsRef<Icon> for Icon {
    fn as_ref(&self) -> &Icon { self }
}

impl Icon {
    pub(crate) unsafe fn from_ptr(ptr: *mut saucer_icon) -> Self {
        let ptr = NonNull::new(ptr).expect("icon ptr should be non-null");
        Self { ptr, _marker: PhantomData }
    }

    /// Loads an icon from the given file.
    pub fn from_file(fp: impl Into<Vec<u8>>) -> crate::error::Result<Self> {
        let mut ex = -1;
        let ptr = use_string!(
            fp: fp;
            unsafe { saucer_icon_new_from_file(fp, &raw mut ex) }
        );

        let ptr = NonNull::new(ptr).ok_or(crate::error::Error::Saucer(ex))?;

        Ok(Self { ptr, _marker: PhantomData })
    }

    /// Loads an icon from the given [`Stash`].
    pub fn from_data<'a>(stash: impl AsRef<Stash<'a>>) -> crate::error::Result<Self> {
        let mut ex = -1;
        let ptr = unsafe {
            // The stash is read immediately. If it's lazy, then it's polled on the same thread,
            // which won't invalidate references in the Rust world. Stashes are !Sync, thus we can
            // take a ref here.
            saucer_icon_new_from_stash(stash.as_ref().as_ptr(), &raw mut ex)
        };

        let ptr = NonNull::new(ptr).ok_or(crate::error::Error::Saucer(ex))?;
        Ok(Self { ptr, _marker: PhantomData })
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
    pub fn save(&self, fp: impl Into<Vec<u8>>) {
        use_string!(fp: fp; unsafe { saucer_icon_save(self.as_ptr(), fp) })
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_icon { self.ptr.as_ptr() }
}
