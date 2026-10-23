use std::cmp::Reverse;
use std::fmt;

use crate::registration::Registration;
use crate::{Handler, entry};

/// A group of handlers assembled by [`Router::build`].
///
/// Routers are declared with the `router!` macro, which also creates the
/// `static` they live in: construction is not public, so every router is a
/// single, address-stable object.
///
/// ```
/// use teloxide_router as txr;
///
/// txr::router!(ROUTER);
/// # #[txr::message(ROUTER)]
/// # async fn hi(_: teloxide::types::Message) -> txr::Result<()> { Ok(()) }
///
/// # #[tokio::main]
/// # async fn main() {
/// let handler = ROUTER.build();
/// // pass `handler` to `Dispatcher::builder`, or dispatch updates yourself
/// # drop(handler);
/// # }
/// ```
pub struct Router {
    location: (&'static str, u32, u32),
    setup: fn(Handler) -> Handler,
    children: &'static [&'static Router],
}

impl Router {
    pub(crate) const fn new(
        location: (&'static str, u32, u32),
        setup: fn(Handler) -> Handler,
        children: &'static [&'static Router],
    ) -> Self {
        Self {
            location,
            setup,
            children,
        }
    }

    /// Assembles this router's handler tree.
    ///
    /// This router's own adapters from `router!` run first, then child
    /// routers' trees are branched in `include` order, and finally this
    /// router's registered handlers are branched sorted by priority (higher
    /// first) and, on ties, by `file:line:column`. A parent's fallback handler
    /// therefore runs last.
    ///
    /// # Panics
    ///
    /// Panics if this router transitively includes itself, naming both
    /// routers locations. Including the same router twice through unrelated
    /// branches (a diamond) is allowed and not deduplicated.
    #[must_use]
    pub fn build(&'static self) -> Handler {
        crate::wasm::run_ctors();
        self.build_inner(&mut Vec::new())
    }

    fn build_inner(&'static self, stack: &mut Vec<&'static Router>) -> Handler {
        if let Some(includer) = stack.last()
            && stack.iter().any(|r| std::ptr::eq(*r, self))
        {
            panic!(
                "router cycle: router at {} is included again by {}; routers must not include themselves cyclically",
                loc(self.location),
                loc(includer.location),
            );
        }
        stack.push(self);
        let mut handler = (self.setup)(entry());
        for &child in self.children {
            handler = handler.branch(child.build_inner(stack));
        }
        // inventory's iteration order is unspecified; the sort makes dispatch
        // order deterministic.
        let mut registrations: Vec<&Registration> = inventory::iter::<Registration>
            .into_iter()
            .filter(|r| std::ptr::eq(r.router, self))
            .collect();
        registrations.sort_by_key(|r| (Reverse(r.priority), r.location));
        for r in registrations {
            handler = (r.build)(handler);
        }
        stack.pop();
        handler
    }
}

fn loc(location: (&'static str, u32, u32)) -> String {
    format!("{}:{}:{}", location.0, location.1, location.2)
}

impl fmt::Debug for Router {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Router")
            .field("location", &loc(self.location))
            .field("children", &self.children.len())
            .finish_non_exhaustive()
    }
}
