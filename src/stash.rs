use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::capi::*;

/// An immutable interface to interact with binary data.
///
/// A stash can either own its data, or borrows data defined elsewhere. An owning stash manages its data internally and
/// has a static lifetime. A borrowed stash, on the other hand, acts like a shared reference to the data with the
/// specified lifetime.
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
    /// Creates a new stash that borrows the given data.
    ///
    /// As stash may be used at any place, the data source must be [`Sync`] to prevent data racing. The stash borrows
    /// the data for its entire lifetime.
    pub fn view(data: &'a (impl AsRef<[u8]> + Sync + ?Sized)) -> Self {
        // A stash cannot be modified, but it may be read from other threads and should be `Sync`.
        let ptr = unsafe {
            let data = data.as_ref();
            saucer_stash_view(data.as_ptr(), data.len())
        };

        Self::from_ptr(ptr)
    }

    /// Gets the size of the stash.
    pub fn size(&self) -> usize { unsafe { saucer_stash_size(self.ptr.as_ptr()) } }

    /// Tries to borrow and return the inner data. If the stash is empty or corrupted, [`None`] is returned.
    pub fn data(&self) -> Option<&[u8]> {
        if self.size() == 0 {
            return None;
        }

        let ptr = unsafe { saucer_stash_data(self.ptr.as_ptr()) };

        if ptr.is_null() {
            return None;
        }

        let dat = unsafe { std::slice::from_raw_parts(ptr, self.size()) };

        Some(dat)
    }
}

impl Stash<'static> {
    /// Creates a new stash by taking the given data.
    ///
    /// The provided [`Vec`] is disassembled and moved to the C API. It will no longer be managed by Rust after being
    /// moved. As this operation involves allocating and freeing data using different allocators, the
    /// [`core::alloc::Allocator`] of the [`Vec`] must be compatible with the system allocator.
    pub fn take(data: Vec<u8>) -> Self {
        let (buf, len, _) = data.into_raw_parts();
        let ptr = unsafe { saucer_stash_from(buf, len) };

        Self::from_ptr(ptr)
    }
}
