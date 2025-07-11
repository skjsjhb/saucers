use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use saucers::app::App;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
use saucers::scheme::register_scheme;
use saucers::scheme::Response;
use saucers::script::Script;
use saucers::script::ScriptLoadTime;
use saucers::stash::Stash;
use saucers::webview::events::ClosedEvent;
use saucers::webview::events::DomReadyEvent;
use saucers::webview::events::FaviconEvent;
use saucers::webview::events::MinimizeEvent;
use saucers::webview::events::TitleEvent;
use saucers::webview::Webview;

#[test]
fn webview_test() { do_webview_test(); }

fn do_webview_test() {
    register_scheme("foo");

    let (app, cc) = App::new(AppOptions::new("saucer"));
    let w = Webview::new(&Preferences::new(&app)).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let arc = Arc::new(());

    w.set_url("https://saucer.app");
    w.set_size(1152, 648);
    w.set_dev_tools(true);
    w.set_title("Saucers");
    w.show();

    w.on(
        ClosedEvent,
        Box::new({
            let arc = arc.clone();
            move |_| {
                let _ = &arc;
            }
        })
    );

    w.once(
        FaviconEvent,
        Box::new({
            |_, icon| {
                assert!(
                    icon.data().data().is_some_and(|it| it.len() > 0),
                    "Icon should be retrieved"
                );
            }
        })
    );

    w.clear(ClosedEvent);

    assert_eq!(Arc::strong_count(&arc), 1, "Cleared event handlers should be dropped");

    // This will not be fired, so we can validate whether auto-dropping for once handlers works
    w.once(
        MinimizeEvent,
        Box::new({
            let arc = arc.clone();
            move |_, _| {
                let _ = &arc;
            }
        })
    );

    // Checks concurrent modification
    // The event handler is checked to be able to remove itself properly if the `Arc` is not leaked
    let id = Arc::new(AtomicU64::new(0));
    id.store(
        w.on(
            DomReadyEvent,
            Box::new({
                let id = id.clone();
                let arc = arc.clone();
                move |w| {
                    let id = id.load(Ordering::Relaxed);
                    w.off(DomReadyEvent, id);
                    let _ = &arc;
                }
            })
        ),
        Ordering::Relaxed
    );

    w.on(
        DomReadyEvent,
        Box::new({
            let arc = arc.clone();
            move |_| {
                let _ = &arc;
            }
        })
    );

    w.on(
        TitleEvent,
        Box::new({
            let arc = arc.clone();
            move |_, title: &str| {
                let _ = &arc;
                tx.send(title.to_owned()).unwrap();
            }
        })
    );

    w.once(
        ClosedEvent,
        Box::new({
            let app = app.clone();
            let arc = arc.clone();
            move |_| {
                let _ = &arc;
                app.quit();
            }
        })
    );

    w.inject(&Script::new(
        r#"
        (async ()=>{
            const res = await fetch("foo://domain/ping", {
               method: "POST",
               body: "ping!"
            });

            window.saucer.internal.send_message(await res.text());
        })();
        "#,
        ScriptLoadTime::Ready
    ));

    w.handle_scheme("foo", {
        let arc = arc.clone();
        move |_, req, exc| {
            let _ = &arc;
            let st = req.content();
            let body = String::from_utf8_lossy(st.data().unwrap());
            assert_eq!(req.method(), "POST", "Method of scheme request should be correct");
            assert_eq!(body, "ping!", "Body of scheme request should be correct");
            let res = Response::new(&Stash::view("pong!".into()), "text/plain");

            res.set_header("Access-Control-Allow-Origin", "*");
            exc.resolve(&res);
        }
    });

    w.on_message({
        let arc = arc.clone();
        move |w, msg| -> bool {
            let _ = &arc;
            assert_eq!(msg, "pong!", "Message content should be correct");
            w.close();
            true
        }
    });

    app.run();

    assert!(
        rx.recv().unwrap().len() > 0,
        "Event handler should receive correct arguments"
    );

    w.once(
        ClosedEvent,
        Box::new({
            let arc = arc.clone();
            move |_| {
                let _ = &arc;
            }
        })
    );

    std::thread::spawn(move || {
        drop(w);
    })
    .join()
    .unwrap();

    drop(app);
    drop(cc);

    assert_eq!(Arc::strong_count(&arc), 1, "Event handlers should be dropped");
}
