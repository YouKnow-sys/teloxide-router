//! Argument parsing shared by the handler attributes and `router!`.

use proc_macro2::{Span, TokenStream};
use syn::parse::{Parse, ParseStream, Parser};
use syn::spanned::Spanned;
use syn::{Expr, Ident, LitInt, Pat, Path, Token};

const HANDLER_KEYS: &[&str] = &[
    "filter",
    "map",
    "filter_map",
    "try_filter",
    "try_map",
    "try_filter_map",
    "inspect",
    "state",
    "command",
    "priority",
];
const ROUTER_KEYS: &[&str] = &[
    "filter",
    "map",
    "filter_map",
    "try_filter",
    "try_map",
    "try_filter_map",
    "inspect",
    "state",
    "include",
];

#[derive(Clone, Copy)]
enum Ctx {
    Handler,
    Router,
}

impl Ctx {
    fn keys(self) -> &'static [&'static str] {
        match self {
            Ctx::Handler => HANDLER_KEYS,
            Ctx::Router => ROUTER_KEYS,
        }
    }
}

/// One adapter to fold onto the handler chain, in written order.
pub(crate) enum Adapter {
    Filter(Expr),
    Map(Expr),
    FilterMap(Expr),
    TryFilter(Expr),
    TryMap(Expr),
    TryFilterMap(Expr),
    Inspect(Expr),
    State(Pat),
    Command { enum_path: Path, pat: Pat },
    Include(Vec<Expr>),
}

pub(crate) struct HandlerArgs {
    pub router: Path,
    pub adapters: Vec<Adapter>,
    pub priority: i32,
}

pub(crate) struct RouterArgs {
    pub adapters: Vec<Adapter>,
}

pub(crate) fn parse_handler(input: TokenStream) -> syn::Result<HandlerArgs> {
    let Parsed {
        router,
        adapters,
        priority,
        first_span,
    } = parse(input, Ctx::Handler)?;
    let Some(router) = router else {
        return Err(syn::Error::new(
            first_span.unwrap_or_else(Span::call_site),
            "expected a router path as the first argument, e.g. `#[txr::message(ROUTER)]`",
        ));
    };
    Ok(HandlerArgs {
        router,
        adapters,
        priority: priority.unwrap_or(0),
    })
}

pub(crate) fn parse_router(input: TokenStream) -> syn::Result<RouterArgs> {
    let Parsed { adapters, .. } = parse(input, Ctx::Router)?;
    Ok(RouterArgs { adapters })
}

struct Parsed {
    router: Option<Path>,
    adapters: Vec<Adapter>,
    priority: Option<i32>,
    first_span: Option<Span>,
}

struct Item {
    key: Option<Ident>,
    kind: ItemKind,
}

