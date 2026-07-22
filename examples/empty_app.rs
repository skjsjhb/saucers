use saucers::app::AppManager;
use saucers::app::AppOptions;
use saucers::webview::Webview;
use saucers::webview::WebviewOptions;
use saucers::window::Window;

/// This example shows how to create a well-behaved app with minimal code.
fn main() {
    let app = AppManager::new(AppOptions::new_with_id("empty"));

    app.run(
        |app| {
            let window = Window::new(&app, ()).unwrap();

            // Title bar may not display correctly if size is not set
            window.set_size((1152, 648));
            window.show();

            let webview = Webview::new(WebviewOptions::default(), window, (), ()).unwrap();

            webview.set_url_str("about:blank");

            // Returning the webview keeps it alive until the app finishes.
            webview
        },
        (),
    )
    .unwrap();
}
