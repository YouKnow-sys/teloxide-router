use teloxide::{dispatching::DpHandlerDescription, dptree};

/// The error type produced by handlers; locked to `anyhow`.
pub type Error = anyhow::Error;

/// The result type produced by handlers.
pub type Result<T = ()> = anyhow::Result<T>;

/// The output type of every handler in a router tree.
pub type Output = Result<()>;

/// A teloxide dispatching handler with this crate's output type.
pub type Handler = dptree::Handler<'static, Output, DpHandlerDescription>;

/// An empty dptree entry typed for this crate's [`Handler`].
#[must_use]
pub fn entry() -> Handler {
    dptree::entry()
}
