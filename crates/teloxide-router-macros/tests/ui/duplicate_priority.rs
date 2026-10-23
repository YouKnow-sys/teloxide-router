use teloxide_router as txr;

txr::router!(ROUTER);

#[txr::message(ROUTER, priority = 1, priority = 2)]
async fn handler(msg: teloxide::types::Message) -> txr::Result<()> {
    let _ = msg;
    Ok(())
}

fn main() {}
