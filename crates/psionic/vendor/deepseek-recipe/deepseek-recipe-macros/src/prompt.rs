use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Expr, Ident, LitStr, Token};

/// An argument of `prompt!`: `name = expr` is named, anything else is
/// positional.
enum Arg {
    Positional(Expr),
    Named(Ident, Expr),
}

pub(crate) struct PromptInput {
    fmt: LitStr,
    args: Vec<Arg>,
}

impl Parse for PromptInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let fmt: LitStr = input.parse().map_err(|_| {
            syn::Error::new(input.span(), "format argument must be a string literal")
        })?;

        let mut args = Vec::new();
        let mut seen_named = false;
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            let arg = input.call(parse_arg)?;
            match &arg {
                Arg::Named(..) => seen_named = true,
                // As in `format!`, positional arguments cannot follow named arguments.
                Arg::Positional(expr) if seen_named => {
                    return Err(syn::Error::new(
                        expr.span(),
                        "positional arguments cannot follow named arguments",
                    ));
                }
                Arg::Positional(..) => {}
            }
            args.push(arg);
        }
        if !input.is_empty() {
            return Err(input.error("expected `,`"));
        }

        Ok(Self { fmt, args })
    }
}

fn parse_arg(input: ParseStream) -> syn::Result<Arg> {
    let expr: Expr = input.parse()?;
    // `name = expr` with a bare identifier on the left is a named argument.
    if let Expr::Assign(assign) = &expr
        && let Expr::Path(path) = &*assign.left
        && path.qself.is_none()
        && path.path.leading_colon.is_none()
        && path.path.segments.len() == 1
        && path.path.segments[0].arguments.is_none()
    {
        return Ok(Arg::Named(
            path.path.segments[0].ident.clone(),
            (*assign.right).clone(),
        ));
    }
    Ok(Arg::Positional(expr))
}

/// One format placeholder parsed from the format string.
enum Slot {
    /// `{}`: the next positional argument.
    Auto,
    /// `{name}`: the named argument `name`, or a capture of the enclosing
    /// variable `name` when no such argument exists.
    Named { ident: Ident },
}

enum Piece {
    Lit(String),
    Slot(Slot),
}

/// Split the format string into literal pieces and format placeholders,
/// resolving the `{{` and `}}` escapes.
fn parse_format(fmt: &str, span: Span) -> syn::Result<Vec<Piece>> {
    let mut pieces = Vec::new();
    let mut lit = String::new();
    let mut rest = fmt;

    loop {
        let Some(brace) = rest.find(['{', '}']) else {
            lit.push_str(rest);
            break;
        };
        let (before, after) = rest.split_at(brace);
        lit.push_str(before);

        let bytes = after.as_bytes();
        match bytes[0] {
            b'{' => {
                if bytes.get(1) == Some(&b'{') {
                    lit.push('{');
                    rest = &after[2..];
                    continue;
                }
                let Some(close) = after.find('}') else {
                    return Err(syn::Error::new(
                        span,
                        "invalid format string: expected `}` but string was terminated",
                    ));
                };
                if !lit.is_empty() {
                    pieces.push(Piece::Lit(std::mem::take(&mut lit)));
                }
                pieces.push(Piece::Slot(parse_slot(&after[1..close], span)?));
                rest = &after[close + 1..];
            }
            b'}' => {
                if bytes.get(1) == Some(&b'}') {
                    lit.push('}');
                    rest = &after[2..];
                    continue;
                }
                return Err(syn::Error::new(
                    span,
                    "invalid format string: unmatched `}` found",
                ));
            }
            _ => unreachable!("find only matches braces"),
        }
    }

    if !lit.is_empty() {
        pieces.push(Piece::Lit(lit));
    }
    Ok(pieces)
}

