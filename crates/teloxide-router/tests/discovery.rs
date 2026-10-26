// Single-threaded current_thread runtime: holding the serial guard across
// an await cannot deadlock.
#![allow(clippy::await_holding_lock)]

//! Router discovery: roots only, ordered by priority then declaration.

#[allow(dead_code)]
mod common;

use teloxide::types::{Message, Update};
use teloxide_router::{self as txr, RouterExt};

fn rejects_all(_: Update) -> bool {
    false
}

// Roots in ascending declaration order, with mixed priorities.
txr::router!(FIRST_ZERO, filter = rejects_all);
txr::router!(HIGH, priority = 10, filter = rejects_all);
txr::router!(SECOND_ZERO);

// Included routers build with their parent, never on their own; the parent
// itself is still a root.
txr::router!(NESTED);
txr::router!(PARENT, include = [&NESTED]);

#[txr::message(SECOND_ZERO)]
async fn seen(_: Message) -> txr::Result {
    common::record("seen");
    Ok(())
}

#[txr::message(NESTED)]
async fn nested_seen(_: Message) -> txr::Result {
    common::record("nested");
    Ok(())
}

#[tokio::test]
async fn discovered_roots_skip_children_and_sort() {
    let roots = txr::discover_routers();
    let got: Vec<*const txr::Router> = roots.into_iter().map(|r| r as *const _).collect();
    assert_eq!(
        got,
        vec![
            &HIGH as *const _,
            &FIRST_ZERO as *const _,
            &SECOND_ZERO as *const _,
            &PARENT as *const _,
        ]
    );

    // Mounting the discovered roots reaches the handlers behind them.
    let _ser = common::serial();
    let handler = txr::entry().include_routers(txr::discover_routers());
    assert!(common::handled(&handler, common::MSG).await);
    assert_eq!(common::take(), ["seen"]);
}
