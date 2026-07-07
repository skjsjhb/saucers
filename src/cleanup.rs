use std::borrow::Cow;
use std::ptr::NonNull;

use saucer_sys::*;

use crate::macros::use_string;
use crate::webview::SchemeHandlerData;

/// Provides a unified interface for handles to transfer resources that must be
/// dropped on the event thread.
pub(crate) enum CleanUpHolder {
    App {
        ptr: NonNull<saucer_application>,
    },
    Window {
        ptr: NonNull<saucer_window>,
        event_listener_data: *mut crate::window::EventListenerData,
    },
    Webview {
        ptr: NonNull<saucer_webview>,
        schemes: Vec<Cow<'static, str>>,
        event_listener_data: *mut crate::webview::EventListenerData,
        scheme_handler_data: *mut SchemeHandlerData,
    },
}

// SAFETY: Pointers are C-managed and owned, others are [`Send`].
unsafe impl Send for CleanUpHolder {}

impl CleanUpHolder {
    /// Discards the inner resources.
    ///
    /// SAFETY: Must be called on the event thread.
    pub unsafe fn discard(self) {
        match self {
            Self::App { ptr } => unsafe { saucer_application_free(ptr.as_ptr()) },
            Self::Window {
                ptr,
                event_listener_data,
            } => unsafe {
                saucer_window_free(ptr.as_ptr()); // Will also off events

                // Event handler trampolines keep a copy of the handle, which prevents dropping.
                // So we must be the only owner when reaching here.
                drop(Box::from_raw(event_listener_data));
            },
            Self::Webview {
                ptr,
                schemes,
                event_listener_data,
                scheme_handler_data,
            } => unsafe {
                let ptr = ptr.as_ptr();

                for s in schemes {
                    use_string!(s: s.as_ref(); saucer_webview_remove_scheme(ptr, s));
                }

                // Technically, a webview may be freed after its corresponding window due to the
                // deferred posting, which may introduce broken states. However, such broken
                // states will only happen when the handle is being dropped,
                // which is not visible to the safe world.
                saucer_webview_free(ptr); // Will also off events

                drop(Box::from_raw(scheme_handler_data));
                drop(Box::from_raw(event_listener_data));
            },
        }
    }
}
