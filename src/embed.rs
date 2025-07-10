use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::capi::*;
use crate::stash::Stash;

/// Contains details for embedding certain content using [`crate::webview::Webview::embed_file`].
pub struct EmbedFile {
    ptr: NonNull<saucer_embedded_file>,
    _owns: PhantomData<saucer_embedded_file>
}

unsafe impl Send for EmbedFile {}
unsafe impl Sync for EmbedFile {}

impl Drop for EmbedFile {
    fn drop(&mut self) { unsafe { saucer_embed_free(self.ptr.as_ptr()) } }
}

impl EmbedFile {
    /// Creates a new embedded file using the given [`Stash`] and MIME type. The stash can be dropped immediately once
    /// this method returns.
    ///
    /// As embedded files can be used arbitrarily, it must be created from a static stash. However, the stash does not
    /// need to be an owning stash. One common usage of embedded files is, as its name suggests, serve data with static
    /// lifetime, like those embedded using [`include!`] or [`include_bytes!`].
    pub fn new(content: &Stash<'static>, mime: impl AsRef<str>) -> Self {
        let cst = CString::new(mime.as_ref()).unwrap();
        // The stash is copied internally, so it can be dropped after return, as long as the underlying data lives
        // for the given lifetime.
        let ptr = unsafe { saucer_embed(content.as_ptr(), cst.as_ptr()) };

        Self {
            ptr: NonNull::new(ptr).expect("Invalid embedded file"),
            _owns: PhantomData
        }
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_embedded_file { self.ptr.as_ptr() }
}
