extern crate core;

use std::sync::Arc;

use saucers::app::App;
use saucers::collector::Collector;
use saucers::options::AppOptions;

#[test]
fn app_test() { do_app_test(); }

fn do_app_test() {
    let cc = Collector::new();
    let app = App::new(&cc, AppOptions::new("saucer"));

    // Check for use-after-free.
    // Memory error shall occur if called `run_once` with an invalid pointer.
    let app1 = app.clone();
    drop(app1);
    app.run_once();

    let app2 = app.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let arc = Arc::new(());
    let arc1 = arc.clone();

    app.pool_submit(move || {
        assert!(!app2.is_thread_safe(), "Submitted tasks should run on new threads");
        tx.send(()).unwrap()
    });

    assert!(rx.try_recv().is_ok(), "Submitting a task should wait for it to finish");

    let app3 = app.clone();
    app.pool_emplace(move || {
        let app4 = app3.clone();
        app3.post(move || {
            assert!(app4.is_thread_safe(), "Posted tasks should run on event thread");
            app4.quit();
        });
    });

    let app5 = app.clone();
    std::thread::spawn(move || {
        app5.post(move || {
            let _ = &arc1;
        });
    })
    .join()
    .unwrap();

    app.run();

    assert_eq!(Arc::strong_count(&arc), 1, "Posted closures should be dropped");

    // Tests a foreign drop, `Collector` will panic if this failed to be dropped.
    std::thread::spawn(move || {
        drop(app);
    })
    .join()
    .unwrap();
}
