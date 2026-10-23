//! Argument parsing shared by the handler attributes and `router!`.

use proc_macro2::{Span, TokenStream};
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Expr, Ident, LitInt, Pat, Path, Token};

use crate::adapt::{COMBINATORS, Combinator, Link};

#[derive(Clone, Copy)]
enum Ctx {
    Handler,
    Router,
}

impl Ctx {
    /// In error-message order: the shared combinators, then context extras.
    fn keys(self) -> Vec<&'static str> {
        const HANDLER_EXTRAS: &[&str] = &["state", "command", "priority"];
        const ROUTER_EXTRAS: &[&str] = &["state", "include"];
        let extras = match self {
            Ctx::Handler => HANDLER_EXTRAS,
            Ctx::Router => ROUTER_EXTRAS,
        };
        COMBINATORS
            .iter()
            .copied()
            .chain(extras.iter().copied())
            .collect()
    }
}

pub(crate) enum Step {
    Link(Link),
    Command { enum_path: Path, pat: Pat },
}

pub(crate) struct HandlerArgs {
    pub router: Path,
    pub steps: Vec<Step>,
    pub priority: i32,
}

pub(crate) struct RouterArgs {
    pub links: Vec<Link>,
    pub children: Vec<Expr>,
}

pub(crate) fn parse_handler(input: TokenStream) -> syn::Result<HandlerArgs> {
    let Parsed {
        router,
        steps,
        priority,
        first_span,
        ..
    } = parse(input, Ctx::Handler)?;
    let Some(router) = router else {
        return Err(syn::Error::new(
            first_span.unwrap_or_else(Span::call_site),
            "expected a router path as the first argument, e.g. `#[txr::message(ROUTER)]`",
        ));
    };
    Ok(HandlerArgs {
        router,
        steps,
        priority: priority.unwrap_or(0),
    })
}

pub(crate) fn parse_router(input: TokenStream) -> syn::Result<RouterArgs> {
    let Parsed {
        links, children, ..
    } = parse(input, Ctx::Router)?;
    Ok(RouterArgs { links, children })
}

/// Accumulates errors so every problem is reported in one pass.
pub(crate) fn combine(err: &mut Option<syn::Error>, e: syn::Error) {
    match err {
        Some(prev) => prev.combine(e),
        None => *err = Some(e),
    }
}

struct Parsed {
    router: Option<Path>,
    steps: Vec<Step>,    // handler context only
    links: Vec<Link>,    // router context only
    children: Vec<Expr>, // router context only
    priority: Option<i32>,
    first_span: Option<Span>,
}

struct Item {
    key: Ident,
    kind: ItemKind,
}

enum ItemKind {
    Link(Vec<Link>),
    State(Vec<Pat>),
    Command(Vec<Pat>),
    Include(Vec<Expr>),
    Priority(i32, Span),
}

fn parse(input: TokenStream, ctx: Ctx) -> syn::Result<Parsed> {
    // The router path is the one leading argument that is not `key = value`.
    let (router, items) = (|input: ParseStream<'_>| {
        let router = if matches!(ctx, Ctx::Handler)
            && !input.is_empty()
            && !(input.peek(Ident) && input.peek2(Token![=]))
        {
            let path = input.parse::<Path>()?;
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
            Some(path)
        } else {
            None
        };
        Ok((router, parse_items(input, ctx)?))
    })
    .parse2(input)?;

    let mut parsed = Parsed {
        router,
        steps: Vec::new(),
        links: Vec::new(),
        children: Vec::new(),
        priority: None,
        first_span: items.first().map(|item| item.key.span()),
    };
    let mut err = None;

    for item in items {
        let result = match item.kind {
            ItemKind::Priority(value, span) => {
                if parsed.priority.is_some() {
                    Err(syn::Error::new(span, "priority may appear at most once"))
                } else {
                    parsed.priority = Some(value);
                    Ok(())
                }
            }
            ItemKind::Link(links) => {
                match ctx {
                    Ctx::Handler => parsed.steps.extend(links.into_iter().map(Step::Link)),
                    Ctx::Router => parsed.links.extend(links),
                }
                Ok(())
            }
            ItemKind::State(pats) => {
                for pat in pats {
                    match validate_case_pat(&pat, false) {
                        Ok(_) => match ctx {
                            Ctx::Handler => parsed.steps.push(Step::Link(Link::State(pat))),
                            Ctx::Router => parsed.links.push(Link::State(pat)),
                        },
                        Err(e) => combine(&mut err, e),
                    }
                }
                Ok(())
            }
            ItemKind::Command(pats) => {
                for pat in pats {
                    match validate_case_pat(&pat, true) {
                        Ok(path) => parsed.steps.push(Step::Command {
                            enum_path: enum_path(path),
                            pat,
                        }),
                        Err(e) => combine(&mut err, e),
                    }
                }
                Ok(())
            }
            ItemKind::Include(values) => {
                parsed.children.extend(values);
                Ok(())
            }
        };
        if let Err(e) = result {
            combine(&mut err, e);
        }
    }
    match err {
        Some(err) => Err(err),
        None => Ok(parsed),
    }
}

