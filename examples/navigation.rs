use std::cell::RefCell;
use std::rc::Rc;
use std::rc::Weak;

use saucers::app::App;
use saucers::app::AppManager;
use saucers::app::AppOptions;
use saucers::navigation::Navigation;
use saucers::policy::Policy;
use saucers::webview::Webview;
use saucers::webview::WebviewEventListener;
use saucers::webview::WebviewOptions;
use saucers::window::Window;
use saucers::NoOp;

/// This example demonstrates how to open navigation requests that creates new windows.
/// By default, only in-page navigations are allowed. Saucer does not create a new window
/// automatically when the navigation requests one. To enable such behavior, the user will need to
/// implement custom window managing.
fn main() {
    let app = AppManager::new(AppOptions::new_with_id("navigation"));

    app.run(
        |app, fin| {
            let webviews = Rc::new(RefCell::new(Vec::new()));

            #[derive(Clone)]
            struct WebviewEv {
                // Event handlers are stored inside the webview, thus Weak must be used to prevent
                // circular references.
                webviews: Weak<RefCell<Vec<Webview>>>,
                app: App,
            }

            let webview_ev = WebviewEv { webviews: Rc::downgrade(&webviews), app: app.clone() };

            impl WebviewEventListener for WebviewEv {
                fn on_navigate(&self, _webview: Webview, nav: &Navigation) -> Policy {
                    if nav.is_new_window() && nav.is_user_initiated() {
                        let new_window = Window::new(&self.app, NoOp).unwrap();
                        new_window.set_size((1152, 648));
                        new_window.show();

                        let new_webview = Webview::new(
                            WebviewOptions::default(),
                            new_window,
                            self.clone(),
                            NoOp,
                            vec![],
                        )
                        .unwrap();

                        new_webview.set_url(nav.url());

                        self.webviews.upgrade().unwrap().borrow_mut().push(new_webview);
                    }
                    Policy::Allow
                }
            }

            let window = Window::new(&app, NoOp).unwrap();

            window.set_size((1152, 648));
            window.show();

            let webview =
                Webview::new(WebviewOptions::default(), window, webview_ev, NoOp, vec![]).unwrap();

            webview.set_html("<a target=\"_new\" href=\"about:blank\">Link</a>");

            webviews.borrow_mut().push(webview);

            // Drop the webviews or the app will block infinitely.
            fin.set(|_| drop(webviews));
        },
        NoOp,
    )
    .unwrap();
}
