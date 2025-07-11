use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::app::App;
use crate::capi::*;

/// Preferences object for creating a [`crate::webview::Webview`].
///
/// Each preferences object references to an [`App`], which is later passed to the [`crate::webview::Webview`] that's
/// created using it. The lifetime of this object and the created [`crate::webview::Webview`]s are then tied to the
/// [`App`]. See the docs of [`crate::webview::Webview`] for details.
pub struct Preferences<'a> {
    inner: NonNull<saucer_preferences>,
    app: &'a App,
    _owns: PhantomData<saucer_preferences>
}

unsafe impl Send for Preferences<'_> {}
unsafe impl Sync for Preferences<'_> {}

impl Drop for Preferences<'_> {
    fn drop(&mut self) {
        unsafe {
            saucer_preferences_free(self.inner.as_ptr());
        }
    }
}

impl<'a> Preferences<'a> {
    /// Creates a new preferences object that references to the given app.
    pub fn new(app: &'a App) -> Self {
        let p = unsafe {
            // SAFETY: The preferences is later passed to a webview window which (we believe) will use the pointer
            // safely, due to the fact that multiple webviews can be created from one app (in the C++ API).
            saucer_preferences_new(app.as_ptr())
        };

        Self {
            inner: NonNull::new(p).expect("Failed to create preferences"),
            app,
            _owns: PhantomData
        }
    }

    /// Sets whether cookies should be persistent. Cookies are persistent by default.
    pub fn set_persistent_cookies(&mut self, persist: bool) {
        unsafe { saucer_preferences_set_persistent_cookies(self.inner.as_ptr(), persist) }
    }

    /// Sets whether hardware acceleration is enabled. Hardware acceleration is enabled by default.
    pub fn set_hardware_acceleration(&mut self, acc: bool) {
        unsafe { saucer_preferences_set_hardware_acceleration(self.inner.as_ptr(), acc) }
    }

    /// Sets whether default scripts are injected. Default scripts are enabled by default.
    ///
    /// Disabling default scripts allow the framework to provide more fine-grained APIs than the default injected
    /// scripts. However, this will also disable certain built-in features:
    ///
    /// - All exposed JavaScript APIs under `window.saucer` won't be available, including messaging and exposed C++
    ///   functions.
    /// - [`crate::webview::events::DomReadyEvent`] won't be fired for Qt, WebKitGTK and Cocoa/WebKit.
    /// - [`crate::script::Script`]s injected at [`crate::script::ScriptLoadTime::Ready`] stage won't be executed for
    ///   Qt, WebKitGTK and Cocoa/WebKit.
    /// - DOM elements cannot be used to interact with frameless window by adding data attributes.
    /// - Other potentially undocumented features relying on the injected scripts.
    ///
    /// Such features can be re-implemented by the framework if needed.
    ///
    /// Once the default scripts are configured to be disabled for a webview, there is no way to re-enable them after
    /// the creation.
    ///
    /// This method is not part of the saucer API and does not follow its semantic versioning.
    pub fn set_default_scripts(&mut self, def: bool) {
        unsafe { saucer_preferences_set_default_scripts(self.inner.as_ptr(), def) }
    }

    /// Sets the path to store browser data.
    ///
    /// By default, saucer chooses a path either by computing a default value, or use implementation-defined defaults.
    /// Such behavior is not guaranteed to be consistent, thus it's recommended to always override the path manually.
    pub fn set_storage_path(&mut self, pt: impl AsRef<str>) {
        unsafe {
            let cstr = CString::new(pt.as_ref()).unwrap();
            saucer_preferences_set_storage_path(self.inner.as_ptr(), cstr.as_ptr())
        }
    }

    /// Adds a browser flag. Available flags and their usage are implementation-defined.
    pub fn add_browser_flag(&mut self, flag: impl AsRef<str>) {
        unsafe {
            let cstr = CString::new(flag.as_ref()).unwrap();
            saucer_preferences_add_browser_flag(self.inner.as_ptr(), cstr.as_ptr())
        }
    }

    /// Sets the user agent. The default UA string is implementation-defined.
    pub fn set_user_agent(&mut self, ua: impl AsRef<str>) {
        unsafe {
            let cstr = CString::new(ua.as_ref()).unwrap();
            saucer_preferences_set_user_agent(self.inner.as_ptr(), cstr.as_ptr())
        }
    }

    /// SAFETY: The user must not mutate the returned pointer when it's being used by a webview.
    pub(crate) unsafe fn as_ptr(&self) -> *mut saucer_preferences { self.inner.as_ptr() }

    /// Clones an [`App`] and returns it.
    pub(crate) fn get_app(&self) -> App { self.app.clone() }
}
