//! A bot assembled by discovery: each module owns a router, the user module
//! nests a buttons child, and `main` mounts whatever no other router
//! includes, ordered by priority.

use teloxide::{Bot, prelude::*};
use teloxide_router::{self as txr, RouterExt};

// Admin commands behind a router-level filter, so non-admins never reach
// any handler in this module. Tried first through its root priority.
mod admin {
    use teloxide::{prelude::*, utils::command::BotCommands};
    use teloxide_router::{self as txr};

    const ADMINS: &[u64] = &[1];

    fn is_admin(update: Update) -> bool {
        update
            .from()
            .is_some_and(|user| ADMINS.contains(&user.id.0))
    }

    txr::router!(pub ROUTER, priority = 10, filter = is_admin);

    #[derive(BotCommands, Clone)]
    #[command(rename_rule = "lowercase")]
    enum Command {
        Ban(String),
        Stats,
    }

    #[txr::message(ROUTER, command = Command::Ban(username))]
    async fn ban(bot: Bot, msg: Message, username: String) -> txr::Result {
        bot.send_message(msg.chat.id, format!("banning {username}..."))
            .await?;
        Ok(())
    }

    #[txr::message(ROUTER, command = Command::Stats)]
    async fn stats(bot: Bot, msg: Message) -> txr::Result {
        bot.send_message(msg.chat.id, "all systems nominal").await?;
        Ok(())
    }
}

// The user-facing flow: a greeting command and a text echo. The nested
// buttons router is a child, so discovery never returns it; the parent
// builds it together with its own handlers.
mod user {
    use teloxide::{prelude::*, utils::command::BotCommands};
    use teloxide_router::{self as txr};

    mod buttons {
        use teloxide::prelude::*;
        use teloxide_router::{self as txr};

        txr::router!(pub ROUTER);

        #[txr::callback_query(ROUTER)]
        async fn pressed(bot: Bot, q: CallbackQuery) -> txr::Result {
            bot.answer_callback_query(q.id).await?;
            Ok(())
        }
    }

    txr::router!(pub ROUTER, include = [&buttons::ROUTER]);

    #[derive(BotCommands, Clone)]
    #[command(rename_rule = "lowercase")]
    enum Command {
        Start,
    }

    #[txr::message(ROUTER, command = Command::Start)]
    async fn start(bot: Bot, msg: Message) -> txr::Result {
        bot.send_message(msg.chat.id, "hi! send me text and I will echo it")
            .await?;
        Ok(())
    }

    fn plain_text(msg: Message) -> Option<String> {
        msg.text().map(ToOwned::to_owned)
    }

    #[txr::message(ROUTER, filter_map = plain_text)]
    async fn echo(bot: Bot, msg: Message, text: String) -> txr::Result {
        bot.send_message(msg.chat.id, text).await?;
        Ok(())
    }
}

// A last-resort router, tried after the others because of its priority.
mod fallback {
    use teloxide::prelude::*;
    use teloxide_router::{self as txr};

    txr::router!(pub ROUTER, priority = -5);

    #[txr::message(ROUTER)]
    async fn shrug(bot: Bot, msg: Message) -> txr::Result {
        bot.send_message(msg.chat.id, "not sure what to do with that")
            .await?;
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let bot = Bot::from_env();

    // Roots only, ordered admin, user, fallback; buttons stays inside user.
    let handler = txr::entry().include_routers(txr::discover_routers());

    Dispatcher::builder(bot, handler).build().dispatch().await;
}
