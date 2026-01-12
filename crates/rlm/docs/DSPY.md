# DSPy Integration

The RLM crate integrates [dsrs](../../dsrs/docs/README.md) to provide declarative LLM programming
with typed signatures, composable modules, and automatic prompt optimization.

## Overview

DSPy and RLMs complement each other:

- **RLM**: Handles arbitrarily long inputs via recursive sub-calls and external memory
- **DSPy**: Provides structured, optimizable LLM programming with typed signatures

Together, they enable long-context agents that are both *scalable* (via RLM) and *tunable with evals* (via DSPy).

## Enabling the Feature

The DSPy integration is an optional feature. Enable it in your `Cargo.toml`:

```toml
[dependencies]
rlm = { path = "../rlm", features = ["dspy"] }
```

Or via command line:

```bash
cargo build -p rlm --features dspy
cargo test -p rlm --features dspy --lib
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         DspyOrchestrator                                │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  ┌─────────┐   ┌───────────┐   ┌─────────┐   ┌──────────┐       │   │
│  │  │ Router  │ → │ Extractor │ → │ Reducer │ → │ Verifier │       │   │
│  │  │(Predict)│   │ (Predict) │   │(Predict)│   │ (Predict)│       │   │
│  │  └────┬────┘   └─────┬─────┘   └────┬────┘   └────┬─────┘       │   │
│  │       │              │              │              │             │   │
│  │       └──────────────┴──────────────┴──────────────┘             │   │
│  │                         DSPy Modules                              │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│                                  │                                       │
│  ┌───────────────────────────────┴────────────────────────────────┐     │
│  │                      RLM Environment                            │     │
│  │  ┌──────────┐  ┌────────────┐  ┌───────────┐  ┌─────────────┐  │     │
│  │  │ GrepTool │  │ReadLinesTool│  │ListFilesTool│  │SymbolsTool │  │     │
│  │  └─────┬────┘  └──────┬─────┘  └──────┬────┘  └──────┬──────┘  │     │
│  │        └──────────────┼───────────────┼──────────────┘         │     │
│  │                       │               │                         │     │
│  │              SpanRef (Provenance Tracking)                      │     │
│  └─────────────────────────────────────────────────────────────────┘     │
├─────────────────────────────────────────────────────────────────────────┤
│                         dspy_bridge                                      │
│   ┌──────────────────────┐    ┌─────────────────────────────────┐       │
│   │  LM Configuration    │    │   LmRouterDspyBridge            │       │
│   │  (global or per-req) │    │   (unified cost tracking)       │       │
│   └──────────────────────┘    └─────────────────────────────────┘       │
├─────────────────────────────────────────────────────────────────────────┤
│                             dsrs                                        │
│        (Signatures, Predict, Module, COPRO, MIPROv2)                    │
└─────────────────────────────────────────────────────────────────────────┘
```

## Quick Start

### Basic Usage (Global LM)

```rust
use rlm::{DspyOrchestrator, DspyOrchestratorConfig, configure_dspy_lm};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Configure the global LM
    configure_dspy_lm("openai:gpt-4o-mini", None, None).await?;

    // 2. Create orchestrator with default config
    let orchestrator = DspyOrchestrator::new();

    // 3. Analyze a document
    let document = std::fs::read_to_string("large_document.txt")?;
    let result = orchestrator.analyze(
        "What are the main findings?",
        &document
    ).await?;

    println!("Answer: {}", result.answer);
    println!("Confidence: {:.2}", result.confidence);
    println!("Chunks processed: {}", result.chunks_processed);

    Ok(())
}
```

### Production Usage (Per-Request LM with LmRouter)

