use crate::app::App;
use crate::capi::*;
use crate::prefs::Preferences;
use std::cell::RefCell;
use std::ffi::c_char;
use std::ffi::c_void;
use std::ffi::CStr;
use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr::null_mut;
use std::ptr::NonNull;
use std::rc::Rc;

/// The webview object, containing handler to a window and its inner web contents.
pub struct Webview<'w> {
    inner: NonNull<saucer_handle>,
    _app: App,
    message_handler: Option<*mut c_void>,
    _owns: PhantomData<saucer_handle>,
    _owns_listener: PhantomData<&'w ()>,
}

unsafe impl Sync for Webview<'_> {}

impl Drop for Webview<'_> {
    fn drop(&mut self) {
        self.off_message();
        unsafe {
            // SAFETY: Following operations must be done on the thread that creates the handle.
            // We did exactly that, as `ManagedWebview` is `!Send` and `!Sync`.
            saucer_free(self.inner.as_ptr());
        }
    }
}

impl<'w> Webview<'w> {
    /// Creates a new webview using the specified [`Preferences`].
    ///
    /// This method must be called on the event thread, or [`None`] will be returned.
    pub fn new(pref: &Preferences) -> Option<Self> {
        if !pref.get_shared_app().is_thread_safe() {
            return None;
        }

        let ptr = unsafe {
            // SAFETY: The C API only reads the preferences when initializing and data are copied.
            // The shared app pointer is stored in the webview to ensure it remains valid when the webview lives.
            saucer_new(pref.as_ptr())
        };

        Some(Self {
            inner: NonNull::new(ptr).expect("Failed to create webview"),
            _app: pref.get_shared_app(),
            message_handler: None,
            _owns: PhantomData,
            _owns_listener: PhantomData,
        })
    }

