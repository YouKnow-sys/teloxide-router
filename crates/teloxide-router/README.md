# teloxide-router [![Rust CI](https://github.com/YouKnow-sys/teloxide-router/actions/workflows/ci.yml/badge.svg)](https://github.com/YouKnow-sys/teloxide-router/actions/workflows/ci.yml) [![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/YouKnow-sys/teloxide-router/blob/master/LICENSE)

Register `teloxide` handlers where you write them, and let the router assemble itself.

`teloxide` builds dispatch trees with `dptree`, which often separates routing logic (`Update::filter_message()`, `filter_command`, priorities) from the handlers themselves. `teloxide-router` keeps them together: attribute macros register handlers directly onto a router, assembling a `dptree` handler tree automatically at startup.

## Example

```rust
use teloxide::{prelude::*, utils::command::BotCommands};
use teloxide_router::{self as txr, RouterExt};

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum Command {
    Start,
}

// Declare a router (supports router-level filters, maps, states, and child routers)
txr::router!(pub ROUTER, filter = from_humans);

fn from_humans(update: Update) -> bool {
    update.from().is_some_and(|user| !user.is_bot)
}

#[txr::message(ROUTER, priority = 10, command = Command::Start)]
async fn start(bot: Bot, msg: Message) -> txr::Result {
    bot.send_message(msg.chat.id, "hi").await?;
    Ok(())
}

fn is_admin_button(q: CallbackQuery) -> bool {
    q.data.as_deref().is_some_and(|d| d.starts_with("admin:"))
}

#[txr::callback_query(ROUTER, filter = is_admin_button)]
async fn on_button(bot: Bot, q: CallbackQuery) -> txr::Result {
    bot.answer_callback_query(q.id).await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    let bot = Bot::from_env();
    let handler = txr::entry().include_router(&ROUTER);
    Dispatcher::builder(bot, handler).build().dispatch().await;
}
```

## Features

* **All update kinds:** Every `UpdateFilterExt::filter_*` method has a matching macro (`#[txr::message]`, `#[txr::callback_query]`, `#[txr::chat_member]`, etc.), plus `#[txr::update]` as a catch-all.
* **Rich adapters:** Chain `filter`, `map`, `filter_map`, `inspect`, dialogue `state`, `command`, and fallible `try_*` variants on both handlers and routers.
* **Modular composition:** Nest routers across modules with `include = [&admin::ROUTER]`.
* **WASM ready:** Works on `wasm32-unknown-unknown`.

See the [API documentation](https://docs.rs/teloxide-router) for the full list of attribute keys, adapter signatures, and handler ordering rules.

## Installation

```toml
[dependencies]
teloxide-router = "0.1"
# or txr = { package = "teloxide-router", version = "0.1" }
# if you want easier access without manual `teloxide_router as txr`
```
