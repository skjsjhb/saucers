use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use saucers::app::App;
use saucers::collector::Collector;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
use saucers::script::Script;
use saucers::script::ScriptLoadTime;
use saucers::webview::Webview;

#[test]
fn webview_test() {
    let cc = Collector::new();
    let app = App::new(&cc, AppOptions::new("saucer"));
    let w = Webview::new(&Preferences::new(&app)).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let arc = Arc::new(());

    w.set_url("https://saucer.app");
    w.set_size(1152, 648);
    w.set_dev_tools(true);
    w.set_title("Saucers");
    w.show();

    w.on_closed({
        let arc = arc.clone();
        move || {
            let _ = &arc;
        }
    });

    w.once_favicon(|icon| {
        assert!(
            icon.data().data().is_some_and(|it| it.len() > 0),
            "Icon should be retrieved"
        );
    });

    w.inject(&Script::new(
        "window.saucer.internal.send_message('')",
        ScriptLoadTime::Ready
    ));

    w.clear_closed();

    assert_eq!(Arc::strong_count(&arc), 1, "Cleared event handlers should be dropped");

    // This will not be fired, so we can validate whether auto-dropping for once handlers works
    w.once_minimize({
        let arc = arc.clone();
        move |_| {
            let _ = &arc;
        }
    });

    // Checks concurrent modification
    // The event handler is checked to be able to remove itself properly if the `Arc` is not leaked
    let id = Arc::new(AtomicU64::new(0));
    id.store(
        w.on_dom_ready({
            let id = id.clone();
            let w = w.clone();
            let arc = arc.clone();
            move || {
                let id = id.load(Ordering::Relaxed);
                w.off_dom_ready(id);
                let _ = &arc;
            }
        })
        .unwrap(),
        Ordering::Relaxed
    );

    let id1 = w
        .on_dom_ready({
            let arc = arc.clone();
            move || {
                let _ = &arc;
            }
        })
        .unwrap();

    w.on_title({
        let arc = arc.clone();
        move |title: &str| {
            let _ = &arc;
            tx.send(title.to_owned()).unwrap();
        }
    })
    .unwrap();

    w.once_closed({
        let app = app.clone();
        let arc = arc.clone();
        move || {
            let _ = &arc;
            app.quit();
        }
    });

    w.on_message({
        let w = w.clone();
        move |_: &str| -> bool {
            w.close();
            true
        }
    });

    app.run();

    assert!(
        rx.recv().unwrap().len() > 0,
        "Event handler should receive correct arguments"
    );

    w.off_dom_ready(id1);
    w.clear_title();
    w.off_message();

    w.once_closed({
        let arc = arc.clone();
        move || {
            let _ = &arc;
        }
    });

    std::thread::spawn(move || {
        drop(w);
    })
    .join()
    .unwrap();

    drop(app);
    drop(cc);

    assert_eq!(Arc::strong_count(&arc), 1, "Event handlers should be dropped");
}
