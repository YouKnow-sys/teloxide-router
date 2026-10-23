use teloxide::{Bot, prelude::*, sugar::bot::BotMessagesExt, types::ReplyParameters};
use teloxide_router::{self as txr, RouterExt};

txr::router!(ROUTER);

#[txr::message(ROUTER, filter = |msg: Message| msg.text().is_some_and(|t| t.to_lowercase().starts_with("hi")))]
async fn say_hi(message: Message, bot: Bot) -> txr::Result<()> {
    let fullname = message
        .from
        .map_or_else(|| "unknown-user".to_owned(), |u| u.full_name());

    bot.send_message(message.chat.id, format!("Hi, {fullname}!"))
        .reply_parameters(ReplyParameters::new(message.id))
        .await?;

    Ok(())
}

#[txr::message(ROUTER)]
async fn echo(message: Message, bot: Bot) -> txr::Result<()> {
    bot.copy(message.chat.id, &message).await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    let bot = Bot::new("BOT_TOKEN");
    Dispatcher::builder(bot, txr::entry().include_router(&ROUTER))
        .build()
        .dispatch()
        .await
}
