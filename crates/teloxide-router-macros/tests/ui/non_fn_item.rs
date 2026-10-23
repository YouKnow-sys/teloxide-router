use teloxide_router as txr;

txr::router!(ROUTER);

#[txr::message(ROUTER)]
#[allow(dead_code)]
struct Handler;

fn main() {}
