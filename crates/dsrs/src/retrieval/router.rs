//! Multi-lane retrieval router.
//!
//! Combines multiple retrieval backends and routes queries
//! to the appropriate lane(s).

use super::{RepoIndex, RetrievalConfig, RetrievalResult, RetrievalStats};
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// Multi-lane retrieval router.
pub struct LaneRouter {
    /// Available retrieval lanes.
    lanes: HashMap<String, Arc<dyn RepoIndex>>,

    /// Default lane for queries.
    default_lane: String,

    /// Repository root path.
    repo_path: PathBuf,
}

impl LaneRouter {
    /// Create a new lane router.
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            lanes: HashMap::new(),
            default_lane: "ripgrep".to_string(),
            repo_path: repo_path.into(),
        }
    }

    /// Add a retrieval lane.
    pub fn add_lane(mut self, name: impl Into<String>, index: Arc<dyn RepoIndex>) -> Self {
        self.lanes.insert(name.into(), index);
        self
    }

    /// Set the default lane.
    pub fn with_default_lane(mut self, lane: impl Into<String>) -> Self {
        self.default_lane = lane.into();
        self
    }

    /// Create a router with all available backends auto-detected.
    pub async fn auto_detect(repo_path: impl Into<PathBuf>) -> Result<Self> {
        use super::{GitIndex, LspIndex, RipgrepIndex, SemanticIndex};

        let path = repo_path.into();
        let mut router = Self::new(path.clone());

        // Ripgrep (usually available)
        let rg = RipgrepIndex::new(path.clone());
        if rg.is_available().await {
            router.lanes.insert("ripgrep".to_string(), Arc::new(rg));
        }

        // LSP/ctags
        let lsp = LspIndex::new(path.clone());
        if lsp.is_available().await {
            router.lanes.insert("lsp".to_string(), Arc::new(lsp));
        }

        // Git
        let git = GitIndex::new(path.clone());
        if git.is_available().await {
            router.lanes.insert("git".to_string(), Arc::new(git));
        }

        // Semantic (check for Ollama)
        let semantic = SemanticIndex::new(path.clone()).with_ollama("nomic-embed-text");
        if semantic.is_available().await {
            router
                .lanes
                .insert("semantic".to_string(), Arc::new(semantic));
        }

        // Set default to first available
        if router.lanes.contains_key("ripgrep") {
            router.default_lane = "ripgrep".to_string();
        } else if let Some(first) = router.lanes.keys().next() {
            router.default_lane = first.clone();
        }

        Ok(router)
    }

    /// Query a specific lane.
    pub async fn query_lane(
        &self,
        lane: &str,
        query: &str,
        config: &RetrievalConfig,
    ) -> Result<(Vec<RetrievalResult>, RetrievalStats)> {
        let index = self
            .lanes
            .get(lane)
            .ok_or_else(|| anyhow::anyhow!("Lane '{}' not found", lane))?;

        let start = Instant::now();
        let mut results = index.query(query, config).await?;

        // Ensure lane is set on all results
        for result in &mut results {
            if result.lane.is_empty() {
                result.lane = lane.to_string();
            }
        }

        let stats = RetrievalStats {
            result_count: results.len(),
            duration_ms: start.elapsed().as_millis() as u64,
            lane: lane.to_string(),
            files_searched: None,
        };

        Ok((results, stats))
    }

    /// Query the default lane.
    pub async fn query(
        &self,
        query: &str,
        config: &RetrievalConfig,
    ) -> Result<(Vec<RetrievalResult>, RetrievalStats)> {
        self.query_lane(&self.default_lane, query, config).await
    }

    /// Query all available lanes in parallel.
    pub async fn query_all(
        &self,
        query: &str,
        config: &RetrievalConfig,
    ) -> Result<HashMap<String, (Vec<RetrievalResult>, RetrievalStats)>> {
        use futures::future::join_all;

        let futures: Vec<_> = self
            .lanes
            .keys()
            .map(|lane| {
                let lane = lane.clone();
                let query = query.to_string();
                let config = config.clone();
                async move {
                    let result = self.query_lane(&lane, &query, &config).await;
                    (lane, result)
                }
            })
            .collect();

        let results = join_all(futures).await;

        let mut map = HashMap::new();
        for (lane, result) in results {
            if let Ok(r) = result {
                map.insert(lane, r);
            }
        }

        Ok(map)
    }

    /// Query multiple lanes and merge results.
    pub async fn query_lanes(
        &self,
        lanes: &[&str],
        query: &str,
        config: &RetrievalConfig,
    ) -> Result<Vec<RetrievalResult>> {
        use futures::future::join_all;

        let futures: Vec<_> = lanes
            .iter()
            .filter(|lane| self.lanes.contains_key(**lane))
            .map(|lane| {
                let lane = (*lane).to_string();
                let query = query.to_string();
                let config = config.clone();
                async move { self.query_lane(&lane, &query, &config).await }
            })
            .collect();

        let results = join_all(futures).await;

        // Merge and deduplicate results
        let mut all_results: Vec<RetrievalResult> = results
            .into_iter()
            .filter_map(|r| r.ok())
            .flat_map(|(results, _)| results)
            .collect();

        // Sort by score
        all_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Deduplicate by path+line
        let mut seen = std::collections::HashSet::new();
        all_results.retain(|r| seen.insert(format!("{}:{}", r.path, r.start_line)));

        // Limit to k
        all_results.truncate(config.k);

        Ok(all_results)
    }

    /// Get available lane names.
    pub fn available_lanes(&self) -> Vec<String> {
        self.lanes.keys().cloned().collect()
    }

    /// Check if a lane is available.
    pub fn has_lane(&self, lane: &str) -> bool {
        self.lanes.contains_key(lane)
    }

    /// Get the default lane name.
    pub fn default_lane(&self) -> &str {
        &self.default_lane
    }

    /// Build indexes for all lanes that support it.
    pub async fn build_indexes(&self) -> Result<()> {
        for (name, index) in &self.lanes {
            if let Err(e) = index.build_index(&self.repo_path).await {
                eprintln!("Warning: Failed to build index for lane '{}': {}", name, e);
            }
        }
        Ok(())
    }
}

