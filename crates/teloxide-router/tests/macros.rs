// Single-threaded current_thread runtime: holding the serial guard across
// an await cannot deadlock.
#![allow(clippy::await_holding_lock)]

//! End-to-end tests through the attribute macros and `router!`.

#[allow(dead_code)]
mod common;

use std::ops::ControlFlow;
use teloxide::types::{CallbackQuery, Message, Update};
use teloxide::utils::command::BotCommands;
use teloxide_router as txr;

fn msg_with_text(text: &str) -> String {
    format!(
        r#"{{"update_id":1,"message":{{"message_id":42,"date":1721592028,"chat":{{"id":1,"type":"private"}},"text":"{text}"}}}}"#
    )
}

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

// Commands with unit, tuple and struct variants.

#[derive(BotCommands, Clone, Debug)]
#[command(rename_rule = "lowercase")]
enum Cmd {
    Start,
    Set(u32),
    Get { flag: u32 },
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

#[tokio::test]
async fn command_variants_dispatch() {
    let _ser = common::serial();
    let h = CMDS.build();
    assert!(common::handled(&h, &msg_with_text("/start")).await);
    assert_eq!(common::take(), ["unit"]);
    assert!(common::handled(&h, &msg_with_text("/set 42")).await);
    assert_eq!(common::take(), ["tuple"]);
    assert!(common::handled(&h, &msg_with_text("/get 7")).await);
    assert_eq!(common::take(), ["struct"]);
}

// State adapters match a dependency by pattern.

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

    let dispatch = |dialogue: Dialogue| {
        let h = h.clone();
        // Dispatch with an extra dependency besides Update and Me.
        async move {
            let mut dm = teloxide::dptree::di::DependencyMap::new();
            dm.insert(common::update(common::MSG));
            dm.insert(common::me());
            dm.insert(dialogue);
            matches!(h.dispatch(dm).await, ControlFlow::Break(Ok(())))
        }
    };

    assert!(dispatch(Dialogue::Start).await);
    assert_eq!(common::take(), ["state_start"]);
    assert!(
        dispatch(Dialogue::Ask {
            name: "rust".into()
        })
        .await
    );
    assert_eq!(common::take(), ["state_ask"]);
}
