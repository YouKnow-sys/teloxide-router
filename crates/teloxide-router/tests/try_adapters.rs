// Single-threaded current_thread runtime: holding the serial guard across
// an await cannot deadlock.
#![allow(clippy::await_holding_lock)]
// always-`Ok` adapter fns: the wrapper types are the contracts under test
#![allow(clippy::unnecessary_wraps)]

//! Fallible (`try_*`) adapters, end to end: `Ok` flows, `Ok(false)`/`Ok(None)`
//! fall through to the next handler, `Err` short-circuits the whole dispatch.

#[allow(dead_code)]
mod common;

use std::ops::ControlFlow;
use teloxide::types::{Message, Update};
use teloxide_router as txr;

#[derive(Clone)]
struct Meta {
    id: i32,
}

#[derive(Clone)]
struct Flag;

/// The error message carried by `ControlFlow::Break(Err(..))`, if any.
async fn err_of(h: &txr::Handler, upd: &str) -> Option<String> {
    let mut dm = teloxide::dptree::di::DependencyMap::new();
    dm.insert(common::update(upd));
    dm.insert(common::me());
    match h.dispatch(dm).await {
        ControlFlow::Break(Err(e)) => Some(format!("{e:#}")),
        _ => None,
    }
}

// Ok flows through, sync and async.

fn ok(_: Message) -> txr::Result<bool> {
    Ok(true)
}

async fn ok_async(_: Message) -> txr::Result<bool> {
    Ok(true)
}

fn meta_of(_: Message) -> txr::Result<Meta> {
    Ok(Meta { id: 1 })
}

async fn meta_async(_: Message) -> txr::Result<Meta> {
    Ok(Meta { id: 2 })
}

fn flag_of(_: Message) -> txr::Result<Option<Flag>> {
    Ok(Some(Flag))
}

async fn flag_async(_: Message) -> txr::Result<Option<Flag>> {
    Ok(Some(Flag))
}

txr::router!(T_FILTER);
txr::router!(T_FILTER_A);
txr::router!(T_MAP);
txr::router!(T_MAP_A);
txr::router!(T_FMAP);
txr::router!(T_FMAP_A);

#[txr::message(T_FILTER, try_filter = ok)]
async fn t_filter(_: Message) -> txr::Result<()> {
    common::record("t_filter");
    Ok(())
}

#[txr::message(T_FILTER_A, try_filter = ok_async)]
async fn t_filter_a(_: Message) -> txr::Result<()> {
    common::record("t_filter_a");
    Ok(())
}

#[txr::message(T_MAP, try_map = meta_of)]
async fn t_map(_: Message, m: Meta) -> txr::Result<()> {
    common::record(if m.id == 1 { "t_map" } else { "t_map_bad" });
    Ok(())
}

#[txr::message(T_MAP_A, try_map = meta_async)]
async fn t_map_a(_: Message, m: Meta) -> txr::Result<()> {
    common::record(if m.id == 2 { "t_map_a" } else { "t_map_a_bad" });
    Ok(())
}

#[txr::message(T_FMAP, try_filter_map = flag_of)]
async fn t_fmap(_: Message, _: Flag) -> txr::Result<()> {
    common::record("t_fmap");
    Ok(())
}

#[txr::message(T_FMAP_A, try_filter_map = flag_async)]
async fn t_fmap_a(_: Message, _: Flag) -> txr::Result<()> {
    common::record("t_fmap_a");
    Ok(())
}

#[tokio::test]
async fn try_adapters_flow_ok_through_sync_and_async() {
    let _ser = common::serial();
    let cases = [
        ("t_filter", T_FILTER.build()),
        ("t_filter_a", T_FILTER_A.build()),
        ("t_map", T_MAP.build()),
        ("t_map_a", T_MAP_A.build()),
        ("t_fmap", T_FMAP.build()),
        ("t_fmap_a", T_FMAP_A.build()),
    ];
    for (name, handler) in cases {
        assert!(common::handled(&handler, common::MSG).await, "{name}");
        assert_eq!(common::take(), [name], "{name}");
    }
}

// Ok(false) and Ok(None) fall through to the next handler.

