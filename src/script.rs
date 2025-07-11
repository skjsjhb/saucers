use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::capi::*;

/// Contains details for scripts to be [`crate::webview::Webview::inject`]ed into webview frames.
///
/// # Security Concerns
///
/// Scripts can be constructed using arbitrary string, but one should almost never do so. Saucer exposes a set of
/// unsafe APIs to the JavaScript world. Such APIs are hidden to the scripts on the page and only safe subsets are
/// exposed. However, these protections do not apply to injected scripts, as they have the same execution priority as
/// the sanitizers themselves. In other words, injected scripts can do anything that the exposed unsafe APIs can do,
/// like sending an empty message and crash the program, which should never happen under the protection of safe APIs.
///
/// Based on the description above, one should **NEVER USE UNTRUSTED SCRIPTS**. Instead, only use scripts whose code you
/// can fully control, like those hardcoded in the program. Don't use script execution to exchange data as it's both
/// inefficient and vulnerable (use scheme handlers instead). If user input must be included, make sure to sanitize them
/// with extra care. Keep in mind that malicious code can be injected in more ways that you would expect, so it's better
/// not to give them the chance!
pub struct Script {
    ptr: NonNull<saucer_script>,
    _owns: PhantomData<saucer_script>
}

/// Possible values of the time that a script is injected.
pub enum ScriptLoadTime {
    /// Executes the script when the document is created.
    Creation,
    /// Executes the script when the DOM is ready.
    ///
    /// Scripts scheduled to be run at this stage may not be executed if default scripts are disabled. Try to use
    /// JavaScript APIs instead.
    Ready
}

/// Possible values of the frame that a script is injected.
pub enum ScriptWebFrame {
    /// Executes the script only in the top frame.
    Top,
    /// Executes the script in all frames.
    All
}

unsafe impl Send for Script {}
unsafe impl Sync for Script {}

impl Drop for Script {
    fn drop(&mut self) { unsafe { saucer_script_free(self.ptr.as_ptr()) } }
}

impl From<ScriptLoadTime> for SAUCER_LOAD_TIME {
    fn from(value: ScriptLoadTime) -> Self {
        match value {
            ScriptLoadTime::Creation => SAUCER_LOAD_TIME_SAUCER_LOAD_TIME_CREATION,
            ScriptLoadTime::Ready => SAUCER_LOAD_TIME_SAUCER_LOAD_TIME_READY
        }
    }
}

impl From<ScriptWebFrame> for SAUCER_WEB_FRAME {
    fn from(value: ScriptWebFrame) -> Self {
        match value {
            ScriptWebFrame::Top => SAUCER_WEB_FRAME_SAUCER_WEB_FRAME_TOP,
            ScriptWebFrame::All => SAUCER_WEB_FRAME_SAUCER_WEB_FRAME_ALL
        }
    }
}

impl Script {
    /// Creates a new script with the given code and load time.
    ///
    /// **DO NOT USE UNTRUSTED CODE**. See the security section in [`Script`] for details.
    pub fn new(code: impl AsRef<str>, time: ScriptLoadTime) -> Self {
        let cst = CString::new(code.as_ref()).unwrap();
        let ptr = unsafe { saucer_script_new(cst.as_ptr(), time.into()) };
        Self {
            ptr: NonNull::new(ptr).unwrap(),
            _owns: PhantomData
        }
    }

    /// Sets the frame this script should be injected to.
    ///
    /// On some platforms this feature is implemented using concatenated JavaScript. Specially designed code content
    /// can bypass the limitation and get injected into all frames. It's better not to rely on the actual behavior
    /// of this method. Instead, make an assumption that your script will always be injected into all frames on the
    /// page.
    pub fn set_frame(&mut self, frame: ScriptWebFrame) {
        unsafe { saucer_script_set_frame(self.ptr.as_ptr(), frame.into()) }
    }

    /// Sets the time to inject the script.
    ///
    /// Scripts can be scheduled to run when DOM is ready, but such behavior relies on default scripts and may not
    /// function when they are disabled. Consider using JavaScript APIs instead.
    pub fn set_time(&mut self, time: ScriptLoadTime) {
        unsafe { saucer_script_set_time(self.ptr.as_ptr(), time.into()) }
    }

    /// Sets whether the script should be permanent.
    ///
    /// Permanent scripts cannot be removed by [`crate::webview::Webview::clear_scripts`]. In fact, there is no way to
    /// remove them. This method is designed to be used by frameworks, rather than being included in the public APIs.
    pub fn set_permanent(&mut self, permanent: bool) {
        unsafe { saucer_script_set_permanent(self.ptr.as_ptr(), permanent) }
    }

    /// Sets the code content of the script.
    pub fn set_code(&mut self, code: impl AsRef<str>) {
        let cst = CString::new(code.as_ref()).unwrap();
        unsafe { saucer_script_set_code(self.ptr.as_ptr(), cst.as_ptr()) }
    }

    pub(crate) fn as_ptr(&self) -> *mut saucer_script { self.ptr.as_ptr() }
}
