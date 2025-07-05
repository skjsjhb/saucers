#[macro_export]
macro_rules! ctor {
    (free, $ptr:expr) => {{
        ctor!($ptr, { saucer_memory_free($ptr as *mut c_void); })
    }};

    ($ptr:expr) => {{
        ctor!($ptr, {})
    }};

    ($ptr:expr, $drop:tt) => {{
        use std::ffi::*;
        unsafe {
            if $ptr.is_null() {
                "".to_owned()
            } else {
                let st = CStr::from_ptr($ptr).to_str().expect("Invalid UTF-8 string").to_owned();
                $drop
                st
            }
        }
    }};
}

#[macro_export]
macro_rules! rtoc {
    ($($arg:expr => $ptr:ident),+ ; $ex: expr) => {{
        use std::ffi::*;
        $(let $ptr = CString::new($arg.as_ref()).unwrap();)+
        unsafe { $ex }
    }};
}
