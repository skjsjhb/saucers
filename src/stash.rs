use std::ffi::c_void;
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
    /// When set to true, does not free the stash when this handle is dropped. This is required to transfer the inner
    /// stash object to the C API.
    leak: bool,
    _owns: PhantomData<(saucer_stash, &'a ())>
}

unsafe impl Send for Stash<'_> {}
unsafe impl Sync for Stash<'_> {}

impl Drop for Stash<'_> {
    fn drop(&mut self) {
        if !self.leak {
            unsafe { saucer_stash_free(self.ptr.as_ptr()) }
        }
    }
}

impl Stash<'_> {
    pub(crate) fn from_ptr(ptr: *mut saucer_stash) -> Self {
        Self {
            ptr: NonNull::new(ptr).expect("Invalid stash data"),
            leak: false,
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

    /// Returns the inner data.
    pub fn data(&self) -> &[u8] {
        // Each stash should have a non-null data pointer (empty stashes have empty vectors).
        // Borrowed stashes can only be created by user and all references should be nonnull in Rust.
        let ptr = unsafe { saucer_stash_data(self.ptr.as_ptr()) };
        unsafe { std::slice::from_raw_parts(ptr, self.size()) }
    }
}

impl Stash<'static> {
    /// Creates a new stash by copying the given data.
    pub fn copy(data: impl AsRef<[u8]>) -> Self {
        let r = data.as_ref();
        let ptr = unsafe { saucer_stash_from(r.as_ptr(), r.len()) };

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

impl<'a> AsRef<[u8]> for Stash<'a> {
    fn as_ref(&self) -> &[u8] { self.data() }
}

extern "C" fn stash_lazy_trampoline(arg: *mut c_void) -> *mut saucer_stash {
    let bb = unsafe { Box::from_raw(arg as *mut Box<dyn FnOnce() -> Stash<'static>>) };
    // The C library will free the stash object, so only the handle is dropped.
    let mut st = bb();
    st.leak = true;
    st.as_ptr()
}