/// Builder for configuring lane router.
pub struct LaneRouterBuilder {
    repo_path: PathBuf,
    lanes: Vec<(String, Arc<dyn RepoIndex>)>,
    default_lane: Option<String>,
}

impl LaneRouterBuilder {
    /// Create a new builder.
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
            lanes: Vec::new(),
            default_lane: None,
        }
    }

    /// Add ripgrep lane.
    pub fn with_ripgrep(mut self) -> Self {
        use super::RipgrepIndex;
        let index = RipgrepIndex::new(self.repo_path.clone());
        self.lanes.push(("ripgrep".to_string(), Arc::new(index)));
        self
    }

    /// Add LSP lane.
    pub fn with_lsp(mut self) -> Self {
        use super::LspIndex;
        let index = LspIndex::new(self.repo_path.clone());
        self.lanes.push(("lsp".to_string(), Arc::new(index)));
        self
    }

    /// Add git lane.
    pub fn with_git(mut self) -> Self {
        use super::GitIndex;
        let index = GitIndex::new(self.repo_path.clone());
        self.lanes.push(("git".to_string(), Arc::new(index)));
        self
    }

    /// Add semantic lane with Ollama.
    pub fn with_semantic_ollama(mut self, model: impl Into<String>) -> Self {
        use super::SemanticIndex;
        let index = SemanticIndex::new(self.repo_path.clone()).with_ollama(model);
        self.lanes.push(("semantic".to_string(), Arc::new(index)));
        self
    }

    /// Set default lane.
    pub fn default_lane(mut self, lane: impl Into<String>) -> Self {
        self.default_lane = Some(lane.into());
        self
    }

    /// Build the router.
    pub fn build(self) -> LaneRouter {
        let mut router = LaneRouter::new(self.repo_path);

        for (name, index) in self.lanes {
            router.lanes.insert(name, index);
        }

        if let Some(default) = self.default_lane {
            router.default_lane = default;
        } else if router.lanes.contains_key("ripgrep") {
            router.default_lane = "ripgrep".to_string();
        } else if let Some(first) = router.lanes.keys().next() {
            router.default_lane = first.clone();
        }

        router
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_router_builder() {
        let router = LaneRouterBuilder::new(env::current_dir().unwrap())
            .with_ripgrep()
            .with_git()
            .default_lane("ripgrep")
            .build();

        assert!(router.has_lane("ripgrep"));
        assert!(router.has_lane("git"));
        assert_eq!(router.default_lane(), "ripgrep");
    }

    #[test]
    fn test_available_lanes() {
        let router = LaneRouter::new(".");
        let lanes = router.available_lanes();
        assert!(lanes.is_empty()); // No lanes added yet
    }
}
