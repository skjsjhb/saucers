use saucers::app::AppManager;
use saucers::app::AppOptions;
use saucers::stash::Stash;
use saucers::webview::Webview;
use saucers::webview::WebviewOptions;
use saucers::window::Window;
use saucers::NoOp;

// Files can be included in the binary using macros.
// This is usually automated by frameworks in real-world applications.
static HTML_FILE: &[u8] = include_bytes!("index.html");
static JS_FILE: &[u8] = include_bytes!("main.js");

/// This example shows how to embed, serve and use resource files.
fn main() {
    let app = AppManager::new(AppOptions::new_with_id("embed"));

    app.run(
        |app, fin| {
            let window = Window::new(&app, NoOp).unwrap();

            window.set_size((1152, 648));
            window.show();

            let webview =
                Webview::new(WebviewOptions::default(), window, NoOp, NoOp, vec![]).unwrap();

            // Add embedded files using paths, contents and MIME types.
            webview.embed("/index.html", Stash::new_view(HTML_FILE), "text/html");
            webview.embed("/main.js", Stash::new_view(JS_FILE), "text/javascript");

            // Navigates to the embedded file.
            webview.serve("/index.html");

            fin.set(|_| drop(webview));
        },
        NoOp,
    )
    .unwrap();
}
