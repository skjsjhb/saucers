use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::capi::*;
use crate::macros::rtoc;

/// Options for picking files or folders.
pub struct PickerOptions {
    ptr: NonNull<saucer_picker_options>,
    _owns: PhantomData<saucer_picker_options>
}

unsafe impl Send for PickerOptions {}
unsafe impl Sync for PickerOptions {}

impl Drop for PickerOptions {
    fn drop(&mut self) {
        unsafe {
            saucer_picker_options_free(self.ptr.as_ptr());
        }
    }
}

impl PickerOptions {
    /// Creates picker options with default settings.
    pub fn new() -> Self {
        let ptr = unsafe { saucer_picker_options_new() };
        Self {
            ptr: NonNull::new(ptr).expect("Failed to create picker options"),
            _owns: PhantomData
        }
    }

    /// Sets the initial path displayed in the picker.
    pub fn set_initial(&mut self, path: impl AsRef<str>) {
        rtoc!( path => p; saucer_picker_options_set_initial(self.ptr.as_ptr(), p.as_ptr()) );
    }

    /// Adds a filter to the picker.
    pub fn add_filter(&mut self, filter: impl AsRef<str>) {
        rtoc!( filter => f; saucer_picker_options_add_filter(self.ptr.as_ptr(), f.as_ptr()) );
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_picker_options { self.ptr.as_ptr() }
}

impl Default for PickerOptions {
    fn default() -> Self { Self::new() }
}
