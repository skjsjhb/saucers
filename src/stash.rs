use std::ffi::c_void;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
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

    /// Creates a lazy-populated stash whose content is populated by evaluating the populator on demand.
    ///
    /// The provided populator is polled at most once when reading the stash. It also drops everything it owns when
    /// being called. However, a stash may be dropped without being read, and the content of the populator is leaked if
    /// this happens. This method also makes no attempt to claim these leaked resources. Given such limitation, unless
    /// the usage of the stash can be known for certain in advance (like feeding a [`crate::scheme::Response`]), usage
    /// of this method is discouraged.
    ///
    /// Despite the above limitation, this method can be useful to create an owning stash with zero-cost copying (only
    /// the future is copied in the C++ library), eliminating the need of explicitly moving data together with a
    /// borrowed stash.
    pub fn lazy(populator: impl FnOnce() -> Stash<'static> + Send + 'static) -> Self {
        // A lazy stash internally maintains a future and polls the populator at most once.
        // The populator may be moved to another thread, but it won't be shared as only the future object in C++ is
        // copied when copying the stash.
        let bb = Box::new(populator) as Box<dyn FnOnce() -> Stash<'static>>;
        let arg = Box::into_raw(Box::new(bb));
        let ptr = unsafe { saucer_stash_lazy_with_arg(Some(stash_lazy_trampoline), arg as *mut c_void) };

        Self::from_ptr(ptr)
    }
}

extern "C" fn stash_lazy_trampoline(arg: *mut c_void) -> *mut saucer_stash {
    let bb = unsafe { Box::from_raw(arg as *mut Box<dyn FnOnce() -> Stash<'static>>) };
    // The C library will take care of freeing the returned stash, thus the drop method must not be run
    let st = ManuallyDrop::new(bb());
    // Both the stash and its data are taken by the C library, no free needed
    st.as_ptr()
}
