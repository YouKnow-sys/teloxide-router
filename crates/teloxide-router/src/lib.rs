//! Compose `teloxide` handlers with attribute macros and self-assembling
//! routers.
//!
//! Each attribute macro registers a free, non-generic `async fn` handler on a
//! [`Router`]. [`Router::build`] collects a router's handlers and folds them
//! into a `dptree` handler tree.
//!
//! ## Example
//!
//! ```
//! use teloxide::prelude::*;
//! use teloxide_router::{self as txr, RouterExt};
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
//! async fn say_hi_admin(msg: Message, bot: Bot) -> txr::Result {
//!     bot.send_message(msg.chat.id, "Hi admin!").await?;
//!     Ok(())
//! }
//!
//! #[txr::message(ROUTER)]
//! async fn say_hi(msg: Message, bot: Bot) -> txr::Result {
//!     bot.send_message(msg.chat.id, "Hi!").await?;
//!     Ok(())
//! }
//!
//! # #[tokio::main]
//! # async fn main() {
//! let bot = Bot::new("BOT_TOKEN");
//! let handler = txr::entry().include_router(&ROUTER);
//! let dispatcher = Dispatcher::builder(bot, handler).build();
//! # drop(dispatcher);
//! # }
//! ```
//!
//! ## Attributes and Options
//!
//! Every `UpdateFilterExt::filter_*` method has a matching macro with the
//! `filter_` prefix stripped (e.g. [`message`], [`callback_query`]), plus
//! [`update`] as a catch-all.
//!
//! The first argument is the target [`Router`], followed by `key = value`
//! options applied in written order. Every key accepts a single item or a list
//! (`filter = [a, b]`), except `dialogue` and `dialogue_on`, which take a
//! single type:
//!
//! | Key | Handler | [`router!`] | Behavior |
//! |---|---|---|---|
//! | `filter` | yes | yes | Keep the update if the function returns `true` |
//! | `map` | yes | yes | Insert the return value as a `dptree` dependency |
//! | `filter_map` | yes | yes | Insert the `Some` value; `None` falls through |
//! | `inspect` | yes | yes | Run for side effects without changing flow |
//! | `try_filter`, `try_map`, `try_filter_map` | yes | yes | Fallible variants; `Err` aborts dispatch, `Ok(false)` / `Ok(None)` falls through |
//! | `state` | yes | yes | Match a dialogue state via `dptree::case!` (e.g. `State::Age { name }`); pairs with `dialogue` or an entry-level `enter_dialogue` |
//! | `command` | yes | no | Filter and match a `BotCommands` variant (e.g. `Command::Start`); bound fields arrive as handler dependencies, a tuple for several |
//! | `priority` | yes | no | Handler ordering within a router (default `0`, higher runs first); bound fields arrive as handler dependencies, a tuple for several |
//! | `include` | no | yes | Mount child routers (e.g. `[&admin::ROUTER]`) |
//! | `dialogue` | no | yes | Enter a dialogue over the given `Dialogue<D, S>` type so handlers can take the dialogue and its state |
//! | `dialogue_on` | no | yes | The update type `dialogue` reads the chat id from (default `Update`) |
//!
//! ## Adapters and Error Handling
//!
//! * **Signatures:** Adapters accept sync or async functions and type-annotated
//!   `Fn` closures (taking up to 12 `dptree` dependencies). Returned futures are
//!   always awaited.
//! * **Errors:** Handlers return [`Result`] (`anyhow::Result`), making the
//!   dispatcher's error type [`anyhow::Error`].
//!
//! See [`Router`] for handler ordering and cycle detection rules.

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
