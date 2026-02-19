# Retrieval System (Wave 4)

- **Status:** Needs audit
- **Last verified:** d44f9cd3f
- **Source of truth:** `crates/dsrs/src/retrieval/`
- **Doc owner:** dsrs
- **If this doc conflicts with code, code wins.**

Multi-lane retrieval system for intelligent code exploration.

## Overview

The retrieval system provides pluggable backends for different search strategies, allowing agents to efficiently explore codebases using the most appropriate method for each query.

## Retrieval Backends

### Available Lanes

| Lane | Backend | Best For | Speed | Semantic |
|------|---------|----------|-------|----------|
| `ripgrep` | rg | Text/regex, identifiers | Fast | No |
| `lsp` | ctags/rg | Symbol definitions | Medium | Partial |
| `semantic` | Ollama/OpenAI | Conceptual queries | Slow | Yes |
| `git` | git log/blame | Recent changes, authors | Medium | No |

### RipgrepIndex

Fast text search using ripgrep.

```rust
use dsrs::retrieval::{RipgrepIndex, RetrievalConfig};

let index = RipgrepIndex::new("/path/to/repo")
    .with_case_insensitive(true)
    .with_file_types(vec!["rs".into(), "ts".into()]);

let config = RetrievalConfig::new().with_k(10);
let results = index.query("fn main", &config).await?;
```

### LspIndex

Symbol-aware search using ctags or pattern matching.

```rust
use dsrs::retrieval::LspIndex;

let index = LspIndex::new("/path/to/repo");
let results = index.query("MyStruct", &config).await?;

// Results include symbol kind (function, struct, trait, etc.)
for result in results {
    if let Some(kind) = result.metadata.get("kind") {
        println!("{}: {} at {}:{}", kind, result.content, result.path, result.start_line);
    }
}
```

### SemanticIndex

Vector similarity search using embeddings.

```rust
use dsrs::retrieval::SemanticIndex;

// With Ollama
let index = SemanticIndex::new("/path/to/repo")
    .with_ollama("nomic-embed-text");

// With OpenAI
let index = SemanticIndex::new("/path/to/repo")
    .with_openai(std::env::var("OPENAI_API_KEY")?);

let results = index.query("error handling patterns", &config).await?;
```

### GitIndex

Git-aware search for blame, history, and diffs.

```rust
use dsrs::retrieval::GitIndex;

let index = GitIndex::new("/path/to/repo")
    .with_commit_limit(100);

let results = index.query("authentication", &config).await?;

// Results include git metadata
for result in results {
    println!("Author: {}", result.metadata.get("author").unwrap_or(&"unknown".into()));
    println!("Commit: {}", result.metadata.get("commit").unwrap_or(&"".into()));
}
```

## LaneRouter

Combines multiple backends for intelligent query routing.

### Auto-Detection

```rust
use dsrs::retrieval::LaneRouter;

// Auto-detect available backends
let router = LaneRouter::auto_detect("/path/to/repo").await?;

println!("Available lanes: {:?}", router.available_lanes());
```

### Manual Configuration

```rust
use dsrs::retrieval::LaneRouterBuilder;

let router = LaneRouterBuilder::new("/path/to/repo")
    .with_ripgrep()
    .with_lsp()
    .with_git()
    .with_semantic_ollama("nomic-embed-text")
    .default_lane("ripgrep")
    .build();
```

### Querying

```rust
// Query default lane
let (results, stats) = router.query("fn main", &config).await?;

// Query specific lane
let (results, stats) = router.query_lane("semantic", "authentication flow", &config).await?;

// Query multiple lanes
let results = router.query_lanes(&["ripgrep", "lsp"], "MyStruct", &config).await?;

// Query all lanes in parallel
let all_results = router.query_all("error handling", &config).await?;
```

## RetrievalResult

All backends return `RetrievalResult`:

```rust
pub struct RetrievalResult {
    pub path: String,        // File path
    pub start_line: usize,   // Starting line (1-indexed)
    pub end_line: usize,     // Ending line (inclusive)
    pub content: String,     // Retrieved content
    pub score: f32,          // Relevance score (0.0-1.0)
    pub lane: String,        // Which lane produced this
    pub metadata: HashMap<String, String>,  // Lane-specific metadata
}
```

## RetrievalConfig

Configure query behavior:

```rust
let config = RetrievalConfig::new()
    .with_k(20)              // Max results
    .with_min_score(0.5)     // Minimum relevance score
    .with_context(5);        // Context lines around matches
```

## Integration with Signatures

Use with `RetrievalRouterSignature` for intelligent lane selection:

```rust
use dsrs::signatures::RetrievalRouterSignature;
use dsrs::predictors::Predict;

let router_sig = RetrievalRouterSignature::new();
let predictor = Predict::new(router_sig);

// The signature decides which lane to use
let result = predictor.forward(example! {
    "query" : "input" => "authentication middleware",
    "available_lanes" : "input" => vec!["ripgrep", "lsp", "semantic"],
    "budget_remaining" : "input" => 10000u64
}).await?;

let lane = result.get("lane", None).as_str().unwrap();
let k = result.get("k", None).as_u64().unwrap() as usize;
```

## Best Practices

1. **Start with ripgrep** for exact strings and identifiers
2. **Use LSP** when looking for symbol definitions
3. **Use semantic** for conceptual queries only when needed (costs more)
4. **Use git** to understand code history and ownership
5. **Query multiple lanes** when unsure which is best
6. **Set appropriate K values** - start small, increase if needed
