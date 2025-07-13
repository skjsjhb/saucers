use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread::ThreadId;

/// The sharable struct backing [`Collector`] with private access.
///
/// SAFETY: This collector must not be dropped outside the event thread.
pub(crate) struct UnsafeCollector {
    tx: Sender<Box<dyn Collect>>,
    rx: Receiver<Box<dyn Collect>>,
    /// A counter that checks whether there are still unfreed raw handles.
    counter: Arc<()>,
    host_thread: ThreadId
}

// A collector holds a receiver which is `!Send`. However, the collector only accesses the receiver object on the
// thread that creates it, such guarantee is hold by assertions and should not cause problem.
unsafe impl Send for UnsafeCollector {}
unsafe impl Sync for UnsafeCollector {}

impl Drop for UnsafeCollector {
    fn drop(&mut self) {
        self.try_collect();
        let unfreed = Arc::strong_count(&self.counter) - 1;
        if unfreed > 0 {
            panic!("Unfreed handles detected (total {unfreed}). Consider drop this collector later.");
        }
    }
}

impl UnsafeCollector {
    pub(crate) fn try_collect(&self) {
        self.assert_host_thread();
        while let Ok(a) = self.rx.try_recv() {
            a.collect();
        }
    }

    pub(crate) fn collect_now(&self) {
        self.assert_host_thread();
        // New handles cannot be created and existing handles are only possible to be dropped in the loop below,
        // making this count reliable.
        let mut eh = Arc::strong_count(&self.counter);

        while eh > 1 {
            self.rx.recv().unwrap().collect();
            eh -= 1;
        }
    }

    pub(crate) fn get_sender(&self) -> Sender<Box<dyn Collect>> { self.tx.clone() }

    pub(crate) fn count(&self) -> Arc<()> { self.counter.clone() }

    fn assert_host_thread(&self) {
        if self.host_thread != std::thread::current().id() {
            panic!("Collection cannot happen outside the host thread.")
        }
    }

    fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();

        Self {
            tx,
            rx,
            counter: Arc::new(()),
            host_thread: std::thread::current().id()
        }
    }
}

pub(crate) trait Collect: Send + Sync {
    fn collect(self: Box<Self>);
}

/// Resource management helper.
///
/// Automatic cleanup jobs in Saucer can be very hard due to the fact that most handles must be dropped on the event
/// thread. This struct provides a guard to collect drop requests and free the passed handles with guaranteed free
/// (default) or best-effort (configurable).
///
/// A collector must be created before creating any [`crate::app::App`]s or [`crate::webview::Webview`]s. The collector
/// being referred by these [`crate::app::App`]s and [`crate::webview::Webview`] must not be dropped until these
/// handlers are dropped. Failing to follow this rule will likely to cause resource leaks and the program will panic
/// with a message like:
///
/// ```text
/// Unfreed handles detected (total X). Consider drop this collector later.
/// ```
///
/// Note that this behavior can also be useful to detect circular references.
///
/// # Panics
///
/// As described above, panics when there are remaining handlers when being dropped.
///
/// # Example
///
/// Most of the time, dropping of this guard happens implicitly, thanks to the order of dropping:
///
/// ```
/// use saucers::app::App;
/// use saucers::collector::Collector;
/// use saucers::options::AppOptions;
/// fn main() {
///     let cc = Collector::new();
///     let app = App::create(&cc, AppOptions::new("app_id"));
///
///     // The following happens implicitly:
///     // drop(app);
///     // drop(cc);
/// }
/// ```
///
/// However, such order may get broken when it comes to threads or event handlers, as they may hold a handle longer than
/// expected:
///
/// ```no_run
/// use saucers::app::App;
/// use saucers::collector::Collector;
/// use saucers::options::AppOptions;
///
/// fn main() {
///     let cc = Collector::new();
///     let app = App::create(&cc, AppOptions::new("app_id"));
///
///     std::thread::spawn(move || {
///         let _ = &app;
///     });
///
///     // Oh, no! That thread might still hold the `app` handle!
///     // The implicit call below might panic:
///     // drop(cc);
/// }
/// ```
///
/// Make sure to join the threads and clean event handlers before leaving:
///
/// ```
/// use saucers::app::App;
/// use saucers::collector::Collector;
/// use saucers::options::AppOptions;
///
/// fn main() {
///     let cc = Collector::new();
///     let app = App::create(&cc, AppOptions::new("app_id"));
///
///     let th = std::thread::spawn(move || {
///         let _ = &app;
///     });
///
///     // Make sure to join the thread
///     let _ = th.join();
///
///     // The implicit drop is now fine:
///     // drop(cc);
/// }
/// ```
///
/// Things can get even more complex when async handlers are involved, as these threads are created by saucer. One
/// solution is to use semaphores. Alternatively, use the [`Collector::collect_now`] method.
pub struct Collector(Arc<UnsafeCollector>, PhantomData<*const ()>);

impl Collector {
    pub(crate) fn get_inner(&self) -> Arc<UnsafeCollector> { self.0.clone() }

    /// Creates a new collector.
    ///
    /// Once a collector is created, it must be held until all handlers referencing it has been dropped. See [Collector]
    /// for details.
    pub fn new() -> Self { Self(Arc::new(UnsafeCollector::new()), PhantomData) }

    /// Polls and drops all handles, blocks if needed.
    ///
    /// This method eliminates the need of joining all threads and async event handlers, but may worsen a circular
    /// reference into a deadlock. Calls to this method will block the event thread, given that it only returns after
    /// all handles are dropped, one would probably want to [`drop`] handles on the event thread explicitly before
    /// invoking this method.
    ///
    /// # Example
    ///
    /// ```
    /// use saucers::app::App;
    /// use saucers::collector::Collector;
    /// use saucers::options::AppOptions;
    ///
    /// fn main() {
    ///     let cc = Collector::new();
    ///     let app = App::create(&cc, AppOptions::new("app_id"));
    ///
    ///     std::thread::spawn(move || {
    ///         let _ = &app;
    ///     });
    ///
    ///     // This will wait for the thread to quit
    ///     cc.collect_now();
    /// }
    /// ```
    pub fn collect_now(&self) { self.0.collect_now() }
}

impl Default for Collector {
    fn default() -> Self { Self::new() }
}
