use saucers::app::App;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
use saucers::scheme::register_scheme;
use saucers::webview::Webview;

/// This example demonstrates how to send message from webview to host and vice versa.
/// Note that this example uses [`Webview::execute`] to message back, which can be inefficient for more complex
/// responses. Consider using a scheme handler for binary or large payloads.
fn main() {
    register_scheme("foo");

    let (_cc, app) = App::new(AppOptions::new("Messaging"));

    let w = Webview::new(&{
        let mut prefs = Preferences::new(&app);
        prefs.set_hardware_acceleration(false);
        prefs
    })
    .unwrap();

    w.execute(
        r#"
        window.addEventListener("host-reply", (e) => {
            document.body.innerHTML = "Host said: " + e.detail;
        });

        void window.saucer.internal.send_message("Ping!");
    "#
    );

    w.on_message(|w, msg| {
        println!("Browser said: {msg}");
        w.execute("window.dispatchEvent(new CustomEvent('host-reply', { detail: 'Pong!' }));");
    });

    w.set_dev_tools(true);
    w.set_size(1152, 648);
    w.set_url("data:text/html,");
    w.show();

    app.run();
}
