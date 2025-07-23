use std::ffi::CStr;
use std::ffi::c_char;
use std::ffi::c_void;

use crate::capi::*;

pub(crate) unsafe fn take_str(ptr: *mut c_char) -> Option<String> {
    if ptr.is_null() {
        None
    } else {
        let s = unsafe { shot_str(ptr) };
        unsafe { saucer_memory_free(ptr as *mut c_void) }
        s
    }
}

pub(crate) unsafe fn shot_str(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        None
    } else {
        Some(
            unsafe { CStr::from_ptr(ptr) }
                .to_str()
                .expect("Invalid UTF-8 string")
                .to_string()
        )
    }
}
