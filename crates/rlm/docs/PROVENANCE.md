# Provenance Tracking

SpanRef provides Git-aware, content-addressed references for precise evidence tracking in the RLM system. Every piece of evidence, extraction, or citation can be traced back to its exact source location.

## Overview

Provenance tracking solves several critical problems:

1. **Citation Accuracy** - Know exactly where information came from
2. **Reproducibility** - Pin references to specific Git commits
3. **Verification** - Detect if source content has changed
4. **Evidence Chains** - Track how findings flow through the pipeline

## SpanRef Type

```rust
use rlm::SpanRef;

/// A reference to a specific span of content within a repository.
pub struct SpanRef {
    /// Unique identifier within an execution context
    pub id: String,

    /// File path relative to repository root
    pub path: String,

    /// Git commit SHA for reproducibility (optional but recommended)
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
    pub content_hash: Option<String>,
}
```

## Creating SpanRefs

### From Chunk Metadata

The most common way to create SpanRefs during document analysis:

```rust
use rlm::SpanRef;

let span = SpanRef::from_chunk(
    chunk_id,           // usize: chunk identifier
    "docs/spec.md",     // path in repo
    Some("abc123def"),  // Git commit SHA
    10,                 // start line
    25,                 // end line
    500,                // start byte
    1200,               // end byte
    &chunk_content,     // content for hash computation
);

// The content hash is computed automatically
assert!(span.content_hash.is_some());
```

### With Range

For manual creation with position information:

```rust
let span = SpanRef::with_range(
    "chunk-0",        // id
    "src/lib.rs",     // path
    10,               // start line
    20,               // end line
    100,              // start byte
    500,              // end byte
);

// Add Git commit for reproducibility
let span = span.with_commit("abc123def456789");

// Add content hash for verification
let span = span.with_content(&content);
```

### Using the Builder

For incremental construction:

```rust
use rlm::SpanRefBuilder;

let span = SpanRefBuilder::new()
    .id("evidence-1")
    .path("src/main.rs")
    .commit("abc123")
    .lines(10, 20)
    .bytes(100, 500)
    .content("fn main() {}")
    .build()?;
```

### Minimal Creation

For simple references:

```rust
let span = SpanRef::new("test-1", "src/main.rs");
// Defaults: line 1-1, bytes 0-0, no commit, no hash
```

## Content Verification

SpanRefs include a SHA256 hash of the content for verification:

```rust
// Check if content matches what was referenced
if span.verify_content(&current_content) {
    println!("Content unchanged");
} else {
    println!("WARNING: Content has changed since analysis");
}

// Manual hash computation
let hash = SpanRef::compute_hash("fn main() {}");
// Returns 64-character hex string
```

## Serialization

SpanRefs serialize to JSON for use in DSPy signatures:

```rust
// Single SpanRef
let json = span.to_json();
// {"id":"chunk-0","path":"lib.rs","start_line":10,...}

let parsed = SpanRef::from_json(&json)?;

// Array of SpanRefs
let spans = vec![span1, span2, span3];
let json_array = SpanRef::to_json_array(&spans);

let parsed_array = SpanRef::parse_array(&json_array)?;
```

## Span Operations

### Overlap Detection

Check if two spans reference overlapping regions:

```rust
let span1 = SpanRef::with_range("s1", "file.rs", 10, 20, 0, 0);
let span2 = SpanRef::with_range("s2", "file.rs", 15, 25, 0, 0);
let span3 = SpanRef::with_range("s3", "other.rs", 10, 20, 0, 0);

assert!(span1.overlaps(&span2));  // Lines 15-20 overlap
assert!(!span1.overlaps(&span3)); // Different files
```

### Containment

Check if one span contains another:

```rust
let outer = SpanRef::with_range("outer", "file.rs", 10, 30, 0, 0);
let inner = SpanRef::with_range("inner", "file.rs", 15, 25, 0, 0);

assert!(outer.contains(&inner));  // inner is within outer
assert!(!inner.contains(&outer)); // outer extends beyond inner
```

### Sub-Spans

Create a reference to a region within a span:

```rust
let chunk_span = SpanRef::with_range("chunk", "file.rs", 10, 50, 0, 0);

// Reference lines 5-10 relative to the chunk start (absolute: 15-20)
let evidence_span = chunk_span.sub_span("evidence-1", 5, 10);

assert_eq!(evidence_span.start_line, 15);
assert_eq!(evidence_span.end_line, 20);
```

### Metrics

```rust
let span = SpanRef::with_range("s", "f.rs", 10, 20, 100, 500);

println!("Lines: {}", span.line_count());  // 11
println!("Bytes: {}", span.byte_len());    // 400
```