// `parse_terminated_with` takes a fn pointer, not a closure.
fn parse_handler_item(input: ParseStream<'_>) -> syn::Result<Item> {
    Item::parse(input, Ctx::Handler)
}

fn parse_router_item(input: ParseStream<'_>) -> syn::Result<Item> {
    Item::parse(input, Ctx::Router)
}

fn parse_items(input: ParseStream<'_>, ctx: Ctx) -> syn::Result<Vec<Item>> {
    let parse_one = match ctx {
        Ctx::Handler => parse_handler_item,
        Ctx::Router => parse_router_item,
    };
    let items: Punctuated<Item, Token![,]> = Punctuated::parse_terminated_with(input, parse_one)?;
    Ok(items.into_iter().collect())
}

impl Item {
    fn parse(input: ParseStream<'_>, ctx: Ctx) -> syn::Result<Self> {
        if !input.peek(Ident) {
            return Err(syn::Error::new(input.span(), "expected `key = value`"));
        }
        let key: Ident = input.parse()?;
        if !input.peek(Token![=]) {
            return Err(syn::Error::new(
                key.span(),
                format!("expected `=` after `{key}`"),
            ));
        }
        input.parse::<Token![=]>()?;
        let kind = Self::parse_value(input, &key, ctx)?;
        Ok(Item { key, kind })
    }

    fn parse_value(input: ParseStream<'_>, key: &Ident, ctx: Ctx) -> syn::Result<ItemKind> {
        let expr = |input: ParseStream<'_>| <Expr as Parse>::parse(input);
        let name = key.to_string();
        if let Some(combinator) = Combinator::from_key(&name) {
            let links = list(input, expr)?
                .into_iter()
                .map(|expr| Link::Call { combinator, expr })
                .collect();
            return Ok(ItemKind::Link(links));
        }
        match (name.as_str(), ctx) {
            ("state", _) => Ok(ItemKind::State(list(input, Pat::parse_single)?)),
            ("command", Ctx::Handler) => Ok(ItemKind::Command(list(input, Pat::parse_single)?)),
            ("include", Ctx::Router) => {
                let values = list(input, expr)?;
                for value in &values {
                    let is_router_ref = matches!(
                        value,
                        Expr::Reference(reference) if matches!(*reference.expr, Expr::Path(_))
                    );
                    if !is_router_ref {
                        return Err(syn::Error::new(
                            value.span(),
                            "include takes router references, e.g. `&admin::ROUTER`",
                        ));
                    }
                }
                Ok(ItemKind::Include(values))
            }
            ("priority", Ctx::Handler) => {
                let negated = input.parse::<Option<Token![-]>>()?.is_some();
                let lit: LitInt = input.parse()?;
                // `-2147483648` is representable but its digits alone are
                // not, so the sign is re-attached before parsing.
                let digits = if negated {
                    format!("-{}", lit.base10_digits())
                } else {
                    lit.base10_digits().to_owned()
                };
                let value: i32 = digits.parse().map_err(|e| syn::Error::new(lit.span(), e))?;
                Ok(ItemKind::Priority(value, lit.span()))
            }
            (name, ctx) => Err(syn::Error::new(
                key.span(),
                format!(
                    "unknown key `{name}`; expected one of: {}",
                    ctx.keys().join(", ")
                ),
            )),
        }
    }
}

