//! Compose teloxide handlers with attribute macros and self-assembling
//! routers.
//!
//! Each attribute macro registers a free `async fn` handler on a
//! [`Router`]; [`Router::build`] collects this router's handlers and folds
//! them into a dptree handler tree.
//!
//! ## Example:
//! ```
//! use teloxide::{prelude::*, sugar::bot::BotMessagesExt};
//! use teloxide_router as txr;
//! use teloxide_router::RouterExt;
//!
//! const ADMINS: &[u64] = &[12345];
//!
//! fn is_admin(update: Update) -> bool {
//!     update.from().is_some_and(|u| ADMINS.contains(&u.id.0))
//! }
//!
//! txr::router!(ADMIN, filter = is_admin);
//! txr::router!(ROUTER, include = [&ADMIN]);
//!
//! #[txr::message(ADMIN)]
//! async fn say_hi_admin(msg: Message, bot: Bot) -> txr::Result<()> {
//!     bot.send_message(msg.chat.id, "Hi admin!");
//!     Ok(())
//! }
//!
//! #[txr::message(ROUTER)]
//! async fn say_hi(msg: Message, bot: Bot) -> txr::Result<()> {
//!     bot.send_message(msg.chat.id, "Hi!");
//!     Ok(())
//! }
//!
//! # #[tokio::main]
//! # async fn main() {
//! let bot = Bot::new("BOT_TOKEN");
//! let handler = txr::entry().include_router(&ROUTER);
//! let dispatcher = Dispatcher::builder(bot, handler).build();
//! // Run it with dispatcher.dispatch().await (polling), or feed it updates
//! // yourself, e.g. from a webhook.
//! # }
//! ```
//!
//! The `filter`, `map`, `filter_map`, `try_` and `inspect` adapters accept
//! sync or async functions and closures; a returned future is always awaited,
//! including for `map`.
//!
//! Because handlers return `anyhow::Result`, the dispatcher's error type is
//! [`anyhow::Error`]. Routers are declared with the `router!` macro;
//! [`Router::build`] panics on inclusion cycles. See [`Router`] for the
//! ordering rules.

#![warn(missing_docs)]
// Not `forbid`: the wasm ctor module needs one locally-allowed `unsafe` call.
#![deny(unsafe_code)]

mod ext;
mod handler;
mod pick;
mod private;
mod registration;
mod router;
mod wasm;

pub use ext::RouterExt;
pub use handler::{Error, Handler, Output, Result, entry};
#[doc(hidden)]
pub use private::__private;
pub use router::Router;
pub use teloxide_router_macros::*;
