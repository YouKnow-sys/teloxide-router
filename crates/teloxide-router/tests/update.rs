//! The `update` catch-all attribute.

#[allow(dead_code)]
mod common;

use teloxide::types::{Message, Update};
use teloxide_router as txr;

txr::router!(CATCH_ALL);

#[txr::update(CATCH_ALL)]
async fn catch_all(update: Update) -> txr::Result<()> {
    let _ = &update;
    Ok(())
}

#[tokio::test]
async fn catches_updates_of_any_kind() {
    let h = CATCH_ALL.build();
    assert!(common::handled(&h, common::MSG).await);
    assert!(common::handled(&h, common::INLINE).await);
}

// A kind-specific handler takes precedence; the catch-all is the fallback.
// Only the fallback test records, so no serial guard is needed.

txr::router!(WITH_FALLBACK);

#[txr::message(WITH_FALLBACK, priority = 1)]
async fn only_messages(_: Message) -> txr::Result<()> {
    common::record("message");
    Ok(())
}

#[txr::update(WITH_FALLBACK, filter = |_u: Update| true)]
async fn fallback(_: Update) -> txr::Result<()> {
    common::record("fallback");
    Ok(())
}

#[tokio::test]
async fn catch_all_serves_as_the_fallback() {
    let h = WITH_FALLBACK.build();

    assert!(common::handled(&h, common::MSG).await);
    assert_eq!(common::take(), ["message"]);

    assert!(common::handled(&h, common::CALLBACK).await);
    assert_eq!(common::take(), ["fallback"]);
}
