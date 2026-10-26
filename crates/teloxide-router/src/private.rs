//! Implementation details shared with macro expansions; not public API.

#[doc(hidden)]
pub mod __private {
    use teloxide::dispatching::dialogue::{Dialogue, Storage};

    pub use crate::pick::{
        AsyncPick, Probe, SyncPick, filter_description, filter_map_description,
        inspect_description, map_description, try_filter_description, try_filter_map_description,
        try_map_description,
    };
    pub use crate::registration::{Registration, RouterRegistration};
    pub use crate::{Handler, Output};
    pub use inventory;
    pub use teloxide::dispatching::{HandlerExt, UpdateFilterExt};
    pub use teloxide::dptree;
    pub use teloxide::types::Update;

    /// Constructs a [`Router`](crate::Router); used by the `router!` macro.
    pub const fn new_router(
        location: (&'static str, u32, u32),
        setup: fn(crate::Handler) -> crate::Handler,
        children: &'static [&'static crate::Router],
        priority: i32,
    ) -> crate::Router {
        crate::Router::new(location, setup, children, priority)
    }

    /// Splits a `Dialogue<D, S>` type into its state and storage; used by the
    /// `dialogue` router key.
    pub trait DialogueSpec {
        type State: Default + Clone + Send + Sync + 'static;
        type Store: Storage<Self::State, Error: std::fmt::Debug + Send>
            + ?Sized
            + Send
            + Sync
            + 'static;
    }

    impl<D, S> DialogueSpec for Dialogue<D, S>
    where
        S: Storage<D> + ?Sized + Send + Sync + 'static,
        <S as Storage<D>>::Error: std::fmt::Debug + Send,
        D: Default + Clone + Send + Sync + 'static,
    {
        type State = D;
        type Store = S;
    }
}
