use teloxide_router as txr;

txr::router!(ROUTER);

#[txr::message(ROUTER, command = Cmd::Set(mut value))]
#[allow(dead_code)]
async fn handler(msg: teloxide::types::Message, value: u32) -> txr::Result<()> {
    let _ = (msg, value);
    Ok(())
}

fn main() {}