/// Return the first character of `inner` that format placeholder syntax does
/// not allow.
fn first_invalid_char(inner: &str) -> Option<char> {
    inner
        .char_indices()
        .find(|(i, c)| {
            let ok = if *i == 0 {
                *c == '_' || c.is_alphabetic()
            } else {
                *c == '_' || c.is_alphanumeric()
            };
            !ok
        })
        .map(|(_, c)| c)
}

/// Parse the text between the braces of one format placeholder.
fn parse_slot(placeholder: &str, span: Span) -> syn::Result<Slot> {
    if placeholder.is_empty() {
        return Ok(Slot::Auto);
    }
    if let Some((_, spec)) = placeholder.split_once(':') {
        return Err(syn::Error::new(
            span,
            format!("invalid format string: unknown format specifier `{spec}`"),
        ));
    }
    let name = placeholder;
    if let Some(found) = first_invalid_char(name) {
        return Err(syn::Error::new(
            span,
            format!("invalid format string: expected `}}`, found `{found}`"),
        ));
    }
    // Keywords become raw identifiers, as in `format!`: `{type}` captures
    // `r#type`.
    let ident = syn::parse_str::<Ident>(name)
        .or_else(|_| syn::parse_str::<Ident>(&format!("r#{name}")))
        .map_err(|_| {
            syn::Error::new(
                span,
                format!("invalid format string: expected `}}`, found `{name}`"),
            )
        })?;
    let mut ident = ident;
    ident.set_span(span);
    Ok(Slot::Named { ident })
}

fn argument_phrase(count: usize) -> String {
    match count {
        0 => "no arguments were given".to_owned(),
        1 => "there is 1 argument".to_owned(),
        count => format!("there are {count} arguments"),
    }
}

fn positional_phrase(count: usize) -> String {
    if count == 1 {
        "1 positional argument".to_owned()
    } else {
        format!("{count} positional arguments")
    }
}

/// A format-string piece after resolution: literal text, an argument, or a
/// captured variable.
enum Action {
    Lit(String),
    /// The argument at `index`, counting the positional arguments and then
    /// the named arguments; `named_index` is its index among the named
    /// arguments when a `{name}` placeholder selected it.
    Arg {
        index: usize,
        named_index: Option<usize>,
    },
    Capture {
        ident: Ident,
    },
}

