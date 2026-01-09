use std::marker::PhantomData;
use std::ptr::NonNull;

use saucer_sys::*;

/// An immutable interface to interact with binary data.
///
/// A stash can own its data or borrow data defined elsewhere (as long as it outlives the stash
/// handle).
pub struct Stash<'a> {
    ptr: NonNull<saucer_stash>,
    /// When set to true, does not free the stash when this handle is dropped. This is required to
    /// transfer the inner stash object to the C API.
    leak: bool,
    _marker: PhantomData<(saucer_stash, &'a ())>,
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

impl Default for Stash<'_> {
    fn default() -> Self { Self::new_empty() }
}

impl Clone for Stash<'_> {
    fn clone(&self) -> Self {
        let ptr = unsafe { saucer_stash_copy(self.as_ptr()) };
        Self::from_ptr(ptr)
    }
}

impl Stash<'_> {
    pub(crate) fn from_ptr(ptr: *mut saucer_stash) -> Self {
        Self {
            ptr: NonNull::new(ptr).expect("invalid stash ptr"),
            leak: false,
            _marker: PhantomData,
        }
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_stash { self.ptr.as_ptr() }

    /// Creates an empty stash.
    pub fn new_empty() -> Self { Self::from_ptr(unsafe { saucer_stash_new_empty() }) }

    /// Creates a new stash by copying the given data.
    pub fn new_copy(data: impl AsRef<[u8]>) -> Self {
        let data = data.as_ref();
        // Data copied internally
        let ptr = unsafe { saucer_stash_new_from(data.as_ptr() as *mut u8, data.len()) };

        Self::from_ptr(ptr)
    }

    /// Creates a lazy-populated stash whose content is populated by evaluating the populator on
    /// demand.
    ///
    /// The provided populator is polled at most once when reading the stash and will drop itself
    /// after the invocation. However, a stash may be dropped without being read, and the content of
    /// the populator is then leaked. Given such limitation, unless the usage of the stash can be
    /// known in advance (like feeding a [`crate::scheme::Response`]), usage of this method is
    /// discouraged.
    ///
    /// Despite the above limitations, this method can be useful to create a stash that "carries"
    /// owned data, but without copying or the need of explicitly moving data with a borrowed stash.
    ///
    /// This method is disabled for now as it requires [`Self::data`] to take `&mut self`, breaks
    /// [`Sync`] and [`Clone`]. Also, the safety of passing a lazy stash into C APIs are not yet
    /// fully verified.
    #[doc(hidden)]
    #[cfg(false)]
    pub fn new_lazy(populator: impl FnOnce() -> Stash<'static> + Send + 'static) -> Self {
        // A lazy stash internally caches the value and will call the populator at most once.
        // However, it's uncertain when it will be called, thus Send + 'static. The returned stash
        // may also be used for unknown lifetime, thus 'static.
        let data = LazyCallbackData { callback: Box::new(populator) };
        let data = Box::into_raw(Box::new(data)) as *mut c_void;
        let ptr = unsafe { saucer_stash_new_lazy(Some(stash_lazy_tp), data) };

        Self::from_ptr(ptr)
    }

    /// Gets the size of the stash.
    pub fn size(&self) -> usize { unsafe { saucer_stash_size(self.ptr.as_ptr()) } }

    /// Returns the inner data.
    pub fn data(&self) -> &[u8] {
        // Each stash should have a non-null data pointer (empty stashes have empty vectors).
        // Borrowed stashes can only be created by user and all references should be nonnull in
        // Rust.
        let ptr = unsafe { saucer_stash_data(self.ptr.as_ptr()) };
        unsafe { std::slice::from_raw_parts(ptr, self.size()) }
    }
}

impl<'a> Stash<'a> {
    /// Creates a new stash that borrows the given data.
    pub fn new_view(data: &'a [u8]) -> Self {
        let ptr = unsafe { saucer_stash_new_view(data.as_ptr(), data.len()) };
        Self::from_ptr(ptr)
    }
}

impl AsRef<[u8]> for Stash<'_> {
    fn as_ref(&self) -> &[u8] { self.data() }
}

#[cfg(false)]
struct LazyCallbackData {
    callback: Box<dyn FnOnce() -> Stash<'static> + Send + 'static>,
}

#[cfg(false)]
extern "C" fn stash_lazy_tp(data: *mut std::ffi::c_void) -> *mut saucer_stash {
    let bb = unsafe { Box::from_raw(data as *mut LazyCallbackData) };
    // The C library will free the stash object, so only the handle is dropped.
    let mut st = (bb.callback)();
    st.leak = true;
    st.as_ptr()
}
