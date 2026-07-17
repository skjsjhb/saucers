use std::ffi::CStr;
use std::ffi::c_char;
use std::panic::UnwindSafe;
use std::panic::catch_unwind;

/// Copies the given C string into an owned [`String`]. Performs lossy UTF-8
/// conversion if needed.
///
/// SAFETY: See [`CStr::from_ptr`].
pub(crate) unsafe fn make_owned_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        "".to_owned()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

/// Loads null-split string array from the given source.
pub(crate) fn inflate_strings(mut src: &[u8]) -> Vec<String> {
    if src.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();

    while let Ok(f) = CStr::from_bytes_until_nul(src) {
        out.push(f.to_string_lossy().into_owned());
        let bc = f.count_bytes() + 1;

        src = match src.get(bc..) {
            Some(s) => s,
            None => break,
        };
    }

    out
}

/// Runs a Rust callback without allowing a panic to unwind across an FFI
/// boundary.
///
/// The panic payload is intentionally leaked because dropping an arbitrary
/// payload can itself panic. The panic hook still runs before the unwind is
/// caught.
pub(crate) fn ffi_callback<R>(fallback: R, callback: impl FnOnce() -> R + UnwindSafe) -> R {
    match catch_unwind(callback) {
        Ok(result) => result,
        Err(payload) => {
            std::mem::forget(payload);
            fallback
        }
    }
}

#[cfg(test)]
mod tests {
    use std::panic::panic_any;

    use super::ffi_callback;

    struct PanicOnDrop;

    impl Drop for PanicOnDrop {
        fn drop(&mut self) { panic!("panic payload was dropped") }
    }

    #[test]
    fn ffi_callback_contains_panics() {
        assert_eq!(ffi_callback(0, || 1), 1);
        assert_eq!(ffi_callback(0, || panic!("callback panicked")), 0);
        assert_eq!(ffi_callback(0, || panic_any(PanicOnDrop)), 0);
    }
}
