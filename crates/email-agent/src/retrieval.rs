use crate::NormalizedConversationItem;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RetrievalQuery {
    pub text: String,
    pub thread_id: Option<String>,
    pub participant_email: Option<String>,
    pub label: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RetrievedContextChunk {
    pub source_message_id: String,
    pub normalized_id: String,
    pub thread_id: String,
    pub snippet: String,
    pub score_milli: u32,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct RetrievalIndex {
    items: Vec<NormalizedConversationItem>,
}

impl RetrievalIndex {
    pub fn upsert(&mut self, item: NormalizedConversationItem) {
        if let Some(existing) = self
            .items
            .iter_mut()
            .find(|existing| existing.normalized_id == item.normalized_id)
        {
            *existing = item;
            return;
        }
        self.items.push(item);
    }

    pub fn search(&self, query: &RetrievalQuery) -> Vec<RetrievedContextChunk> {
        if query.limit == 0 {
            return Vec::new();
        }

        let query_terms = tokenize(query.text.as_str());
        let mut rows =
            self.items
                .iter()
                .filter(|item| {
                    query
                        .thread_id
                        .as_deref()
                        .is_none_or(|thread| item.thread_id == thread)
                })
                .filter(|item| {
                    query
                        .participant_email
                        .as_deref()
                        .is_none_or(|participant| {
                            item.sender_email.eq_ignore_ascii_case(participant)
                                || item
                                    .recipient_emails
                                    .iter()
                                    .any(|recipient| recipient.eq_ignore_ascii_case(participant))
                        })
                })
                .filter(|item| {
                    query.label.as_deref().is_none_or(|label| {
                        item.labels.iter().any(|item_label| item_label == label)
                    })
                })
                .map(|item| {
                    let lexical = lexical_score(item, query_terms.as_slice());
                    let semantic = semantic_score(item.body_summary.as_str(), query.text.as_str());
                    let blended = lexical
                        .saturating_mul(7)
                        .saturating_add(semantic.saturating_mul(3))
                        / 10;
                    RetrievedContextChunk {
                        source_message_id: item.source_message_id.clone(),
                        normalized_id: item.normalized_id.clone(),
                        thread_id: item.thread_id.clone(),
                        snippet: item.body_summary.chars().take(180).collect(),
                        score_milli: blended,
                    }
                })
                .collect::<Vec<RetrievedContextChunk>>();

        rows.sort_by(|left, right| {
            right
                .score_milli
                .cmp(&left.score_milli)
                .then_with(|| left.normalized_id.cmp(&right.normalized_id))
        });
        rows.truncate(query.limit);
        rows
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|token| {
            token
                .trim_matches(|char: char| !char.is_alphanumeric())
                .to_ascii_lowercase()
        })
        .filter(|token| !token.is_empty())
        .collect()
}

fn lexical_score(item: &NormalizedConversationItem, query_terms: &[String]) -> u32 {
    if query_terms.is_empty() {
        return 0;
    }

    let mut corpus = tokenize(item.subject.as_str());
    corpus.extend(tokenize(item.body_summary.as_str()));
    let corpus_set = corpus.into_iter().collect::<BTreeSet<String>>();

    let overlap = query_terms
        .iter()
        .filter(|term| corpus_set.contains(*term))
        .count();

    ((overlap as u32).saturating_mul(1000)) / (query_terms.len() as u32)
}

fn semantic_score(corpus_text: &str, query_text: &str) -> u32 {
    let corpus = char_trigrams(corpus_text);
    let query = char_trigrams(query_text);
    if corpus.is_empty() || query.is_empty() {
        return 0;
    }

    let intersection_count = corpus.intersection(&query).count() as u32;
    let union_count = corpus.union(&query).count() as u32;
    if union_count == 0 {
        return 0;
    }
    intersection_count.saturating_mul(1000) / union_count
}

fn char_trigrams(text: &str) -> BTreeSet<String> {
    let normalized = text.to_ascii_lowercase();
    let chars = normalized.chars().collect::<Vec<char>>();
    if chars.len() < 3 {
        return BTreeSet::new();
    }

    let mut grams = BTreeSet::new();
    for index in 0..=chars.len().saturating_sub(3) {
        grams.insert(chars[index..index + 3].iter().collect());
    }
    grams
}

#[cfg(test)]
mod tests {
    use super::{RetrievalIndex, RetrievalQuery};
    use crate::NormalizedConversationItem;

    fn normalized_item(
        normalized_id: &str,
        source_message_id: &str,
        thread_id: &str,
        subject: &str,
        body_summary: &str,
    ) -> NormalizedConversationItem {
        NormalizedConversationItem {
            normalized_id: normalized_id.to_string(),
            source_message_id: source_message_id.to_string(),
            thread_id: thread_id.to_string(),
            subject: subject.to_string(),
            sender_email: "sender@example.com".to_string(),
            recipient_emails: vec!["recipient@example.com".to_string()],
            body_summary: body_summary.to_string(),
            quoted_blocks: Vec::new(),
            signature_block: None,
            timestamp_ms: 1,
            labels: vec!["INBOX".to_string()],
        }
    }

    #[test]
    fn retrieval_ranks_by_score_then_id() {
        let mut index = RetrievalIndex::default();
        index.upsert(normalized_item(
            "gmail:m1",
            "m1",
            "thread-1",
            "Invoice follow up",
            "Following up on the invoice status and payment confirmation",
        ));
        index.upsert(normalized_item(
            "gmail:m2",
            "m2",
            "thread-2",
            "Calendar",
            "Scheduling meeting next week",
        ));

        let rows = index.search(&RetrievalQuery {
            text: "invoice payment status".to_string(),
            thread_id: None,
            participant_email: None,
            label: None,
            limit: 2,
        });

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].source_message_id, "m1");
        assert!(rows[0].score_milli >= rows[1].score_milli);
    }

    #[test]
    fn retrieval_applies_filters() {
        let mut index = RetrievalIndex::default();
        index.upsert(normalized_item(
            "gmail:m1",
            "m1",
            "thread-a",
            "Status",
            "Alpha body",
        ));
        index.upsert(normalized_item(
            "gmail:m2",
            "m2",
            "thread-b",
            "Status",
            "Beta body",
        ));

        let rows = index.search(&RetrievalQuery {
            text: "status".to_string(),
            thread_id: Some("thread-b".to_string()),
            participant_email: None,
            label: Some("INBOX".to_string()),
            limit: 5,
        });
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].thread_id, "thread-b");
    }
}
