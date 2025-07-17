//! PDF module.
//!
//! See [`Pdf`] for details.
use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::capi::*;
use crate::macros::rtoc;
use crate::webview::Webview;

/// The PDF printing module.
///
/// This struct holds a [`Webview`] internally, making it an equivalent to a webview handle when considering the usage
/// of webview handles.
pub struct Pdf {
    ptr: NonNull<saucer_pdf>,
    _webview: Webview,
    _owns: PhantomData<saucer_pdf>
}

unsafe impl Send for Pdf {}
unsafe impl Sync for Pdf {}

impl Drop for Pdf {
    fn drop(&mut self) { unsafe { saucer_pdf_free(self.ptr.as_ptr()) } }
}

impl Pdf {
    /// Creates and mounts the PDF module to the given [`Webview`].
    pub fn new(w: &Webview) -> Self {
        let ptr = unsafe { saucer_pdf_new(w.as_ptr()) };
        Self {
            ptr: NonNull::new(ptr).expect("Failed to create pdf module"),
            _webview: w.clone(),
            _owns: PhantomData
        }
    }

    /// Prints the content of the current page into a PDF file using the given settings.
    ///
    /// This method blocks until the printing process finishes. It internally polls app events so the UI won't freeze,
    /// but the processor usage may grow high when printing.
    pub fn save(&self, settings: &PrintSettings) { unsafe { saucer_pdf_save(self.ptr.as_ptr(), settings.as_ptr()) } }
}

/// PDF output layout.
pub enum Layout {
    Portrait,
    Landscape
}

impl From<Layout> for SAUCER_LAYOUT {
    fn from(l: Layout) -> Self {
        match l {
            Layout::Portrait => SAUCER_LAYOUT_SAUCER_LAYOUT_PORTRAIT,
            Layout::Landscape => SAUCER_LAYOUT_SAUCER_LAYOUT_LANDSCAPE
        }
    }
}

/// Settings for PDF printing.
pub struct PrintSettings {
    ptr: NonNull<saucer_print_settings>,
    _owns: PhantomData<saucer_print_settings>
}

unsafe impl Send for PrintSettings {}
unsafe impl Sync for PrintSettings {}

impl Drop for PrintSettings {
    fn drop(&mut self) { unsafe { saucer_print_settings_free(self.ptr.as_ptr()) } }
}

impl PrintSettings {
    /// Creates a new set of print settings.
    pub fn new() -> Self {
        let ptr = unsafe { saucer_print_settings_new() };
        Self {
            ptr: NonNull::new(ptr).expect("Failed to create print settings"),
            _owns: PhantomData
        }
    }

    /// Sets the output file.
    pub fn set_file(&mut self, file: impl AsRef<str>) {
        rtoc!(file => f; saucer_print_settings_set_file(self.ptr.as_ptr(), f.as_ptr()));
    }

    /// Sets the output orientation.
    pub fn set_orientation(&mut self, orientation: Layout) {
        unsafe { saucer_print_settings_set_orientation(self.ptr.as_ptr(), orientation.into()) };
    }

    /// Sets the output width.
    pub fn set_width(&mut self, width: f64) { unsafe { saucer_print_settings_set_width(self.ptr.as_ptr(), width) }; }

    /// Sets the output height.
    pub fn set_height(&mut self, height: f64) {
        unsafe { saucer_print_settings_set_height(self.ptr.as_ptr(), height) };
    }

    fn as_ptr(&self) -> *mut saucer_print_settings { self.ptr.as_ptr() }
}
