use std::collections::BTreeSet;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct KnowledgeDocument {
    pub document_id: String,
    pub title: String,
    pub source_uri: String,
    pub body: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct KnowledgeChunk {
    pub chunk_id: String,
    pub document_id: String,
    pub title: String,
    pub source_uri: String,
    pub text: String,
    pub tags: Vec<String>,
    pub ordinal: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GroundingReference {
    pub chunk_id: String,
    pub document_id: String,
    pub source_uri: String,
    pub snippet: String,
    pub score_milli: u32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct KnowledgeChunkingConfig {
    pub max_words_per_chunk: usize,
}

impl Default for KnowledgeChunkingConfig {
    fn default() -> Self {
        Self {
            max_words_per_chunk: 120,
        }
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct KnowledgeBase {
    chunks: Vec<KnowledgeChunk>,
}

impl KnowledgeBase {
    pub fn ingest_document(
        &mut self,
        document: &KnowledgeDocument,
        config: &KnowledgeChunkingConfig,
    ) -> Vec<KnowledgeChunk> {
        if config.max_words_per_chunk == 0 {
            return Vec::new();
        }

        let words = document.body.split_whitespace().collect::<Vec<&str>>();
        if words.is_empty() {
            return Vec::new();
        }

        let mut emitted = Vec::<KnowledgeChunk>::new();
        let mut ordinal = 0usize;
        for start in (0..words.len()).step_by(config.max_words_per_chunk) {
            let end = (start + config.max_words_per_chunk).min(words.len());
            let text = words[start..end].join(" ");
            let chunk = KnowledgeChunk {
                chunk_id: format!("{}:chunk:{ordinal:04}", document.document_id),
                document_id: document.document_id.clone(),
                title: document.title.clone(),
                source_uri: document.source_uri.clone(),
                text,
                tags: document.tags.clone(),
                ordinal,
            };
            ordinal = ordinal.saturating_add(1);
            emitted.push(chunk.clone());
            self.upsert_chunk(chunk);
        }

        emitted
    }

    pub fn search(
        &self,
        query: &str,
        required_tags: &[String],
        limit: usize,
    ) -> Vec<GroundingReference> {
        if limit == 0 {
            return Vec::new();
        }

        let query_terms = tokenize(query);
        let required_tags = required_tags.iter().cloned().collect::<BTreeSet<String>>();

        let mut rows = self
            .chunks
            .iter()
            .filter(|chunk| {
                required_tags.is_empty()
                    || required_tags
                        .iter()
                        .all(|tag| chunk.tags.iter().any(|chunk_tag| chunk_tag == tag))
            })
            .map(|chunk| {
                let score_milli =
                    lexical_overlap_score(chunk.text.as_str(), query_terms.as_slice());
                GroundingReference {
                    chunk_id: chunk.chunk_id.clone(),
                    document_id: chunk.document_id.clone(),
                    source_uri: chunk.source_uri.clone(),
                    snippet: chunk.text.chars().take(200).collect(),
                    score_milli,
                }
            })
            .collect::<Vec<GroundingReference>>();

        rows.sort_by(|left, right| {
            right
                .score_milli
                .cmp(&left.score_milli)
                .then_with(|| left.chunk_id.cmp(&right.chunk_id))
        });
        rows.truncate(limit);
        rows
    }

    fn upsert_chunk(&mut self, chunk: KnowledgeChunk) {
        if let Some(existing) = self
            .chunks
            .iter_mut()
            .find(|existing| existing.chunk_id == chunk.chunk_id)
        {
            *existing = chunk;
            return;
        }
        self.chunks.push(chunk);
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

fn lexical_overlap_score(corpus: &str, query_terms: &[String]) -> u32 {
    if query_terms.is_empty() {
        return 0;
    }
    let corpus_terms = tokenize(corpus).into_iter().collect::<BTreeSet<String>>();
    let overlap = query_terms
        .iter()
        .filter(|term| corpus_terms.contains(*term))
        .count();
    ((overlap as u32).saturating_mul(1000)) / (query_terms.len() as u32)
}

#[cfg(test)]
mod tests {
    use super::{KnowledgeBase, KnowledgeChunkingConfig, KnowledgeDocument};

    #[test]
    fn ingest_document_chunks_by_configured_word_limit() {
        let mut kb = KnowledgeBase::default();
        let document = KnowledgeDocument {
            document_id: "doc-1".to_string(),
            title: "Operations".to_string(),
            source_uri: "kb://ops".to_string(),
            body: "one two three four five six seven eight".to_string(),
            tags: vec!["ops".to_string()],
        };

        let chunks = kb.ingest_document(
            &document,
            &KnowledgeChunkingConfig {
                max_words_per_chunk: 3,
            },
        );

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].chunk_id, "doc-1:chunk:0000");
        assert_eq!(chunks[1].text, "four five six");
    }

    #[test]
    fn search_respects_required_tags_and_returns_scored_references() {
        let mut kb = KnowledgeBase::default();
        let doc_ops = KnowledgeDocument {
            document_id: "doc-ops".to_string(),
            title: "Ops".to_string(),
            source_uri: "kb://ops".to_string(),
            body: "invoice policy payment reminders".to_string(),
            tags: vec!["ops".to_string()],
        };
        let doc_sales = KnowledgeDocument {
            document_id: "doc-sales".to_string(),
            title: "Sales".to_string(),
            source_uri: "kb://sales".to_string(),
            body: "pipeline forecast outbound prospecting".to_string(),
            tags: vec!["sales".to_string()],
        };

        kb.ingest_document(&doc_ops, &KnowledgeChunkingConfig::default());
        kb.ingest_document(&doc_sales, &KnowledgeChunkingConfig::default());

        let refs = kb.search("invoice payment", &["ops".to_string()], 5);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].document_id, "doc-ops");
        assert!(refs[0].score_milli > 0);
    }
}
