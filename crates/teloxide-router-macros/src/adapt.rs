//! Chain steps shared by the handler attributes and `router!`.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned as _;
use syn::{Expr, Pat, Path, Type};

macro_rules! combinators {
    ($(($key:literal, $variant:ident)),* $(,)?) => {
        /// The adapter keys that take callables, in the order they appear
        /// in error messages.
        pub(crate) const COMBINATORS: &[&str] = &[$($key),*];

        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub(crate) enum Combinator {
            $($variant),*
        }

        impl Combinator {
            pub(crate) fn from_key(key: &str) -> Option<Self> {
                match key {
                    $($key => Some(Self::$variant),)*
                    _ => None,
                }
            }

            fn constructor_and_description(self) -> (proc_macro2::Ident, proc_macro2::Ident) {
                match self {
                    $(Self::$variant => (
                        format_ident!("{}_async_with_description", $key),
                        format_ident!("{}_description", $key),
                    ),)*
                }
            }
        }
    };
}

combinators! {
    ("filter", Filter),
    ("map", Map),
    ("filter_map", FilterMap),
    ("try_filter", TryFilter),
    ("try_map", TryMap),
    ("try_filter_map", TryFilterMap),
    ("inspect", Inspect),
}

pub(crate) enum Link {
    Call {
        combinator: Combinator,
        expr: Expr,
    },
    State(StatePat),
    Dialogue {
        spec: Box<Type>,
        on: Option<Box<Type>>,
    },
}

/// A `state`/`command` variant pattern, applied as a `dptree::case!` step;
/// `enum_path` also feeds the `filter_command` step.
pub(crate) struct StatePat {
    pub(crate) enum_path: Path,
    pub(crate) pat: Pat,
}

impl StatePat {
    /// Binds the pattern's fields into the dependency map, a tuple for
    /// several.
    pub(crate) fn case_chain(&self, krate: &TokenStream, base: TokenStream) -> TokenStream {
        let pat = &self.pat;
        quote! { #base.chain(#krate::__private::dptree::case![#pat]) }
    }
}

impl Link {
    pub(crate) fn chain(&self, krate: &TokenStream, base: TokenStream) -> TokenStream {
        match self {
            Link::Call { combinator, expr } => {
                let (constructor, description) = combinator.constructor_and_description();
                // `mixed_site` so the scaffolding's locals cannot collide
                // with names in `expr`; interpolated tokens keep their spans.
                let step = quote_spanned! {Span::mixed_site()=>
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
                };
                quote! { #base.chain(#step) }
            }
            Link::State(pat) => pat.case_chain(krate, base),
            Link::Dialogue { spec, on } => {
                let on = match on {
                    Some(ty) => quote! { #ty },
                    None => quote! { #krate::__private::Update },
                };
                // Spanned on the user's type so unmet bounds point at the
                // `dialogue = ...` value, not at the macro.
                quote_spanned! {spec.span()=>
                    #krate::__private::HandlerExt::enter_dialogue::<
                        #on,
                        <#spec as #krate::__private::DialogueSpec>::Store,
                        <#spec as #krate::__private::DialogueSpec>::State,
                    >(#base)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn every_key_round_trips_and_emits_its_idents() {
        let krate = quote!(::teloxide_router);
        let expr: Expr = syn::parse2(quote!(f)).unwrap();
        for key in COMBINATORS {
            let combinator = Combinator::from_key(key).unwrap();
            let link = Link::Call {
                combinator,
                expr: expr.clone(),
            };
            let tokens = link.chain(&krate, quote!(h)).to_string();
            assert!(
                tokens.contains(&format!("{key}_async_with_description")),
                "{key}"
            );
            assert!(tokens.contains(&format!("{key}_description")), "{key}");
        }
        assert_eq!(Combinator::from_key("flat_map"), None);
    }

    #[test]
    fn dialogue_emits_enter_dialogue_with_projections() {
        let krate = quote!(::teloxide_router);
        let spec: Type = syn::parse2(quote!(StepDialogue)).unwrap();
        let on: Type = syn::parse2(quote!(CallbackQuery)).unwrap();

        let default = Link::Dialogue {
            spec: Box::new(spec.clone()),
            on: None,
        }
        .chain(&krate, quote!(h));
        let tokens = default.to_string();
        assert!(tokens.contains("enter_dialogue"));
        assert!(tokens.contains(
            "< StepDialogue as :: teloxide_router :: __private :: DialogueSpec > :: Store"
        ));
        assert!(tokens.contains(
            "< StepDialogue as :: teloxide_router :: __private :: DialogueSpec > :: State"
        ));
        assert!(tokens.contains("Update"));

        let explicit = Link::Dialogue {
            spec: Box::new(spec),
            on: Some(Box::new(on)),
        }
        .chain(&krate, quote!(h));
        assert!(explicit.to_string().contains("CallbackQuery"));
    }
}
