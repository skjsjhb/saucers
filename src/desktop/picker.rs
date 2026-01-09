use std::ffi::c_char;
use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr::NonNull;

use saucer_sys::*;

use crate::macros::use_string;

/// Options for picking files or folders.
pub struct PickerOptions {
    ptr: NonNull<saucer_picker_options>,
    _marker: PhantomData<saucer_picker_options>,
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
            ptr: NonNull::new(ptr).expect("picker options should be created"),
            _marker: PhantomData,
        }
    }

    /// Sets the initial path displayed in the picker.
    pub fn set_initial(&mut self, path: impl Into<Vec<u8>>) {
        use_string!(path; unsafe { saucer_picker_options_set_initial(self.as_ptr(), path) });
    }

    /// Sets the filters applied to the picker.
    pub fn set_filters(&mut self, filters: impl IntoIterator<Item = impl Into<Vec<u8>>>) {
        let mut out = Vec::new();

        for f in filters {
            let st = CString::new(f).expect("FFI strings should not contain zeros");
            out.extend_from_slice(st.as_bytes());
            out.push(0);
        }

        unsafe {
            saucer_picker_options_set_filters(
                self.as_ptr(),
                out.as_ptr() as *const c_char,
                out.len(),
            )
        };
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_picker_options { self.ptr.as_ptr() }
}

impl Default for PickerOptions {
    fn default() -> Self { Self::new() }
}
