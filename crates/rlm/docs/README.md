# RLM Paper Replication Infrastructure

Infrastructure for replicating the Recursive Language Models paper (arXiv 2512.24601v1) using modular Rust crates designed for reuse across future paper replications.

## Overview

This crate provides the RLM execution engine for long-context tasks, with optional DSPy
integration for declarative orchestration. The benchmarking crates that previously accompanied it
(`bench-harness`, `bench-datasets`, `rlm-methods`) are archived out of the current workspace.

| Crate | Purpose |
|-------|---------|
| `lm-router` | Multi-backend LM routing with usage tracking |
| `rlm` | Core engine with DSPy integration, provenance tracking, and tools |

## Key Features

### DSPy Integration (Feature: `dspy`)

The RLM crate integrates with [dsrs](../../dsrs/docs/README.md) for declarative LLM programming:

- **Provenance-first signatures** - All signatures track evidence origins via SpanRef
- **LmRouter bridge** - Per-request LM routing with unified cost tracking
- **Environment tools** - Repository traversal tools that return SpanRefs
- **4-phase orchestrator** - Router → Extractor → Reducer → Verifier pipeline

### Provenance Tracking

Every piece of evidence can be traced to its exact source:

```rust
use rlm::SpanRef;

let span = SpanRef::from_chunk(
    chunk_id,
    "docs/spec.md",
    Some("abc123def"),  // Git commit
    10, 25,             // Line range
    500, 1200,          // Byte range
    &content,           // For hash computation
);

// Verify content hasn't changed
if span.verify_content(&current_content) {
    println!("Evidence verified");
}
```

### Environment Tools

Tools for repository exploration that return provenance-tracked results:

| Tool | Purpose |
|------|---------|
| `GrepTool` | Pattern search with SpanRef results |
| `ReadLinesTool` | Read file ranges with provenance |
| `ListFilesTool` | Directory traversal with metadata |
| `SymbolsTool` | Extract code symbols (functions, classes, etc.) |

## Quick Start

```rust
use std::sync::Arc;
use lm_router::LmRouter;
use rlm::{LmRouterClient, MockExecutor, RlmEngine};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Set up the LM router (configure backends as needed).
    let router = Arc::new(LmRouter::builder().build());
    let client = LmRouterClient::new(router, "model-name");

    // 2. Choose an execution environment (mock or real).
    let executor = MockExecutor::new();
    let engine = RlmEngine::new(client, executor);

    // 3. Run a query.
    let result = engine.run("What is 2 + 2?").await?;
    println!("Answer: {}", result.output);

    Ok(())
}
```

### DSPy Quick Start

```rust
use rlm::{DspyOrchestrator, SpanRef, GrepTool};
use lm_router::LmRouter;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Set up LmRouter
    let router = Arc::new(LmRouter::builder()
        .add_backend(/* your backend */)
        .build());

    // 2. Create orchestrator with per-request LM
    let orchestrator = DspyOrchestrator::with_lm_router(router.clone(), "model-name")
        .await?
        .with_document_path("docs/spec.md")
        .with_commit("abc123");

    // 3. Analyze a document
    let result = orchestrator.analyze(
        "What authentication methods are supported?",
        &document_content
    ).await?;

    // 4. Results include provenance
    for extraction in &result.extractions {
        println!("Finding: {}", extraction.findings);
        if let Some(span) = &extraction.span_ref {
            println!("  Source: {}:{}-{}", span.path, span.start_line, span.end_line);
        }
    }

    // 5. Use tools for additional exploration
    let grep = GrepTool::new(repo_path);
    let hits = grep.search("TODO", &["**/*.rs"], 20).await?;
    for hit in hits {
        println!("{}:{} - {}", hit.span.path, hit.span.start_line, hit.line);
    }

    Ok(())
}
```

## Documentation Index

### Core Documentation

- [Architecture](./ARCHITECTURE.md) - Crate structure, module organization, and design decisions
- [DSPy Integration](./DSPY.md) - DSPy (dspy-rs) for declarative LLM programming
- [Provenance Tracking](./PROVENANCE.md) - SpanRef type for Git-aware evidence tracking
- [Environment Tools](./TOOLS.md) - Repository traversal tools (grep, read, list, symbols)

### Benchmarking Infrastructure (Archived)

The benchmarking crates are archived out of the current workspace. These docs are kept for
historical reference:

- [LM Router](./LM_ROUTER.md) - Multi-backend LM routing
- [Bench Harness](./BENCH_HARNESS.md) - Experiment infrastructure (archived)
- [Datasets](./DATASETS.md) - Dataset formats and loaders (archived)
- [Methods](./METHODS.md) - Method implementations (archived)
- [Running Experiments](./RUNNING_EXPERIMENTS.md) - How to run benchmarks (archived)

