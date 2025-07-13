#[macro_export]
macro_rules! ctor {
    (free, nullable, $ptr:expr) => {{
        unsafe {
            match $ptr {
                ptr => {
                    ctor!(ptr, { saucer_memory_free(ptr as *mut c_void); }, st, None, { Some(st) })
                }
            }
        }
    }};

    (nullable, $ptr:expr) => {{
        ctor!($ptr, {}, st, None, { Some(st) })
    }};

    (free, $ptr:expr) => {{
        unsafe {
            match $ptr {
                ptr => {
                    ctor!(ptr, { saucer_memory_free(ptr as *mut c_void); }, st, { "".to_owned() }, st)
                }
            }
        }
    }};

    ($ptr:expr) => {{
        ctor!($ptr, {}, st, { "".to_owned() }, st)
    }};

    ($ptr:expr, $drop:tt, $vn:ident, $ifn:tt, $ifv:tt) => {{
        use std::ffi::*;
        #[allow(unused_unsafe)]
        unsafe {
            match $ptr {
                ptr => {
                    if ptr.is_null() {
                        $ifn
                    } else {
                        let $vn = CStr::from_ptr(ptr).to_str().expect("Invalid UTF-8 string").to_owned();
                        $drop
                        $ifv
                    }
                }
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
