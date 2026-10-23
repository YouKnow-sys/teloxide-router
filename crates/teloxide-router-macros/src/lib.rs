//! Attribute macros for teloxide-router.
//!
//! Every macro registers a free `async fn` handler on a `static Router` and
//! leaves the function itself unchanged. The first attribute argument is the
//! router; the rest are adapters applied in written order:
//!
//! ```
//! use teloxide_router as txr;
//!
//! txr::router!(ROUTER);
//!
//! #[txr::message(ROUTER, priority = 10, filter = |msg: teloxide::types::Message| msg.chat.is_private())]
//! async fn hello(msg: teloxide::types::Message) -> txr::Result<()> {
//!     let _ = msg;
//!     Ok(())
//! }
//!
//! # #[tokio::main]
//! # async fn main() {
//! let handler = ROUTER.build();
//! // pass `handler` to `Dispatcher::builder`, or dispatch updates yourself
//! # drop(handler);
//! # }
//! ```
//!
//! One attribute exists per update kind (e.g. `callback_query`, `inline_query`,
//! `poll`), named after teloxide's `UpdateFilterExt::filter_*` methods; `update`
//! is the catch-all that runs for every kind.
//!
//! `filter`, `map`, `filter_map`, their fallible `try_` variants and
//! `inspect` accept sync or async functions and closures, freely mixed. A returned future is
//! always awaited: a `map` whose function returns a future inserts the
//! future's output, not the future. Closure parameters need type annotations,
//! the callable must be `Fn`, and it may take at most 12 arguments.
//!
//! The `try_` adapters take functions returning `Result` (with the router's
//! `anyhow` error type): `Ok` flows on (`Ok(false)`/`Ok(None)` falls through
//! to the next handler in case of `try_filter` and `try_filter_map`) while
//! `Err` short-circuits the whole dispatch with that error.

mod adapt;
mod args;
mod crate_path;
mod expand;
mod router_expand;

use proc_macro::TokenStream;

macro_rules! kinds {
    ($(($kind:ident, $filter:ident)),* $(,)?) => {
        $(
            #[doc = concat!("Registers a free `async fn` handler for `", stringify!($filter), "` updates on a `Router`.")]
            #[proc_macro_attribute]
            pub fn $kind(attr: TokenStream, item: TokenStream) -> TokenStream {
                expand::handler(proc_macro2::Ident::new(stringify!($filter), proc_macro2::Span::call_site()), attr, item)
            }
        )*
    };
}

kinds! {
    (message, filter_message),
    (edited_message, filter_edited_message),
    (channel_post, filter_channel_post),
    (edited_channel_post, filter_edited_channel_post),
    (business_connection, filter_business_connection),
    (business_message, filter_business_message),
    (edited_business_message, filter_edited_business_message),
    (deleted_business_messages, filter_deleted_business_messages),
    (message_reaction_updated, filter_message_reaction_updated),
    (message_reaction_count_updated, filter_message_reaction_count_updated),
    (inline_query, filter_inline_query),
    (chosen_inline_result, filter_chosen_inline_result),
    (callback_query, filter_callback_query),
    (shipping_query, filter_shipping_query),
    (pre_checkout_query, filter_pre_checkout_query),
    (purchased_paid_media, filter_purchased_paid_media),
    (poll, filter_poll),
    (poll_answer, filter_poll_answer),
    (my_chat_member, filter_my_chat_member),
    (chat_member, filter_chat_member),
    (chat_join_request, filter_chat_join_request),
    (chat_boost, filter_chat_boost),
    (removed_chat_boost, filter_removed_chat_boost),
}

/// Registers a free `async fn` handler that runs for every update kind.
#[proc_macro_attribute]
pub fn update(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand::update(attr, item)
}

/// Declares a `static Router` with an optional visibility
/// (`router!(pub ROUTER, …)`) along with router-wide filters/maps/state and
/// child routers. Router-level adapters may be sync or async; they run
/// before any of the router's own handlers:
///
/// ```
/// use teloxide_router as txr;
///
/// fn messages_only(update: teloxide::types::Update) -> bool {
///     matches!(update.kind, teloxide::types::UpdateKind::Message(_))
/// }
///
/// txr::router!(pub ROUTER, filter = messages_only);
/// # #[txr::message(ROUTER)]
/// # async fn echo(_: teloxide::types::Message) -> txr::Result<()> { Ok(()) }
/// # #[tokio::main]
/// # async fn main() {
/// let handler = ROUTER.build();
/// // pass `handler` to `Dispatcher::builder`, or dispatch updates yourself
/// # drop(handler);
/// # }
/// ```
///
/// Child routers are included with `include = [&admin::ROUTER]`; the included
/// routers' handlers run before this router's own.
#[proc_macro]
pub fn router(input: TokenStream) -> TokenStream {
    router_expand::expand(input)
}
