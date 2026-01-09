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

#[allow(unused)] // Template
pub trait WebviewEventListener {
    fn on_permission(&self, webview: Webview, req: PermissionRequest) -> HandleStatus {
        HandleStatus::Unhandled
    }
    fn on_fullscreen(&self, webview: Webview, is_fullscreen: bool) -> Policy { Policy::Allow }
    fn on_dom_ready(&self, webview: Webview) {}
    fn on_navigated(&self, webview: Webview, url: Url) {}
    fn on_navigate(&self, webview: Webview, nav: &Navigation) -> Policy { Policy::Allow }
    fn on_message(&self, webview: Webview, msg: Cow<str>) -> HandleStatus {
        HandleStatus::Unhandled
    }
    fn on_request(&self, webview: Webview, url: Url) {}
    fn on_favicon(&self, webview: Webview, icon: Icon) {}
    fn on_title(&self, webview: Webview, title: String) {}
    fn on_load(&self, webview: Webview, state: LoadState) {}
}

#[allow(unused)] // Template
pub trait WebviewSchemeHandler {
    fn handle_scheme(&self, webview: Webview, req: Request, exc: Executor) {
        exc.reject(SchemeError::NotFound)
    }
}
