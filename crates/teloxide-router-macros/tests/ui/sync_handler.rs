use teloxide_router as txr;

txr::router!(ROUTER);

#[txr::message(ROUTER)]
#[allow(dead_code)]
fn handler(msg: teloxide::types::Message) -> txr::Result<()> {
    let _ = msg;
    Ok(())
}

fn main() {}
