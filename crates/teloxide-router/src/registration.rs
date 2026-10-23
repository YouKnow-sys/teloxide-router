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
