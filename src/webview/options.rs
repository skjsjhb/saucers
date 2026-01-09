use std::marker::PhantomData;
use std::ptr::NonNull;

use saucer_sys::*;

use crate::macros::use_string;
use crate::window::Window;

/// Options for configuring webview creation.
///
/// Saucer provides sensible defaults for webview setup. Leaving a field [`None`] picks these preset
/// values.
#[derive(Default)]
pub struct WebviewOptions {
    pub allow_attributes: Option<bool>,
    pub persistent_cookies: Option<bool>,
    pub hardware_acceleration: Option<bool>,
    pub storage_path: Option<String>,
    pub user_agent: Option<String>,
    pub browser_flags: Vec<String>,
}

pub(crate) struct RawWebviewOptions {
    inner: NonNull<saucer_webview_options>,
    _marker: PhantomData<saucer_webview_options>,
}

impl Drop for RawWebviewOptions {
    fn drop(&mut self) { unsafe { saucer_webview_options_free(self.as_ptr()) } }
}

impl RawWebviewOptions {
    pub(crate) fn new(opt: WebviewOptions, window: Window) -> Self {
        let ptr = unsafe { saucer_webview_options_new(window.as_ptr()) };
        let inner = NonNull::new(ptr).expect("invalid webview options ptr");

        unsafe {
            if let Some(t) = opt.allow_attributes {
                saucer_webview_options_set_attributes(ptr, t);
            }

            if let Some(t) = opt.persistent_cookies {
                saucer_webview_options_set_persistent_cookies(ptr, t);
            }

            if let Some(t) = opt.hardware_acceleration {
                saucer_webview_options_set_hardware_acceleration(ptr, t);
            }

            if let Some(p) = opt.storage_path {
                use_string!(p; saucer_webview_options_set_storage_path(ptr, p));
            }

            if let Some(p) = opt.user_agent {
                use_string!(p; saucer_webview_options_set_user_agent(ptr, p));
            }

            for f in opt.browser_flags {
                use_string!(f; saucer_webview_options_append_browser_flag(ptr, f)); // Value copied
            }
        }

        Self { inner, _marker: PhantomData }
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_webview_options { self.inner.as_ptr() }
}
