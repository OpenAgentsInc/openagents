# DSPy Integration

The RLM crate integrates [DSRs (dspy-rs)](https://github.com/krypticmouse/DSRs) to provide declarative LLM programming with typed signatures, composable modules, and automatic prompt optimization.

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
┌─────────────────────────────────────────────────────────────┐
│                    DspyOrchestrator                         │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────┐   ┌───────────┐   ┌─────────┐   ┌──────────┐  │
│  │ Router  │ → │ Extractor │ → │ Reducer │ → │ Verifier │  │
│  │(Predict)│   │ (Predict) │   │(Predict)│   │ (Predict)│  │
│  └────┬────┘   └─────┬─────┘   └────┬────┘   └────┬─────┘  │
│       │              │              │              │        │
│       └──────────────┴──────────────┴──────────────┘        │
│                         │                                    │
│                    DSPy Modules                              │
│                         │                                    │
├─────────────────────────┴───────────────────────────────────┤
│                    dspy_bridge                               │
│   (re-exports dspy-rs types + LM configuration helpers)     │
├─────────────────────────────────────────────────────────────┤
│                      dspy-rs                                 │
│        (Signatures, Predict, Module, Optimizers)            │
└─────────────────────────────────────────────────────────────┘
```

## Quick Start

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

## Core Components

### 1. DSPy Bridge (`dspy_bridge`)

Re-exports key DSRs types and provides LM configuration helpers.

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

### 2. DSPy Orchestrator (`dspy_orchestrator`)

Multi-phase document analysis using typed DSPy signatures.

#### Pipeline Phases

1. **Router** - Identifies relevant document sections from a preview
2. **Extractor** - Extracts findings from each chunk (parallel, with optional CoT)
3. **Reducer** - Synthesizes findings into a final answer
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

#### Result Structure

```rust
pub struct DspyAnalysisResult {
    pub answer: String,           // Final synthesized answer
    pub citations: String,        // Supporting citations
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
}
```

## Typed Signatures

The orchestrator uses typed DSPy signatures for each phase. The `#[Signature]` macro creates structured LLM interfaces:

```rust
use dspy_rs::Signature;

#[Signature]
struct ExtractorSignature {
    /// Extract relevant information from this chunk.

    #[input]
    pub query: String,

    #[input]
    pub chunk: String,

    #[output]
    pub findings: String,
}

// With chain-of-thought reasoning
#[Signature(cot)]
struct DetailedExtractorSignature {
    #[input]
    pub query: String,

    #[input]
    pub chunk: String,

    #[input]
    pub section: String,

    #[output]
    pub findings: String,

    #[output]
    pub evidence: String,

    #[output]
    pub relevance: f32,
}
```

### Built-in Signatures

The orchestrator defines these signatures internally:

| Signature | Purpose | I/O |
|-----------|---------|-----|
| `RouterSignature` | Identify relevant sections | query, preview → sections, confidence |
| `ExtractorSignature` | Extract with CoT | query, chunk, section → findings, evidence, relevance |
| `SimpleExtractorSignature` | Fast extraction | query, chunk → findings |
| `ReducerSignature` | Synthesize findings | query, findings → answer, citations, confidence |
| `VerifierSignature` | Validate answer | query, answer, evidence → verdict, explanation, corrections |

## Integration with RLM

The DSPy orchestrator integrates with RLM's existing infrastructure:

### Chunking

Uses RLM's structure-aware chunking (`chunking` module):

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

### Custom Signatures

Define your own signatures for specialized tasks:

```rust
use dspy_rs::{Signature, Predict, Predictor, example};

#[Signature]
struct ClassifySignature {
    #[input]
    pub text: String,

    #[input]
    pub categories: String,

    #[output]
    pub category: String,

    #[output]
    pub confidence: f32,
}

let classifier = Predict::new(ClassifySignature::new());
let result = classifier.forward(example! {
    "text": "input" => "This document discusses machine learning...",
    "categories": "input" => "tech, finance, health, other"
}).await?;
```

## Future: Optimization

DSRs supports automatic prompt optimization. Future work:

```rust
use rlm::{COPRO, MIPROv2, Evaluator};

// Define evaluation metric
let evaluator = |pred: &Prediction, gold: &Example| -> f32 {
    // Compare prediction to ground truth
    if pred.get("answer", None) == gold.get("answer", None) { 1.0 } else { 0.0 }
};

// Optimize the extractor's prompts
let optimizer = COPRO::new(evaluator);
let optimized_extractor = optimizer.compile(&extractor, &train_set).await?;
```

## Related Documentation

- [DSRs Documentation](https://dsrs.herumbshandilya.com/)
- [DSRs API Reference](https://docs.rs/dspy-rs)
- [RLM Architecture](./ARCHITECTURE.md)
- [DSPy + RLM Conceptual Overview](../../../docs/dspy/rlm.md)

## Examples

See `crates/rlm/examples/` for runnable examples (coming soon).
