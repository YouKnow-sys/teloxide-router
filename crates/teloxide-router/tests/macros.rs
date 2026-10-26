// Single-threaded current_thread runtime: holding the serial guard across
// an await cannot deadlock.
#![allow(clippy::await_holding_lock)]

//! End-to-end tests through the attribute macros and `router!`.

#[allow(dead_code)]
mod common;

use teloxide::types::{CallbackQuery, Message, Update};
use teloxide::utils::command::BotCommands;
use teloxide_router as txr;

#[derive(Clone)]
struct Settings {
    allowed: bool,
}

// Update kinds route to their own handlers.

txr::router!(KINDS);

#[txr::message(KINDS)]
async fn kinds_msg(_: Message) -> txr::Result<()> {
    common::record("message");
    Ok(())
}

#[txr::callback_query(KINDS)]
async fn kinds_callback(_: CallbackQuery) -> txr::Result<()> {
    common::record("callback_query");
    Ok(())
}

#[tokio::test]
async fn update_kinds_route_to_their_own_handlers() {
    let _ser = common::serial();
    let h = KINDS.build();
    assert!(common::handled(&h, common::MSG).await);
    assert_eq!(common::take(), ["message"]);
    assert!(common::handled(&h, common::CALLBACK).await);
    assert_eq!(common::take(), ["callback_query"]);
}

// List adapters run in written order; router-level maps reach endpoints.

txr::router!(ADAPTERS);

fn flt_a(_: Message) -> bool {
    common::record("flt_a");
    true
}

fn mark_m(_: Message) -> Settings {
    common::record("mark_m");
    Settings { allowed: true }
}

fn flt_b(_: Message, _: Settings) -> bool {
    common::record("flt_b");
    true
}

#[txr::message(ADAPTERS, filter = flt_a, map = mark_m, filter = flt_b)]
async fn adapters_ep(_: Message, _: Settings) -> txr::Result<()> {
    common::record("endpoint");
    Ok(())
}

#[tokio::test]
async fn adapters_run_in_written_order() {
    let _ser = common::serial();
    let h = ADAPTERS.build();
    assert!(common::handled(&h, common::MSG).await);
    assert_eq!(common::take(), ["flt_a", "mark_m", "flt_b", "endpoint"]);
}

// A router-level filter short-circuits every handler in the router.

txr::router!(GATED, filter = reject_all);

fn reject_all(_: Update) -> bool {
    false
}

#[txr::message(GATED)]
async fn gated_ep(_: Message) -> txr::Result<()> {
    common::record("gated");
    Ok(())
}

#[tokio::test]
async fn router_filter_short_circuits_all_handlers() {
    let _ser = common::serial();
    let h = GATED.build();
    assert!(!common::handled(&h, common::MSG).await);
    assert!(common::take().is_empty());
}

// A router-level map is visible to every handler.

txr::router!(ENRICHED, map = load_settings);

fn load_settings(update: Update) -> Settings {
    let allowed = matches!(update.kind, teloxide::types::UpdateKind::Message(_));
    Settings { allowed }
}

#[txr::message(ENRICHED)]
async fn enriched_ep(_: Message, settings: Settings) -> txr::Result<()> {
    common::record(if settings.allowed {
        "saw_settings"
    } else {
        "no_settings"
    });
    Ok(())
}

#[tokio::test]
async fn router_map_is_visible_to_handlers() {
    let _ser = common::serial();
    let h = ENRICHED.build();
    assert!(common::handled(&h, common::MSG).await);
    assert_eq!(common::take(), ["saw_settings"]);
}

// Children run before the parent's own handlers; the parent fallback is last.

txr::router!(NEST_CHILD);
txr::router!(NEST_PARENT, include = [&NEST_CHILD]);

#[txr::message(NEST_CHILD)]
async fn nested_child(_: Message) -> txr::Result<()> {
    common::record("child");
    Ok(())
}

#[txr::message(NEST_PARENT)]
async fn nested_parent_fallback(_: Message) -> txr::Result<()> {
    common::record("parent_fallback");
    Ok(())
}

#[tokio::test]
async fn children_first_then_own_handlers() {
    let _ser = common::serial();
    let h = NEST_PARENT.build();
    assert!(common::handled(&h, common::MSG).await);
    // Both handlers would match; the child branched first, and its Break
    // stops the parent's own (fallback) handler from running.
    assert_eq!(common::take(), ["child"]);
}