```rust
use std::sync::Arc;
use rlm::{DspyOrchestrator, LmRouterDspyBridge};
use lm_router::LmRouter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Set up LmRouter for unified cost tracking
    let router = Arc::new(LmRouter::builder()
        .add_backend(my_backend)
        .default_backend("openai")
        .build());

    // 2. Create bridge for per-request LM
    let bridge = LmRouterDspyBridge::new(router.clone(), "gpt-4o-mini")
        .with_temperature(0.7)
        .with_max_tokens(4096);

    // Configure the LM globally (bridge creates compatible LM)
    bridge.configure_global().await?;

    // 3. Create orchestrator with provenance tracking
    let orchestrator = DspyOrchestrator::new()
        .with_document_path("docs/report.md")
        .with_commit("abc123def");

    // 4. Analyze document
    let result = orchestrator.analyze("Summarize the findings", &document).await?;

    // 5. Check usage from router
    let usage = bridge.usage_report();
    println!("Total cost: ${:.4}", usage.total_cost);

    Ok(())
}
```

## Core Components

### 1. DSPy Bridge (`dspy_bridge`)

Re-exports key dsrs types and provides LM configuration helpers.

#### LM Configuration

```rust
use rlm::{configure_dspy_lm, create_lm_for_openrouter, create_lm_for_local};

// Option A: Direct provider (reads API key from env)
configure_dspy_lm("openai:gpt-4o-mini", None, None).await?;

// Option B: OpenRouter
configure_dspy_lm(
    "openai/gpt-4o-mini",
    Some("your-openrouter-key"),
    Some("https://openrouter.ai/api/v1")
).await?;

// Option C: Create LM instance directly
let lm = create_lm_for_openrouter("openai/gpt-4o-mini", "key").await?;
let local_lm = create_lm_for_local("llama3", "http://localhost:11434").await?;
```

#### LmRouter Bridge (Production)

For production use with unified cost tracking:

```rust
use rlm::LmRouterDspyBridge;

// Create bridge with LmRouter
let bridge = LmRouterDspyBridge::new(router, "gpt-4o-mini");

// Configure options
let bridge = bridge
    .with_temperature(0.7)
    .with_max_tokens(4096);

// Option A: Configure globally
bridge.configure_global().await?;

// Option B: Direct completions (bypasses DSPy)
let response = bridge.complete("Hello", Some(100)).await?;

// Get usage statistics
let report = bridge.usage_report();
```

#### Re-exported Types

| Type | Description |
|------|-------------|
| `Signature` | Proc macro for defining typed LLM interfaces |
| `Predict` | Basic predictor module |
| `Module` | Trait for composable LLM operations |
| `Example` | Input data structure |
| `Prediction` | Output data structure |
| `LM` | Language model client |
| `COPRO` | Instruction optimization |
| `MIPROv2` | Advanced multi-stage optimization |
| `Evaluator` | For running optimization evaluations |
| `LmRouterDspyBridge` | Bridge for LmRouter integration |
| `LmRouterDspyConfig` | Configuration for the bridge |

### 2. Provenance Tracking (`span`)

SpanRef provides Git-aware references for evidence tracking. See [PROVENANCE.md](./PROVENANCE.md) for details.

```rust
use rlm::SpanRef;

// Create from chunk metadata
let span = SpanRef::from_chunk(
    chunk_id,
    "docs/report.md",
    Some("abc123def"),  // Git commit
    10, 25,             // Lines 10-25
    500, 1200,          // Byte range
    &chunk_content,
);

// Verify content hasn't changed
assert!(span.verify_content(&chunk_content));

// Serialize for DSPy signatures
let json = span.to_json();
```

### 3. DSPy Orchestrator (`dspy_orchestrator`)

Multi-phase document analysis using typed DSPy signatures with provenance tracking.

#### Pipeline Phases

1. **Router** - Identifies relevant document sections from a preview
2. **Extractor** - Extracts findings from each chunk (parallel, with optional CoT)
3. **Reducer** - Synthesizes findings into a final answer with citations
4. **Verifier** - Validates the answer against evidence (optional)

#### Configuration

```rust
use rlm::{DspyOrchestrator, DspyOrchestratorConfig};

let config = DspyOrchestratorConfig {
    chunk_size: 6000,           // Target chars per chunk
    overlap: 200,               // Overlap between chunks
    max_chunks: 50,             // Cost control
    semantic_chunking: true,    // Use structure-aware chunking
    use_cot_extraction: true,   // Chain-of-thought extraction
    verify_answer: false,       // Skip verification pass
    max_concurrency: 5,         // Parallel extractions
    verbose: true,              // Progress output
};

let orchestrator = DspyOrchestrator::with_config(config);
```

