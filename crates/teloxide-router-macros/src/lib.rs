//! Procedural macros for teloxide-router.
//!
//! Every attribute macro registers a free `async fn` handler on a `static Router`
//! and leaves the function itself unchanged. The first argument is the target
//! router, followed by adapters applied in written order:
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
//! These macros are re-exported by `teloxide-router`. See its crate
//! documentation for the full list of adapter keys, signatures, and ordering
//! rules.

mod adapt;
mod args;
mod crate_path;
mod handler;
mod router;

use proc_macro::TokenStream;

macro_rules! kinds {
    ($(($kind:ident, $ty:literal)),* $(,)?) => {
        $(
            #[doc = concat!("Registers a free `async fn` handler for `", stringify!($kind), "` updates on a `Router`.")]
            #[doc = ""]
            #[doc = "The first argument names the router; the rest are adapters, applied in"]
            #[doc = "written order. Each adapter takes one callable or a bracketed list of"]
            #[doc = "them, sync or async:"]
            #[doc = ""]
            #[doc = "```ignore"]
            #[doc = "use teloxide_router as txr;"]
            #[doc = ""]
            #[doc = "txr::router!(ROUTER);"]
            #[doc = ""]
            #[doc = concat!("#[txr::", stringify!($kind), "(ROUTER, filter = is_private, map = [parse, log])]")]
            #[doc = concat!("async fn handler(event: teloxide::types::", $ty, ") -> txr::Result<()> { /* ... */ }")]
            #[doc = "```"]
            #[doc = ""]
            #[doc = "`state = Step::Start` matches a state variant and `command = Cmd::Start`"]
            #[doc = "matches a `BotCommands` variant, supplying their bound fields as"]
            #[doc = "dependencies (a tuple for several). `priority = 5` (default 0) orders"]
            #[doc = "handlers within the router, higher first."]
            #[proc_macro_attribute]
            pub fn $kind(attr: TokenStream, item: TokenStream) -> TokenStream {
                handler::attribute(Some(quote::format_ident!("filter_{}", stringify!($kind))), attr, item)
            }
        )*
    };
}

kinds! {
    (message, "Message"),
    (edited_message, "Message"),
    (channel_post, "Message"),
    (edited_channel_post, "Message"),
    (business_connection, "BusinessConnection"),
    (business_message, "Message"),
    (edited_business_message, "Message"),
    (deleted_business_messages, "BusinessMessagesDeleted"),
    (message_reaction_updated, "MessageReactionUpdated"),
    (message_reaction_count_updated, "MessageReactionCountUpdated"),
    (inline_query, "InlineQuery"),
    (chosen_inline_result, "ChosenInlineResult"),
    (callback_query, "CallbackQuery"),
    (shipping_query, "ShippingQuery"),
    (pre_checkout_query, "PreCheckoutQuery"),
    (purchased_paid_media, "PaidMediaPurchased"),
    (poll, "Poll"),
    (poll_answer, "PollAnswer"),
    (my_chat_member, "ChatMemberUpdated"),
    (chat_member, "ChatMemberUpdated"),
    (chat_join_request, "ChatJoinRequest"),
    (chat_boost, "ChatBoostUpdated"),
    (removed_chat_boost, "ChatBoostRemoved"),
}

/// Registers a free `async fn` handler that runs for every update kind.
///
/// Takes the same arguments as the per-kind attributes; a kind-specific
/// handler always runs before it.
#[proc_macro_attribute]
pub fn update(attr: TokenStream, item: TokenStream) -> TokenStream {
    handler::attribute(None, attr, item)
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
///
/// `dialogue = StepDialogue` enters a dialogue for the whole router, like
/// `enter_dialogue` on the entry: it takes a `Dialogue<D, S>` type, and
/// the storage comes from the dispatcher's dependencies. `dialogue_on = T`
/// replaces the default `Update` as the type the chat id is read from; `T` must
/// already be in context where the router is mounted. Updates without a chat id
/// never match a dialogue router.
#[proc_macro]
pub fn router(input: TokenStream) -> TokenStream {
    router::expand(input)
}
