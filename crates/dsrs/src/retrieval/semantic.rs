//! Semantic/vector embedding retrieval backend.
//!
//! Provides semantic search via embeddings:
//! - Local embeddings (via Ollama or other local models)
//! - Swarm embeddings (via Pylon)
//! - External API embeddings (OpenAI, Voyage, etc.)

use super::{RepoIndex, RetrievalConfig, RetrievalResult};
use crate::adapter::swarm_dispatch::{SwarmDispatchConfig, SwarmDispatcher};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Semantic retrieval backend using vector embeddings.
pub struct SemanticIndex {
    /// Root path of the repository.
    #[allow(dead_code)]
    repo_path: PathBuf,

    /// Embedding provider configuration.
    provider: EmbeddingProvider,

    /// Cached embeddings (path -> embedding).
    embeddings: HashMap<String, Vec<f32>>,

    /// Chunk size for splitting files.
    chunk_size: usize,

    /// Overlap between chunks.
    chunk_overlap: usize,
}

/// Embedding provider configuration.
#[derive(Debug, Clone)]
#[derive(Default)]
pub enum EmbeddingProvider {
    /// Local Ollama embeddings.
    Ollama { model: String, base_url: String },

    /// OpenAI embeddings.
    OpenAI { model: String, api_key: String },

    /// Pylon swarm embeddings.
    Swarm { relay_url: String },

    /// No embeddings (disabled).
    #[default]
    None,
}


/// A chunk of code with its embedding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChunk {
    /// File path.
    pub path: String,

    /// Start line.
    pub start_line: usize,

    /// End line.
    pub end_line: usize,

    /// Content.
    pub content: String,

    /// Embedding vector (if computed).
    #[serde(skip)]
    pub embedding: Option<Vec<f32>>,
}

impl SemanticIndex {
    /// Create a new semantic index for a repository.
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
            provider: EmbeddingProvider::None,
            embeddings: HashMap::new(),
            chunk_size: 500,
            chunk_overlap: 50,
        }
    }

    /// Configure Ollama as the embedding provider.
    pub fn with_ollama(mut self, model: impl Into<String>) -> Self {
        self.provider = EmbeddingProvider::Ollama {
            model: model.into(),
            base_url: "http://localhost:11434".to_string(),
        };
        self
    }

    /// Configure OpenAI as the embedding provider.
    pub fn with_openai(mut self, api_key: impl Into<String>) -> Self {
        self.provider = EmbeddingProvider::OpenAI {
            model: "text-embedding-3-small".to_string(),
            api_key: api_key.into(),
        };
        self
    }

    /// Configure Pylon swarm as the embedding provider.
    pub fn with_swarm(mut self, relay_url: impl Into<String>) -> Self {
        self.provider = EmbeddingProvider::Swarm {
            relay_url: relay_url.into(),
        };
        self
    }

    /// Set chunk size.
    pub fn with_chunk_size(mut self, size: usize, overlap: usize) -> Self {
        self.chunk_size = size;
        self.chunk_overlap = overlap;
        self
    }

    /// Compute embedding for text using configured provider.
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        match &self.provider {
            EmbeddingProvider::Ollama { model, base_url } => {
                self.embed_ollama(text, model, base_url).await
            }
            EmbeddingProvider::OpenAI { model, api_key } => {
                self.embed_openai(text, model, api_key).await
            }
            EmbeddingProvider::Swarm { relay_url } => self.embed_swarm(text, relay_url).await,
            EmbeddingProvider::None => {
                anyhow::bail!("No embedding provider configured")
            }
        }
    }

    /// Embed using Ollama.
    async fn embed_ollama(&self, text: &str, model: &str, base_url: &str) -> Result<Vec<f32>> {
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/api/embeddings", base_url))
            .json(&serde_json::json!({
                "model": model,
                "prompt": text
            }))
            .send()
            .await
            .context("Failed to call Ollama embeddings API")?;

        let json: serde_json::Value = response.json().await?;
        let embedding = json["embedding"]
            .as_array()
            .context("Missing embedding in response")?
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect();

        Ok(embedding)
    }

    /// Embed using OpenAI.
    async fn embed_openai(&self, text: &str, model: &str, api_key: &str) -> Result<Vec<f32>> {
        let client = reqwest::Client::new();
        let response = client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&serde_json::json!({
                "model": model,
                "input": text
            }))
            .send()
            .await
            .context("Failed to call OpenAI embeddings API")?;

        let json: serde_json::Value = response.json().await?;
        let embedding = json["data"][0]["embedding"]
            .as_array()
            .context("Missing embedding in response")?
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect();

        Ok(embedding)
    }

    /// Embed using Pylon swarm.
    ///
    /// Submits a NIP-90 embeddings job to the swarm and awaits the result.
    /// This runs in offline mode (no actual submission) unless a private key
    /// is configured via environment or the dispatcher is built with credentials.
    async fn embed_swarm(&self, text: &str, relay_url: &str) -> Result<Vec<f32>> {
        // Create a dispatcher configured for the specified relay
        let config = SwarmDispatchConfig {
            relays: vec![relay_url.to_string()],
            default_budget_msats: 100, // Embeddings are cheap
            timeout: Duration::from_secs(30),
            wait_for_ok: true,
        };

        let dispatcher = SwarmDispatcher::generate().with_config(config);

        // Dispatch the embeddings job
        let result = dispatcher
            .dispatch_embeddings(vec![text.to_string()])
            .await
            .context("Failed to dispatch embeddings job to swarm")?;

        // Extract the first embedding
        result
            .result
            .embeddings
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No embedding returned from swarm"))
    }

    /// Embed a batch of texts using Pylon swarm.
    ///
    /// More efficient than calling embed_swarm repeatedly.
    pub async fn embed_batch_swarm(
        &self,
        texts: &[String],
        relay_url: &str,
    ) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let config = SwarmDispatchConfig {
            relays: vec![relay_url.to_string()],
            default_budget_msats: 100 * texts.len() as u64,
            timeout: Duration::from_secs(60),
            wait_for_ok: true,
        };

        let dispatcher = SwarmDispatcher::generate().with_config(config);

        let result = dispatcher
            .dispatch_embeddings(texts.to_vec())
            .await
            .context("Failed to dispatch batch embeddings job to swarm")?;

        Ok(result.result.embeddings)
    }

    /// Compute cosine similarity between two vectors.
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }

    /// Split content into chunks.
    #[allow(dead_code)]
    fn chunk_content(&self, path: &str, content: &str) -> Vec<CodeChunk> {
        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::new();

        let chunk_lines = self.chunk_size / 80; // Approximate lines per chunk
        let overlap_lines = self.chunk_overlap / 80;

        let mut start = 0;
        while start < lines.len() {
            let end = (start + chunk_lines).min(lines.len());
            let chunk_content: String = lines[start..end].join("\n");

            chunks.push(CodeChunk {
                path: path.to_string(),
                start_line: start + 1,
                end_line: end,
                content: chunk_content,
                embedding: None,
            });

            start = if end >= lines.len() {
                lines.len()
            } else {
                end - overlap_lines
            };
        }

        chunks
    }
}