// Commands with unit, tuple and struct variants; bindings reach the body.

#[derive(BotCommands, Clone, Debug)]
#[command(rename_rule = "lowercase")]
enum Cmd {
    Start,
    Set(u32),
    #[command(parse_with = "split")]
    Swap(u32, u32),
    Get {
        flag: u32,
    },
}

txr::router!(CMDS);

#[txr::message(CMDS, command = Cmd::Start)]
async fn cmd_start(_: Message) -> txr::Result<()> {
    common::record("unit");
    Ok(())
}

#[txr::message(CMDS, command = Cmd::Set(value))]
async fn cmd_set(_: Message, value: u32) -> txr::Result<()> {
    common::record(if value == 42 { "tuple" } else { "tuple_bad" });
    Ok(())
}

#[txr::message(CMDS, command = Cmd::Get { flag })]
async fn cmd_get(_: Message, flag: u32) -> txr::Result<()> {
    common::record(if flag == 7 { "struct" } else { "struct_bad" });
    Ok(())
}

#[txr::message(CMDS, command = Cmd::Swap(a, b))]
async fn cmd_swap(_: Message, (a, b): (u32, u32)) -> txr::Result {
    common::record(if (a, b) == (4, 2) { "swap" } else { "swap_bad" });
    Ok(())
}

#[tokio::test]
async fn command_variants_dispatch() {
    let _ser = common::serial();
    let h = CMDS.build();
    assert!(common::handled(&h, &common::msg_with_text("/start")).await);
    assert_eq!(common::take(), ["unit"]);
    assert!(common::handled(&h, &common::msg_with_text("/set 42")).await);
    assert_eq!(common::take(), ["tuple"]);
    assert!(common::handled(&h, &common::msg_with_text("/get 7")).await);
    assert_eq!(common::take(), ["struct"]);
    assert!(common::handled(&h, &common::msg_with_text("/swap 4 2")).await);
    assert_eq!(common::take(), ["swap"]);
}

// State adapters filter by variant; their bindings become body locals.

#[derive(Clone)]
enum Dialogue {
    Start,
    Ask { name: String },
}

txr::router!(STATE);

#[txr::message(STATE, state = Dialogue::Start)]
async fn state_start(_: Message) -> txr::Result<()> {
    common::record("state_start");
    Ok(())
}

#[txr::message(STATE, state = Dialogue::Ask { name })]
async fn state_ask(_: Message, name: String) -> txr::Result<()> {
    common::record(if name == "rust" {
        "state_ask"
    } else {
        "state_bad"
    });
    Ok(())
}

#[tokio::test]
async fn state_matches_the_dialogue_dependency() {
    let _ser = common::serial();
    let h = STATE.build();

    assert!(common::handled_with(&h, common::MSG, |dm| dm.insert(Dialogue::Start)).await);
    assert_eq!(common::take(), ["state_start"]);
    assert!(
        common::handled_with(&h, common::MSG, |dm| dm.insert(Dialogue::Ask {
            name: "rust".into()
        }))
        .await
    );
    assert_eq!(common::take(), ["state_ask"]);
}

// Router-level `state` gates the whole router on the variant; the enum stays
// available to handlers.

txr::router!(ROUTER_STATE, state = Dialogue::Ask { name });

#[txr::message(ROUTER_STATE)]
async fn router_state_handler(_: Message, _: Dialogue) -> txr::Result<()> {
    common::record("router_state");
    Ok(())
}

#[tokio::test]
async fn router_state_gates_and_keeps_the_enum() {
    let _ser = common::serial();
    let h = ROUTER_STATE.build();

    assert!(
        common::handled_with(&h, common::MSG, |dm| dm
            .insert(Dialogue::Ask { name: "any".into() }))
        .await
    );
    assert_eq!(common::take(), ["router_state"]);
    // The variant does not match, so the whole router falls through.
    assert!(!common::handled_with(&h, common::MSG, |dm| dm.insert(Dialogue::Start)).await);
    assert!(common::take().is_empty());
}

// The generated adapter block's locals are hygienic: user code naming a
// variable or function `__txr_f` still resolves to its own.

