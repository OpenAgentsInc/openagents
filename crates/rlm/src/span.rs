//! Git-aware span references for provenance tracking.
//!
//! SpanRef provides a content-addressed reference to a specific region of text
//! within a git repository, enabling:
//! - Precise citation of evidence sources
//! - Reproducible references (pinned to commit SHA)
//! - Content verification via hash

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// A reference to a specific span of content within a repository.
///
/// SpanRef is the foundation for provenance tracking in the RLM system.
/// Every piece of evidence, extraction, or citation should be traceable
/// back to its source via SpanRef.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpanRef {
    /// Unique identifier within an execution context
    pub id: String,

    /// File path relative to repository root
    pub path: String,

    /// Git commit SHA for reproducibility (optional, but recommended)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,

    /// Starting line number (1-indexed, inclusive)
    pub start_line: u32,

    /// Ending line number (1-indexed, inclusive)
    pub end_line: u32,

    /// Starting byte offset from file start
    pub start_byte: u64,

    /// Ending byte offset from file start
    pub end_byte: u64,

    /// SHA256 hash of the content for verification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
}

impl SpanRef {
    /// Create a new SpanRef with minimal required fields.
    pub fn new(id: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            commit: None,
            start_line: 1,
            end_line: 1,
            start_byte: 0,
            end_byte: 0,
            content_hash: None,
        }
    }

    /// Create a SpanRef with full position information.
    pub fn with_range(
        id: impl Into<String>,
        path: impl Into<String>,
        start_line: u32,
        end_line: u32,
        start_byte: u64,
        end_byte: u64,
    ) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            commit: None,
            start_line,
            end_line,
            start_byte,
            end_byte,
            content_hash: None,
        }
    }

    /// Create a SpanRef from chunk metadata.
    #[expect(clippy::too_many_arguments)]
    pub fn from_chunk(
        chunk_id: usize,
        path: impl Into<String>,
        commit: Option<&str>,
        start_line: u32,
        end_line: u32,
        start_byte: u64,
        end_byte: u64,
        content: &str,
    ) -> Self {
        let content_hash = Self::compute_hash(content);
        Self {
            id: format!("chunk-{}", chunk_id),
            path: path.into(),
            commit: commit.map(String::from),
            start_line,
            end_line,
            start_byte,
            end_byte,
            content_hash: Some(content_hash),
        }
    }

    /// Set the git commit SHA for this span.
    pub fn with_commit(mut self, commit: impl Into<String>) -> Self {
        self.commit = Some(commit.into());
        self
    }

    /// Set the content hash for this span.
    pub fn with_content_hash(mut self, hash: impl Into<String>) -> Self {
        self.content_hash = Some(hash.into());
        self
    }

    /// Compute content hash from the actual content.
    pub fn with_content(mut self, content: &str) -> Self {
        self.content_hash = Some(Self::compute_hash(content));
        self
    }

    /// Compute SHA256 hash of content.
    pub fn compute_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let result = hasher.finalize();
        hex::encode(result)
    }

    /// Verify that content matches the stored hash.
    pub fn verify_content(&self, content: &str) -> bool {
        match &self.content_hash {
            Some(expected) => Self::compute_hash(content) == *expected,
            None => true, // No hash means no verification possible
        }
    }

    /// Serialize to JSON string for use in DSPy signatures.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Deserialize from JSON string.
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    /// Parse a JSON array of SpanRefs.
    pub fn parse_array(s: &str) -> Result<Vec<Self>, serde_json::Error> {
        serde_json::from_str(s)
    }

    /// Serialize a slice of SpanRefs to JSON array.
    pub fn to_json_array(spans: &[SpanRef]) -> String {
        serde_json::to_string(spans).unwrap_or_else(|_| "[]".to_string())
    }

    /// Get the number of lines in this span.
    pub fn line_count(&self) -> u32 {
        self.end_line.saturating_sub(self.start_line) + 1
    }

    /// Get the byte length of this span.
    pub fn byte_len(&self) -> u64 {
        self.end_byte.saturating_sub(self.start_byte)
    }

    /// Check if this span overlaps with another.
    pub fn overlaps(&self, other: &SpanRef) -> bool {
        if self.path != other.path {
            return false;
        }
        // Check if line ranges overlap
        !(self.end_line < other.start_line || self.start_line > other.end_line)
    }

    /// Check if this span contains another.
    pub fn contains(&self, other: &SpanRef) -> bool {
        if self.path != other.path {
            return false;
        }
        self.start_line <= other.start_line && self.end_line >= other.end_line
    }

    /// Create a sub-span within this span.
    pub fn sub_span(
        &self,
        id: impl Into<String>,
        relative_start_line: u32,
        relative_end_line: u32,
    ) -> Self {
        Self {
            id: id.into(),
            path: self.path.clone(),
            commit: self.commit.clone(),
            start_line: self.start_line + relative_start_line,
            end_line: self.start_line + relative_end_line,
            // Byte offsets would need content to compute accurately
            start_byte: self.start_byte,
            end_byte: self.end_byte,
            content_hash: None,
        }
    }

    /// Generate a unique ID based on path and position.
    pub fn generate_id(path: &str, start_line: u32, end_line: u32) -> String {
        format!(
            "span-{}-{}-{}",
            path.replace('/', "-").replace('.', "_"),
            start_line,
            end_line
        )
    }
}

