use std::borrow::Cow;

use crate::icon::Icon;
use crate::navigation::Navigation;
use crate::permission::PermissionRequest;
use crate::policy::Policy;
use crate::scheme::Executor;
use crate::scheme::Request;
use crate::scheme::SchemeError;
use crate::state::LoadState;
use crate::status::HandleStatus;
use crate::url::Url;
use crate::webview::Webview;

/// A trait containing webview events.
///
/// Because the listener is stored inside the [`Webview`] handle, capturing any handle directly will
/// form circular references and prevent them from dropping. It's advised to use the passed argument
/// or [`crate::webview::WebviewRef`] instead.
#[allow(unused)] // Template
pub trait WebviewEventListener {
    /// Fired when the webview requests a permission.
    fn on_permission(&self, webview: Webview, req: PermissionRequest) -> HandleStatus {
        HandleStatus::Unhandled
    }

    /// Fired when the webview enters or leaves fullscreen.
    fn on_fullscreen(&self, webview: Webview, is_fullscreen: bool) -> Policy { Policy::Allow }

    /// Fired when the DOM is ready.
    fn on_dom_ready(&self, webview: Webview) {}

    /// Fired when the webview has changed its href.
    fn on_navigated(&self, webview: Webview, url: Url) {}

    /// Fired when the webview is about to navigate to a new URL.
    fn on_navigate(&self, webview: Webview, nav: &Navigation) -> Policy { Policy::Allow }

    /// Fired when the webview sends a message.
    fn on_message(&self, webview: Webview, msg: Cow<str>) -> HandleStatus {
        HandleStatus::Unhandled
    }

    /// Fired when the webview starts a network request.
    fn on_request(&self, webview: Webview, url: Url) {}

    /// Fired when the webview loads a favicon.
    fn on_favicon(&self, webview: Webview, icon: Icon) {}

    /// Fired when the webview title changes.
    fn on_title(&self, webview: Webview, title: String) {}

    /// Fired when the webview page is loaded.
    fn on_load(&self, webview: Webview, state: LoadState) {}
}

/// A trait for handling schemes.
#[allow(unused)] // Template
pub trait WebviewSchemeHandler {
    /// Handles a scheme request.
    ///
    /// This method is called for all requests coming from the given webview and does not
    /// distinguish between protocols. Check [`Request::url`] if you have multiple schemes.
    fn handle_scheme(&self, webview: Webview, req: Request, exc: Executor) {
        exc.reject(SchemeError::NotFound)
    }
}
