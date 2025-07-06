<hr/>

<div align="center">
<img width="300" src="saucers.png"/>
</div>

<p align="center">Rust bindings for <a href="https://github.com/saucer/saucer">saucer</a></p>

---

# Why?

[Saucer](https://github.com/saucer/saucer) is a cool webview library.

[Rust](https://rust-lang.org) is a cool language.

And by putting them together you can build cooler hybrid apps.

# Example

> [!WARNING]
>
> This project is still under development and the API is subject to change at any time.

The most updated example is in [`src/main.rs`](src/main.rs):

```rust
use saucers::app::App;
use saucers::collector::Collector;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
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
    w.once_dom_ready({
        let w = w.clone();
        move || w.execute("window.saucer.internal.send_message(`Hello! Your user agent is '${navigator.userAgent}'!`);")
    });

    // Registers a repeatable event handler for favicon event.
    let on_favicon_id = w
        .on_favicon(|icon| {
            println!("Wow, you have a favicon of {} bytes!", icon.data().size());
        })
        .unwrap();

    // Handles incoming webview messages.
    // This API forwards the message as-is, allowing more complex channels to be built on it.
    w.on_message(|msg| {
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
    w.off_favicon(on_favicon_id);

    // Rust will clean up everything in correct order. But to make it clear, we will drop it manually.
    drop(w);
    drop(app);
    drop(cc);
}
```

# Known Limitations

- This project is built on top of the [C-Bindings for saucer](https://github.com/saucer/bindings), which exports only a
  subset (major parts, but not all) of the C++ API. We currently have no plan to integrate with the C++ API.
- When building for Windows, only MSVC is currently supported.
- Backend cannot be customized yet.
- Capturing webview handles inside event handlers can easily lead to cycle references and trigger an assertion from the
  collector.
- Safety (mostly the `Send` trait) of certain APIs are not fully verified.

# License

This project is released under the [MIT License](https://mit-license.org) to make licensing consistent with saucer
itself. 