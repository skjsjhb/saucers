use saucers::app::App;
use saucers::collector::Collector;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
use saucers::webview::events::DomReadyEvent;
use saucers::webview::events::FaviconEvent;
use saucers::webview::Webview;

fn main() {
    // Create a collector to help freeing up resources.
    // The collector must be kept to live longer than all `App`s and `Webview`s.
    // It detects leaks internally and gives a panic when dropped incorrectly.
    let cc = Collector::new();

    // Create an app to manage the event cycle.
    let app = App::new(&cc, AppOptions::new("saucer"));

    // Customize webview behavior using a preference set.
    let mut prefs = Preferences::new(&app);
    prefs.set_user_agent("saucer");

    // Create a new webview instance.
    let w = Webview::new(&prefs).unwrap();
    drop(prefs);

    // Register a one-time listener for DOM ready event.
    // Prefer using the handle argument instead of capturing to prevent cycle references.
    w.once(
        DomReadyEvent,
        Box::new(move |w| {
            w.execute("window.saucer.internal.send_message(`Hello! Your user agent is '${navigator.userAgent}'!`);");
        })
    );

    // Registers a repeatable event handler for favicon event.
    let on_favicon_id = w
        .on(
            FaviconEvent,
            Box::new(|_, icon| {
                println!("Wow, you have a favicon of {} bytes!", icon.data().size());
            })
        )
        .unwrap();

    // Handles incoming webview messages.
    // This API forwards the message as-is, allowing more complex channels to be built on it.
    w.on_message(|_, msg| {
        println!("Browser: {msg}");
        true
    });

    // Set several runtime properties for webview.
    w.set_url("https://saucer.app");
    w.set_size(1152, 648);
    w.set_dev_tools(true);
    w.set_title("Saucer + Rust");

    // Show and run the app.
    w.show();
    app.run();

    // An event handler can be cleared using its ID.
    w.off(FaviconEvent, on_favicon_id);

    // Rust will clean up everything in correct order. But to make it clear, we will drop it manually.
    drop(w);
    drop(app);
    drop(cc);
}
