extern crate core;

use std::sync::Arc;

use saucers::app::App;
use saucers::options::AppOptions;

#[test]
fn app_test() { do_app_test(); }

fn do_app_test() {
    let (app, cc) = App::new(AppOptions::new("saucer"));

    // Check for use-after-free.
    // Memory error shall occur if called `run_once` with an invalid pointer.
    let app1 = app.clone();
    drop(app1);
    app.run_once();

    let app2 = app.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let arc = Arc::new(());
    let arc1 = arc.clone();
    #[allow(deprecated)]
    app.pool_submit(move || {
        assert!(!app2.is_thread_safe(), "Submitted tasks should run on new threads");
        tx.send(()).unwrap()
    });

    assert!(rx.try_recv().is_ok(), "Submitting a task should wait for it to finish");

    let app3 = app.clone();
    #[allow(deprecated)]
    app.pool_emplace(move || {
        app3.post(move |app| {
            assert!(app.is_thread_safe(), "Posted tasks should run on event thread");
            app.quit();
        });
    });

    let app5 = app.clone();
    std::thread::spawn(move || {
        app5.post(move |_| {
            let _ = &arc1;
        });
    })
    .join()
    .unwrap();

    app.run();

    // Posted closure are usually cleared by the event loop. What if there isn't an event loop?
    // We can rely on the internal cleanup table for it.
    app.post({
        let arc = arc.clone();
        move |_| {
            let _ = &arc;
        }
    });

    // Tests a foreign drop, `Collector` will panic if this failed to be dropped.
    std::thread::spawn(move || {
        drop(app);
    })
    .join()
    .unwrap();

    drop(cc);
    assert_eq!(Arc::strong_count(&arc), 1, "Posted closures should be dropped");
}
