// Single-threaded current_thread runtime: holding the serial guard across
// an await cannot deadlock.
#![allow(clippy::await_holding_lock)]

//! Ordering and isolation semantics of `Router::build`.

#[allow(dead_code)]
mod common;

use teloxide::types::Message;
use teloxide_router as txr;
use txr::RouterExt;

// Recording filters make the branch order observable: both branches would
// match, but the earlier ones reject, so dispatch falls through them in turn.

txr::router!(PRIO);

#[txr::message(PRIO, priority = 10, filter = |_: Message| { common::record("p10"); false })]
async fn prio_10() -> txr::Result<()> {
    Ok(())
}

#[txr::message(PRIO, priority = 5, filter = |_: Message| { common::record("p5"); false })]
async fn prio_5() -> txr::Result<()> {
    Ok(())
}

#[txr::message(PRIO)]
async fn prio_0() -> txr::Result<()> {
    common::record("p0");
    Ok(())
}

#[tokio::test]
async fn higher_priority_branches_first() {
    let _ser = common::serial();
    let h = PRIO.build();
    assert!(common::handled(&h, common::MSG).await);
    assert_eq!(common::take(), ["p10", "p5", "p0"]);
}

// Equal priority: the earlier registration wins, so only its endpoint runs.

txr::router!(TIE);

#[txr::message(TIE)]
async fn tie_first(_: Message) -> txr::Result<()> {
    common::record("first");
    Ok(())
}

#[txr::message(TIE)]
async fn tie_second(_: Message) -> txr::Result<()> {
    common::record("second");
    Ok(())
}

#[tokio::test]
async fn equal_priority_breaks_ties_by_source_position() {
    let _ser = common::serial();
    let h = TIE.build();
    assert!(common::handled(&h, common::MSG).await);
    assert_eq!(common::take(), ["first"]);
}

// A registration only ever lands on its own router.

txr::router!(ISO_A);
txr::router!(ISO_B);

#[txr::message(ISO_A)]
async fn isolated(_: Message) -> txr::Result<()> {
    common::record("isolated");
    Ok(())
}

#[tokio::test]
async fn routers_are_isolated() {
    let _ser = common::serial();
    let h = ISO_B.build();
    assert!(!common::handled(&h, common::MSG).await);
    assert!(common::take().is_empty());
}

txr::router!(INCL);

#[txr::message(INCL)]
async fn included(_: Message) -> txr::Result<()> {
    common::record("include");
    Ok(())
}

#[tokio::test]
async fn include_router_branches_the_tree() {
    let _ser = common::serial();
    let h = txr::entry().include_router(&INCL);
    assert!(common::handled(&h, common::MSG).await);
    assert_eq!(common::take(), ["include"]);
}

txr::router!(CYCLE_A, include = [&CYCLE_B]);
txr::router!(CYCLE_B, include = [&CYCLE_A]);

#[test]
#[should_panic(expected = "router cycle")]
fn inclusion_cycle_panics() {
    // No serial guard: the panic would poison it. Nothing records here.
    let _ = CYCLE_A.build();
}
