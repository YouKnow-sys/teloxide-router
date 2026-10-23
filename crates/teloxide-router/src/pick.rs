use std::future::Future;
use std::marker::PhantomData;

use teloxide::dptree::HandlerDescription;
use teloxide::dptree::di::Asyncify;

/// Receiver for the sync-or-async `pick` probe; carries no value.
pub struct Probe<F>(PhantomData<fn() -> F>);

impl<F> Probe<F> {
    /// Creates a probe for `F`'s type; used by the macro expansion.
    pub fn of(_: &F) -> Self {
        Probe(PhantomData)
    }
}

// Method probing tries the by-value receiver before the autoref'd one, so
// future-returning functions resolve to `AsyncPick` and everything else to
// `SyncPick`.
/// Probe arm selected when the function returns a future.
pub trait AsyncPick<F, Args> {
    /// Returns the function itself, flagged as async.
    fn pick(self, f: F) -> (bool, F);
}

/// Probe arm selected when the function returns its value directly.
pub trait SyncPick<F, Args> {
    /// Returns the function wrapped for sync use, flagged as sync.
    fn pick(self, f: F) -> (bool, Asyncify<F>);
}

macro_rules! impl_pick {
    ($($g:ident),*) => {
        impl<F, R, $($g,)*> AsyncPick<F, ($($g,)*)> for Probe<F>
        where
            F: Fn($($g),*) -> R,
            R: Future,
        {
            fn pick(self, f: F) -> (bool, F) {
                (true, f)
            }
        }

        impl<F, R, $($g,)*> SyncPick<F, ($($g,)*)> for &Probe<F>
        where
            F: Fn($($g),*) -> R,
        {
            fn pick(self, f: F) -> (bool, Asyncify<F>) {
                (false, Asyncify(f))
            }
        }
    };
}

impl_pick!();
impl_pick!(A1);
impl_pick!(A1, A2);
impl_pick!(A1, A2, A3);
impl_pick!(A1, A2, A3, A4);
impl_pick!(A1, A2, A3, A4, A5);
impl_pick!(A1, A2, A3, A4, A5, A6);
impl_pick!(A1, A2, A3, A4, A5, A6, A7);
impl_pick!(A1, A2, A3, A4, A5, A6, A7, A8);
impl_pick!(A1, A2, A3, A4, A5, A6, A7, A8, A9);
impl_pick!(A1, A2, A3, A4, A5, A6, A7, A8, A9, A10);
impl_pick!(A1, A2, A3, A4, A5, A6, A7, A8, A9, A10, A11);
impl_pick!(A1, A2, A3, A4, A5, A6, A7, A8, A9, A10, A11, A12);

/// The `filter` or `filter_async` description for the probe result.
pub fn filter_description<D: HandlerDescription>(is_async: bool) -> D {
    if is_async {
        D::filter_async()
    } else {
        D::filter()
    }
}

/// The `map` or `map_async` description for the probe result.
pub fn map_description<D: HandlerDescription>(is_async: bool) -> D {
    if is_async { D::map_async() } else { D::map() }
}

/// The `filter_map` or `filter_map_async` description for the probe result.
pub fn filter_map_description<D: HandlerDescription>(is_async: bool) -> D {
    if is_async {
        D::filter_map_async()
    } else {
        D::filter_map()
    }
}

/// The `try_filter` or `try_filter_async` description for the probe result.
pub fn try_filter_description<D: HandlerDescription>(is_async: bool) -> D {
    if is_async {
        D::try_filter_async()
    } else {
        D::try_filter()
    }
}

/// The `try_map` or `try_map_async` description for the probe result.
pub fn try_map_description<D: HandlerDescription>(is_async: bool) -> D {
    if is_async {
        D::try_map_async()
    } else {
        D::try_map()
    }
}

/// The `try_filter_map` or `try_filter_map_async` description for the probe result.
pub fn try_filter_map_description<D: HandlerDescription>(is_async: bool) -> D {
    if is_async {
        D::try_filter_map_async()
    } else {
        D::try_filter_map()
    }
}

