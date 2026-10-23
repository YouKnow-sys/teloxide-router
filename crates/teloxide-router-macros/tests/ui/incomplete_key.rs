use teloxide_router as txr;

txr::router!(ROUTER);

#[txr::message(ROUTER, fi)]
#[allow(dead_code)]
async fn handler(msg: teloxide::types::Message) -> txr::Result<()> {
    let _ = msg;
    Ok(())
}

fn main() {}
