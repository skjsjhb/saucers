use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::capi::*;

pub struct Script {
    ptr: NonNull<saucer_script>,
    _owns: PhantomData<saucer_script>
}

pub enum ScriptLoadTime {
    Creation,
    Ready
}

pub enum ScriptWebFrame {
    Top,
    All
}

unsafe impl Send for Script {}
unsafe impl Sync for Script {}

impl Drop for Script {
    fn drop(&mut self) { unsafe { saucer_script_free(self.ptr.as_ptr()) } }
}

impl Into<SAUCER_LOAD_TIME> for ScriptLoadTime {
    fn into(self) -> SAUCER_LOAD_TIME {
        match self {
            ScriptLoadTime::Creation => SAUCER_LOAD_TIME_SAUCER_LOAD_TIME_CREATION,
            ScriptLoadTime::Ready => SAUCER_LOAD_TIME_SAUCER_LOAD_TIME_READY
        }
    }
}

impl Into<SAUCER_WEB_FRAME> for ScriptWebFrame {
    fn into(self) -> SAUCER_WEB_FRAME {
        match self {
            ScriptWebFrame::Top => SAUCER_WEB_FRAME_SAUCER_WEB_FRAME_TOP,
            ScriptWebFrame::All => SAUCER_WEB_FRAME_SAUCER_WEB_FRAME_ALL
        }
    }
}

impl Script {
    pub fn new(code: impl AsRef<str>, time: ScriptLoadTime) -> Self {
        let cst = CString::new(code.as_ref()).unwrap();
        let ptr = unsafe { saucer_script_new(cst.as_ptr(), time.into()) };
        Self {
            ptr: NonNull::new(ptr).unwrap(),
            _owns: PhantomData
        }
    }

    pub fn set_frame(&mut self, frame: ScriptWebFrame) {
        unsafe { saucer_script_set_frame(self.ptr.as_ptr(), frame.into()) }
    }

    pub fn set_time(&mut self, time: ScriptLoadTime) {
        unsafe { saucer_script_set_time(self.ptr.as_ptr(), time.into()) }
    }

    pub fn set_permanent(&mut self, permanent: bool) {
        unsafe { saucer_script_set_permanent(self.ptr.as_ptr(), permanent) }
    }

    pub fn set_code(&mut self, code: impl AsRef<str>) {
        let cst = CString::new(code.as_ref()).unwrap();
        unsafe { saucer_script_set_code(self.ptr.as_ptr(), cst.as_ptr()) }
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_script { self.ptr.as_ptr() }
}
