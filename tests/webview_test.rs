use std::sync::Arc;

use saucers::app::App;
use saucers::collector::Collector;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
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
        icon.save("a.ico");
        assert!(std::fs::read("a.ico").unwrap().len() > 0, "Icon should be saved");
        std::fs::remove_file("a.ico").unwrap();
    });

    w.once_load({
        let w = w.clone();
        move |_| {
            w.execute("window.saucer.internal.send_message('')");
        }
    });

    w.clear_closed();

    assert_eq!(Arc::strong_count(&arc), 1, "Cleared event handlers should be dropped");

    let id = w
        .on_dom_ready({
            let arc = arc.clone();
            move || {
                let _ = &arc;
            }
        })
        .unwrap();

    let id1 = w
        .on_title({
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

    w.off_dom_ready(id);
    w.off_title(id1);
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
