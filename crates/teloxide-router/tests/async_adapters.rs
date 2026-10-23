// Single-threaded current_thread runtime: holding the serial guard across
// an await cannot deadlock.
#![allow(clippy::await_holding_lock)]
// always-`Ok` adapter fns: the wrapper types are the contracts under test
#![allow(clippy::unnecessary_wraps)]

//! Sync and async adapter callables, end to end through the attribute macros
//! and `router!`.

#[allow(dead_code)]
mod common;

use std::future::Future;
use teloxide::types::{Me, Message, Update};
use teloxide_router as txr;

#[derive(Clone)]
struct Meta {
    id: i32,
}

#[derive(Clone)]
struct Flag;

#[derive(Clone, Copy)]
struct D1;
#[derive(Clone, Copy)]
struct D2;
#[derive(Clone, Copy)]
struct D3;
#[derive(Clone, Copy)]
struct D4;
#[derive(Clone, Copy)]
struct D5;
#[derive(Clone, Copy)]
struct D6;
#[derive(Clone, Copy)]
struct D7;
#[derive(Clone, Copy)]
struct D8;
#[derive(Clone, Copy)]
struct D9;
#[derive(Clone, Copy)]
struct D10;

// Every callable shape must land on the right probe arm. `filter` carries the
// full matrix; the other adapters share the same probe and emitter, so one
// sync and one async case each covers their wiring.

fn allow(_: Message) -> bool {
    true
}

async fn allow_async(_: Message) -> bool {
    true
}

fn meta_of(_: Message) -> Meta {
    Meta { id: 1 }
}

async fn meta_async(_: Message) -> Meta {
    Meta { id: 2 }
}

fn flag_of(_: Message) -> Option<Flag> {
    Some(Flag)
}

async fn flag_async(_: Message) -> Option<Flag> {
    Some(Flag)
}

txr::router!(H_FILTER_FN);
txr::router!(H_FILTER_AFN);
txr::router!(H_FILTER_CLS);
txr::router!(H_FILTER_AMV);
txr::router!(H_FILTER_ACLS);
txr::router!(H_MAP_FN);
txr::router!(H_MAP_AFN);
txr::router!(H_FMAP_FN);
txr::router!(H_FMAP_AFN);

#[txr::message(H_FILTER_FN, filter = allow)]
async fn h_filter_fn(_: Message) -> txr::Result<()> {
    common::record("filter_fn");
    Ok(())
}

#[txr::message(H_FILTER_AFN, filter = allow_async)]
async fn h_filter_afn(_: Message) -> txr::Result<()> {
    common::record("filter_afn");
    Ok(())
}

#[txr::message(H_FILTER_CLS, filter = |m: Message| m.chat.is_private())]
async fn h_filter_cls(_: Message) -> txr::Result<()> {
    common::record("filter_cls");
    Ok(())
}

#[txr::message(H_FILTER_AMV, filter = |m: Message| async move { m.chat.is_private() })]
async fn h_filter_amv(_: Message) -> txr::Result<()> {
    common::record("filter_amv");
    Ok(())
}

#[txr::message(H_FILTER_ACLS, filter = async |m: Message| m.chat.is_private())]
async fn h_filter_acls(_: Message) -> txr::Result<()> {
    common::record("filter_acls");
    Ok(())
}

#[txr::message(H_MAP_FN, map = meta_of)]
async fn h_map_fn(_: Message, m: Meta) -> txr::Result<()> {
    common::record(if m.id == 1 { "map_fn" } else { "map_fn_bad" });
    Ok(())
}

#[txr::message(H_MAP_AFN, map = meta_async)]
async fn h_map_afn(_: Message, m: Meta) -> txr::Result<()> {
    common::record(if m.id == 2 { "map_afn" } else { "map_afn_bad" });
    Ok(())
}

#[txr::message(H_FMAP_FN, filter_map = flag_of)]
async fn h_fmap_fn(_: Message, _: Flag) -> txr::Result<()> {
    common::record("fmap_fn");
    Ok(())
}

#[txr::message(H_FMAP_AFN, filter_map = flag_async)]
async fn h_fmap_afn(_: Message, _: Flag) -> txr::Result<()> {
    common::record("fmap_afn");
    Ok(())
}

#[tokio::test]
async fn adapters_accept_every_callable_shape() {
    let _ser = common::serial();
    let cases = [
        ("filter_fn", H_FILTER_FN.build()),
        ("filter_afn", H_FILTER_AFN.build()),
        ("filter_cls", H_FILTER_CLS.build()),
        ("filter_amv", H_FILTER_AMV.build()),
        ("filter_acls", H_FILTER_ACLS.build()),
        ("map_fn", H_MAP_FN.build()),
        ("map_afn", H_MAP_AFN.build()),
        ("fmap_fn", H_FMAP_FN.build()),
        ("fmap_afn", H_FMAP_AFN.build()),
    ];
    for (name, handler) in cases {
        assert!(common::handled(&handler, common::MSG).await, "{name}");
        assert_eq!(common::take(), [name], "{name}");
    }
}

// Router-level adapters accept the same callables.

fn upd_allow(_: Update) -> bool {
    true
}

async fn upd_allow_async(_: Update) -> bool {
    true
}

async fn upd_meta(_: Update) -> Meta {
    Meta { id: 3 }
}

async fn upd_flag(_: Update) -> Option<Flag> {
    Some(Flag)
}

