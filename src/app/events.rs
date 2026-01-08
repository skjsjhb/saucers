use crate::app::App;
use crate::policy::Policy;

/// A trait that handles app events.
pub trait AppEventListener {
    /// Invoked when the app is about to quit.
    ///
    /// Note that this event may be fired multiple times event if it's [`Policy::Allow`]ed, as it's
    /// actually fired inside [`App::quit`]. This also means that [`App::quit`] must not be called
    /// in this listener. Consider using [`crate::app::FinishListener`] if you need a one-time
    /// callback.
    fn on_quit(&self, _app: App) -> Policy { Policy::Allow }
}