fn __txr_f(m: Message) -> bool {
    m.chat.is_private()
}

txr::router!(HYGIENE);
txr::router!(HYGIENE_LOCAL);

#[txr::message(HYGIENE, filter = __txr_f)]
async fn hygiene_path(_: Message) -> txr::Result<()> {
    common::record("path");
    Ok(())
}

#[txr::message(HYGIENE_LOCAL, filter = |m: Message| {
    let __txr_f = m.chat.is_private();
    if __txr_f {
        common::record("hygiene_local");
    }
    __txr_f
})]
async fn hygiene_local(_: Message) -> txr::Result<()> {
    common::record("local");
    Ok(())
}

#[tokio::test]
async fn generated_locals_do_not_collide_with_user_names() {
    let _ser = common::serial();

    let h = HYGIENE.build();
    assert!(common::handled(&h, common::MSG).await);
    assert_eq!(common::take(), ["path"]);

    let h = HYGIENE_LOCAL.build();
    assert!(common::handled(&h, common::MSG).await);
    assert_eq!(common::take(), ["hygiene_local", "local"]);
}

// Router-level state extraction: the router takes the variant it serves from
// a sum-type state, and its handlers work with the inner state.

#[derive(Clone)]
enum States {
    Users(UserStates),
    Admins,
}

#[derive(Clone)]
enum UserStates {
    ReceiveName,
    ReceiveAge { name: String },
}

txr::router!(EXTRACT, state = States::Users(us));

#[txr::message(EXTRACT, state = UserStates::ReceiveName)]
async fn extract_name(_: Message) -> txr::Result<()> {
    common::record("extract_name");
    Ok(())
}

#[txr::message(EXTRACT, state = UserStates::ReceiveAge { name })]
async fn extract_age(_: Message, name: String) -> txr::Result<()> {
    common::record(if name == "rust" {
        "extract_age"
    } else {
        "extract_bad"
    });
    Ok(())
}

#[tokio::test]
async fn router_state_extracts_the_inner_state() {
    let _ser = common::serial();
    let h = EXTRACT.build();

    let dispatch = |state: States| common::handled_with(&h, common::MSG, |dm| dm.insert(state));

    assert!(
        dispatch(States::Users(UserStates::ReceiveName)).await,
        "the extracted state routes to the handler"
    );
    assert_eq!(common::take(), ["extract_name"]);
    assert!(
        dispatch(States::Users(UserStates::ReceiveAge {
            name: "rust".into()
        }))
        .await
    );
    assert_eq!(common::take(), ["extract_age"]);
    // Other variants of the outer state never reach this router's handlers.
    assert!(!dispatch(States::Admins).await);
    assert!(common::take().is_empty());
}

// Two distinct state enums can gate one router at once; both extractions
// insert their inner state for the handlers below.

#[derive(Clone)]
enum Steps {
    Wizard(WizardStep),
    Done,
}

#[derive(Clone)]
enum WizardStep {
    Name,
}

txr::router!(DUAL, state = [States::Users(us), Steps::Wizard(wizard)]);

#[txr::message(DUAL, state = UserStates::ReceiveName)]
async fn dual_name(_: Message, wizard: WizardStep) -> txr::Result {
    common::record(if matches!(wizard, WizardStep::Name) {
        "dual_name"
    } else {
        "dual_bad"
    });
    Ok(())
}

#[tokio::test]
async fn two_state_enums_must_both_match() {
    let _ser = common::serial();
    let h = DUAL.build();

    let dispatch = |states: States, steps: Steps| {
        common::handled_with(&h, common::MSG, |dm| {
            dm.insert(states);
            dm.insert(steps);
        })
    };

    assert!(
        dispatch(
            States::Users(UserStates::ReceiveName),
            Steps::Wizard(WizardStep::Name)
        )
        .await,
        "both states match, both inner states reach the handler"
    );
    assert_eq!(common::take(), ["dual_name"]);

    // Either enum failing its pattern falls the whole router through.
    assert!(
        !dispatch(States::Admins, Steps::Wizard(WizardStep::Name)).await,
        "States does not match"
    );
    assert!(
        !dispatch(States::Users(UserStates::ReceiveName), Steps::Done).await,
        "Steps does not match"
    );
    assert!(common::take().is_empty());
}
