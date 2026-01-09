use saucers::app::AppManager;
use saucers::app::AppOptions;
use saucers::webview::Webview;
use saucers::webview::WebviewOptions;
use saucers::window::Window;
use saucers::NoOp;

/// This example shows how to create a well-behaved app with minimal code.
fn main() {
    let app = AppManager::new(AppOptions::new_with_id("empty"));

    app.run(
        |app, fin| {
            let window = Window::new(&app, NoOp).unwrap();

            // Title bar may not display correctly if size is not set
            window.set_size((1152, 648));
            window.show();

            let webview =
                Webview::new(WebviewOptions::default(), window, NoOp, NoOp, vec![]).unwrap();

            webview.set_url_str("about:blank");

            // Make sure to capture the webview somewhere, or it will be closed immediately!
            // We can just put it in the finish callback, it will be dropped when the app quits.
            fin.set(|_| drop(webview));
        },
        NoOp,
    )
    .unwrap();
}
