use crate::{Handler, Router};

/// `include_router` sugar on handlers, mirroring dptree's builder style.
///
/// ```
/// use teloxide_router::RouterExt;
/// use teloxide_router as txr;
///
/// txr::router!(ADMIN);
/// # #[txr::message(ADMIN)]
/// # async fn hi(_: teloxide::types::Message) -> txr::Result<()> { Ok(()) }
///
/// # #[tokio::main]
/// # async fn main() {
/// let handler = txr::entry().include_router(&ADMIN);
/// # drop(handler);
/// # }
/// ```
pub trait RouterExt {
    /// Branches the router's assembled handler tree into `self`; sugar for
    /// `self.branch(router.build())`.
    fn include_router(self, router: &'static Router) -> Self;

    /// Branches multiple routers' assembled handler trees into `self` in iteration order;
    /// sugar for chaining [`include_router`](Self::include_router) calls.
    fn include_routers(self, routers: impl IntoIterator<Item = &'static Router>) -> Self;
}

impl RouterExt for Handler {
    fn include_router(self, router: &'static Router) -> Self {
        self.branch(router.build())
    }

    fn include_routers(mut self, routers: impl IntoIterator<Item = &'static Router>) -> Self {
        for router in routers.into_iter() {
            self = self.include_router(router);
        }
        self
    }
}
