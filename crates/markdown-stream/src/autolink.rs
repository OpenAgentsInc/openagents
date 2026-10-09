//! Bare URLs in reply text, found so every renderer can draw them as links
//! (GFM's extended autolinks, narrowed to what a reply needs).
//!
//! A link starts at `https://` or `http://`, or at `openagents.com` (with
//! or without a path), at the start of a word. It runs to the next space or
//! `<`, `>`, `"`, or backtick, less trailing punctuation (`.`, `,`, `:`,
//! `;`, `!`, `?`, `'`, `*`, `_`, `~`) and a closing parenthesis or bracket
//! with no partner inside the link. While a reply streams, a link that runs
//! to the end of the text may still be growing, so [`find`] with
//! `streaming` leaves it out until a space or punctuation follows.

use std::ops::Range;

/// Our site, linked even without a scheme.
pub const SITE_HOST: &str = "openagents.com";

/// One bare URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Autolink {
    /// Where the link's text is.
    pub range: Range<usize>,
    /// Where it goes: the text, with `https://` added to a bare
    /// `openagents.com`.
    pub href: String,
}

fn starts_url(rest: &str) -> Option<usize> {
    for scheme in ["https://", "http://"] {
        if rest
            .get(..scheme.len())
            .is_some_and(|s| s.eq_ignore_ascii_case(scheme))
        {
            return Some(scheme.len());
        }
    }
    let host = rest.get(..SITE_HOST.len())?;
    if !host.eq_ignore_ascii_case(SITE_HOST) {
        return None;
    }
    let next = rest[SITE_HOST.len()..].chars().next();
    // `openagents.community` or `openagents.com.evil` is not our site.
    match next {
        None => Some(0),
        Some(c)
            if c == '/'
                || !(c.is_alphanumeric() || c == '.' || c == '-' || c == '_' || c == '@') =>
        {
            Some(0)
        }
        Some('.')
            if rest[SITE_HOST.len() + 1..]
                .chars()
                .next()
                .is_none_or(|c| !c.is_alphanumeric()) =>
        {
            Some(0)
        }
        _ => None,
    }
}

fn stops(c: char) -> bool {
    c.is_whitespace() || matches!(c, '<' | '>' | '"' | '`')
}

/// The bare URLs in `text`, in order. With `streaming`, a URL that runs to
/// the end of `text` is left out.
#[must_use]
pub fn find(text: &str, streaming: bool) -> Vec<Autolink> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < text.len() {
        let rest = &text[at..];
        let before = text[..at].chars().next_back();
        let word_start = before.is_none_or(|c| {
            !(c.is_alphanumeric() || matches!(c, '/' | '.' | '@' | '-' | '_' | ':' | '%' | '='))
        });
        let Some(scheme) = word_start.then(|| starts_url(rest)).flatten() else {
            at += rest.chars().next().map_or(1, char::len_utf8);
            continue;
        };
        let run = rest.find(stops).unwrap_or(rest.len());
        let mut end = run;
        loop {
            let link = &rest[..end];
            let Some(last) = link.chars().next_back() else {
                break;
            };
            let unpaired = |open: char, close: char| {
                last == close && link.matches(close).count() > link.matches(open).count()
            };
            if matches!(
                last,
                '.' | ',' | ':' | ';' | '!' | '?' | '\'' | '*' | '_' | '~'
            ) || unpaired('(', ')')
                || unpaired('[', ']')
            {
                end -= last.len_utf8();
            } else {
                break;
            }
        }
        let link = &rest[..end];
        let host_ok = scheme == 0
            || link[scheme..]
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric());
        let growing = streaming && at + run == text.len();
        if host_ok && !growing && end > scheme {
            let href = if scheme == 0 {
                format!("https://{link}")
            } else {
                link.to_owned()
            };
            out.push(Autolink {
                range: at..at + end,
                href,
            });
        }
        at += run.max(1);
    }
    out
}

/// The site path for a link to our own site (`https://openagents.com/x`
/// is `/x`), so it opens in the same tab; `None` for any other link.
#[must_use]
pub fn same_site(href: &str) -> Option<String> {
    let rest = href
        .get(..8)
        .filter(|s| s.eq_ignore_ascii_case("https://"))
        .map(|_| &href[8..])?;
    let host = rest.get(..SITE_HOST.len())?;
    if !host.eq_ignore_ascii_case(SITE_HOST) {
        return None;
    }
    let path = &rest[SITE_HOST.len()..];
    match path.chars().next() {
        None => Some("/".to_owned()),
        Some('/') => Some(path.to_owned()),
        Some('?' | '#') => Some(format!("/{path}")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hrefs(text: &str, streaming: bool) -> Vec<(String, String)> {
        find(text, streaming)
            .into_iter()
            .map(|l| (text[l.range].to_owned(), l.href))
            .collect()
    }

    #[test]
    fn bare_urls_are_found() {
        assert_eq!(
            hrefs(
                "Sign in at https://openagents.com/projects, then approve it at openagents.com/device.",
                false
            ),
            [
                (
                    "https://openagents.com/projects".into(),
                    "https://openagents.com/projects".into()
                ),
                (
                    "openagents.com/device".into(),
                    "https://openagents.com/device".into()
                ),
            ]
        );
        assert_eq!(
            hrefs(
                "(see https://en.wikipedia.org/wiki/Rust_(language)) and http://x.example!",
                false
            ),
            [
                (
                    "https://en.wikipedia.org/wiki/Rust_(language)".into(),
                    "https://en.wikipedia.org/wiki/Rust_(language)".into()
                ),
                ("http://x.example".into(), "http://x.example".into()),
            ]
        );
        assert_eq!(hrefs("Visit openagents.com.", false).len(), 1);
        for none in [
            "openagents.community",
            "docs.openagents.com/x",
            "me@openagents.com",
            "https://",
            "https://.x",
            "javascript:alert(1)",
            "x/https://a.example",
        ] {
            assert!(
                hrefs(none, false).is_empty(),
                "{none}: {:?}",
                hrefs(none, false)
            );
        }
        // Stops at a backtick, quote, or angle bracket.
        assert_eq!(
            hrefs("`https://a.example`", false)[0].0,
            "https://a.example"
        );
    }

    #[test]
    fn a_url_still_streaming_is_not_linked() {
        assert!(hrefs("Go to https://openagents.com/pro", true).is_empty());
        assert!(hrefs("Go to https://openagents.com/projects.", true).is_empty());
        assert_eq!(
            hrefs("Go to https://openagents.com/projects. ", true).len(),
            1
        );
        assert_eq!(hrefs("Go to https://openagents.com/pro", false).len(), 1);
    }

    #[test]
    fn same_site_links_become_paths() {
        assert_eq!(
            same_site("https://openagents.com/device").as_deref(),
            Some("/device")
        );
        assert_eq!(same_site("https://OpenAgents.com").as_deref(), Some("/"));
        assert_eq!(
            same_site("https://openagents.com?x=1").as_deref(),
            Some("/?x=1")
        );
        assert_eq!(same_site("https://openagents.com.evil.example/x"), None);
        assert_eq!(same_site("http://openagents.com/x"), None);
        assert_eq!(same_site("https://github.com/x"), None);
    }
}
