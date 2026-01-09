use saucers::app::AppManager;
use saucers::app::AppOptions;
use saucers::webview::Webview;
use saucers::webview::WebviewEventListener;
use saucers::webview::WebviewOptions;
use saucers::window::Window;
use saucers::NoOp;

/// This example shows a way to sync the window title with the web page.
/// This shows a way to use the webview event system.
fn main() {
    let app = AppManager::new(AppOptions::new_with_id("title"));

    app.run(
        |app, fin| {
            let window = Window::new(&app, NoOp).unwrap();

            window.set_size((1152, 648));
            window.set_title("Super Secret Title");
            window.show();

            struct WebviewEv;

            impl WebviewEventListener for WebviewEv {
                fn on_title(&self, webview: Webview, title: String) {
                    webview.window().set_title(title);
                }
            }

            let webview =
                Webview::new(WebviewOptions::default(), window, WebviewEv, NoOp, vec![]).unwrap();

            webview.set_html("<title>You didn't see anything!</title>");

            fin.set(|_| drop(webview));
        },
        NoOp,
    )
    .unwrap();
}
