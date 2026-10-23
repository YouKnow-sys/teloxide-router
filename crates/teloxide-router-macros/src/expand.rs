//! Shared expansion for every update-kind attribute.

use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::quote;
use syn::ItemFn;
use syn::spanned::Spanned;

use crate::adapt;
use crate::args::{self, Adapter};
use crate::crate_path::crate_path;

pub(crate) fn handler(kind: Ident, attr: TokenStream, item: TokenStream) -> TokenStream {
    match expand(Some(kind), attr.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

pub(crate) fn update(attr: TokenStream, item: TokenStream) -> TokenStream {
    match expand(None, attr.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand(
    kind: Option<Ident>,
    attr: TokenStream2,
    item: TokenStream2,
) -> syn::Result<TokenStream2> {
    let handler_fn: ItemFn = syn::parse2(item)
        .map_err(|e| syn::Error::new(e.span(), "handlers must be free functions"))?;
    validate_handler_fn(&handler_fn)?;
    let args = args::parse_handler(attr)?;
    let krate = crate_path();
    let name = &handler_fn.sig.ident;
    let router = &args.router;
    let priority = args.priority;

    let mut leaf = match kind {
        Some(kind) => quote! {
            <#krate::__private::Update as #krate::__private::UpdateFilterExt<#krate::Output>>
                ::#kind()
        },
        None => quote! { #krate::entry() },
    };
    for adapter in &args.adapters {
        leaf = match adapter {
            Adapter::Filter(expr) => {
                let adapter = adapt::adapter(&krate, adapt::Combinator::Filter, expr);
                quote! { #leaf.chain(#adapter) }
            }
            Adapter::Map(expr) => {
                let adapter = adapt::adapter(&krate, adapt::Combinator::Map, expr);
                quote! { #leaf.chain(#adapter) }
            }
            Adapter::FilterMap(expr) => {
                let adapter = adapt::adapter(&krate, adapt::Combinator::FilterMap, expr);
                quote! { #leaf.chain(#adapter) }
            }
            Adapter::TryFilter(expr) => {
                let adapter = adapt::adapter(&krate, adapt::Combinator::TryFilter, expr);
                quote! { #leaf.chain(#adapter) }
            }
            Adapter::TryMap(expr) => {
                let adapter = adapt::adapter(&krate, adapt::Combinator::TryMap, expr);
                quote! { #leaf.chain(#adapter) }
            }
            Adapter::TryFilterMap(expr) => {
                let adapter = adapt::adapter(&krate, adapt::Combinator::TryFilterMap, expr);
                quote! { #leaf.chain(#adapter) }
            }
            Adapter::Inspect(expr) => {
                let adapter = adapt::adapter(&krate, adapt::Combinator::Inspect, expr);
                quote! { #leaf.chain(#adapter) }
            }
            Adapter::State(pat) => quote! { #leaf.chain(#krate::__private::dptree::case![#pat]) },
            Adapter::Command { enum_path, pat } => quote! {
                #krate::__private::HandlerExt::filter_command::<#enum_path>(#leaf)
                    .chain(#krate::__private::dptree::case![#pat])
            },
            // `include` is rejected during parsing and never reaches here.
            Adapter::Include(_) => unreachable!("include is not a handler adapter"),
        };
    }

    Ok(quote! {
        #handler_fn

        #krate::__private::inventory::submit! {
            #krate::__private::Registration {
                router: &#router,
                priority: #priority,
                location: (file!(), line!(), column!()),
                build: |__txr_h: #krate::Handler| __txr_h.branch(#leaf.endpoint(#name)),
            }
        }
    })
}

fn validate_handler_fn(handler_fn: &ItemFn) -> syn::Result<()> {
    let mut err = None;
    if !handler_fn.sig.generics.params.is_empty() {
        let e = syn::Error::new(
            handler_fn.sig.generics.span(),
            "handlers must not be generic",
        );
        err = Some(e);
    }
    if let Some(syn::FnArg::Receiver(receiver)) = handler_fn.sig.inputs.first() {
        let e = syn::Error::new(receiver.span(), "handlers must be free functions");
        match &mut err {
            Some(prev) => prev.combine(e),
            None => err = Some(e),
        }
    }
    match err {
        Some(err) => Err(err),
        None => Ok(()),
    }
}
