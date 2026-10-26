use crate::{Handler, Router};

/// One collected handler registration; produced by the attribute macros.
pub struct Registration {
    /// The router this registration belongs to, matched by pointer identity.
    pub router: &'static Router,
    /// Sort key; higher priorities are branched earlier.
    pub priority: i32,
    /// `file:line:column` of the attribute; tie-breaker within a priority.
    pub location: (&'static str, u32, u32),
    /// Branches the handler built by this registration onto the tree.
    pub build: fn(Handler) -> Handler,
}

impl std::fmt::Debug for Registration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registration")
            .field("router", &self.location)
            .field("priority", &self.priority)
            .finish_non_exhaustive()
    }
}

inventory::collect!(Registration);

/// One collected router declaration; produced by `router!` so
/// [`discover_routers`](crate::discover_routers) can find them.
pub struct RouterRegistration {
    /// The declared router.
    pub router: &'static Router,
}

impl std::fmt::Debug for RouterRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouterRegistration")
            .field("router", &self.router.location)
            .finish_non_exhaustive()
    }
}

inventory::collect!(RouterRegistration);
