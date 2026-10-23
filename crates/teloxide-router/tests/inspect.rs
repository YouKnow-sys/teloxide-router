//! The `inspect` adapter: runs for its side effects without changing the flow.

#[allow(dead_code)]
mod common;

use teloxide::types::{Message, Update};
use teloxide_router as txr;

#[derive(Clone)]
struct Meta;

fn note(_: Message) {
    common::record("note");
}

async fn note_async(_: Message) {
    common::record("note_async");
}

fn note_update(_: Update) {
    common::record("setup_inspect");
}

async fn note_update_async(_: Update) {
    common::record("setup_inspect_async");
}

txr::router!(INSPECT_FN);
txr::router!(INSPECT_CLS);
txr::router!(INSPECT_ORDER);
txr::router!(SETUP_INSPECT, inspect = [note_update, note_update_async]);

#[txr::message(INSPECT_FN, inspect = note)]
async fn inspect_fn(_: Message) -> txr::Result<()> {
    common::record("endpoint");
    Ok(())
}

#[txr::message(INSPECT_CLS, inspect = async |m: Message| {
    common::record(if m.chat.is_private() {
        "note_cls"
    } else {
        "note_cls_bad"
    });
})]
async fn inspect_cls(_: Message) -> txr::Result<()> {
    common::record("endpoint");
    Ok(())
}

#[txr::message(INSPECT_ORDER, filter = |m: Message| { common::record("filter"); m.chat.is_private() }, inspect = note_async, map = |_m: Message| Meta)]
async fn inspect_order(_: Message, _: Meta) -> txr::Result<()> {
    common::record("endpoint");
    Ok(())
}

#[txr::message(SETUP_INSPECT)]
async fn setup_inspect_ep(_: Message) -> txr::Result<()> {
    common::record("endpoint");
    Ok(())
}

// One test, one recording log: no serial guard needed.
#[tokio::test]
async fn inspect_runs_without_changing_the_flow() {
    let h = INSPECT_FN.build();
    assert!(common::handled(&h, common::MSG).await);
    assert_eq!(common::take(), ["note", "endpoint"]);

    let h = INSPECT_CLS.build();
    assert!(common::handled(&h, common::MSG).await);
    assert_eq!(common::take(), ["note_cls", "endpoint"]);

    let h = INSPECT_ORDER.build();
    assert!(common::handled(&h, common::MSG).await);
    assert_eq!(common::take(), ["filter", "note_async", "endpoint"]);

    let h = SETUP_INSPECT.build();
    assert!(common::handled(&h, common::MSG).await);
    assert_eq!(
        common::take(),
        ["setup_inspect", "setup_inspect_async", "endpoint"]
    );
}
