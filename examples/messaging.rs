use std::borrow::Cow;

use saucers::app::AppManager;
use saucers::app::AppOptions;
use saucers::scheme::register_scheme;
use saucers::status::HandleStatus;
use saucers::webview::Webview;
use saucers::webview::WebviewEventListener;
use saucers::webview::WebviewOptions;
use saucers::window::Window;
use saucers::NoOp;

/// This example demonstrates how to send message from webview to host and vice versa.
/// Note that this example uses [`Webview::execute`] to message back, which can be inefficient for
/// large payloads. Consider using a scheme handler as needed.
fn main() {
    register_scheme("foo");

    let app = AppManager::new(AppOptions::new_with_id("messaging"));

    app.run(
        |app, fin| {
            let window = Window::new(&app, NoOp).unwrap();

            window.set_size((1152, 648));
            window.show();

            struct WebviewEv;

            impl WebviewEventListener for WebviewEv {
                fn on_message(&self, webview: Webview, msg: Cow<str>) -> HandleStatus {
                    println!("Browser said: {msg}");
                    webview.execute(
                        "window.dispatchEvent(new CustomEvent('host-reply', { detail: 'Pong!' }));",
                    );
                    HandleStatus::Handled
                }
            }

            let webview =
                Webview::new(WebviewOptions::default(), window, WebviewEv, NoOp, vec![]).unwrap();

            // Browser scripts are not injected until a navigation (URL or HTML). Scripts enable
            // `window.saucer.internal.message` so that you have a unified API on all platforms.
            // Platform-specific APIs can still be used without scripts, like `window.chrome`,
            // `window.webkit` or `QWebChannel` (with the prelude script).
            webview.set_html("");

            webview.execute(
                r#"
                window.addEventListener("host-reply", (e) => {
                    alert("Host said: " + e.detail);
                });

                void window.saucer.internal.message("Ping!");
                "#,
            );

            fin.set(|_| drop(webview));
        },
        NoOp,
    )
    .unwrap();
}
