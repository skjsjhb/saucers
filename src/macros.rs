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

/// Forwards Rust calls to C FFI functions with `self` pointer as receiver.
macro_rules! ffi_forward {
    () => {};

    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident($self_ty:ty $(, $arg:ident: $ty:ty)*) $(-> $ret:ty)? => $ffi:path;
        $($rest:tt)*
    ) => {
        $(#[$meta])*
        $vis fn $name(self: $self_ty $(, $arg: $ty)*) $(-> $ret)? {
            unsafe { $ffi(self.as_ptr(), $($arg),*) }
        }

        ffi_forward! { $($rest)* }
    };
}

pub(crate) use ffi_forward;
pub(crate) use load_range;
pub(crate) use use_string;