#### Provenance Configuration

```rust
// Enable provenance tracking
let orchestrator = DspyOrchestrator::new()
    .with_document_path("src/main.rs")    // Path for SpanRefs
    .with_commit("abc123def456");          // Git commit for pinning

let result = orchestrator.analyze(query, &document).await?;

// Extractions now include SpanRefs
for extraction in &result.extractions {
    if let Some(span) = &extraction.span_ref {
        println!("Evidence from {}:{}-{}",
            span.path, span.start_line, span.end_line);
    }
}
```

#### Result Structure

```rust
pub struct DspyAnalysisResult {
    pub answer: String,           // Final synthesized answer
    pub citations: String,        // Supporting citations (JSON SpanRefs)
    pub confidence: f32,          // Confidence score (0-1)
    pub chunks_processed: usize,  // Number of chunks analyzed
    pub extractions: Vec<ChunkExtraction>,  // Per-chunk results
    pub verification: Option<VerificationResult>,
    pub relevant_sections: String,
}

pub struct ChunkExtraction {
    pub chunk_id: usize,
    pub section: String,
    pub findings: String,
    pub evidence: String,
    pub relevance: f32,
    pub span_ref: Option<SpanRef>,  // Provenance tracking
}
```

### 4. RLM Environment Tools

Tools expose the repository environment to DSPy predictors. See [TOOLS.md](./TOOLS.md) for details.

```rust
use rlm::{GrepTool, ReadLinesTool, ListFilesTool, SymbolsTool};
use std::path::PathBuf;

let repo = PathBuf::from(".");

// Pattern search with SpanRefs
let grep = GrepTool::new(repo.clone());
let hits = grep.search("fn main", &["**/*.rs"], 20).await?;
for hit in hits {
    println!("{}: {}", hit.span.path, hit.line);
}

// Read specific lines
let reader = ReadLinesTool::new(repo.clone());
let result = reader.read("src/lib.rs", 10, 50).await?;
println!("{}", result.content);

// List files by language
let lister = ListFilesTool::new(repo.clone());
let rust_files = lister.list_by_language("rust").await?;

// Extract symbols
let symbols = SymbolsTool::new(repo);
let syms = symbols.extract("src/lib.rs").await?;
for sym in syms {
    println!("{}: {} at line {}", sym.kind, sym.name, sym.span.start_line);
}
```

## Typed Signatures

### Provenance-First Signatures

The orchestrator uses signatures that track evidence origins:

```rust
use dspy_rs::Signature;

// Router returns candidate spans as JSON
#[Signature]
struct RouterSignature {
    #[input] pub query: String,
    #[input] pub document_preview: String,
    #[output] pub candidate_spans: String,  // JSON array of SpanRefs
    #[output] pub confidence: f32,
}

// Extractor tracks evidence spans
#[Signature(cot)]
struct ExtractorSignature {
    #[input] pub query: String,
    #[input] pub chunk: String,
    #[input] pub span_ref: String,     // JSON SpanRef for this chunk
    #[output] pub findings: String,
    #[output] pub evidence_spans: String,  // JSON array of sub-SpanRefs
    #[output] pub relevance: f32,
}

// Reducer includes citations
#[Signature]
struct ReducerSignature {
    #[input] pub query: String,
    #[input] pub findings: String,
    #[input] pub evidence_spans: String,
    #[output] pub answer: String,
    #[output] pub citations: String,   // JSON array of SpanRefs
    #[output] pub confidence: f32,
}

// Verifier identifies missing evidence
#[Signature]
struct VerifierSignature {
    #[input] pub query: String,
    #[input] pub answer: String,
    #[input] pub citations: String,
    #[output] pub verdict: String,     // PASS, FAIL, or PARTIAL
    #[output] pub explanation: String,
    #[output] pub missing_spans: String,  // What evidence is needed
    #[output] pub corrections: String,
}
```

### Helper Types for Parsing

