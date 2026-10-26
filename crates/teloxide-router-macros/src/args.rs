//! Argument parsing shared by the handler attributes and `router!`.

use proc_macro2::{Span, TokenStream};
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Expr, Ident, LitInt, Pat, Path, Token, Type};

use crate::adapt::{COMBINATORS, Combinator, Link, StatePat};

#[derive(Clone, Copy)]
enum Ctx {
    Handler,
    Router,
}

const SHARED_EXTRAS: &[&str] = &["state", "priority"];
const HANDLER_ONLY: &[&str] = &["command"];
const ROUTER_ONLY: &[&str] = &["include", "dialogue", "dialogue_on"];

impl Ctx {
    /// The valid keys in the order they appear in error messages.
    fn keys(self) -> Vec<&'static str> {
        let own = match self {
            Ctx::Handler => HANDLER_ONLY,
            Ctx::Router => ROUTER_ONLY,
        };
        COMBINATORS
            .iter()
            .chain(SHARED_EXTRAS)
            .chain(own)
            .copied()
            .collect()
    }
}

pub(crate) enum Step {
    Link(Link),
    State(StatePat),
    Command(StatePat),
}

pub(crate) struct HandlerArgs {
    pub(crate) router: Path,
    pub(crate) steps: Vec<Step>,
    pub(crate) priority: i32,
}

pub(crate) struct RouterArgs {
    pub(crate) links: Vec<Link>,
    pub(crate) children: Vec<Expr>,
    pub(crate) priority: i32,
}

pub(crate) fn combine(err: &mut Option<syn::Error>, e: syn::Error) {
    match err {
        Some(prev) => prev.combine(e),
        None => *err = Some(e),
    }
}