txr::router!(SETUP_FILTER, filter = [upd_allow, upd_allow_async]);
txr::router!(SETUP_MAP, map = upd_meta);
txr::router!(SETUP_FMAP, filter_map = upd_flag);

#[txr::message(SETUP_FILTER)]
async fn setup_filter_ep(_: Message) -> txr::Result<()> {
    common::record("setup_filter");
    Ok(())
}

#[txr::message(SETUP_MAP)]
async fn setup_map_ep(_: Message, m: Meta) -> txr::Result<()> {
    common::record(if m.id == 3 {
        "setup_map"
    } else {
        "setup_map_bad"
    });
    Ok(())
}

#[txr::message(SETUP_FMAP)]
async fn setup_fmap_ep(_: Message, _: Flag) -> txr::Result<()> {
    common::record("setup_fmap");
    Ok(())
}

#[tokio::test]
async fn router_adapters_accept_sync_and_async() {
    let _ser = common::serial();
    let cases = [
        ("setup_filter", SETUP_FILTER.build()),
        ("setup_map", SETUP_MAP.build()),
        ("setup_fmap", SETUP_FMAP.build()),
    ];
    for (name, handler) in cases {
        assert!(common::handled(&handler, common::MSG).await, "{name}");
        assert_eq!(common::take(), [name], "{name}");
    }
}

// A plain `fn` returning a future (not `async fn`) is awaited, not inserted
// as a dependency.

#[allow(clippy::manual_async_fn)]
fn future_meta(_: Message) -> impl Future<Output = Meta> {
    async { Meta { id: 4 } }
}

txr::router!(MAP_FUTURE);

#[txr::message(MAP_FUTURE, map = future_meta)]
async fn map_future(_: Message, m: Meta) -> txr::Result<()> {
    common::record(if m.id == 4 {
        "map_future"
    } else {
        "map_future_bad"
    });
    Ok(())
}

#[tokio::test]
async fn map_awaits_future_returning_functions() {
    let _ser = common::serial();
    let h = MAP_FUTURE.build();
    assert!(common::handled(&h, common::MSG).await);
    assert_eq!(common::take(), ["map_future"]);
}

// Mixed lists run in written order and short-circuit at the first rejection.

fn pass_1(_: Message) -> bool {
    common::record("pass_1");
    true
}

async fn pass_2(_: Message) -> bool {
    common::record("pass_2");
    true
}

async fn reject(_: Message) -> bool {
    false
}

txr::router!(MIXED);
txr::router!(MIXED_CUT);

#[txr::message(MIXED, filter = [pass_1, pass_2, |_m: Message| async move { common::record("pass_3"); true }])]
async fn mixed_ep(_: Message) -> txr::Result<()> {
    common::record("endpoint");
    Ok(())
}

#[txr::message(MIXED_CUT, filter = [pass_1, reject, pass_2])]
async fn mixed_cut_ep(_: Message) -> txr::Result<()> {
    common::record("endpoint");
    Ok(())
}

#[tokio::test]
async fn mixed_lists_run_in_written_order_and_short_circuit() {
    let _ser = common::serial();
    let h = MIXED.build();
    assert!(common::handled(&h, common::MSG).await);
    assert_eq!(common::take(), ["pass_1", "pass_2", "pass_3", "endpoint"]);

    let h = MIXED_CUT.build();
    assert!(!common::handled(&h, common::MSG).await);
    assert_eq!(common::take(), ["pass_1"]);
}

// The full 0..=12 arity range dptree's Injectable covers.

txr::router!(ARITY12);

#[txr::message(ARITY12, filter = |_m: Message, _me: Me, _d1: D1, _d2: D2, _d3: D3, _d4: D4, _d5: D5, _d6: D6, _d7: D7, _d8: D8, _d9: D9, _d10: D10| async move { true })]
async fn arity12_ep(_: Message) -> txr::Result<()> {
    common::record("arity12");
    Ok(())
}

#[tokio::test]
async fn adapters_accept_twelve_arguments() {
    let _ser = common::serial();
    let h = ARITY12.build();
    let mut dm = teloxide::dptree::di::DependencyMap::new();
    dm.insert(common::update(common::MSG));
    dm.insert(common::me());
    dm.insert(D1);
    dm.insert(D2);
    dm.insert(D3);
    dm.insert(D4);
    dm.insert(D5);
    dm.insert(D6);
    dm.insert(D7);
    dm.insert(D8);
    dm.insert(D9);
    dm.insert(D10);
    assert!(matches!(
        h.dispatch(dm).await,
        std::ops::ControlFlow::Break(Ok(()))
    ));
    assert_eq!(common::take(), ["arity12"]);
}

// Adapters keep the tree's update-kind interest exact.

txr::router!(DESC_SYNC);
txr::router!(DESC_ASYNC);

#[txr::message(DESC_SYNC, filter = allow)]
async fn desc_sync(_: Message) -> txr::Result<()> {
    Ok(())
}

#[txr::message(DESC_ASYNC, filter = allow_async)]
async fn desc_async(_: Message) -> txr::Result<()> {
    Ok(())
}

#[tokio::test]
async fn adapters_keep_the_update_kind_interest() {
    // `filtered` is a full HashSet whose Debug order is unstable, so compare
    // the observed set, which is the part that selects update kinds.
    let sync = format!("{:?}", DESC_SYNC.build().description());
    let async_ = format!("{:?}", DESC_ASYNC.build().description());
    assert!(sync.contains("observed: {Kind(Message)}"), "{sync}");
    assert!(async_.contains("observed: {Kind(Message)}"), "{async_}");
}
