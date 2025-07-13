use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::app::App;
use crate::capi::*;
use crate::ctor;
use crate::desktop::PickerOptions;
use crate::rtoc;

/// The desktop module providing file picking and URL opening.
///
/// Like [`crate::webview::Webview`], this struct holds an [`App`] internally and should be seen as an equivalent app
/// handler when counting for dropping.
pub struct Desktop {
    ptr: NonNull<saucer_desktop>,
    _app: App,
    _owns: PhantomData<saucer_desktop>
}

unsafe impl Send for Desktop {}
unsafe impl Sync for Desktop {}

impl Drop for Desktop {
    fn drop(&mut self) { unsafe { saucer_desktop_free(self.ptr.as_ptr()) } }
}

impl Desktop {
    /// Creates and mounts the desktop module to the given [`App`].
    pub fn new(app: &App) -> Self {
        let ptr = unsafe { saucer_desktop_new(app.as_ptr()) };
        Self {
            ptr: NonNull::new(ptr).expect("Failed to create desktop module"),
            _app: app.clone(),
            _owns: PhantomData
        }
    }

    /// Opens then given URL or file using system-wide handler.
    ///
    /// # Security Concerns
    ///
    /// This method passes the URL to the underlying system API (e.g. `open` command on Windows) without validation.
    /// Passing any user input can cause **SEVERE SECURITY RISK** to the application. It's highly recommended to provide
    /// only controlled content to this method.
    pub fn open(&self, url: impl AsRef<str>) {
        rtoc!(url => u; saucer_desktop_open(self.ptr.as_ptr(), u.as_ptr()));
    }

    /// Picks a file with the given options.
    pub fn pick_file(&self, opt: &PickerOptions) -> Option<String> {
        ctor!(
            free,
            nullable,
            saucer_desktop_pick_file(self.ptr.as_ptr(), opt.as_ptr())
        )
    }

    /// Picks a folder with the given options.
    pub fn pick_folder(&self, opt: &PickerOptions) -> Option<String> {
        ctor!(
            free,
            nullable,
            saucer_desktop_pick_folder(self.ptr.as_ptr(), opt.as_ptr())
        )
    }

    /// Picks multiple files with the given options.
    pub fn pick_files(&self, opt: &PickerOptions) -> Vec<String> {
        let mut count = 0usize;
        let ptr =
            unsafe { saucer_desktop_pick_files_with_size(self.ptr.as_ptr(), opt.as_ptr(), &mut count as *mut usize) };

        if ptr.is_null() {
            return Vec::new();
        }

        let mut files = Vec::with_capacity(count);

        for i in 0..count {
            files.push(ctor!(free, *ptr.add(i)));
        }

        unsafe { saucer_memory_free(ptr as *mut c_void) }

        files
    }

    /// Picks multiple folders with the given options.
    ///
    /// # Deprecation Notes
    ///
    /// This method is implemented as an alias to [`Self::pick_files`] in the C API. This is possibly a bindings-error
    /// in saucer and this method may subject to change.
    #[deprecated]
    pub fn pick_folders(&self, opt: &PickerOptions) -> Vec<String> {
        let mut count = 0usize;
        // The underlying implementation seems to be identical to file picker
        // Not sure whether it's a bug
        let ptr =
            unsafe { saucer_desktop_pick_folders_with_size(self.ptr.as_ptr(), opt.as_ptr(), &mut count as *mut usize) };

        if ptr.is_null() {
            return Vec::new();
        }

        let mut folders = Vec::with_capacity(count);

        for i in 0..count {
            folders.push(ctor!(free, *ptr.add(i)));
        }

        unsafe { saucer_memory_free(ptr as *mut c_void) }

        folders
    }
}
