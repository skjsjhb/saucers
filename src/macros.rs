/// Wraps some strings as C strings and gives pointers for using them.
macro_rules! use_string {
    ($($ptr:ident : $arg:expr),+ ; $ex: expr) => {{
        use std::ffi::CString;

        match ($(CString::new($arg).expect("FFI strings should not contain zeros"),)+) {
            ($($ptr,)+) => match ($($ptr.as_ptr(),)+) {
                ($($ptr,)+) => $ex
            }
        }
    }};

    ($($ptr:ident),+ ; $ex: expr) => {{
        use_string!($($ptr : $ptr),+ ; $ex)
    }};
}

/// Loads a range using saucer-defined two-step invocation.
macro_rules! load_range {
    ($ptr:ident[$size:ident] = $default_value:expr; $ex:tt) => {{
        let mut size = 0;
        let $size = &raw mut size;
        let $ptr = std::ptr::null_mut();
        $ex;
        if size == 0 {
            Vec::new()
        } else {
            let mut data = vec![$default_value; size];
            let $ptr = data.as_mut_ptr();
            $ex;
            data
        }
    }};
}

pub(crate) use load_range;
pub(crate) use use_string;
