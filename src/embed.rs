use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::capi::*;
use crate::stash::Stash;

pub struct EmbedFile<'a> {
    ptr: NonNull<saucer_embedded_file>,
    _owns: PhantomData<(saucer_embedded_file, &'a ())>
}

unsafe impl Send for EmbedFile<'_> {}
unsafe impl Sync for EmbedFile<'_> {}

impl Drop for EmbedFile<'_> {
    fn drop(&mut self) { unsafe { saucer_embed_free(self.ptr.as_ptr()) } }
}

impl<'a> EmbedFile<'a> {
    pub fn new(content: &Stash<'a>, mime: impl AsRef<str>) -> Self {
        let cst = CString::new(mime.as_ref()).unwrap();
        let ptr = unsafe { saucer_embed(content.as_ptr(), cst.as_ptr()) };

        Self {
            ptr: NonNull::new(ptr).expect("Invalid embedded file"),
            _owns: PhantomData
        }
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_embedded_file { self.ptr.as_ptr() }
}
