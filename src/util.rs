use std::ffi::c_char;
use std::ffi::CStr;

/// Copies the given C string into an owned [`String`]. Performs lossy UTF-8 conversion if needed.
///
/// SAFETY: See [`CStr::from_ptr`].
pub(crate) unsafe fn make_owned_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        "".to_owned()
    } else {
        unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
    }
}

/// Loads null-split string array from the given source.
pub(crate) fn inflate_strings(mut src: &[u8]) -> Vec<String> {
    if src.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();

    loop {
        let Ok(f) = CStr::from_bytes_until_nul(src) else {
            break;
        };

        out.push(f.to_string_lossy().into_owned());
        let bc = f.count_bytes() + 1;

        src = match src.get(bc..) {
            Some(s) => s,
            None => break,
        };
    }

    out
}