fn ok_false(_: Message) -> txr::Result<bool> {
    common::record("ok_false");
    Ok(false)
}

fn ok_none(_: Message) -> txr::Result<Option<Flag>> {
    common::record("ok_none");
    Ok(None)
}

txr::router!(FALL_FILTER);
txr::router!(FALL_FMAP);

#[txr::message(FALL_FILTER, try_filter = ok_false)]
async fn fall_filter_first(_: Message) -> txr::Result<()> {
    common::record("first_endpoint");
    Ok(())
}

#[txr::message(FALL_FILTER)]
async fn fall_filter_second(_: Message) -> txr::Result<()> {
    common::record("second_endpoint");
    Ok(())
}

#[txr::message(FALL_FMAP, try_filter_map = ok_none)]
async fn fall_fmap_first(_: Message) -> txr::Result<()> {
    common::record("first_endpoint");
    Ok(())
}

#[txr::message(FALL_FMAP)]
async fn fall_fmap_second(_: Message) -> txr::Result<()> {
    common::record("second_endpoint");
    Ok(())
}

#[tokio::test]
async fn ok_false_and_ok_none_fall_through() {
    let _ser = common::serial();
    let h = FALL_FILTER.build();
    assert!(common::handled(&h, common::MSG).await);
    assert_eq!(common::take(), ["ok_false", "second_endpoint"]);

    let h = FALL_FMAP.build();
    assert!(common::handled(&h, common::MSG).await);
    assert_eq!(common::take(), ["ok_none", "second_endpoint"]);
}

// Err short-circuits the whole dispatch: no endpoint runs, no sibling branch
// is tried, and the error is the dispatch result.

fn boom(_: Message) -> txr::Result<bool> {
    Err(txr::Error::msg("boom"))
}

async fn boom_async(_: Message) -> txr::Result<bool> {
    Err(txr::Error::msg("boom_async"))
}

fn boom_map(_: Message) -> txr::Result<Meta> {
    Err(txr::Error::msg("boom_map"))
}

txr::router!(ERR_FILTER);
txr::router!(ERR_FILTER_ASYNC);
txr::router!(ERR_MAP);

#[txr::message(ERR_FILTER, try_filter = boom)]
async fn err_filter_first(_: Message) -> txr::Result<()> {
    common::record("endpoint");
    Ok(())
}

#[txr::message(ERR_FILTER)]
async fn err_filter_second(_: Message) -> txr::Result<()> {
    common::record("endpoint");
    Ok(())
}

#[txr::message(ERR_FILTER_ASYNC, try_filter = boom_async)]
async fn err_filter_async_ep(_: Message) -> txr::Result<()> {
    common::record("endpoint");
    Ok(())
}

#[txr::message(ERR_MAP, try_map = boom_map)]
async fn err_map_ep(_: Message, _: Meta) -> txr::Result<()> {
    common::record("endpoint");
    Ok(())
}

#[tokio::test]
async fn err_short_circuits_the_dispatch() {
    let _ser = common::serial();
    for (router, message) in [
        (ERR_FILTER.build(), "boom"),
        (ERR_FILTER_ASYNC.build(), "boom_async"),
        (ERR_MAP.build(), "boom_map"),
    ] {
        let err = err_of(&router, common::MSG).await;
        assert_eq!(err.as_deref(), Some(message));
    }
    assert!(common::take().is_empty());
}

// Router-level try adapters, sync and async.

fn upd_ok(_: Update) -> txr::Result<bool> {
    Ok(true)
}

async fn upd_ok_async(_: Update) -> txr::Result<bool> {
    Ok(true)
}

txr::router!(SETUP_TRY, try_filter = [upd_ok, upd_ok_async]);

#[txr::message(SETUP_TRY)]
async fn setup_try_ep(_: Message) -> txr::Result<()> {
    common::record("setup_try");
    Ok(())
}

#[tokio::test]
async fn router_try_adapters_accept_sync_and_async() {
    let _ser = common::serial();
    let h = SETUP_TRY.build();
    assert!(common::handled(&h, common::MSG).await);
    assert_eq!(common::take(), ["setup_try"]);
}