pub(crate) fn expand(input: PromptInput) -> syn::Result<TokenStream> {
    let PromptInput { fmt, args } = input;
    let span = fmt.span();
    let pieces = parse_format(&fmt.value(), span)?;

    let mut positional: Vec<Expr> = Vec::new();
    let mut named: Vec<(Ident, Expr)> = Vec::new();
    for arg in args {
        match arg {
            Arg::Positional(expr) => positional.push(expr),
            Arg::Named(ident, expr) => {
                if named.iter().any(|(existing, _)| *existing == ident) {
                    return Err(syn::Error::new(
                        ident.span(),
                        format!("duplicate argument named `{ident}`"),
                    ));
                }
                named.push((ident, expr));
            }
        }
    }

    // As in `format!`, named arguments follow positional ones, and `{}` can take
    // them too.
    let arg_count = positional.len() + named.len();
    let arg_at = |index: usize| -> Option<&Expr> {
        if index < positional.len() {
            positional.get(index)
        } else {
            named.get(index - positional.len()).map(|(_, expr)| expr)
        }
    };

    let mut actions = Vec::new();
    let mut auto_slots = 0usize;
    for piece in pieces {
        match piece {
            Piece::Lit(text) => actions.push(Action::Lit(text)),
            Piece::Slot(Slot::Auto) => {
                let index = auto_slots;
                auto_slots += 1;
                actions.push(Action::Arg {
                    index,
                    named_index: None,
                });
            }
            Piece::Slot(Slot::Named { ident }) => {
                match named.iter().position(|(name, _)| *name == ident) {
                    Some(index) => actions.push(Action::Arg {
                        index: positional.len() + index,
                        named_index: Some(index),
                    }),
                    None => actions.push(Action::Capture { ident }),
                }
            }
        }
    }

    let required = auto_slots;
    if required > arg_count {
        return Err(syn::Error::new(
            span,
            format!(
                "{} in format string, but {}",
                positional_phrase(required),
                argument_phrase(arg_count)
            ),
        ));
    }

    let mut used = vec![false; arg_count];
    let mut used_by_name = vec![false; named.len()];
    for action in &actions {
        if let Action::Arg { index, named_index } = action {
            used[*index] = true;
            if let Some(index) = named_index {
                used_by_name[*index] = true;
            }
        }
    }

    // A named argument taken by `{}` counts as used, as in `format!`.
    let unused_positional = (0..positional.len()).filter(|index| !used[*index]).count();
    let unused_named = (0..named.len())
        .filter(|index| !used[positional.len() + index] && !used_by_name[*index])
        .count();
    let unused = unused_positional + unused_named;
    if unused > 0 {
        let message = if unused > 1 {
            "multiple unused formatting arguments"
        } else if unused_positional == 1 {
            "argument never used"
        } else {
            "named argument never used"
        };
        let unused_span = positional
            .iter()
            .enumerate()
            .find(|(index, _)| !used[*index])
            .map(|(_, expr)| expr.span())
            .or_else(|| {
                named
                    .iter()
                    .enumerate()
                    .find(|(index, (_, _))| {
                        !used[positional.len() + index] && !used_by_name[*index]
                    })
                    .map(|(_, (ident, _))| ident.span())
            })
            .unwrap_or(span);
        return Err(syn::Error::new(unused_span, message));
    }

    // Mixed-site hygiene keeps the generated locals from clashing with a local
    // of the same name. Each binding is located at its argument so that a type
    // error points at that argument.
    let prompt = Ident::new("__prompt", Span::mixed_site());
    let prompt_ty = quote!(deepseek_recipe_core::prompt::Prompt);
    let push_into = quote!(deepseek_recipe_core::prompt::PromptFragment::push_into);
    let exprs: Vec<&Expr> = (0..arg_count)
        .map(|index| arg_at(index).expect("every index below `arg_count` has an argument"))
        .collect();
    let bindings: Vec<Ident> = exprs
        .iter()
        .enumerate()
        .map(|(index, expr)| {
            Ident::new(
                &format!("__prompt_arg_{index}"),
                Span::mixed_site().located_at(expr.span()),
            )
        })
        .collect();

    // rust-analyzer resolves format strings through the built-in `format_args!`.
    // Captures resolve to the enclosing variables; named arguments get a `""`
    // stand-in so that they are not reported as missing captures.
    let named_args = named.iter().map(|(ident, _)| quote!(#ident = ""));

    let mut stmts = Vec::new();
    for action in &actions {
        match action {
            Action::Lit(text) => {
                let lit = LitStr::new(text, span);
                stmts.push(quote! { #prompt.push_plain(#lit); });
            }
            Action::Arg { index, .. } => {
                let binding = &bindings[*index];
                stmts.push(quote! { #push_into(#binding, &mut #prompt); });
            }
            Action::Capture { ident } => {
                stmts.push(quote! { #push_into(&#ident, &mut #prompt); });
            }
        }
    }

    let body = quote! {
        let mut #prompt = #prompt_ty::new();
        #(#stmts)*
        #prompt
    };
    // The match evaluates every argument exactly once, from left to right, as
    // `format!` does, and keeps the temporaries that the arguments borrow
    // alive until the prompt is built. The bindings are references, so the
    // expansion does not move the caller's values.
    let evaluated = if exprs.is_empty() {
        body
    } else {
        quote! {
            match (#(&(#exprs),)*) {
                (#(#bindings,)*) => { #body }
            }
        }
    };

    Ok(quote! {{
      #[allow(unexpected_cfgs)]
      {
        #[cfg(rust_analyzer)]
        let _ = ::core::format_args!(#fmt #(, #named_args)*);
      }
      #evaluated
    }})
}
