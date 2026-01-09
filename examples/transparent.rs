use saucers::app::AppManager;
use saucers::app::AppOptions;
use saucers::webview::Webview;
use saucers::webview::WebviewOptions;
use saucers::window::Window;
use saucers::NoOp;

/// This example demonstrates how to create a window with transparent/semi-transparent background.
/// Transparent windows work best with decorations disabled, but to keep it simple this example
/// preserves control widgets.
fn main() {
    let app = AppManager::new(AppOptions::new_with_id("transparent"));

    app.run(
        |app, fin| {
            let window = Window::new(&app, NoOp).unwrap();

            window.set_size((1152, 648));

            window.show();

            let webview =
                Webview::new(WebviewOptions::default(), window.clone(), NoOp, NoOp, vec![])
                    .unwrap();

            // Both the window and the webview must be transparent, or everything will just be
            // solid.
            window.set_background((0, 0, 0, 0));
            webview.set_background(0, 0, 0, 0);

            webview.set_html(
                r#"
                <style>
                    body {
                        background: #ffaec880;
                    };
                </style>
                "#,
            );

            fin.set(|_| drop(webview));
        },
        NoOp,
    )
    .unwrap();
}
