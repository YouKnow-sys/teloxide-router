//! A multi-step dialogue over in-memory storage: /start asks for the user's
//! first name, then last name, then age, and echoes a summary back. Messages
//! that do not fit the current step fall through to `retry`.

use teloxide::{
    Bot,
    dispatching::dialogue::{Dialogue, InMemStorage},
    prelude::*,
    utils::command::BotCommands,
};
use teloxide_router::{self as txr, RouterExt};

type Storage = InMemStorage<Step>;
type StepDialogue = Dialogue<Step, Storage>;

// Enters the dialogue for every update with a chat id, so handlers below can
// take `StepDialogue` and match `state = ...`.
txr::router!(ROUTER, dialogue = StepDialogue);

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum Cmd {
    Start,
}

#[derive(Clone, Default)]
enum Step {
    #[default]
    Start,
    ReceiveFirstName,
    ReceiveLastName {
        first_name: String,
    },
    ReceiveAge {
        first_name: String,
        last_name: String,
    },
}

fn is_text(msg: Message) -> bool {
    msg.text().is_some()
}

fn age_of(msg: Message) -> Option<u8> {
    msg.text()?.trim().parse().ok()
}

// Registered first, so /start is handled before any state-specific handler.
#[txr::message(ROUTER, command = Cmd::Start)]
async fn start(bot: Bot, msg: Message, dialogue: StepDialogue) -> txr::Result {
    dialogue.update(Step::ReceiveFirstName).await?;
    bot.send_message(msg.chat.id, "What is your first name?")
        .await?;
    Ok(())
}

#[txr::message(ROUTER, state = Step::ReceiveFirstName, filter = is_text)]
async fn first_name(bot: Bot, msg: Message, dialogue: StepDialogue) -> txr::Result {
    let first_name = msg.text().unwrap_or_default().to_owned();
    dialogue
        .update(Step::ReceiveLastName { first_name })
        .await?;
    bot.send_message(msg.chat.id, "What is your last name?")
        .await?;
    Ok(())
}

#[txr::message(ROUTER, state = Step::ReceiveLastName { first_name }, filter = is_text)]
async fn last_name(
    bot: Bot,
    msg: Message,
    dialogue: StepDialogue,
    first_name: String,
) -> txr::Result {
    let last_name = msg.text().unwrap_or_default().to_owned();
    dialogue
        .update(Step::ReceiveAge {
            first_name,
            last_name,
        })
        .await?;
    bot.send_message(msg.chat.id, "How old are you?").await?;
    Ok(())
}

#[txr::message(ROUTER, state = Step::ReceiveAge { first_name, last_name }, filter_map = age_of)]
async fn age(
    bot: Bot,
    msg: Message,
    dialogue: StepDialogue,
    (first_name, last_name): (String, String),
    age: u8,
) -> txr::Result {
    dialogue.reset().await?;
    bot.send_message(
        msg.chat.id,
        format!(
            "You are {first_name} {last_name} and you are {age} years old. Send /start to restart."
        ),
    )
    .await?;
    Ok(())
}

// Catches messages that no handler above accepted.
#[txr::message(ROUTER)]
async fn retry(bot: Bot, msg: Message, step: Step) -> txr::Result {
    let hint = match step {
        Step::ReceiveFirstName | Step::ReceiveLastName { .. } => "Please send text.",
        Step::ReceiveAge { .. } => "Please send your age as a number.",
        Step::Start => "Send /start to begin.",
    };
    bot.send_message(msg.chat.id, hint).await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    let bot = Bot::new("BOT_TOKEN");

    let handler = txr::entry().include_router(&ROUTER);

    Dispatcher::builder(bot, handler)
        .dependencies(teloxide::dptree::deps![Storage::new()])
        .build()
        .dispatch()
        .await;
}
