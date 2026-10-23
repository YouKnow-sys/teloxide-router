use teloxide_router as txr;

txr::router!(ROUTER);

#[txr::message(ROUTER)]
async fn handler<T>(msg: teloxide::types::Message) -> txr::Result<()> {
    let _ = msg;
    Ok(())
}

fn main() {}