#[async_trait]
impl RepoIndex for SemanticIndex {
    async fn query(&self, query: &str, config: &RetrievalConfig) -> Result<Vec<RetrievalResult>> {
        // Check if embeddings are available
        if matches!(self.provider, EmbeddingProvider::None) {
            anyhow::bail!("Semantic search requires an embedding provider");
        }

        // Embed the query
        let query_embedding = self.embed(query).await?;

        // Find similar chunks
        let mut results: Vec<(f32, RetrievalResult)> = Vec::new();

        for (path, embedding) in &self.embeddings {
            let similarity = Self::cosine_similarity(&query_embedding, embedding);
            if similarity >= config.min_score {
                results.push((
                    similarity,
                    RetrievalResult::new(path, 1, 1, "")
                        .with_score(similarity)
                        .with_lane("semantic"),
                ));
            }
        }

        // Sort by similarity (descending)
        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Take top k
        let top_results: Vec<RetrievalResult> =
            results.into_iter().take(config.k).map(|(_, r)| r).collect();

        Ok(top_results)
    }

    fn lane_name(&self) -> &str {
        "semantic"
    }

    fn supports_semantic(&self) -> bool {
        true
    }

    async fn build_index(&self, repo_path: &PathBuf) -> Result<()> {
        // Walk repo and embed all files
        // This is a placeholder - real implementation would:
        // 1. Walk the repository
        // 2. Split files into chunks
        // 3. Embed each chunk
        // 4. Store in vector index
        let _ = repo_path;
        Ok(())
    }

    async fn is_available(&self) -> bool {
        match &self.provider {
            EmbeddingProvider::Ollama { base_url, .. } => {
                reqwest::get(format!("{}/api/tags", base_url))
                    .await
                    .map(|r| r.status().is_success())
                    .unwrap_or(false)
            }
            EmbeddingProvider::OpenAI { api_key, .. } => !api_key.is_empty(),
            EmbeddingProvider::Swarm { .. } => true, // Assume available
            EmbeddingProvider::None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((SemanticIndex::cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((SemanticIndex::cosine_similarity(&a, &c)).abs() < 0.001);

        let d = vec![0.707, 0.707, 0.0];
        let sim = SemanticIndex::cosine_similarity(&a, &d);
        assert!((sim - 0.707).abs() < 0.01);
    }

    #[test]
    fn test_chunk_content() {
        let index = SemanticIndex::new(".");
        let content = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10";
        let chunks = index.chunk_content("test.rs", content);

        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].path, "test.rs");
        assert_eq!(chunks[0].start_line, 1);
    }
}