```rust
use rlm::{CandidateSpan, MissingSpanRequest};

// Parse router output
let candidates = CandidateSpan::parse_array(&routing.candidate_spans)?;
for candidate in candidates {
    println!("Check {}:{}-{} because: {}",
        candidate.path, candidate.start_line, candidate.end_line, candidate.why);
}

// Parse verifier output
let missing = MissingSpanRequest::parse_array(&verification.missing_spans)?;
for req in missing {
    println!("Need evidence for '{}': {}", req.claim, req.description);
}
```

### Custom Signatures

Define your own signatures for specialized tasks:

```rust
use dspy_rs::{Signature, Predict, Predictor, example};

#[Signature]
struct ClassifySignature {
    #[input] pub text: String,
    #[input] pub categories: String,
    #[output] pub category: String,
    #[output] pub confidence: f32,
}

let classifier = Predict::new(ClassifySignature::new());
let result = classifier.forward(example! {
    "text": "input" => "This document discusses machine learning...",
    "categories": "input" => "tech, finance, health, other"
}).await?;
```

## Usage Patterns

### Quick Analysis (No CoT, No Verification)

```rust
let result = orchestrator.analyze_quick("What is the topic?", &doc).await?;
```

### Full Pipeline with Verification

```rust
let config = DspyOrchestratorConfig {
    use_cot_extraction: true,
    verify_answer: true,
    ..Default::default()
};
let orchestrator = DspyOrchestrator::with_config(config);
let result = orchestrator.analyze("Summarize the key points", &doc).await?;

if let Some(verification) = &result.verification {
    if verification.verdict == "FAIL" {
        println!("Corrections needed: {}", verification.corrections);
    }
}
```

### With Full Provenance

```rust
let orchestrator = DspyOrchestrator::new()
    .with_document_path("docs/spec.md")
    .with_commit(get_current_commit());

let result = orchestrator.analyze(query, &document).await?;

// Build citation report
for extraction in &result.extractions {
    if let Some(span) = &extraction.span_ref {
        println!("[{}] {} (lines {}-{})",
            extraction.section,
            span.path,
            span.start_line,
            span.end_line
        );
        // Verify content hasn't changed
        if !span.verify_content(&chunk_content) {
            println!("  WARNING: Content has changed since analysis");
        }
    }
}
```

## Optimization (Offline)

dsrs supports automatic prompt optimization. This should be run offline during development, not at runtime.

See `examples/optimize_signatures.rs` for scaffolding:

```rust
use rlm::{COPRO, MIPROv2, Evaluator};

// 1. Load training examples
let train_data = load_training_data("assets/training_data/router.json")?;

// 2. Define evaluation metric
let evaluator = |pred: &Prediction, gold: &Example| -> f32 {
    // Compare prediction to ground truth
};

// 3. Run optimizer
let optimizer = COPRO::new(evaluator);
let optimized = optimizer.compile(&predictor, &train_data).await?;

// 4. Save optimized prompts
save_optimized("assets/optimized_prompts/router.json", &optimized)?;
```

## Integration with RLM

### Chunking

Uses RLM's structure-aware chunking:

```rust
use rlm::{detect_structure, chunk_by_structure};

let structure = detect_structure(&document);
let chunks = chunk_by_structure(&document, &structure, chunk_size, overlap);
```

### Error Handling

Converts DSPy errors to RLM errors:

```rust
.await
.map_err(|e: anyhow::Error| RlmError::ExecutionError(e.to_string()))?;
```

### Tool Integration

Attach tools to predictors for environment access:

```rust
use rlm::{GrepTool, RlmTool};

let grep = GrepTool::new(repo_path);

// Tools implement RlmTool trait
let result = grep.execute(serde_json::json!({
    "pattern": "fn main",
    "paths": ["**/*.rs"],
    "max_hits": 10
})).await?;
```

## Related Documentation

- [Provenance Tracking](./PROVENANCE.md) - SpanRef and evidence tracking
- [Environment Tools](./TOOLS.md) - Grep, read, list, symbols tools
- [dsrs Documentation](../../dsrs/docs/README.md)
- [dsrs Signatures](../../dsrs/docs/SIGNATURES.md)
- [RLM Architecture](./ARCHITECTURE.md)

## Examples

- `crates/rlm/examples/optimize_signatures.rs` - Optimizer scaffolding
