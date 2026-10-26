//! Shared expansion for the handler attributes.

use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::quote;
use syn::spanned::Spanned;
use syn::{Item, ItemFn};

use crate::args::{self, Step};
use crate::crate_path::crate_path;

pub(crate) fn attribute(kind: Option<Ident>, attr: TokenStream, item: TokenStream) -> TokenStream {
    let original = item.clone();
    match expand(kind, attr.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => {
            let mut out = TokenStream::from(e.into_compile_error());
            out.extend(original);
            out
        }
    }
}

fn expand(
    kind: Option<Ident>,
    attr: TokenStream2,
    item: TokenStream2,
) -> syn::Result<TokenStream2> {
    let handler_fn: ItemFn = match syn::parse2(item.clone()) {
        Ok(handler_fn) => handler_fn,
        Err(e) => {
            // A well-formed item that is not a `fn` gets a clearer message
            // than syn's `expected `fn``; a broken `fn` keeps syn's error.
            return Err(if syn::parse2::<Item>(item).is_ok() {
                syn::Error::new(e.span(), "handlers must be free functions")
            } else {
                e
            });
        }
    };
    let err = validate_handler_fn(&handler_fn);
    let args = args::parse_handler(attr);
    let args = match (args, err) {
        (Ok(args), None) => args,
        (Ok(_), Some(err)) => return Err(err),
        (Err(e), None) => return Err(e),
        (Err(e), Some(mut err)) => {
            err.combine(e);
            return Err(err);
        }
    };
    let krate = crate_path();
    let router = &args.router;
    let priority = args.priority;

    let leaf = match kind {
        Some(kind) => quote! {
            <#krate::__private::Update as #krate::__private::UpdateFilterExt<#krate::Output>>
                ::#kind()
        },
        None => quote! { #krate::entry() },
    };
    let leaf = args.steps.iter().fold(leaf, |base, step| match step {
        Step::Link(link) => link.chain(&krate, base),
        Step::State(pat) => pat.case_chain(&krate, base),
        Step::Command(pat) => {
            let enum_path = &pat.enum_path;
            let base = quote! {
                #krate::__private::HandlerExt::filter_command::<#enum_path>(#base)
            };
            pat.case_chain(&krate, base)
        }
    });

    let name = &handler_fn.sig.ident;
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

fn validate_handler_fn(handler_fn: &ItemFn) -> Option<syn::Error> {
    let mut err = None;
    let sig = &handler_fn.sig;
    if !sig.generics.params.is_empty() {
        args::combine(
            &mut err,
            syn::Error::new(sig.generics.span(), "handlers must not be generic"),
        );
    }
    if let Some(syn::FnArg::Receiver(receiver)) = sig.inputs.first() {
        args::combine(
            &mut err,
            syn::Error::new(receiver.span(), "handlers must be free functions"),
        );
    }
    if sig.asyncness.is_none() {
        args::combine(
            &mut err,
            syn::Error::new(sig.span(), "handlers must be `async fn`"),
        );
    }
    err
}