impl fmt::Display for SpanRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(commit) = &self.commit {
            write!(
                f,
                "{}:{}:{}-{}@{}",
                self.path,
                self.start_line,
                self.end_line,
                &commit[..7.min(commit.len())],
                self.id
            )
        } else {
            write!(
                f,
                "{}:{}-{}@{}",
                self.path, self.start_line, self.end_line, self.id
            )
        }
    }
}

/// Builder for creating SpanRefs incrementally.
#[derive(Default)]
pub struct SpanRefBuilder {
    id: Option<String>,
    path: Option<String>,
    commit: Option<String>,
    start_line: u32,
    end_line: u32,
    start_byte: u64,
    end_byte: u64,
    content_hash: Option<String>,
}

impl SpanRefBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn commit(mut self, commit: impl Into<String>) -> Self {
        self.commit = Some(commit.into());
        self
    }

    pub fn lines(mut self, start: u32, end: u32) -> Self {
        self.start_line = start;
        self.end_line = end;
        self
    }

    pub fn bytes(mut self, start: u64, end: u64) -> Self {
        self.start_byte = start;
        self.end_byte = end;
        self
    }

    pub fn content(mut self, content: &str) -> Self {
        self.content_hash = Some(SpanRef::compute_hash(content));
        self
    }

    pub fn build(self) -> Result<SpanRef, &'static str> {
        let id = self.id.ok_or("id is required")?;
        let path = self.path.ok_or("path is required")?;

        Ok(SpanRef {
            id,
            path,
            commit: self.commit,
            start_line: self.start_line,
            end_line: self.end_line,
            start_byte: self.start_byte,
            end_byte: self.end_byte,
            content_hash: self.content_hash,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_ref_creation() {
        let span = SpanRef::new("test-1", "src/main.rs");
        assert_eq!(span.id, "test-1");
        assert_eq!(span.path, "src/main.rs");
        assert_eq!(span.start_line, 1);
    }

    #[test]
    fn test_span_ref_with_range() {
        let span = SpanRef::with_range("chunk-0", "lib.rs", 10, 20, 100, 500);
        assert_eq!(span.start_line, 10);
        assert_eq!(span.end_line, 20);
        assert_eq!(span.line_count(), 11);
        assert_eq!(span.byte_len(), 400);
    }

    #[test]
    fn test_content_hash() {
        let content = "fn main() {}";
        let hash = SpanRef::compute_hash(content);
        assert_eq!(hash.len(), 64); // SHA256 hex is 64 chars

        let span = SpanRef::new("test", "main.rs").with_content(content);
        assert!(span.verify_content(content));
        assert!(!span.verify_content("fn other() {}"));
    }

    #[test]
    fn test_json_serialization() {
        let span =
            SpanRef::with_range("chunk-0", "src/lib.rs", 1, 10, 0, 100).with_commit("abc123def456");

        let json = span.to_json();
        let parsed = SpanRef::from_json(&json).unwrap();

        assert_eq!(span, parsed);
    }

    #[test]
    fn test_json_array() {
        let spans = vec![SpanRef::new("s1", "a.rs"), SpanRef::new("s2", "b.rs")];

        let json = SpanRef::to_json_array(&spans);
        let parsed = SpanRef::parse_array(&json).unwrap();

        assert_eq!(spans, parsed);
    }

    #[test]
    fn test_overlaps() {
        let span1 = SpanRef::with_range("s1", "file.rs", 10, 20, 0, 0);
        let span2 = SpanRef::with_range("s2", "file.rs", 15, 25, 0, 0);
        let span3 = SpanRef::with_range("s3", "file.rs", 25, 30, 0, 0);
        let span4 = SpanRef::with_range("s4", "other.rs", 10, 20, 0, 0);

        assert!(span1.overlaps(&span2));
        assert!(!span1.overlaps(&span3));
        assert!(!span1.overlaps(&span4)); // Different file
    }

    #[test]
    fn test_contains() {
        let outer = SpanRef::with_range("outer", "file.rs", 10, 30, 0, 0);
        let inner = SpanRef::with_range("inner", "file.rs", 15, 25, 0, 0);
        let partial = SpanRef::with_range("partial", "file.rs", 25, 35, 0, 0);

        assert!(outer.contains(&inner));
        assert!(!outer.contains(&partial));
        assert!(!inner.contains(&outer));
    }

    #[test]
    fn test_builder() {
        let span = SpanRefBuilder::new()
            .id("test-span")
            .path("src/main.rs")
            .commit("abc123")
            .lines(10, 20)
            .bytes(100, 500)
            .content("fn test() {}")
            .build()
            .unwrap();

        assert_eq!(span.id, "test-span");
        assert_eq!(span.path, "src/main.rs");
        assert_eq!(span.commit, Some("abc123".to_string()));
        assert!(span.content_hash.is_some());
    }

    #[test]
    fn test_display() {
        let span = SpanRef::with_range("chunk-0", "src/lib.rs", 10, 20, 0, 0)
            .with_commit("abc123def456789");

        let display = format!("{}", span);
        assert!(display.contains("src/lib.rs"));
        assert!(display.contains("10"));
        assert!(display.contains("20"));
        assert!(display.contains("abc123d")); // Truncated commit
    }

    /// SpanRef is a pointer/reference, not embedded content.
    ///
    /// Per Omar: "prompts/requests accessible through pointers as an object"
    ///
    /// This test validates that SpanRef contains metadata (the pointer) rather
    /// than the actual content, enabling symbolic access to large contexts.
    #[test]
    fn test_spanref_is_pointer_not_content() {
        // Create a span that references 5000 bytes of content
        let span = SpanRef::with_range("span-1", "/path/to/file.rs", 1, 100, 0, 5000)
            .with_commit("abc123def456789");

        // The SpanRef struct contains METADATA about where to find content
        // It does NOT contain the 5000 bytes of content itself
        // This is the "pointer as object" pattern Omar describes

        // Verify the struct is small (just metadata)
        let serialized = span.to_json();
        assert!(
            serialized.len() < 300,
            "SpanRef JSON should be small (metadata only), got {} bytes",
            serialized.len()
        );

        // The span knows WHERE the content is (path, lines, bytes)
        // but doesn't store WHAT the content is
        assert_eq!(span.path, "/path/to/file.rs");
        assert_eq!(span.start_line, 1);
        assert_eq!(span.end_line, 100);
        assert_eq!(span.start_byte, 0);
        assert_eq!(span.end_byte, 5000);
        assert_eq!(span.byte_len(), 5000); // Knows the SIZE

        // The optional content_hash allows VERIFICATION without storing content
        // This is key for provenance: we can verify later without embedding now
    }

    /// SpanRef enables git-aware symbolic references.
    ///
    /// This allows reproducible access to content at specific commits,
    /// a key requirement for symbolic recursion over large repositories.
    #[test]
    fn test_spanref_enables_symbolic_repository_access() {
        // Create spans referencing different parts of a repository
        let spans = vec![
            SpanRef::with_range("evidence-1", "src/auth/login.rs", 50, 75, 1000, 2500)
                .with_commit("abc123"),
            SpanRef::with_range("evidence-2", "src/auth/session.rs", 100, 150, 3000, 5000)
                .with_commit("abc123"),
            SpanRef::with_range("evidence-3", "tests/auth_test.rs", 10, 30, 200, 800)
                .with_commit("abc123"),
        ];

        // Each span is a lightweight reference (pointer)
        // All together they reference 4100 bytes of content
        // (1500 + 2000 + 600 = 4100)
        // But the spans themselves are very small
        let total_referenced_bytes: u64 = spans.iter().map(|s| s.byte_len()).sum();
        assert_eq!(total_referenced_bytes, 4100);

        // The JSON representation of all spans is tiny compared to referenced content
        let json = SpanRef::to_json_array(&spans);
        assert!(
            json.len() < 1000,
            "SpanRef array should be much smaller than referenced content"
        );

        // This enables O(N) citations without O(N) content duplication
        // The LLM can produce spans; the system resolves them to content
    }

    /// SpanRefs can reference large contexts via symbolic lookup.
    ///
    /// This test demonstrates the pattern for handling 10M+ token contexts:
    /// use pointers (SpanRefs) to reference content, don't embed it.
    #[test]
    fn test_large_context_via_symbolic_references() {
        // Simulate referencing a 10MB file via SpanRefs
        // In practice, these would reference chunks of a large document
        let spans: Vec<SpanRef> = (0..1000)
            .map(|i| {
                SpanRef::with_range(
                    format!("chunk-{}", i),
                    "large_document.md",
                    i * 100 + 1,
                    (i + 1) * 100,
                    i as u64 * 10000,
                    (i + 1) as u64 * 10000,
                )
            })
            .collect();

        // 1000 spans, each referencing 10KB = 10MB total
        let total_bytes: u64 = spans.iter().map(|s| s.byte_len()).sum();
        assert_eq!(total_bytes, 10_000_000);

        // But the spans themselves are tiny
        let json = SpanRef::to_json_array(&spans);
        assert!(
            json.len() < 200_000,
            "1000 SpanRefs should be << 10MB, got {} bytes",
            json.len()
        );

        // This is how RLM handles large contexts:
        // - Content is external (filesystem, git, database)
        // - SpanRefs are lightweight pointers
        // - The system resolves pointers to content on-demand
        // - O(N) references don't require O(N) prompt tokens
    }
}