pub(crate) fn finish(err: Option<syn::Error>) -> syn::Result<()> {
    match err {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

pub(crate) fn parse_handler(input: TokenStream) -> syn::Result<HandlerArgs> {
    let Parsed { router, items } = parse(input, Ctx::Handler)?;
    let first_span = items.first().map(|item| item.key.span());
    let mut priority = None;
    let mut steps = Vec::new();
    let mut err = None;

    for Item { kind, .. } in items {
        match kind {
            ItemKind::Link(links) => steps.extend(links.into_iter().map(Step::Link)),
            ItemKind::State(pats) => push_patterns(&mut steps, pats, Step::State, &mut err),
            ItemKind::Command(pats) => push_patterns(&mut steps, pats, Step::Command, &mut err),
            ItemKind::Priority(value, span) => {
                once(&mut priority, value, span, "priority", &mut err)
            }
            // Router-only kinds were rejected while parsing.
            ItemKind::Include(_) | ItemKind::Dialogue(_) | ItemKind::DialogueOn(_) => {}
        }
    }
    finish(err)?;

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
    let Parsed { items, .. } = parse(input, Ctx::Router)?;
    let mut args = RouterArgs {
        links: Vec::new(),
        children: Vec::new(),
        priority: 0,
    };
    let mut priority = None;
    let mut dialogue_on = None;
    let mut err = None;

    for Item { key, kind } in items {
        match kind {
            ItemKind::Link(links) => args.links.extend(links),
            ItemKind::State(pats) => push_patterns(&mut args.links, pats, Link::State, &mut err),
            ItemKind::Priority(value, span) => {
                once(&mut priority, value, span, "priority", &mut err)
            }
            ItemKind::Include(values) => args.children.extend(values),
            ItemKind::Dialogue(spec) => {
                if args
                    .links
                    .iter()
                    .any(|l| matches!(l, Link::Dialogue { .. }))
                {
                    combine(
                        &mut err,
                        syn::Error::new(key.span(), "dialogue may appear at most once"),
                    );
                } else {
                    args.links.push(Link::Dialogue {
                        spec: Box::new(spec),
                        on: None,
                    });
                }
            }
            ItemKind::DialogueOn(on) => {
                let span = key.span();
                once(&mut dialogue_on, (on, span), span, "dialogue_on", &mut err);
            }
            // Handler-only kinds were rejected while parsing.
            ItemKind::Command(_) => {}
        }
    }
    // `dialogue_on` may be written before `dialogue`, so it fills the link now.
    if let Some((on, span)) = dialogue_on {
        match args.links.iter_mut().find_map(|link| match link {
            Link::Dialogue { on, .. } => Some(on),
            _ => None,
        }) {
            Some(slot) => *slot = Some(Box::new(on)),
            None => combine(
                &mut err,
                syn::Error::new(span, "`dialogue_on` requires `dialogue`"),
            ),
        }
    }
    finish(err)?;
    args.priority = priority.unwrap_or(0);
    Ok(args)
}

fn push_patterns<T>(
    out: &mut Vec<T>,
    pats: Vec<Pat>,
    make: fn(StatePat) -> T,
    err: &mut Option<syn::Error>,
) {
    for pat in pats {
        match parse_state_pat(pat) {
            Ok(pat) => out.push(make(pat)),
            Err(e) => combine(err, e),
        }
    }
}

fn once<T>(slot: &mut Option<T>, value: T, span: Span, key: &str, err: &mut Option<syn::Error>) {
    if slot.is_some() {
        combine(
            err,
            syn::Error::new(span, format!("{key} may appear at most once")),
        );
    } else {
        *slot = Some(value);
    }
}

struct Parsed {
    router: Option<Path>,
    items: Vec<Item>,
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
    Dialogue(Type),
    DialogueOn(Type),
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
    Ok(Parsed { router, items })
}

fn parse_items(input: ParseStream<'_>, ctx: Ctx) -> syn::Result<Vec<Item>> {
    let parse_one = match ctx {
        Ctx::Handler => Item::parse_handler,
        Ctx::Router => Item::parse_router,
    };
    let items: Punctuated<Item, Token![,]> = Punctuated::parse_terminated_with(input, parse_one)?;
    Ok(items.into_iter().collect())
}

impl Item {
    fn parse_handler(input: ParseStream<'_>) -> syn::Result<Self> {
        Self::parse(input, Ctx::Handler)
    }

    fn parse_router(input: ParseStream<'_>) -> syn::Result<Self> {
        Self::parse(input, Ctx::Router)
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
            ("priority", _) => {
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
            ("dialogue", Ctx::Router) => {
                Ok(ItemKind::Dialogue(single_type(input, key, "Dialogue")?))
            }
            ("dialogue_on", Ctx::Router) => {
                Ok(ItemKind::DialogueOn(single_type(input, key, "Update")?))
            }
            (name, ctx) => {
                let message = match ctx {
                    Ctx::Handler if ROUTER_ONLY.contains(&name) => {
                        format!("`{name}` is only available on `router!`")
                    }
                    Ctx::Router if HANDLER_ONLY.contains(&name) => {
                        format!("`{name}` is only available on handler attributes")
                    }
                    _ => format!(
                        "unknown key `{name}`; expected one of: {}",
                        ctx.keys().join(", ")
                    ),
                };
                Err(syn::Error::new(key.span(), message))
            }
        }
    }
}

fn single_type(input: ParseStream<'_>, key: &Ident, example_val: &str) -> syn::Result<Type> {
    let ty: Type = input.parse()?;
    if matches!(ty, Type::Slice(_) | Type::Array(_)) {
        return Err(syn::Error::new(
            ty.span(),
            format!("`{key}` takes a single type, e.g. `{key} = {example_val}`"),
        ));
    }
    Ok(ty)
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

/// Turns a `state`/`command` pattern into a [`StatePat`]: plain bindings
/// only, every field bound, the enum named (`Dialogue::Start`, not `Start`).
fn parse_state_pat(pat: Pat) -> syn::Result<StatePat> {
    let path = match &pat {
        Pat::Path(pat) => pat.path.clone(),
        Pat::TupleStruct(pat) => {
            for elem in &pat.elems {
                check_binding(elem)?;
            }
            pat.path.clone()
        }
        Pat::Struct(pat) => {
            if let Some(rest) = &pat.rest {
                return Err(wildcard_error(rest.span()));
            }
            for field in &pat.fields {
                check_binding(&field.pat)?;
            }
            pat.path.clone()
        }
        Pat::Ident(pat) => {
            return Err(syn::Error::new(
                pat.span(),
                "the pattern must name its enum, e.g. `Dialogue::Start`",
            ));
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
    if path.segments.len() < 2 {
        return Err(syn::Error::new(
            path.span(),
            "the pattern must name its enum, e.g. `Dialogue::Start`",
        ));
    }
    Ok(StatePat {
        enum_path: enum_path(&path),
        pat,
    })
}

/// A binding position; `case!` binds through plain idents only.
fn check_binding(pat: &Pat) -> syn::Result<()> {
    match pat {
        Pat::Ident(pat)
            if pat.by_ref.is_none() && pat.mutability.is_none() && pat.subpat.is_none() =>
        {
            Ok(())
        }
        Pat::Wild(_) | Pat::Rest(_) => Err(wildcard_error(pat.span())),
        _ => Err(syn::Error::new(
            pat.span(),
            "patterns support plain bindings, e.g. `Command::Set(a, b)`",
        )),
    }
}

fn wildcard_error(span: Span) -> syn::Error {
    syn::Error::new(
        span,
        "`case!` has no wildcard arm; bind every field, e.g. `Command::Set(a, b)`",
    )
}

// `pop` leaves the punctuator that led into the removed segment.
fn enum_path(path: &Path) -> Path {
    let mut path = path.clone();
    let _ = path.segments.pop();
    path.segments.pop_punct();
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::{ToTokens, quote};

    fn expect_err<T>(result: syn::Result<T>) -> syn::Error {
        match result {
            Err(e) => e,
            Ok(_) => panic!("expected an error"),
        }
    }

    fn call_combinator(link: &Link) -> Combinator {
        match link {
            Link::Call { combinator, .. } => *combinator,
            _ => panic!("expected a call link"),
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

    fn command_step(args: &HandlerArgs) -> &StatePat {
        let Step::Command(pat) = &args.steps[0] else {
            panic!("not a command step");
        };
        pat
    }

    // Quote separates every token with a space; squeeze before comparing.
    fn squeezed(tokens: impl ToTokens) -> String {
        tokens
            .to_token_stream()
            .to_string()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect()
    }

    fn dialogue_link(links: &[Link]) -> (&Type, Option<&Type>) {
        let [Link::Dialogue { spec, on }] = links else {
            panic!("expected exactly one dialogue link");
        };
        (spec, on.as_deref())
    }

    fn names(ty: &Type, ident: &str) -> bool {
        ty.to_token_stream().to_string().contains(ident)
    }

    #[test]
    fn lists_flatten_in_written_order() {
        let args = parse_handler(quote! { ROUTER, filter = [f, g], map = m }).unwrap();
        assert_eq!(
            call_combinators(&args.steps),
            [Combinator::Filter, Combinator::Filter, Combinator::Map]
        );
    }

    #[test]
    fn single_value_is_a_one_element_list() {
        let args = parse_handler(quote! { ROUTER, filter = f }).unwrap();
        assert_eq!(args.steps.len(), 1);
    }

    #[test]
    fn default_priority_is_zero() {
        let args = parse_handler(quote! { ROUTER }).unwrap();
        assert_eq!(args.priority, 0);
        let args = parse_handler(quote! { ROUTER, priority = -5 }).unwrap();
        assert_eq!(args.priority, -5);
    }

    #[test]
    fn priority_accepts_i32_min_and_rejects_overflow() {
        let args = parse_handler(quote! { ROUTER, priority = -2147483648 }).unwrap();
        assert_eq!(args.priority, i32::MIN);
        assert!(parse_handler(quote! { ROUTER, priority = 2147483648 }).is_err());
        assert!(parse_handler(quote! { ROUTER, priority = -2147483649 }).is_err());
    }

    #[test]
    fn router_path_must_come_first() {
        assert!(parse_handler(quote! { filter = f, ROUTER }).is_err());
    }

    #[test]
    fn command_variants_extract_the_enum() {
        for input in [
            quote! { ROUTER, command = Cmd::Start },
            quote! { ROUTER, command = Cmd::Set(a, b) },
            quote! { ROUTER, command = Cmd::Get { x } },
        ] {
            let args = parse_handler(input).unwrap();
            let pat = command_step(&args);
            assert_eq!(pat.enum_path.segments.len(), 1);
            assert_eq!(pat.enum_path.segments[0].ident, "Cmd");
        }
    }

    #[test]
    fn command_requires_the_enum_name() {
        let err = expect_err(parse_handler(quote! { ROUTER, command = Start }));
        assert!(err.to_string().contains("must name its enum"));
        // A bare ident binds instead of matching, so `state` needs it too.
        let err = expect_err(parse_handler(quote! { ROUTER, state = Start }));
        assert!(err.to_string().contains("must name its enum"));
    }

    #[test]
    fn binding_modes_are_rejected() {
        for input in [
            quote! { ROUTER, state = Foo(mut a) },
            quote! { ROUTER, state = Foo(ref a) },
            quote! { ROUTER, state = Foo(a @ _) },
        ] {
            let err = expect_err(parse_handler(input));
            assert!(err.to_string().contains("plain bindings"));
        }
    }

    #[test]
    fn patterns_reject_wildcards() {
        for input in [
            quote! { ROUTER, state = Foo::Bar(_) },
            quote! { ROUTER, state = Foo::Bar(a, _) },
            quote! { ROUTER, state = Foo::Bar { x: _ } },
            quote! { ROUTER, state = Foo::Bar { x, .. } },
            quote! { ROUTER, command = Cmd::Set(a, _) },
        ] {
            let err = expect_err(parse_handler(input).map(|_| ()));
            assert!(err.to_string().contains("no wildcard arm"), "{err}");
        }
        let err = expect_err(parse_router(quote! { state = Foo::Bar { .. } }).map(|_| ()));
        assert!(err.to_string().contains("no wildcard arm"), "{err}");
    }

    #[test]
    fn include_takes_router_references() {
        let args = parse_router(quote! { include = [&admin::ROUTER, &other::ROUTER] }).unwrap();
        assert_eq!(args.children.len(), 2);
        assert!(args.links.is_empty());
        assert!(parse_router(quote! { include = [admin::ROUTER] }).is_err());
    }

    #[test]
    fn state_links_emit_case() {
        for input in [
            quote! { state = Dialogue::Start },
            quote! { state = States::Users(state) },
            quote! { state = States::Users { state } },
            quote! { state = Dialogue::Tell(a, b) },
        ] {
            let args = parse_router(input).unwrap();
            let tokens = squeezed(args.links[0].chain(&quote!(::teloxide_router), quote!(h)));
            assert!(tokens.contains("case!["), "{tokens}");
        }
    }

    #[test]
    fn try_keys_parse_in_both_contexts() {
        let args = parse_handler(
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

        let args = parse_router(quote! { try_map = m }).unwrap();
        assert_eq!(
            args.links.iter().map(call_combinator).collect::<Vec<_>>(),
            [Combinator::TryMap]
        );
    }

    #[test]
    fn router_rejects_handler_only_keys() {
        let err = expect_err(parse_router(quote! { command = C::X }));
        assert!(
            err.to_string()
                .contains("only available on handler attributes"),
            "{err}"
        );
    }

    #[test]
    fn router_state_takes_any_pattern_list() {
        // Same-enum patterns are allowed too; they can never both match,
        // which is the user's call.
        let args =
            parse_router(quote! { state = [States::Users(us), Steps::Done, States::Admins] })
                .unwrap();
        assert_eq!(args.links.len(), 3);
        assert!(args.links.iter().all(|l| matches!(l, Link::State(_))));
    }
    #[test]
    fn unknown_keys_list_the_context_keys() {
        let err = expect_err(parse_router(quote! { nope = 1 }));
        let message = err.to_string();
        assert!(message.contains("unknown key `nope`"), "{message}");
        assert!(message.contains("dialogue_on"), "{message}");
        assert!(message.contains("priority"), "{message}");
        assert!(!message.contains("command"), "{message}");

        let err = expect_err(parse_handler(quote! { ROUTER, nope = 1 }));
        let message = err.to_string();
        assert!(message.contains("priority"), "{message}");
        assert!(!message.contains("dialogue"), "{message}");
    }

    #[test]
    fn dialogue_becomes_a_link_filled_by_dialogue_on() {
        let args = parse_router(quote! { dialogue = StepDialogue }).unwrap();
        let (spec, on) = dialogue_link(&args.links);
        assert!(names(spec, "StepDialogue"));
        assert!(on.is_none());

        for input in [
            quote! { dialogue_on = CallbackQuery, dialogue = StepDialogue },
            quote! { dialogue = StepDialogue, dialogue_on = CallbackQuery },
        ] {
            let args = parse_router(input).unwrap();
            let (spec, on) = dialogue_link(&args.links);
            assert!(names(spec, "StepDialogue"));
            assert!(on.is_some_and(|ty| names(ty, "CallbackQuery")));
        }
    }

    #[test]
    fn dialogue_keeps_its_written_position() {
        let args = parse_router(quote! { filter = f, dialogue = D, map = m }).unwrap();
        assert!(matches!(
            &args.links[0],
            Link::Call {
                combinator: Combinator::Filter,
                ..
            }
        ));
        assert!(matches!(&args.links[1], Link::Dialogue { .. }));
        assert!(matches!(
            &args.links[2],
            Link::Call {
                combinator: Combinator::Map,
                ..
            }
        ));
    }

    #[test]
    fn dialogue_keys_validate() {
        let cases: [(TokenStream, &str); 5] = [
            (
                quote! { dialogue_on = CallbackQuery },
                "requires `dialogue`",
            ),
            (quote! { dialogue = A, dialogue = B }, "at most once"),
            (
                quote! { dialogue_on = A, dialogue_on = B, dialogue = C },
                "at most once",
            ),
            (quote! { dialogue = [A, B] }, "single type"),
            (quote! { dialogue_on = [A] }, "single type"),
        ];
        for (input, message) in cases {
            let err = expect_err(parse_router(input));
            assert!(err.to_string().contains(message), "{err}");
        }
    }

    #[test]
    fn handler_rejects_router_only_keys() {
        for input in [
            quote! { ROUTER, dialogue = D },
            quote! { ROUTER, dialogue_on = U },
            quote! { ROUTER, include = [&R] },
        ] {
            let err = expect_err(parse_handler(input));
            assert!(
                err.to_string().contains("only available on `router!`"),
                "{err}"
            );
        }
    }
}