/// The `inspect` or `inspect_async` description for the probe result.
pub fn inspect_description<D: HandlerDescription>(is_async: bool) -> D {
    if is_async {
        D::inspect_async()
    } else {
        D::inspect()
    }
}

#[cfg(test)]
#[allow(clippy::unnecessary_wraps)] // the signatures under test are the point
mod tests {
    use std::ops::ControlFlow;

    use teloxide::dptree::HandlerDescription;
    use teloxide::dptree::deps;

    use super::*;

    #[derive(Clone, PartialEq, Debug)]
    struct Rec(Vec<&'static str>);

    impl HandlerDescription for Rec {
        fn entry() -> Self {
            Rec(Vec::new())
        }

        fn user_defined() -> Self {
            Rec(vec!["user"])
        }

        fn merge_chain(&self, other: &Self) -> Self {
            Rec(self.0.iter().chain(&other.0).copied().collect())
        }

        fn merge_branch(&self, other: &Self) -> Self {
            Rec(self.0.iter().chain(&other.0).copied().collect())
        }

        fn map() -> Self {
            Rec(vec!["map"])
        }

        fn map_async() -> Self {
            Rec(vec!["map_async"])
        }

        fn filter() -> Self {
            Rec(vec!["filter"])
        }

        fn filter_async() -> Self {
            Rec(vec!["filter_async"])
        }

        fn filter_map() -> Self {
            Rec(vec!["filter_map"])
        }

        fn filter_map_async() -> Self {
            Rec(vec!["filter_map_async"])
        }

        fn try_filter() -> Self {
            Rec(vec!["try_filter"])
        }

        fn try_filter_async() -> Self {
            Rec(vec!["try_filter_async"])
        }

        fn try_map() -> Self {
            Rec(vec!["try_map"])
        }

        fn try_map_async() -> Self {
            Rec(vec!["try_map_async"])
        }

        fn try_filter_map() -> Self {
            Rec(vec!["try_filter_map"])
        }

        fn try_filter_map_async() -> Self {
            Rec(vec!["try_filter_map_async"])
        }

        fn inspect() -> Self {
            Rec(vec!["inspect"])
        }

        fn inspect_async() -> Self {
            Rec(vec!["inspect_async"])
        }
    }

    // Mirrors the macro's emission so the probe, constructor and description
    // helper run exactly as users get them; the recording description makes
    // the chosen arm visible.
    macro_rules! description {
        ($out:ty, $ctor:ident, $desc:ident, $f:expr) => {{
            #[allow(unused_imports)]
            use crate::pick::{AsyncPick as _, SyncPick as _};
            let f = $f;
            let (is_async, f) = Probe::of(&f).pick(f);
            teloxide::dptree::entry::<$out, Rec>()
                .chain(teloxide::dptree::$ctor($desc::<Rec>(is_async), f))
                .description()
                .0
                .clone()
        }};
    }

    fn sync_pred(x: i32) -> bool {
        x > 0
    }

    async fn async_pred(x: i32) -> bool {
        x > 0
    }

    fn sync_proj(x: i32) -> i64 {
        x as i64
    }

    async fn async_proj(x: i32) -> i64 {
        x as i64
    }

    fn sync_opt(x: i32) -> Option<bool> {
        Some(x > 0)
    }

    async fn async_opt(x: i32) -> Option<bool> {
        Some(x > 0)
    }

    fn sync_note(_: i32) {}

    async fn async_note(_: i32) {}

    fn sync_try_filter(x: i32) -> Result<bool, String> {
        Ok(x > 0)
    }

    async fn async_try_filter(x: i32) -> Result<bool, String> {
        Ok(x > 0)
    }

    fn sync_try_map(x: i32) -> Result<i64, String> {
        Ok(x as i64)
    }

    async fn async_try_map(x: i32) -> Result<i64, String> {
        Ok(x as i64)
    }

    fn sync_try_opt(x: i32) -> Result<Option<bool>, String> {
        Ok(Some(x > 0))
    }

    async fn async_try_opt(x: i32) -> Result<Option<bool>, String> {
        Ok(Some(x > 0))
    }

    #[test]
    fn descriptions_follow_the_probe() {
        assert_eq!(
            description!(
                (),
                filter_async_with_description,
                filter_description,
                sync_pred
            ),
            ["filter"]
        );
        assert_eq!(
            description!(
                (),
                filter_async_with_description,
                filter_description,
                async_pred
            ),
            ["filter_async"]
        );
        assert_eq!(
            description!((), map_async_with_description, map_description, sync_proj),
            ["map"]
        );
        assert_eq!(
            description!((), map_async_with_description, map_description, async_proj),
            ["map_async"]
        );
        assert_eq!(
            description!(
                (),
                filter_map_async_with_description,
                filter_map_description,
                sync_opt
            ),
            ["filter_map"]
        );
        assert_eq!(
            description!(
                (),
                filter_map_async_with_description,
                filter_map_description,
                async_opt
            ),
            ["filter_map_async"]
        );
        assert_eq!(
            description!(
                (),
                inspect_async_with_description,
                inspect_description,
                sync_note
            ),
            ["inspect"]
        );
        assert_eq!(
            description!(
                (),
                inspect_async_with_description,
                inspect_description,
                async_note
            ),
            ["inspect_async"]
        );
        assert_eq!(
            description!(Result<(), String>, try_filter_async_with_description, try_filter_description, sync_try_filter),
            ["try_filter"]
        );
        assert_eq!(
            description!(Result<(), String>, try_filter_async_with_description, try_filter_description, async_try_filter),
            ["try_filter_async"]
        );
        assert_eq!(
            description!(Result<(), String>, try_map_async_with_description, try_map_description, sync_try_map),
            ["try_map"]
        );
        assert_eq!(
            description!(Result<(), String>, try_map_async_with_description, try_map_description, async_try_map),
            ["try_map_async"]
        );
        assert_eq!(
            description!(Result<(), String>, try_filter_map_async_with_description, try_filter_map_description, sync_try_opt),
            ["try_filter_map"]
        );
        assert_eq!(
            description!(Result<(), String>, try_filter_map_async_with_description, try_filter_map_description, async_try_opt),
            ["try_filter_map_async"]
        );
    }

    #[tokio::test]
    async fn probed_handlers_dispatch() {
        // The picked constructor must run the function, not just type-check.
        use crate::pick::{AsyncPick as _, SyncPick as _};

        let f = sync_pred;
        let (is_async, f) = Probe::of(&f).pick(f);
        assert!(!is_async);
        let h = teloxide::dptree::entry::<(), Rec>()
            .chain(teloxide::dptree::filter_async_with_description(
                filter_description::<Rec>(false),
                f,
            ))
            .endpoint(|x: i32| async move { assert!(x > 0) });
        assert_eq!(h.dispatch(deps![1]).await, ControlFlow::Break(()));

        let f = async_pred;
        let (is_async, f) = Probe::of(&f).pick(f);
        assert!(is_async);
        let h = teloxide::dptree::entry::<(), Rec>()
            .chain(teloxide::dptree::filter_async_with_description(
                filter_description::<Rec>(is_async),
                f,
            ))
            .endpoint(|x: i32| async move { assert!(x > 0) });
        assert_eq!(h.dispatch(deps![1]).await, ControlFlow::Break(()));
    }

    #[test]
    fn zero_arity_probes() {
        // Arity 0 is covered by the macro table but has no injectable deps,
        // so only the probe itself is exercised here.
        let f = || async { true };
        #[allow(unused_imports)]
        use crate::pick::{AsyncPick as _, SyncPick as _};
        let (is_async, f) = Probe::of(&f).pick(f);
        assert!(is_async);
        let _call = f;
    }
}
