//! Shared emission for the three predicate/projection adapters, which accept
//! sync or async functions and closures.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Expr;

pub(crate) enum Combinator {
    Filter,
    Map,
    FilterMap,
    TryFilter,
    TryMap,
    TryFilterMap,
    Inspect,
}

pub(crate) fn adapter(krate: &TokenStream, combinator: Combinator, expr: &Expr) -> TokenStream {
    let (constructor, description) = match combinator {
        Combinator::Filter => (
            quote!(filter_async_with_description),
            quote!(filter_description),
        ),
        Combinator::Map => (quote!(map_async_with_description), quote!(map_description)),
        Combinator::FilterMap => (
            quote!(filter_map_async_with_description),
            quote!(filter_map_description),
        ),
        Combinator::TryFilter => (
            quote!(try_filter_async_with_description),
            quote!(try_filter_description),
        ),
        Combinator::TryMap => (
            quote!(try_map_async_with_description),
            quote!(try_map_description),
        ),
        Combinator::TryFilterMap => (
            quote!(try_filter_map_async_with_description),
            quote!(try_filter_map_description),
        ),
        Combinator::Inspect => (
            quote!(inspect_async_with_description),
            quote!(inspect_description),
        ),
    };
    quote! {
        {
            #[allow(unused_imports)]
            use #krate::__private::{AsyncPick as _, SyncPick as _};
            let __txr_f = #expr;
            let (__txr_async, __txr_f) = #krate::__private::Probe::of(&__txr_f).pick(__txr_f);
            #krate::__private::dptree::#constructor(
                #krate::__private::#description(__txr_async),
                __txr_f,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn emits_the_right_constructor_and_description() {
        let krate = quote!(::teloxide_router);
        let expr: Expr = syn::parse2(quote!(f)).unwrap();

        let filter = adapter(&krate, Combinator::Filter, &expr).to_string();
        assert!(filter.contains("filter_async_with_description"));
        assert!(filter.contains("filter_description"));

        let map = adapter(&krate, Combinator::Map, &expr).to_string();
        assert!(map.contains("map_async_with_description"));
        assert!(map.contains("map_description"));

        let filter_map = adapter(&krate, Combinator::FilterMap, &expr).to_string();
        assert!(filter_map.contains("filter_map_async_with_description"));
        assert!(filter_map.contains("filter_map_description"));

        let try_filter = adapter(&krate, Combinator::TryFilter, &expr).to_string();
        assert!(try_filter.contains("try_filter_async_with_description"));
        assert!(try_filter.contains("try_filter_description"));

        let try_map = adapter(&krate, Combinator::TryMap, &expr).to_string();
        assert!(try_map.contains("try_map_async_with_description"));
        assert!(try_map.contains("try_map_description"));

        let try_filter_map = adapter(&krate, Combinator::TryFilterMap, &expr).to_string();
        assert!(try_filter_map.contains("try_filter_map_async_with_description"));
        assert!(try_filter_map.contains("try_filter_map_description"));

        let inspect = adapter(&krate, Combinator::Inspect, &expr).to_string();
        assert!(inspect.contains("inspect_async_with_description"));
        assert!(inspect.contains("inspect_description"));
    }
}
