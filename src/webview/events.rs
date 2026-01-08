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

pub trait WebviewEventListener {
    fn on_permission(&self, _webview: Webview, _req: PermissionRequest) -> HandleStatus {
        HandleStatus::Unhandled
    }
    fn on_fullscreen(&self, _webview: Webview, _is_fullscreen: bool) -> Policy { Policy::Allow }
    fn on_dom_ready(&self, _webview: Webview) {}
    fn on_navigated(&self, _webview: Webview, _url: Url) {}
    fn on_navigate(&self, _webview: Webview, _nav: &Navigation) -> Policy { Policy::Allow }
    fn on_message(&self, _webview: Webview, _msg: Cow<str>) -> HandleStatus {
        HandleStatus::Unhandled
    }
    fn on_request(&self, _webview: Webview, _url: Url) {}
    fn on_favicon(&self, _webview: Webview, _icon: Icon) {}
    fn on_title(&self, _webview: Webview, _title: String) {}
    fn on_load(&self, _webview: Webview, _state: LoadState) {}
}

pub trait WebviewSchemeHandler {
    fn handle_scheme(&self, _webview: Webview, _req: Request, _exc: Executor) {
        _exc.reject(SchemeError::NotFound)
    }
}
