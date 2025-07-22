macro_rules! rtoc {
    ($($arg:expr => $ptr:ident),+ ; $ex: expr) => {{
        use std::ffi::*;
        $(let $ptr = CString::new($arg.as_ref()).unwrap();)+
        unsafe { $ex }
    }};
}

pub(crate) use rtoc;
