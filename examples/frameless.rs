use saucers::app::AppManager;
use saucers::app::AppOptions;
use saucers::webview::Webview;
use saucers::webview::WebviewOptions;
use saucers::window::Window;
use saucers::window::WindowDecoration;

/// This example shows how to create a frameless window and use the
/// `data-webview-drag` attribute to allow dragging the window using an HTML
/// element.
fn main() {
    let app = AppManager::new(AppOptions::new_with_id("frameless"));

    app.run(
        |app| {
            let window = Window::new(&app, ()).unwrap();
            window.set_decorations(WindowDecoration::None);
            window.set_size((1152, 648));
            window.show();

            let webview = Webview::new(WebviewOptions::default(), window, (), ()).unwrap();

            // Add buttons. Use attributes to map their actions into the native window.
            webview.set_html(
                r#"
                    <button data-webview-drag>Drag Me</button>
                    <button data-webview-close>Close</button>
                "#,
            );

            webview
        },
        (),
    )
    .unwrap();
}
