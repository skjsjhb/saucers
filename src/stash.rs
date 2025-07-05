use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::capi::*;

pub struct Stash<'a> {
    ptr: NonNull<saucer_stash>,
    _owns: PhantomData<(saucer_stash, &'a ())>
}

unsafe impl Send for Stash<'_> {}
unsafe impl Sync for Stash<'_> {}

impl Drop for Stash<'_> {
    fn drop(&mut self) { unsafe { saucer_stash_free(self.ptr.as_ptr()) } }
}

impl Stash<'_> {
    pub(crate) fn from_ptr(ptr: *mut saucer_stash) -> Self {
        Self {
            ptr: NonNull::new(ptr).expect("Invalid stash data"),
            _owns: PhantomData
        }
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_stash { self.ptr.as_ptr() }
}

impl<'a> Stash<'a> {
    pub fn view(data: impl AsRef<[u8]> + 'a) -> Self {
        let ptr = unsafe {
            let data = data.as_ref();
            saucer_stash_view(data.as_ptr(), data.len())
        };

        Self::from_ptr(ptr)
    }

    pub fn size(&self) -> usize { unsafe { saucer_stash_size(self.ptr.as_ptr()) } }

    pub fn data(&self) -> Option<&[u8]> {
        let ptr = unsafe { saucer_stash_data(self.ptr.as_ptr()) };

        if ptr.is_null() {
            return None;
        }

        let dat = unsafe { std::slice::from_raw_parts(ptr, self.size()) };

        Some(dat)
    }
}

impl Stash<'static> {
    pub fn take(data: Vec<u8>) -> Self {
        let (buf, len, _) = data.into_raw_parts();
        let ptr = unsafe { saucer_stash_from(buf, len) };

        Self::from_ptr(ptr)
    }
}
