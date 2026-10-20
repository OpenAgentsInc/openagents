//! Where a completion starts in the composer's text, shared by the desktop
//! and the web (#12216): an `@` file mention, a `/` command, or a `$`
//! skill, under Markdown's rules so code and links never complete.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use crate::composer_markdown::in_code;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionToken {
    pub range: Range<usize>,
    pub query: String,
}

/// Refine a locally identified token using Markdown source ranges. This is
/// deliberately called only after a trigger has passed cheap boundary checks.
fn completion_markdown_end(
    text: &str,
    start: usize,
    cursor: usize,
    mut end: usize,
) -> Option<usize> {
    use pulldown_cmark::{Event, Options, Parser, Tag};
    let needs_quote_boundary = text[..start].ends_with('>');
    let line_start = text[..start].rfind('\n').map_or(0, |at| at + 1);
    let mut paragraph_start = line_start;
    let mut token_is_prose = false;
    let mut quote_boundary = false;
    let parser = Parser::new_ext(text, Options::ENABLE_STRIKETHROUGH);
    // Definitions have no body events, but their destinations are still link
    // syntax and must never be replaced by a nested canonical link.
    if parser
        .reference_definitions()
        .iter()
        .any(|(_, definition)| definition.span.contains(&start))
    {
        return None;
    }
    for (event, range) in parser.into_offset_iter() {
        if !range.contains(&start) {
            continue;
        }
        match event {
            Event::Start(Tag::Link { .. } | Tag::Image { .. }) => return None,
            Event::Start(Tag::Paragraph | Tag::Heading { .. } | Tag::Item) => {
                paragraph_start = range.start;
            }
            Event::Text(_) => token_is_prose = true,
            Event::Start(Tag::BlockQuote(_)) if needs_quote_boundary => {
                let prefix = &text[range.start.max(line_start)..start];
                quote_boundary |= prefix.chars().all(|ch| matches!(ch, '>' | ' ' | '\t'));
            }
            Event::Start(tag @ (Tag::Strong | Tag::Emphasis | Tag::Strikethrough)) => {
                let delimiter = match tag {
                    Tag::Strong => 2,
                    Tag::Emphasis => 1,
                    _ => text[range.clone()]
                        .bytes()
                        .take_while(|byte| *byte == b'~')
                        .count(),
                };
                let closing = range.end.saturating_sub(delimiter);
                if cursor > closing {
                    return None;
                }
                end = end.min(closing);
            }
            _ => {}
        }
    }
    if needs_quote_boundary && !quote_boundary {
        return None;
    }
    // Duplicate reference definitions are consumed without body events and
    // only the first definition is retained by reference_definitions(). Requiring
    // text also protects those later definitions and raw HTML without excluding
    // tight-list items, which omit paragraph events.
    if !token_is_prose {
        return None;
    }
    // The parser leaves an unfinished link as prose. Once its destination is
    // being authored, inserting a canonical link would create nested syntax.
    // An unfinished destination can continue across a soft line break. Stay
    // within its parsed paragraph so a malformed link in an earlier block does
    // not disable completion in later prose.
    let before = &text[paragraph_start..cursor];
    for (destination, _) in before.rmatch_indices("](") {
        if let Some(label) = before[..destination].rfind('[') {
            let escaped = before[..label]
                .bytes()
                .rev()
                .take_while(|byte| *byte == b'\\')
                .count()
                % 2
                == 1;
            let closing_escaped = before[..destination]
                .bytes()
                .rev()
                .take_while(|byte| *byte == b'\\')
                .count()
                % 2
                == 1;
            if !escaped && !closing_escaped && paragraph_start + destination + 2 <= start {
                let mut depth = 1usize;
                let mut chars = before[destination + 2..].chars();
                while let Some(ch) = chars.next() {
                    match ch {
                        '\\' => {
                            chars.next();
                        }
                        '(' => depth += 1,
                        ')' => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if depth > 0 {
                    return None;
                }
            }
        }
    }
    (!in_code(text, cursor)).then_some(end)
}

/// The `@` must begin a token. This intentionally excludes `name@example.com`
/// and ordinary words while allowing punctuation such as `(@src`.
pub fn mention_token(text: &str, cursor: usize) -> Option<MentionToken> {
    if cursor > text.len() || !text.is_char_boundary(cursor) {
        return None;
    }
    let token_start = text[..cursor]
        .char_indices()
        .rev()
        .find_map(|(at, ch)| ch.is_whitespace().then_some(at + ch.len_utf8()))
        .unwrap_or(0);
    let Some(relative_at) = text[token_start..cursor].rfind('@') else {
        return None;
    };
    let at = token_start + relative_at;
    let valid_boundary = at == 0
        || text[..at]
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_whitespace() || matches!(ch, '(' | '[' | '{' | '>'));
    if text[at + 1..cursor].contains('@') || !valid_boundary {
        return None;
    }
    let closing = text[..at].chars().next_back().and_then(|ch| match ch {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        _ => None,
    });
    let end = text[cursor..]
        .char_indices()
        .find_map(|(offset, ch)| {
            (ch.is_whitespace() || Some(ch) == closing).then_some(cursor + offset)
        })
        .unwrap_or(text.len());
    if closing.is_some_and(|ch| text[at + 1..cursor].contains(ch)) {
        return None;
    }
    let end = completion_markdown_end(text, at, cursor, end)?;
    Some(MentionToken {
        range: at..end,
        query: text[at + 1..cursor].to_string(),
    })
}

/// Invocation triggers share file completion's boundary rules, but never
/// interpret paths, currency amounts or code as invocations.
pub fn slash_token(text: &str, cursor: usize) -> Option<MentionToken> {
    invocation_token(text, cursor, '/')
}

pub fn invocation_token(text: &str, cursor: usize, prefix: char) -> Option<MentionToken> {
    if cursor > text.len() || !text.is_char_boundary(cursor) {
        return None;
    }
    let start = text[..cursor]
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace() || matches!(c, '(' | '[' | '{' | '>'))
        .map_or(0, |(i, c)| i + c.len_utf8());
    if !text[start..cursor].starts_with(prefix) {
        return None;
    }
    let query = &text[start + 1..cursor];
    let name_grapheme = |grapheme: &str| {
        grapheme.chars().next().is_some_and(char::is_alphanumeric)
            || matches!(grapheme, "-" | "_" | ":" | ".")
    };
    if !query.graphemes(true).all(name_grapheme)
        || (prefix == '$' && query.starts_with(char::is_numeric))
    {
        return None;
    }
    // Scan from the token's start, including any combining marks after a caret
    // positioned between code points of the same grapheme.
    let end = text[start + 1..]
        .grapheme_indices(true)
        .find_map(|(offset, grapheme)| (!name_grapheme(grapheme)).then_some(start + 1 + offset))
        .unwrap_or(text.len());
    if text.get(end..end + 1) == Some("/") {
        return None;
    }
    let end = completion_markdown_end(text, start, cursor, end)?;
    Some(MentionToken {
        range: start..end,
        query: query.to_string(),
    })
}