    /// Sets a message handler to process messages from the webview.
    ///
    /// The return value of the handler indicates whether the message has been processed. If returns `false`, message
    /// will be forwarded to other modules. This Rust bindings library does not support modules for now, but the return
    /// value is still kept, for future usages.
    ///
    /// At most one message handler can be set. Setting a new handler will replace the previous one.
    pub fn on_message(&mut self, exec: impl FnMut(&str) -> bool + 'w) {
        // SAFETY: The provided closure is only possible to be executed when the webview is still alive, as the handler
        // is cleared before dropping the webview, and all of these (including handler locating and dispatching) happens
        // on the main thread. The dropping happens completely before or after a message delivery.
        //
        // Variables captured in the closure are guaranteed to live longer than the webview object, so it's safe to
        // access them in a closure. Handlers are called on the main thread and no handler can be called inside a
        // handler (prohibited by a guard in `App`), thus the `FnMut` is safe to be invoked by the trampoline.
        //
        // Replacing a handler inside the handler is considered safe, since the new handler will not be invoked until
        // the current one finishes. An `Rc` is used to prevent resources used by the closure from being dropped when
        // the old handler is removed. Clearing a handler is also safe out of the same reason.
        self.off_message();

        unsafe {
            // I'm not quite confident that the `FnMut` is not borrowed elsewhere when being invoked (though it
            // shouldn't), adding a `RefCell` here will help to find errors, if any.
            // An `Rc` is used to prevent captured variables from being dropped when removing the previous handler.
            let bb = Rc::new(RefCell::new(Box::new(exec) as Box<dyn FnMut(&str) -> bool + 'w>));
            let ptr = Box::into_raw(Box::new(bb)) as *mut c_void;

            saucer_webview_on_message_with_arg(self.inner.as_ptr(), Some(message_handler_trampoline), ptr);
            self.message_handler = Some(ptr);
        }
    }

    /// Removes the webview message handler, if any.
    pub fn off_message(&mut self) {
        unsafe {
            saucer_webview_on_message_with_arg(self.inner.as_ptr(), None, null_mut());
        }

        if let Some(ptr) = self.message_handler.take() {
            unsafe {
                // Takes back the pointer previously lent to the C library and destruct the `Rc` in it.
                // This shall not affect other `Rc`s to the same data, even if inside a closure.
                // Lifetime is cast away as it's always valid when the webview is living.
                drop(Box::from_raw(ptr as *mut Rc<RefCell<Box<dyn FnMut(&str) -> bool>>>));
            }
        }
    }

    // TODO embedding, favicon

    /// Gets the page title.
    pub fn get_page_title(&self) -> String {
        unsafe {
            let cstr = saucer_webview_page_title(self.inner.as_ptr());

            if cstr.is_null() {
                return "".to_owned();
            }

            let st = CStr::from_ptr(cstr).to_str().expect("Invalid UTF-8 title").to_owned();
            saucer_memory_free(cstr as *mut c_void);
            st
        }
    }

    // TODO getters and setters

    pub fn set_url(&self, url: &str) {
        unsafe {
            let cstr = CString::new(url).unwrap();
            saucer_webview_set_url(self.inner.as_ptr(), cstr.as_ptr()); // Value copied in C
        }
    }

    /// Shows the webview window.
    pub fn show(&self) {
        unsafe {
            saucer_window_show(self.inner.as_ptr());
        }
    }

    /// Hides the webview window.
    pub fn hide(&self) {
        unsafe {
            saucer_window_hide(self.inner.as_ptr());
        }
    }

    /// Closes the webview window.
    pub fn close(&self) {
        unsafe {
            saucer_window_close(self.inner.as_ptr());
        }
    }
}

extern "C" fn message_handler_trampoline(msg: *const c_char, arg: *mut c_void) -> bool {
    unsafe {
        let msg = CStr::from_ptr(msg).to_str().expect("Invalid UTF-8 message");
        let frc = Box::from_raw(arg as *mut Rc<RefCell<Box<dyn FnMut(&str) -> bool>>>);
        let nrc = frc.clone(); // Clones a new `Rc` for this invocation (will be dropped)
        let _ = Box::into_raw(frc); // Avoid dropping the raw pointer (will still be used in C)
        nrc.borrow_mut()(msg)
    }
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use crate::options::AppOptions;
    use crate::prefs::Preferences;
    use crate::webview::Webview;
    use crate::webview::message_handler_trampoline;
    use std::cell::RefCell;
    use std::ffi::CString;
    use std::rc::Rc;

    #[test]
    fn test_webview_drop() {
        let opt = AppOptions::new("saucer");
        let app = App::new(opt);
        let pref = Preferences::new(&app);

        let rr = Rc::new(());
        let wv = Rc::new(RefCell::new(Webview::new(&pref).unwrap()));

        let msh2 = {
            let rr = rr.clone();
            move |_: &'_ str| {
                assert_eq!(
                    Rc::strong_count(&rr),
                    2,
                    "Replaced handler should be dropped when exited"
                );
                true
            }
        };

        let mut msh2_opt = Some(msh2);

        let msh1 = {
            let rr = rr.clone();

            let wv = wv.clone();
            move |_: &'_ str| {
                if let Some(mm) = msh2_opt.take() {
                    wv.borrow_mut().on_message(mm);
                    // Though a new handler has been set, the current one should remain valid as it's still being
                    // executed. The counter should decrease when it exits, checked in `msh2`.
                    assert_eq!(
                        Rc::strong_count(&rr),
                        3,
                        "Current handler should not be dropped when being replaced during execution"
                    );
                }
                true
            }
        };

        wv.borrow_mut().on_message(msh1);

        let cst = CString::new("").unwrap();

        // Simulate two calls
        let fp = wv.borrow().message_handler.unwrap(); // Make sure this borrow won't affect testing
        message_handler_trampoline(cst.as_ptr(), fp);

        let fp1 = wv.borrow().message_handler.unwrap();
        assert_ne!(fp, fp1, "Handler should have been changed");
        message_handler_trampoline(cst.as_ptr(), fp1);

        assert_eq!(Rc::strong_count(&rr), 2, "Handler should remain valid");
        drop(wv);
        assert_eq!(Rc::strong_count(&rr), 1, "All handlers should be dropped");
    }
}
