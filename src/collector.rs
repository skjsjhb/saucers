use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::mpmc::Receiver;
use std::sync::mpmc::Sender;

/// The sharable struct backing [`Collector`] with private access.
///
/// SAFETY: This collector must not be dropped outside the event thread.
pub(crate) struct UnsafeCollector {
    tx: Sender<Box<dyn Collect>>,
    rx: Receiver<Box<dyn Collect>>,
    /// A counter that checks whether there are still unfreed raw handles.
    counter: Arc<()>
}

impl Drop for UnsafeCollector {
    fn drop(&mut self) {
        self.collect();
        let unfreed = Arc::strong_count(&self.counter) - 1;
        if unfreed > 0 {
            panic!("Unfreed handles detected (total {unfreed}). Consider drop this collector later.");
        }
    }
}

impl UnsafeCollector {
    pub(crate) fn collect(&self) {
        while let Ok(a) = self.rx.try_recv() {
            a.collect();
        }
    }

    pub(crate) fn get_sender(&self) -> Sender<Box<dyn Collect>> { self.tx.clone() }

    pub(crate) fn count(&self) -> Arc<()> { self.counter.clone() }

    fn new() -> Self {
        let (tx, rx) = std::sync::mpmc::channel();

        Self {
            tx,
            rx,
            counter: Arc::new(())
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
pub struct Collector(Arc<UnsafeCollector>, PhantomData<*const ()>);

impl Collector {
    pub(crate) fn get_inner(&self) -> Arc<UnsafeCollector> { self.0.clone() }

    pub fn new() -> Self { Self(Arc::new(UnsafeCollector::new()), PhantomData) }
}