/// One value or a bracketed list; a single value is sugar for a one-element
/// list. The bracket is peeked first so it is never read as an array
/// expression.
fn list<T>(
    input: ParseStream<'_>,
    parse_one: fn(ParseStream<'_>) -> syn::Result<T>,
) -> syn::Result<Vec<T>> {
    if input.peek(syn::token::Bracket) {
        let content;
        syn::bracketed!(content in input);
        let values: Punctuated<T, Token![,]> =
            Punctuated::parse_terminated_with(&content, parse_one)?;
        Ok(values.into_iter().collect())
    } else {
        Ok(vec![parse_one(input)?])
    }
}

/// Validates a pattern usable by `dptree::case!`, returning the variant path.
fn validate_case_pat(pat: &Pat, require_enum: bool) -> syn::Result<&Path> {
    let path = match pat {
        Pat::Path(pat) => &pat.path,
        Pat::Ident(_) if require_enum => {
            return Err(syn::Error::new(
                pat.span(),
                "the pattern must name its enum, e.g. `Command::Start`",
            ));
        }
        Pat::TupleStruct(pat) => {
            for elem in &pat.elems {
                require_named_binding(elem)?;
            }
            &pat.path
        }
        Pat::Struct(pat) => {
            for field in &pat.fields {
                require_named_binding(&field.pat)?;
            }
            &pat.path
        }
        other => {
            return Err(syn::Error::new(
                other.span(),
                "expected a variant pattern, e.g. `Dialogue::Start` or `Command::Set(a, b)`",
            ));
        }
    };
    for segment in &path.segments {
        if !segment.arguments.is_none() {
            return Err(syn::Error::new(
                segment.span(),
                "generic patterns are not supported",
            ));
        }
    }
    if require_enum && path.segments.len() < 2 {
        return Err(syn::Error::new(
            path.span(),
            "the pattern must name its enum, e.g. `Command::Start`",
        ));
    }
    Ok(path)
}

// `dptree::case!` matches its parameters with `ident` fragments, so anything
// that is not a plain named binding (not even `_`) would fail to expand.
fn require_named_binding(pat: &Pat) -> syn::Result<()> {
    let plain = match pat {
        Pat::Ident(ident) => {
            ident.by_ref.is_none() && ident.mutability.is_none() && ident.subpat.is_none()
        }
        _ => false,
    };
    if plain {
        Ok(())
    } else {
        Err(syn::Error::new(
            pat.span(),
            "dptree::case! supports only named bindings, e.g. `Command::Set(a, b)`",
        ))
    }
}

// `validate_case_pat` guarantees at least two segments here; `pop` leaves
// the punctuator that led into the removed segment.
fn enum_path(path: &Path) -> Path {
    let mut path = path.clone();
    let _ = path.segments.pop();
    path.segments.pop_punct();
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn handler(input: TokenStream) -> syn::Result<HandlerArgs> {
        parse_handler(input)
    }

    fn router(input: TokenStream) -> syn::Result<RouterArgs> {
        parse_router(input)
    }

    fn expect_err<T>(result: syn::Result<T>) -> syn::Error {
        match result {
            Err(e) => e,
            Ok(_) => panic!("expected an error"),
        }
    }

    fn call_combinator(link: &Link) -> Combinator {
        match link {
            Link::Call { combinator, .. } => *combinator,
            Link::State(_) => panic!("expected a call link"),
        }
    }

    fn call_combinators(steps: &[Step]) -> Vec<Combinator> {
        steps
            .iter()
            .map(|step| match step {
                Step::Link(link) => call_combinator(link),
                _ => panic!("expected a link step"),
            })
            .collect()
    }

    #[test]
    fn lists_flatten_in_written_order() {
        let args = handler(quote! { ROUTER, filter = [f, g], map = m }).unwrap();
        assert_eq!(
            call_combinators(&args.steps),
            [Combinator::Filter, Combinator::Filter, Combinator::Map]
        );
    }

    #[test]
    fn single_value_is_a_one_element_list() {
        let args = handler(quote! { ROUTER, filter = f }).unwrap();
        assert_eq!(args.steps.len(), 1);
    }

    #[test]
    fn default_priority_is_zero() {
        let args = handler(quote! { ROUTER }).unwrap();
        assert_eq!(args.priority, 0);
        let args = handler(quote! { ROUTER, priority = -5 }).unwrap();
        assert_eq!(args.priority, -5);
    }

    #[test]
    fn priority_accepts_i32_min_and_rejects_overflow() {
        let args = handler(quote! { ROUTER, priority = -2147483648 }).unwrap();
        assert_eq!(args.priority, i32::MIN);
        assert!(handler(quote! { ROUTER, priority = 2147483648 }).is_err());
        assert!(handler(quote! { ROUTER, priority = -2147483649 }).is_err());
    }

    #[test]
    fn router_path_must_come_first() {
        assert!(handler(quote! { filter = f, ROUTER }).is_err());
    }

    #[test]
    fn command_variants_extract_the_enum() {
        let args = handler(quote! { ROUTER, command = Cmd::Start }).unwrap();
        let Step::Command { enum_path, .. } = &args.steps[0] else {
            panic!("not a command step");
        };
        assert_eq!(enum_path.segments.len(), 1);
        assert_eq!(enum_path.segments[0].ident, "Cmd");

        let args = handler(quote! { ROUTER, command = Cmd::Set(a, b) }).unwrap();
        assert!(matches!(&args.steps[0], Step::Command { .. }));

        let args = handler(quote! { ROUTER, command = Cmd::Get { x } }).unwrap();
        assert!(matches!(&args.steps[0], Step::Command { .. }));
    }

    #[test]
    fn command_requires_the_enum_name() {
        let err = expect_err(handler(quote! { ROUTER, command = Start }));
        assert!(err.to_string().contains("must name its enum"));
    }

    #[test]
    fn wildcards_are_rejected() {
        let err = expect_err(handler(quote! { ROUTER, state = Foo(_) }));
        assert!(err.to_string().contains("named bindings"));
    }

    #[test]
    fn binding_modes_are_rejected() {
        for input in [
            quote! { ROUTER, state = Foo(mut a) },
            quote! { ROUTER, state = Foo(ref a) },
            quote! { ROUTER, state = Foo(a @ _) },
        ] {
            let err = expect_err(handler(input));
            assert!(err.to_string().contains("named bindings"));
        }
    }

    #[test]
    fn include_takes_router_references() {
        let args = router(quote! { include = [&admin::ROUTER, &other::ROUTER] }).unwrap();
        assert_eq!(args.children.len(), 2);
        assert!(args.links.is_empty());
        assert!(router(quote! { include = [admin::ROUTER] }).is_err());
    }

    #[test]
    fn try_keys_parse_in_both_contexts() {
        let args = handler(
            quote! { ROUTER, try_filter = f, try_map = m, try_filter_map = fm, inspect = i },
        )
        .unwrap();
        assert_eq!(
            call_combinators(&args.steps),
            [
                Combinator::TryFilter,
                Combinator::TryMap,
                Combinator::TryFilterMap,
                Combinator::Inspect
            ]
        );

        let args = router(quote! { try_map = m }).unwrap();
        assert_eq!(
            args.links.iter().map(call_combinator).collect::<Vec<_>>(),
            [Combinator::TryMap]
        );
    }

    #[test]
    fn router_rejects_command_and_priority() {
        assert!(router(quote! { command = C::X }).is_err());
        assert!(router(quote! { priority = 1 }).is_err());
    }
}