## Paper Methods

The RLM paper compares 5 methods:

| Method | Description | Implementation |
|--------|-------------|----------------|
| Base | Direct LLM call with full context | Archived (`rlm-methods`) |
| Summary Agent | Iterative context summarization | Archived (`rlm-methods`) |
| CodeAct+BM25 | ReAct with BM25 retrieval | Planned |
| RLM | Recursive LM with `llm_query` | Planned |
| RLM (no sub-calls) | Ablation without recursion | Planned |

## Benchmarks

The datasets below were used in the archived benchmarking pipeline and are listed for reference.

| Dataset | Tasks | Metric | Description |
|---------|-------|--------|-------------|
| S-NIAH | 50 | Exact Match | Single-needle in a haystack |
| BrowseComp-Plus | 150 | Exact Match | Web browsing comprehension |
| OOLONG (TrecCoarse) | - | Numeric Decay | Document counting queries |
| OOLONG (Pairs) | - | F1 | Pairwise aggregation |
| CodeQA | - | Accuracy | Multiple-choice code comprehension |

## Primary Backend

The primary backend in earlier experiments was **FM Bridge** (Apple Foundation Models), but any
`lm-router` backend can be used. A swarm simulator is also available for testing distributed
NIP-90 scenarios.

## Codex Integration

RLM integrates with Codex in two ways:

### Mode A: Codex Calls RLM (MCP Tools)

RLM exposes tools via an MCP server that Codex can invoke:

```bash
# Configure in Codex settings
{
  "mcpServers": {
    "rlm": {
      "type": "stdio",
      "command": "rlm-mcp-server"
    }
  }
}

# Or via CLI
codex --mcp-server "rlm:stdio:rlm-mcp-server"
```

**MCP Server Backend Selection:**

The MCP server can use different backends for RLM execution:

```bash
# Use Ollama (default) - requires Ollama at localhost:11434
RLM_BACKEND=ollama rlm-mcp-server

# Use Codex CLI - requires codex CLI installed
# Build with: cargo build -p rlm --features codex --bin rlm-mcp-server
RLM_BACKEND=codex rlm-mcp-server
```

Available tools:

| Tool | Description |
|------|-------------|
| `rlm_query` | Deep recursive analysis using prompt-execute loop |
| `rlm_fanout` | Distribute query across workers (local/swarm/datacenter) |

### Mode B: Codex AS the RLM Backend

Use Codex (Pro/Max) as the LlmClient for RLM execution. This mode uses Codex's
**structured outputs** to enforce the RLM response format, ensuring Codex always
responds with either code to execute or a final answer:

```rust
use rlm::CodexLlmClient;  // Requires `codex` feature

let client = CodexLlmClient::new("/path/to/workspace");
let engine = RlmEngine::new(client, PythonExecutor::new());
let result = engine.run("Analyze this code").await?;
```

The CodexLlmClient uses a JSON schema to constrain Codex's output:
- `action: "execute"` - with `code` field containing Python to run
- `action: "final"` - with `answer` field containing the final response

## Feature Flags

The `rlm` crate uses feature flags to control optional functionality:

```toml
[dependencies]
rlm = { path = "crates/rlm" }  # Core only

# With DSPy integration
rlm = { path = "crates/rlm", features = ["dspy"] }

# With Codex as LlmClient backend
rlm = { path = "crates/rlm", features = ["codex"] }
```

| Feature | Description |
|---------|-------------|
| `dspy` | DSPy integration: signatures, orchestrator, LmRouter bridge, tools |
| `codex` | Codex as RLM backend via app-server |

## Module Summary

### Always Available

| Module | Description |
|--------|-------------|
| `engine` | RlmEngine execution loop |
| `client` | LlmClient trait and response types |
| `chunking` | Structure-aware document chunking |
| `span` | SpanRef for provenance tracking |
| `context` | Context management |
| `command` | Command parsing |
| `orchestrator` | High-level analysis orchestration |
| `mcp_tools` | MCP tool definitions (rlm_query, rlm_fanout) |

### With `codex` Feature

| Module | Description |
|--------|-------------|
| `codex_client` | CodexLlmClient implementing LlmClient |

### With `dspy` Feature

| Module | Description |
|--------|-------------|
| `dspy_bridge` | Global LM config + LmRouter bridge |
| `dspy_orchestrator` | 4-phase document analysis pipeline |
| `signatures` | Provenance-first DSPy signatures |
| `tools` | Environment tools (grep, read, list, symbols) |

## License

CC-0