## Display Format

SpanRefs have a human-readable display format:

```rust
let span = SpanRef::with_range("chunk-0", "src/lib.rs", 10, 20, 0, 0)
    .with_commit("abc123def456789");

println!("{}", span);
// Output: src/lib.rs:10-20@abc123d@chunk-0
```

## ID Generation

Generate consistent IDs based on path and position:

```rust
let id = SpanRef::generate_id("src/lib.rs", 10, 20);
// Output: "span-src-lib_rs-10-20"
```

## Integration with DSPy

SpanRefs are designed for use in DSPy signatures as JSON strings:

### In Signatures

```rust
#[Signature(cot)]
struct ExtractorSignature {
    #[input] pub query: String,
    #[input] pub chunk: String,
    #[input] pub span_ref: String,     // JSON-encoded SpanRef

    #[output] pub findings: String,
    #[output] pub evidence_spans: String,  // JSON array of SpanRefs
    #[output] pub relevance: f32,
}
```

### Creating Input

```rust
let span = SpanRef::from_chunk(id, path, commit, start, end, byte_start, byte_end, content);
let example = example! {
    "query": "input" => query,
    "chunk": "input" => chunk_content,
    "span_ref": "input" => span.to_json()
};
```

### Parsing Output

```rust
let result = extractor.forward(example).await?;
let evidence_json = result.get("evidence_spans", None).as_str().unwrap_or("[]");
let evidence_spans = SpanRef::parse_array(evidence_json)?;

for span in evidence_spans {
    println!("Evidence at {}:{}-{}", span.path, span.start_line, span.end_line);
}
```

## DspyOrchestrator Integration

The orchestrator automatically generates SpanRefs when configured:

```rust
let orchestrator = DspyOrchestrator::new()
    .with_document_path("docs/report.md")
    .with_commit("abc123def");

let result = orchestrator.analyze(query, &document).await?;

// Each extraction includes its SpanRef
for extraction in &result.extractions {
    if let Some(span) = &extraction.span_ref {
        println!("Section '{}' from {}:{}-{}",
            extraction.section,
            span.path,
            span.start_line,
            span.end_line
        );

        // Verify content is unchanged
        if let Some(hash) = &span.content_hash {
            println!("  Content hash: {}...", &hash[..16]);
        }
    }
}
```

## Best Practices

### 1. Always Include Git Commit

Pin references to specific commits for reproducibility:

```rust
let commit = get_current_git_commit()?;  // Helper function
let span = SpanRef::from_chunk(...).with_commit(&commit);
```

### 2. Compute Content Hashes

Enable verification of unchanged content:

```rust
let span = SpanRef::with_range(id, path, start, end, byte_start, byte_end)
    .with_content(&actual_content);
```

### 3. Use Meaningful IDs

Generate IDs that aid debugging:

```rust
// Good: descriptive, includes context
let id = format!("extract-{}-{}", section_name, chunk_id);

// Avoid: generic, hard to trace
let id = format!("span-{}", uuid::Uuid::new_v4());
```

### 4. Validate on Load

Check content hasn't changed when loading references:

```rust
fn load_evidence(span: &SpanRef, repo_path: &Path) -> Result<String> {
    let content = read_file_range(repo_path, span)?;

    if !span.verify_content(&content) {
        return Err(anyhow!("Content changed since analysis"));
    }

    Ok(content)
}
```

### 5. Preserve Byte Offsets

Include byte offsets for efficient seeking:

```rust
let span = SpanRef::with_range(
    id,
    path,
    start_line,
    end_line,
    byte_start,  // Don't omit these
    byte_end,
);
```

## Helper Types

### CandidateSpan

For router outputs (partial SpanRef without full details):

```rust
use rlm::CandidateSpan;

#[derive(Serialize, Deserialize)]
pub struct CandidateSpan {
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub why: String,  // Reason for selection
}

// Parse from JSON
let candidates = CandidateSpan::parse_array(&router_output)?;
```

### MissingSpanRequest

For verifier outputs (what evidence is needed):

```rust
use rlm::MissingSpanRequest;

#[derive(Serialize, Deserialize)]
pub struct MissingSpanRequest {
    pub description: String,
    pub suggested_path: Option<String>,
    pub claim: String,  // The claim needing support
}

let missing = MissingSpanRequest::parse_array(&verifier_output)?;
for req in missing {
    println!("Need evidence for '{}': {}", req.claim, req.description);
}
```

## Related Documentation

- [DSPy Integration](./DSPY.md) - Using SpanRefs with DSPy signatures
- [Environment Tools](./TOOLS.md) - Tools that return SpanRefs
- [RLM Architecture](./ARCHITECTURE.md) - System overview
