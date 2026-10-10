//! Grounded values in the chat worker's replies (#11114,
//! `inference::grounded`).
//!
//! A product reply (the [`super::Corpus::Product`] grounded tier, where
//! questions about us, our plans, and model prices are answered) is
//! written over a [`Ledger`]: the public rate card, so a price or a model
//! name is a reference to its row rather than retyped, and the passages
//! the model was given. The model reads the ledger's brief beside the
//! passages; its reply streams through a [`Stream`], which holds only an
//! open `{...}` reference until it closes and fills it; and [`finish`]
//! fills the whole reply, drops what does not resolve, and runs the
//! trace check. The filled reply is what the thread keeps, so the next
//! turn's model reads the values its references wrote.

use inference::grounded::{self, Ledger, Stream};
use serde_json::json;

use super::seams::Passage;

/// The public rate card the gateway would serve with no overrides.
#[must_use]
pub fn rate_card() -> inference::rates::Card {
    inference::rates::Card::published(None)
}

/// A product reply's ledger: the rate card as `r1`, then the passages as
/// `r2` (`passages[n]` with `id`, `title`, `source`, and `url` when the
/// source is a public link).
#[must_use]
pub fn product_ledger(passages: &[Passage]) -> Ledger {
    let mut ledger = Ledger::new();
    ledger.record_rates(&rate_card());
    if !passages.is_empty() {
        let shown: Vec<_> = passages
            .iter()
            .map(|passage| {
                let mut item = json!({
                    "id": passage.id,
                    "title": passage.title,
                    "source": passage.source,
                });
                if passage.source.starts_with("https://") {
                    item["url"] = json!(passage.source);
                }
                item
            })
            .collect();
        ledger.record(grounded::source::DOCS_SEARCH, json!({ "passages": shown }));
    }
    ledger
}

/// The note the model reads after the passages.
#[must_use]
pub fn note(ledger: &Ledger) -> String {
    ledger.brief()
}

/// A streaming reply's resolver over `ledger`.
#[must_use]
pub fn stream(ledger: Ledger) -> Stream {
    Stream::new(ledger)
}

/// A finished reply, filled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finished {
    /// The reply as the reader sees it.
    pub text: String,
    /// How many references resolved.
    pub resolved: usize,
    /// The references dropped, as written.
    pub dropped: Vec<String>,
    /// Numbers and URLs in the filled reply that no result holds.
    pub untraced: usize,
}

impl Finished {
    /// The worker's log line: counts and reference shapes only, never
    /// the reply's words.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "router grounded reply: {} resolved, {} dropped, {} untraced",
            self.resolved,
            self.dropped.len(),
            self.untraced
        )
    }
}

/// Fills a whole reply against `ledger`.
#[must_use]
pub fn finish(text: &str, ledger: &Ledger) -> Finished {
    let resolved = grounded::resolve(text, ledger);
    let untraced = grounded::untraced(&resolved.text, ledger).len();
    Finished {
        resolved: resolved.resolutions.len(),
        dropped: resolved
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.reference.clone())
            .collect(),
        untraced,
        text: resolved.text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passage(id: &str, source: &str) -> Passage {
        Passage {
            id: id.into(),
            title: "Connect a Mac".into(),
            text: "Open the app and sign in.".into(),
            source: source.into(),
            relevance: 0.9,
            answer: None,
            off_computer: false,
            in_app: false,
        }
    }

    /// A product reply's prices come from the rate card by reference:
    /// the model's `{r1.models["…"].input.price_usd}` is the card's own
    /// number, streamed and finished alike, and it traces.
    #[test]
    fn a_product_reply_fills_prices_from_the_rate_card() {
        let card = rate_card();
        let row = card
            .rows
            .iter()
            .find(|row| row.kind == inference::rates::Kind::List)
            .expect("a list row");
        let ledger = product_ledger(&[passage(
            "product.connect-a-mac@2",
            "https://openagents.com/docs/connect",
        )]);
        assert!(note(&ledger).contains("- r1 (rate_card)"));
        assert!(note(&ledger).contains("- r2 (docs_search)"));
        let reply = format!(
            "{{r1.models[\"{model}\"].model}} costs ${{r1.models[\"{model}\"].input.price_usd}} \
             per million input tokens. See {{cite:r2.passages[0]}}.",
            model = row.model
        );
        let finished = finish(&reply, &ledger);
        assert_eq!(finished.resolved, 3, "{finished:?}");
        assert!(finished.dropped.is_empty());
        assert!(finished.text.contains(&format!("${}", row.input.price_usd)));
        assert!(
            finished
                .text
                .contains("[Connect a Mac](https://openagents.com/docs/connect)")
        );
        assert_eq!(finished.untraced, 0, "{}", finished.text);
        // Streamed in pieces, the reader sees the same reply.
        let mut streaming = stream(ledger.clone());
        let mut seen = String::new();
        for piece in reply.as_bytes().chunks(7) {
            seen.push_str(&streaming.push(std::str::from_utf8(piece).unwrap()));
        }
        seen.push_str(&streaming.finish());
        assert_eq!(seen, finished.text);
        // A retyped price does not trace, and the line says so without
        // the reply's words.
        let retyped = finish("It costs $123.45 per million.", &ledger);
        assert_eq!(retyped.untraced, 1);
        assert!(!retyped.line().contains("costs"));
    }

    /// A passage from a repository path is recorded without a url, so it
    /// cannot be cited as a link.
    #[test]
    fn a_repository_passage_has_no_link() {
        let ledger = product_ledger(&[passage("product.x@1", "docs/x.md")]);
        let finished = finish("{cite:r2.passages[0]}", &ledger);
        assert_eq!(finished.text, "");
        assert_eq!(finished.dropped, ["{cite:r2.passages[0]}"]);
    }
}