enum ItemKind {
    Filter(Vec<Expr>),
    Map(Vec<Expr>),
    FilterMap(Vec<Expr>),
    TryFilter(Vec<Expr>),
    TryMap(Vec<Expr>),
    TryFilterMap(Vec<Expr>),
    Inspect(Vec<Expr>),
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
        adapters: Vec::new(),
        priority: None,
        first_span: items.first().map(Item::span),
    };
    // Content past the first error doesn't matter; the parse still fails.
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
            ItemKind::Filter(values) => {
                parsed
                    .adapters
                    .extend(values.into_iter().map(Adapter::Filter));
                Ok(())
            }
            ItemKind::Map(values) => {
                parsed.adapters.extend(values.into_iter().map(Adapter::Map));
                Ok(())
            }
            ItemKind::FilterMap(values) => {
                parsed
                    .adapters
                    .extend(values.into_iter().map(Adapter::FilterMap));
                Ok(())
            }
            ItemKind::TryFilter(values) => {
                parsed
                    .adapters
                    .extend(values.into_iter().map(Adapter::TryFilter));
                Ok(())
            }
            ItemKind::TryMap(values) => {
                parsed
                    .adapters
                    .extend(values.into_iter().map(Adapter::TryMap));
                Ok(())
            }
            ItemKind::TryFilterMap(values) => {
                parsed
                    .adapters
                    .extend(values.into_iter().map(Adapter::TryFilterMap));
                Ok(())
            }
            ItemKind::Inspect(values) => {
                parsed
                    .adapters
                    .extend(values.into_iter().map(Adapter::Inspect));
                Ok(())
            }
            ItemKind::State(pats) => {
                for pat in pats {
                    match validate_case_pat(&pat, false) {
                        Ok(_) => parsed.adapters.push(Adapter::State(pat)),
                        Err(e) => combine(&mut err, e),
                    }
                }
                Ok(())
            }
            ItemKind::Command(pats) => {
                for pat in pats {
                    match validate_case_pat(&pat, true) {
                        Ok(path) => match enum_path(path) {
                            Ok(enum_path) => {
                                parsed.adapters.push(Adapter::Command { enum_path, pat })
                            }
                            Err(e) => combine(&mut err, e),
                        },
                        Err(e) => combine(&mut err, e),
                    }
                }
                Ok(())
            }
            ItemKind::Include(values) => {
                parsed.adapters.push(Adapter::Include(values));
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

fn combine(err: &mut Option<syn::Error>, e: syn::Error) {
    match err {
        Some(prev) => prev.combine(e),
        None => *err = Some(e),
    }
}

fn parse_items(input: ParseStream<'_>, ctx: Ctx) -> syn::Result<Vec<Item>> {
    let mut items = Vec::new();
    while !input.is_empty() {
        items.push(Item::parse(input, ctx)?);
        if input.is_empty() {
            break;
        }
        input.parse::<Token![,]>()?;
    }
    Ok(items)
}

impl Item {
    fn span(&self) -> Span {
        match &self.key {
            Some(key) => key.span(),
            None => Span::call_site(),
        }
    }

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
        Ok(Item {
            key: Some(key),
            kind,
        })
    }

    fn parse_value(input: ParseStream<'_>, key: &Ident, ctx: Ctx) -> syn::Result<ItemKind> {
        let expr = |input: ParseStream<'_>| <Expr as Parse>::parse(input);
        let name = key.to_string();
        match (name.as_str(), ctx) {
            ("filter", _) => Ok(ItemKind::Filter(list(input, expr)?)),
            ("map", _) => Ok(ItemKind::Map(list(input, expr)?)),
            ("filter_map", _) => Ok(ItemKind::FilterMap(list(input, expr)?)),
            ("try_filter", _) => Ok(ItemKind::TryFilter(list(input, expr)?)),
            ("try_map", _) => Ok(ItemKind::TryMap(list(input, expr)?)),
            ("try_filter_map", _) => Ok(ItemKind::TryFilterMap(list(input, expr)?)),
            ("inspect", _) => Ok(ItemKind::Inspect(list(input, expr)?)),
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
                let negated = input.peek(Token![-]) && {
                    input.parse::<Token![-]>()?;
                    true
                };
                let lit: LitInt = input.parse()?;
                let value = lit
                    .base10_parse::<i32>()
                    .map_err(|e| syn::Error::new(lit.span(), e))?;
                Ok(ItemKind::Priority(
                    if negated { -value } else { value },
                    lit.span(),
                ))
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
/// list. The bracket is peeked before parsing so `[a, b]` is never read as an
/// array expression.
fn list<T>(
    input: ParseStream<'_>,
    parse_one: fn(ParseStream<'_>) -> syn::Result<T>,
) -> syn::Result<Vec<T>> {
    if input.peek(syn::token::Bracket) {
        let content;
        syn::bracketed!(content in input);
        let mut values = Vec::new();
        while !content.is_empty() {
            values.push(parse_one(&content)?);
            if content.is_empty() {
                break;
            }
            content.parse::<Token![,]>()?;
        }
        Ok(values)
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
    if matches!(pat, Pat::Ident(_)) {
        Ok(())
    } else {
        Err(syn::Error::new(
            pat.span(),
            "dptree::case! supports only named bindings, e.g. `Command::Set(a, b)`",
        ))
    }
}

fn enum_path(path: &Path) -> syn::Result<Path> {
    let mut path = path.clone();
    // `pop` leaves the punctuator that led into the removed segment.
    if path.segments.pop().is_none() {
        return Err(syn::Error::new(
            path.span(),
            "the pattern must name its enum",
        ));
    }
    path.segments.pop_punct();
    Ok(path)
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

    #[test]
    fn lists_flatten_in_written_order() {
        let args = handler(quote! { ROUTER, filter = [f, g], map = m }).unwrap();
        assert!(matches!(args.adapters[0], Adapter::Filter(_)));
        assert!(matches!(args.adapters[1], Adapter::Filter(_)));
        assert!(matches!(args.adapters[2], Adapter::Map(_)));
    }

    #[test]
    fn single_value_is_a_one_element_list() {
        let args = handler(quote! { ROUTER, filter = f }).unwrap();
        assert_eq!(args.adapters.len(), 1);
    }

    #[test]
    fn default_priority_is_zero() {
        let args = handler(quote! { ROUTER }).unwrap();
        assert_eq!(args.priority, 0);
        let args = handler(quote! { ROUTER, priority = -5 }).unwrap();
        assert_eq!(args.priority, -5);
    }

    // Error rendering for these inputs is locked by the trybuild suite in
    // `tests/ui/`; these tests only cover parser behavior it doesn't reach.

    #[test]
    fn router_path_must_come_first() {
        assert!(handler(quote! { filter = f, ROUTER }).is_err());
    }

    #[test]
    fn command_variants_extract_the_enum() {
        let args = handler(quote! { ROUTER, command = Cmd::Start }).unwrap();
        let Adapter::Command { enum_path, .. } = &args.adapters[0] else {
            panic!("not a command adapter");
        };
        assert_eq!(enum_path.segments.len(), 1);
        assert_eq!(enum_path.segments[0].ident, "Cmd");

        let args = handler(quote! { ROUTER, command = Cmd::Set(a, b) }).unwrap();
        assert!(matches!(&args.adapters[0], Adapter::Command { .. }));

        let args = handler(quote! { ROUTER, command = Cmd::Get { x } }).unwrap();
        assert!(matches!(&args.adapters[0], Adapter::Command { .. }));
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
    fn include_takes_router_references() {
        let args = router(quote! { include = [&admin::ROUTER] }).unwrap();
        assert!(matches!(args.adapters[0], Adapter::Include(_)));
        assert!(router(quote! { include = [admin::ROUTER] }).is_err());
    }

    #[test]
    fn try_keys_parse_in_both_contexts() {
        let args = handler(
            quote! { ROUTER, try_filter = f, try_map = m, try_filter_map = fm, inspect = i },
        )
        .unwrap();
        assert!(matches!(args.adapters[0], Adapter::TryFilter(_)));
        assert!(matches!(args.adapters[1], Adapter::TryMap(_)));
        assert!(matches!(args.adapters[2], Adapter::TryFilterMap(_)));
        assert!(matches!(args.adapters[3], Adapter::Inspect(_)));

        let args = router(quote! { try_map = m }).unwrap();
        assert!(matches!(args.adapters[0], Adapter::TryMap(_)));
    }

    #[test]
    fn router_rejects_command_and_priority() {
        assert!(router(quote! { command = C::X }).is_err());
        assert!(router(quote! { priority = 1 }).is_err());
    }
}
