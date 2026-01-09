//! PDF module.
//!
//! See [`Pdf`] for details.
use std::marker::PhantomData;
use std::ptr::NonNull;

use saucer_sys::*;

use crate::macros::use_string;
use crate::webview::Webview;

/// The PDF printing module.
///
/// This struct borrows a [`Webview`] as it uses browser functionalities to print documents, making
/// its lifetime tied to the handle.
pub struct Pdf<'a> {
    ptr: NonNull<saucer_pdf>,
    _webview: &'a Webview,
    _marker: PhantomData<saucer_pdf>,
}

unsafe impl Send for Pdf<'_> {}
unsafe impl Sync for Pdf<'_> {}

impl Drop for Pdf<'_> {
    fn drop(&mut self) { unsafe { saucer_pdf_free(self.ptr.as_ptr()) } }
}

impl<'a> Pdf<'a> {
    /// Creates and mounts the PDF module to the given [`Webview`].
    pub fn new(w: &'a Webview) -> Self {
        let ptr = unsafe { saucer_pdf_new(w.as_ptr()) };
        Self {
            ptr: NonNull::new(ptr).expect("PDF module should be created"),
            _webview: w,
            _marker: PhantomData,
        }
    }

    /// Prints the content of the current page into a PDF file using the given settings.
    ///
    /// This method blocks until the printing process finishes. It internally polls app events so
    /// the UI won't freeze, but the processor usage may grow high when printing.
    pub fn save(&self, settings: impl AsRef<PdfSettings>) {
        unsafe { saucer_pdf_save(self.ptr.as_ptr(), settings.as_ref().as_ptr()) }
    }
}

/// PDF output layout.
pub enum Layout {
    Portrait,
    Landscape,
}

impl From<Layout> for saucer_pdf_layout {
    fn from(l: Layout) -> Self {
        match l {
            Layout::Portrait => SAUCER_PDF_LAYOUT_PORTRAIT,
            Layout::Landscape => SAUCER_PDF_LAYOUT_LANDSCAPE,
        }
    }
}

/// Settings for PDF printing.
pub struct PdfSettings {
    ptr: NonNull<saucer_pdf_settings>,
    _marker: PhantomData<saucer_pdf_settings>,
}

unsafe impl Send for PdfSettings {}
unsafe impl Sync for PdfSettings {}

impl Drop for PdfSettings {
    fn drop(&mut self) { unsafe { saucer_pdf_settings_free(self.ptr.as_ptr()) } }
}

impl PdfSettings {
    /// Creates a settings object that saves to the specified path.
    pub fn new(fp: impl Into<Vec<u8>>) -> Self {
        let ptr = use_string!(fp; unsafe { saucer_pdf_settings_new(fp) });
        Self {
            ptr: NonNull::new(ptr).expect("PDF settings should be created"),
            _marker: PhantomData,
        }
    }

    /// Sets the output orientation.
    pub fn set_orientation(&mut self, orientation: Layout) {
        unsafe { saucer_pdf_settings_set_orientation(self.ptr.as_ptr(), orientation.into()) };
    }

    /// Sets the output size.
    pub fn set_size(&mut self, width: f64, height: f64) {
        unsafe { saucer_pdf_settings_set_size(self.ptr.as_ptr(), width, height) };
    }

    fn as_ptr(&self) -> *mut saucer_pdf_settings { self.ptr.as_ptr() }
}
