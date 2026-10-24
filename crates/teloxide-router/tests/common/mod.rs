use std::sync::{Mutex, MutexGuard};
use teloxide::types::{Me, Update, User, UserId};
use teloxide_router as txr;

pub const MSG: &str = r#"{"update_id":1,"message":{"message_id":42,"date":1721592028,"chat":{"id":1,"type":"private"},"text":"hi"}}"#;

pub const CALLBACK: &str = r#"{"update_id":2,"callback_query":{"id":"cb1","chat_instance":"x","from":{"id":1,"is_bot":false,"first_name":"A"},"data":"payload","message":{"message_id":7,"date":1721592028,"chat":{"id":1,"type":"private"},"text":"page"}}}"#;

pub const INLINE: &str = r#"{"update_id":3,"inline_query":{"id":"q1","query":"hi","offset":"","from":{"id":1,"is_bot":false,"first_name":"A"}}}"#;

pub fn update(json: &str) -> Update {
    serde_json::from_str(json).expect("valid fixture update")
}

/// `Me` fixture; `filter_command` requires a bot username, so one is set.
pub fn me() -> Me {
    Me {
        user: User {
            id: UserId(42),
            is_bot: true,
            first_name: "First".to_owned(),
            last_name: None,
            username: Some("TestBot".to_owned()),
            language_code: None,
            is_premium: false,
            added_to_attachment_menu: false,
        },
        can_join_groups: false,
        can_read_all_group_messages: false,
        supports_inline_queries: false,
        can_connect_to_business: false,
        has_main_web_app: false,
    }
}

/// Dispatches a fabricated update and reports whether a handler handled it.
pub async fn handled(h: &txr::Handler, upd: &str) -> bool {
    let mut dm = teloxide::dptree::di::DependencyMap::new();
    dm.insert(update(upd));
    dm.insert(me());
    matches!(h.dispatch(dm).await, std::ops::ControlFlow::Break(Ok(())))
}

// The whole binary shares one ORDER log, so tests that assert on it must not
// overlap: hold the SERIAL guard for the duration of the test.
static SERIAL: Mutex<()> = Mutex::new(());
static ORDER: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

pub fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap()
}

pub fn record(name: &'static str) {
    ORDER.lock().unwrap().push(name);
}

pub fn take() -> Vec<&'static str> {
    std::mem::take(&mut *ORDER.lock().unwrap())
}
