//! GitHub references in fleet rows as links (#11250).
//!
//! A row's title starts with its issue (`#11250 Fleet rows…`) and its line
//! may name a commit (`landed 248c7ce8d6`, `committed 18d6ee1d2b`) or a
//! pull request's address. [`linked`] draws that text with the issue, the
//! commit and the address as links to GitHub and everything else as plain
//! text. The phone shows the same links as buttons
//! (`openagents-mobile` `link_view::github_refs`).

use maud::{Markup, html};

/// The repository the self-driving runs and the landing queue work on,
/// for rows that don't name one.
pub(crate) const DEFAULT_REPOSITORY: &str = "OpenAgentsInc/openagents";

/// The words before a commit that make it one.
const BEFORE_COMMIT: [&str; 2] = ["landed", "committed"];

fn repository_ok(repository: &str) -> bool {
    repository.split('/').count() == 2
        && repository.split('/').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        })
}

/// One piece of a row's text.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Piece {
    Text(String),
    Link { text: String, href: String },
}

/// `text` split into plain text and links for `repository` (`owner/name`).
pub(crate) fn pieces(text: &str, repository: &str) -> Vec<Piece> {
    let repository = if repository_ok(repository) {
        repository
    } else {
        DEFAULT_REPOSITORY
    };
    let mut out: Vec<Piece> = Vec::new();
    let mut push_text = |out: &mut Vec<Piece>, text: &str| match out.last_mut() {
        Some(Piece::Text(last)) => last.push_str(text),
        _ => out.push(Piece::Text(text.to_owned())),
    };
    let mut previous = "";
    for (index, word) in text.split(' ').enumerate() {
        if index > 0 {
            push_text(&mut out, " ");
        }
        let core = word.trim_end_matches(['.', ',', ':', ';', ')']);
        let tail = &word[core.len()..];
        let link = if let Some(number) = core.strip_prefix('#')
            && !number.is_empty()
            && number.len() <= 9
            && number.bytes().all(|b| b.is_ascii_digit())
        {
            Some(format!("https://github.com/{repository}/issues/{number}"))
        } else if BEFORE_COMMIT.contains(&previous)
            && (7..=40).contains(&core.len())
            && core.bytes().all(|b| b.is_ascii_hexdigit())
        {
            Some(format!("https://github.com/{repository}/commit/{core}"))
        } else if core.starts_with("https://github.com/")
            && core.len() <= 200
            && !core.contains(['"', '<', '>'])
        {
            Some(core.to_owned())
        } else {
            None
        };
        match link {
            Some(href) => {
                out.push(Piece::Link {
                    text: core.to_owned(),
                    href,
                });
                if !tail.is_empty() {
                    push_text(&mut out, tail);
                }
            }
            None => push_text(&mut out, word),
        }
        previous = core;
    }
    out
}

/// `text` with its issue, commit and pull request as links to GitHub.
pub(crate) fn linked(text: &str, repository: &str) -> Markup {
    html! {
        @for piece in pieces(text, repository) {
            @match piece {
                Piece::Text(text) => (text),
                Piece::Link { text, href } => a href=(href) rel="noopener" { (text) },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(text: &str, href: &str) -> Piece {
        Piece::Link {
            text: text.into(),
            href: href.into(),
        }
    }

    #[test]
    fn the_issue_the_commit_and_the_pull_request_are_links() {
        assert_eq!(
            pieces("#11250 Fleet rows for cloud runs", DEFAULT_REPOSITORY),
            [
                link(
                    "#11250",
                    "https://github.com/OpenAgentsInc/openagents/issues/11250"
                ),
                Piece::Text(" Fleet rows for cloud runs".into()),
            ]
        );
        assert_eq!(
            pieces("landed 248c7ce8d6.", "octo/hello"),
            [
                Piece::Text("landed ".into()),
                link(
                    "248c7ce8d6",
                    "https://github.com/octo/hello/commit/248c7ce8d6"
                ),
                Piece::Text(".".into()),
            ]
        );
        assert_eq!(
            pieces(
                "pull request https://github.com/octo/hello/pull/9",
                "octo/hello"
            )[1],
            link(
                "https://github.com/octo/hello/pull/9",
                "https://github.com/octo/hello/pull/9"
            )
        );
        // A hex word that isn't a commit, a bad repository, and plain text.
        assert_eq!(
            pieces("coding deadbeef", DEFAULT_REPOSITORY),
            [Piece::Text("coding deadbeef".into())]
        );
        assert_eq!(
            pieces("#7", "not a repo")[0],
            link("#7", "https://github.com/OpenAgentsInc/openagents/issues/7")
        );
        assert_eq!(
            pieces("queued 20261011T080000Z-x", DEFAULT_REPOSITORY),
            [Piece::Text("queued 20261011T080000Z-x".into())]
        );
        let html = linked("#5 <b>", "octo/hello").into_string();
        assert!(html.contains(r#"<a href="https://github.com/octo/hello/issues/5""#));
        assert!(html.contains("&lt;b&gt;"), "{html}");
    }
}
