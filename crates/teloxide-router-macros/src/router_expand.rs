//! Expansion of the `router!` macro.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::{ParseStream, Parser};
use syn::{Ident, Token, Visibility};

use crate::adapt;
use crate::args::{self, Adapter};
use crate::crate_path::crate_path;

pub(crate) fn expand(input: TokenStream) -> TokenStream {
    match expand_inner(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

struct Declaration {
    visibility: Visibility,
    name: Ident,
    adapters: args::RouterArgs,
}

fn parse_declaration(input: TokenStream2) -> syn::Result<Declaration> {
    (|input: ParseStream<'_>| {
        let visibility: Visibility = input.parse()?;
        let name: Ident = input.parse().map_err(|e| {
            syn::Error::new(e.span(), "expected a router name, e.g. `router!(ROUTER)`")
        })?;
        if !input.is_empty() {
            input.parse::<Token![,]>()?;
        }
        let rest: TokenStream2 = input.parse()?;
        Ok(Declaration {
            visibility,
            name,
            adapters: args::parse_router(rest)?,
        })
    })
    .parse2(input)
}

fn expand_inner(input: TokenStream2) -> syn::Result<TokenStream2> {
    let Declaration {
        visibility,
        name,
        adapters,
    } = parse_declaration(input)?;
    let krate = crate_path();

    let mut setup = quote! { __txr_h };
    let mut children = Vec::new();
    for adapter in &adapters.adapters {
        match adapter {
            Adapter::Filter(expr) => {
                let adapter = adapt::adapter(&krate, adapt::Combinator::Filter, expr);
                setup = quote! { #setup.chain(#adapter) };
            }
            Adapter::Map(expr) => {
                let adapter = adapt::adapter(&krate, adapt::Combinator::Map, expr);
                setup = quote! { #setup.chain(#adapter) };
            }
            Adapter::FilterMap(expr) => {
                let adapter = adapt::adapter(&krate, adapt::Combinator::FilterMap, expr);
                setup = quote! { #setup.chain(#adapter) };
            }
            Adapter::TryFilter(expr) => {
                let adapter = adapt::adapter(&krate, adapt::Combinator::TryFilter, expr);
                setup = quote! { #setup.chain(#adapter) };
            }
            Adapter::TryMap(expr) => {
                let adapter = adapt::adapter(&krate, adapt::Combinator::TryMap, expr);
                setup = quote! { #setup.chain(#adapter) };
            }
            Adapter::TryFilterMap(expr) => {
                let adapter = adapt::adapter(&krate, adapt::Combinator::TryFilterMap, expr);
                setup = quote! { #setup.chain(#adapter) };
            }
            Adapter::Inspect(expr) => {
                let adapter = adapt::adapter(&krate, adapt::Combinator::Inspect, expr);
                setup = quote! { #setup.chain(#adapter) };
            }
            Adapter::State(pat) => {
                setup = quote! { #setup.chain(#krate::__private::dptree::case![#pat]) };
            }
            Adapter::Include(values) => children.extend(values.iter()),
            // Rejected during parsing and never reaches here.
            Adapter::Command { .. } => unreachable!("command is not a router! adapter"),
        }
    }

    Ok(quote! {
        #visibility static #name: #krate::Router = #krate::__private::new_router(
            (file!(), line!(), column!()),
            |__txr_h: #krate::Handler| #setup,
            &[#(#children),*],
        );
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn declaration(input: TokenStream2) -> syn::Result<Declaration> {
        parse_declaration(input)
    }

    fn ok(input: TokenStream2) -> Declaration {
        match declaration(input) {
            Ok(d) => d,
            Err(_) => panic!("expected a valid declaration"),
        }
    }

    fn err(input: TokenStream2) -> syn::Error {
        match declaration(input) {
            Err(e) => e,
            Ok(_) => panic!("expected an error"),
        }
    }

    #[test]
    fn parses_visibility_name_and_adapters() {
        let d = ok(quote! { pub(crate) ADMIN, filter = f, include = [&m::R] });
        assert_eq!(d.name.to_string(), "ADMIN");
        assert_eq!(d.adapters.adapters.len(), 2);
    }

    #[test]
    fn missing_name_is_rejected() {
        assert!(err(quote! { pub }).to_string().contains("router name"));
        assert!(err(quote! {}).to_string().contains("router name"));
    }
}
