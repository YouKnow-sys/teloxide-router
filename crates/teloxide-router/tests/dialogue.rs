// Single-threaded current_thread runtime: holding the serial guard across
// an await cannot deadlock.
#![allow(clippy::await_holding_lock)]

//! Dialogues entered by the `dialogue` router key, driven through dispatches.

#[allow(dead_code)]
mod common;

use std::sync::Arc;
use teloxide::dispatching::UpdateFilterExt;
use teloxide::dispatching::dialogue::{Dialogue, InMemStorage, Storage as _};
use teloxide::types::{CallbackQuery, ChatId, Message, Update};
use teloxide_router::{self as txr, RouterExt};

#[derive(Clone, Default, PartialEq, Debug)]
enum Step {
    #[default]
    Start,
    GotName(String),
}

type StepDialogue = Dialogue<Step, InMemStorage<Step>>;

async fn dispatch<S>(h: &txr::Handler, upd: &str, storage: &Arc<InMemStorage<S>>) -> bool
where
    S: Clone + Send + Sync + 'static,
{
    common::handled_with(h, upd, |dm| dm.insert(storage.clone())).await
}

async fn stored<S>(storage: &Arc<InMemStorage<S>>) -> Option<S>
where
    S: Clone + Send + Sync + 'static,
{
    storage.clone().get_dialogue(ChatId(1)).await.unwrap()
}

fn has_text(msg: Message) -> bool {
    msg.text().is_some()
}

// A multi-step flow: each handler moves the stored state to the next step.

txr::router!(FLOW, dialogue = StepDialogue);

#[txr::message(FLOW, state = Step::Start, filter = has_text)]
async fn flow_start(msg: Message, dialogue: StepDialogue) -> txr::Result {
    common::record("start");
    let name = msg.text().unwrap_or_default().to_owned();
    dialogue.update(Step::GotName(name)).await?;
    Ok(())
}

#[txr::message(FLOW, state = Step::GotName(name), filter = has_text)]
async fn flow_got_name(dialogue: StepDialogue, name: String) -> txr::Result {
    common::record(if name == "rust" {
        "got_name"
    } else {
        "got_bad_name"
    });
    dialogue.reset().await?;
    Ok(())
}

#[tokio::test]
async fn router_dialogue_transitions_state() {
    let _ser = common::serial();
    let h = FLOW.build();
    let storage: Arc<InMemStorage<Step>> = InMemStorage::new();

    assert!(dispatch(&h, &common::msg_with_text("rust"), &storage).await);
    assert_eq!(common::take(), ["start"]);
    assert_eq!(stored(&storage).await, Some(Step::GotName("rust".into())));

    // the stored state routes the second message to the other handler, whose
    // reset writes the default state back into storage
    assert!(dispatch(&h, &common::msg_with_text("again"), &storage).await);
    assert_eq!(common::take(), ["got_name"]);
    assert_eq!(stored(&storage).await, Some(Step::Start));
}

// `dialogue` holds its written position: a filter written first gates it, a
// filter written second runs after the default state was already stored.

fn messages_only(update: Update) -> bool {
    matches!(update.kind, teloxide::types::UpdateKind::Message(_))
}

txr::router!(
    FILTER_FIRST,
    filter = messages_only,
    dialogue = StepDialogue
);
txr::router!(
    DIALOGUE_FIRST,
    dialogue = StepDialogue,
    filter = messages_only
);

#[tokio::test]
async fn dialogue_respects_written_order_with_filter() {
    let _ser = common::serial();

    let h = FILTER_FIRST.build();
    let storage: Arc<InMemStorage<Step>> = InMemStorage::new();
    // the callback has a chat id but fails the filter, so the dialogue is
    // never entered and the default state is never written
    assert!(!dispatch(&h, common::CALLBACK, &storage).await);
    assert_eq!(stored(&storage).await, None);

    let h = DIALOGUE_FIRST.build();
    let storage: Arc<InMemStorage<Step>> = InMemStorage::new();
    // entered (writing the default state) before the filter rejects
    assert!(!dispatch(&h, common::CALLBACK, &storage).await);
    assert_eq!(stored(&storage).await, Some(Step::Start));
}

// Updates without a chat id never enter the dialogue, not even a catch-all.

txr::router!(CATCH, dialogue = StepDialogue);

#[txr::update(CATCH)]
async fn catch_all(_: Update) -> txr::Result {
    common::record("catch_all");
    Ok(())
}

#[tokio::test]
async fn chatless_updates_never_enter_a_dialogue() {
    let _ser = common::serial();
    let h = CATCH.build();
    let storage: Arc<InMemStorage<Step>> = InMemStorage::new();
    assert!(!dispatch(&h, common::INLINE, &storage).await);
    assert!(common::take().is_empty());
}

// Children of a dialogue router run inside its dialogue.

txr::router!(DLG_CHILD);
txr::router!(DLG_PARENT, dialogue = StepDialogue, include = [&DLG_CHILD]);

#[txr::message(DLG_CHILD, state = Step::Start)]
async fn child_in_dialogue(msg: Message, _dialogue: StepDialogue) -> txr::Result {
    let _ = msg;
    common::record("child_in_dialogue");
    Ok(())
}

