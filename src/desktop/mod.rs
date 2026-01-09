//! Desktop module.
//!
//! See [`Desktop`] for details.

mod picker;

use std::ffi::c_char;
use std::marker::PhantomData;
use std::ptr::NonNull;

pub use picker::*;
use saucer_sys::*;

use crate::app::App;
use crate::macros::load_range;
use crate::macros::use_string;
use crate::util::inflate_strings;

/// The desktop module providing file picking and URL opening.
pub struct Desktop {
    ptr: NonNull<saucer_desktop>,
    _app: App, // Prevent the app from being dropped when the module is still being used
    _marker: PhantomData<saucer_desktop>,
}

unsafe impl Send for Desktop {}
unsafe impl Sync for Desktop {}

impl Drop for Desktop {
    fn drop(&mut self) { unsafe { saucer_desktop_free(self.ptr.as_ptr()) } }
}

impl Desktop {
    /// Creates and mounts the desktop module to the given [`App`].
    ///
    /// The provided app handle is captured within the desktop mod, which may affect dropping.
    pub fn new(app: &App) -> Self {
        let ptr = unsafe { saucer_desktop_new(app.as_ptr()) };
        Self {
            ptr: NonNull::new(ptr).expect("desktop module should be created"),
            _app: app.clone(),
            _marker: PhantomData,
        }
    }

    /// Opens then given URL or file using system-wide handler.
    ///
    /// # Security Concerns
    ///
    /// This method passes the URL to the underlying system API (e.g. `open` command on Windows)
    /// without validation. Passing any user input can cause **SEVERE SECURITY RISK** to the
    /// application. It's highly recommended to provide only controlled content to this method.
    pub fn open(&self, url: impl Into<Vec<u8>>) {
        use_string!(url; unsafe { saucer_desktop_open(self.ptr.as_ptr(), url) });
    }

    /// Gets the cursor position.
    pub fn mouse_position(&self) -> (i32, i32) {
        let mut x = 0;
        let mut y = 0;
        unsafe { saucer_desktop_mouse_position(self.ptr.as_ptr(), &raw mut x, &raw mut y) };
        (x, y)
    }

    /// Picks a file with the given options.
    pub fn pick_file(&self, opt: &PickerOptions) -> crate::error::Result<String> {
        let mut ex = -1;
        let buf = load_range!(ptr[size] = 0u8; {
            unsafe {
                saucer_picker_pick_file(self.ptr.as_ptr(), opt.as_ptr(), ptr as *mut c_char, size, &raw mut ex);
            }
        });

        if buf.is_empty() {
            Err(crate::error::Error::Saucer(ex))
        } else {
            Ok(String::from_utf8_lossy(&buf).into_owned())
        }
    }

    /// Picks a folder with the given options.
    pub fn pick_folder(&self, opt: &PickerOptions) -> crate::error::Result<String> {
        let mut ex = -1;
        let buf = load_range!(ptr[size] = 0u8; {
            unsafe {
                saucer_picker_pick_folder(self.ptr.as_ptr(), opt.as_ptr(), ptr as *mut c_char, size, &raw mut ex);
            }
        });

        if buf.is_empty() {
            Err(crate::error::Error::Saucer(ex))
        } else {
            Ok(String::from_utf8_lossy(&buf).into_owned())
        }
    }

    /// Picks multiple files with the given options.
    pub fn pick_files(&self, opt: &PickerOptions) -> crate::error::Result<Vec<String>> {
        let mut ex = -1;
        let mut buf = load_range!(ptr[size] = 0u8; {
            unsafe {
                saucer_picker_pick_files(self.ptr.as_ptr(), opt.as_ptr(), ptr as *mut c_char, size, &raw mut ex);
            }
        });

        buf.push(0);

        if buf.is_empty() {
            Err(crate::error::Error::Saucer(ex))
        } else {
            Ok(inflate_strings(&buf))
        }
    }

    /// Picks a save destination with the given options.
    pub fn pick_save(&self, opt: &PickerOptions) -> crate::error::Result<String> {
        let mut ex = -1;
        let buf = load_range!(ptr[size] = 0u8; {
            unsafe {
                saucer_picker_save(self.ptr.as_ptr(), opt.as_ptr(), ptr as *mut c_char, size, &raw mut ex);
            }
        });

        if buf.is_empty() {
            Err(crate::error::Error::Saucer(ex))
        } else {
            Ok(String::from_utf8_lossy(&buf).into_owned())
        }
    }
}
