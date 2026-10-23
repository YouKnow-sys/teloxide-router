//! Expansion of the `router!` macro.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::{ParseStream, Parser};
use syn::{Ident, Token, Visibility};

use crate::args::{self, RouterArgs};
use crate::crate_path::crate_path;

pub(crate) fn expand(input: TokenStream) -> TokenStream {
    expand_inner(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

struct Declaration {
    visibility: Visibility,
    name: Ident,
    args: RouterArgs,
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
            args: args::parse_router(rest)?,
        })
    })
    .parse2(input)
}

fn expand_inner(input: TokenStream2) -> syn::Result<TokenStream2> {
    let Declaration {
        visibility,
        name,
        args,
    } = parse_declaration(input)?;
    let krate = crate_path();

    let setup = args
        .links
        .iter()
        .fold(quote! { __txr_h }, |acc, link| link.chain(&krate, acc));
    let children = &args.children;

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
        let d = ok(quote! { pub(crate) ADMIN, filter = f, include = [&m::R, &n::R] });
        assert_eq!(d.name.to_string(), "ADMIN");
        assert_eq!(d.args.links.len(), 1);
        assert_eq!(d.args.children.len(), 2);
    }

    #[test]
    fn missing_name_is_rejected() {
        assert!(err(quote! { pub }).to_string().contains("router name"));
        assert!(err(quote! {}).to_string().contains("router name"));
    }
}