#[tokio::test]
async fn children_of_a_dialogue_router_see_the_dialogue() {
    let _ser = common::serial();
    let h = DLG_PARENT.build();
    let storage: Arc<InMemStorage<Step>> = InMemStorage::new();
    assert!(dispatch(&h, common::MSG, &storage).await);
    assert_eq!(common::take(), ["child_in_dialogue"]);
}

// `dialogue_on` narrows the update type the chat id is read from; the narrower
// type must already be in the context, here injected by filter_callback_query.

txr::router!(CB, dialogue = StepDialogue, dialogue_on = CallbackQuery);

#[txr::callback_query(CB, state = Step::Start)]
async fn cb_start(_: CallbackQuery, _dialogue: StepDialogue) -> txr::Result {
    common::record("cb_start");
    Ok(())
}

#[tokio::test]
async fn dialogue_on_reads_a_narrower_update_type() {
    let _ser = common::serial();
    let h = <Update as UpdateFilterExt<txr::Output>>::filter_callback_query().include_router(&CB);
    let storage: Arc<InMemStorage<Step>> = InMemStorage::new();
    assert!(dispatch(&h, common::CALLBACK, &storage).await);
    assert_eq!(common::take(), ["cb_start"]);
}

// The real composition: a dialogue over a sum-type state, with the router
// extracting the sub-flow it serves; the entry point matches the outer state.

#[derive(Clone, Default, PartialEq, Debug)]
enum Session {
    #[default]
    New,
    Signup(Signup),
    Support(Support),
}

#[derive(Clone, PartialEq, Debug)]
enum Signup {
    Name,
    Age { name: String },
}

#[derive(Clone, PartialEq, Debug)]
enum Support {
    Ticket,
}

type SessionDialogue = Dialogue<Session, InMemStorage<Session>>;

// Each child takes one sub-flow out of the session state; sibling routers
// are the sound way to serve more than one, since the state list of a single
// router would have to match every pattern at once.
txr::router!(SIGNUP, state = Session::Signup(signup));
txr::router!(SUPPORT, state = Session::Support(support));

// The parent owns the dialogue and the outer-state entry point.
txr::router!(
    SESSION,
    dialogue = SessionDialogue,
    include = [&SIGNUP, &SUPPORT]
);

// Reached while the outer state is the default, before extraction matches.
#[txr::message(SESSION, state = Session::New)]
async fn session_start(msg: Message, dialogue: SessionDialogue) -> txr::Result {
    let _ = msg;
    common::record("session_start");
    dialogue.update(Session::Signup(Signup::Name)).await?;
    Ok(())
}

#[txr::message(SIGNUP, state = Signup::Name, filter = has_text)]
async fn session_name(msg: Message, dialogue: SessionDialogue) -> txr::Result {
    common::record("session_name");
    let name = msg.text().unwrap_or_default().to_owned();
    dialogue
        .update(Session::Signup(Signup::Age { name }))
        .await?;
    Ok(())
}

#[txr::message(SIGNUP, state = Signup::Age { name })]
async fn session_age(dialogue: SessionDialogue, name: String) -> txr::Result {
    common::record(if name == "rust" {
        "session_age"
    } else {
        "session_bad"
    });
    dialogue.reset().await?;
    Ok(())
}

#[tokio::test]
async fn dialogue_drives_an_extracted_substate() {
    let _ser = common::serial();
    let h = SESSION.build();
    let storage: Arc<InMemStorage<Session>> = InMemStorage::new();

    // The default state routes to the entry handler, which starts the flow.
    assert!(dispatch(&h, &common::msg_with_text("rust"), &storage).await);
    assert_eq!(common::take(), ["session_start"]);
    assert_eq!(stored(&storage).await, Some(Session::Signup(Signup::Name)));

    // From here the extracted Signup state drives the handlers.
    assert!(dispatch(&h, &common::msg_with_text("rust"), &storage).await);
    assert_eq!(common::take(), ["session_name"]);
    assert_eq!(
        stored(&storage).await,
        Some(Session::Signup(Signup::Age {
            name: "rust".into()
        }))
    );

    assert!(dispatch(&h, &common::msg_with_text("30"), &storage).await);
    assert_eq!(common::take(), ["session_age"]);
    // reset writes the default back.
    assert_eq!(stored(&storage).await, Some(Session::New));
}

#[txr::message(SUPPORT, state = Support::Ticket)]
async fn support_ticket(dialogue: SessionDialogue) -> txr::Result {
    common::record("support_ticket");
    dialogue.reset().await?;
    Ok(())
}

#[tokio::test]
async fn sibling_routers_each_extract_their_substate() {
    let _ser = common::serial();
    let h = SESSION.build();
    let storage: Arc<InMemStorage<Session>> = InMemStorage::new();

    // Seed the session straight into the support sub-flow.
    storage
        .clone()
        .update_dialogue(ChatId(1), Session::Support(Support::Ticket))
        .await
        .unwrap();

    // The SIGNUP extraction rejects the state, SUPPORT's accepts it.
    assert!(dispatch(&h, &common::msg_with_text("help"), &storage).await);
    assert_eq!(common::take(), ["support_ticket"]);
    assert_eq!(stored(&storage).await, Some(Session::New));
}
