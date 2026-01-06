# RLM Execution Visualization

Interactive visualization page at `/rlm` showing Recursive Language Model execution powered by DSPy.

## Current State: DSPy Demo with Real Content

**The page now displays a real pre-recorded trace** of DSPy document analysis. When you visit the page or click RUN:
- Shows actual analysis of the Repo-RLM runtime specification
- Displays 8 real chunks from `docs/rlm/git.md`
- Shows extracted findings from each section
- Presents a synthesized final answer with citations
- Loops automatically every ~19 seconds

The demo uses **trace playback** of a real DSPy orchestrator execution, demonstrating:
- **Routing**: Selecting relevant document sections
- **Chunking**: Splitting into semantic units
- **Extraction (CoT)**: Chain-of-thought per chunk
- **Reduce + Verify**: Combining and validating findings

---

## What's On The Page

### Header Section
- Title: "RLM EXECUTION VISUALIZER"
- Status badge showing connection state (READY/STREAMING/COMPLETE/ERROR)
- Subtitle: "DSPy-Powered Document Analysis: Route -> Extract -> Reduce -> Verify"

### Input Section
- **Query field**: Pre-populated with demo query (currently for display)
- **Document field**: Pre-populated with document preview (currently for display)
- **RUN button**: Starts/restarts trace playback / STOP to halt

### Timeline Bar
- Horizontal progress showing 4 phases (DSPy terminology):
  1. Route - Selecting relevant sections
  2. Chunk - Splitting into semantic chunks
  3. Extract - Chain-of-thought per chunk
  4. Reduce - Combining and validating
- Phase dots with color coding (gray=pending, orange=active, green=complete)
- Current phase label and chunk progress counter

### Left Panel: Execution Phases
- Vertical list of all phases with status icons:
  - `>` = currently processing (orange)
  - `=` = completed (green)
  - ` ` = pending (gray)
- Description shown for active phase
- **Chunk grid** during Extraction: grid of small squares, one per chunk, color-coded by status

### Right Panel: Detail View
- Shows info about the currently active chunk:
  - Section title (e.g., "External Contract", "Tool API", "Program Graph")
  - Content preview from the actual document
  - Extracted findings (real DSPy output)
- Streaming status text
- Final answer display when complete

---

## Demo Content

The demo analyzes `docs/rlm/git.md` (Repo-RLM Runtime Spec) with the query:

> "What are the main components of the Repo-RLM runtime spec?"

**8 Chunks analyzed:**
1. External Contract - String-in/string-out API
2. Core Design Goals - 6 design principles
3. Repository Environment - SpanRef provenance
4. Tool API - File discovery, reading, search
5. Program Graph - DSPy-style modules
6. Budgets & Stopping - Cost control
7. Tracing & Provenance - Event logging
8. Module Signatures - Typed interfaces

**Final Answer** summarizes 6 main components with citations.

---

## Technical Implementation

### Trace Playback Architecture

The demo uses pre-recorded traces instead of live LLM calls:

```
assets/rlm-demo-trace.json
  └── Embedded via include_str!()
      └── Parsed on page load
          └── Events played back with timing

views/rlm.rs
  └── tick_demo() processes events at correct timestamps
      └── apply_trace_event() updates RlmVizState
```

### Trace File Format

```json
{
  "query": "What are the main components of the Repo-RLM runtime spec?",
  "document_preview": "## 0) What we're building...",
  "events": [
    {"t": 0, "type": "phase_start", "phase": "routing"},
    {"t": 400, "type": "routing_result", "sections": "..."},
    {"t": 800, "type": "phase_start", "phase": "chunking"},
    {"t": 1000, "type": "chunk_created", "id": 0, "section": "External Contract", "preview": "..."},
    {"t": 3000, "type": "extraction_start", "chunk_id": 0},
    {"t": 3800, "type": "extraction_complete", "chunk_id": 0, "findings": "...", "relevance": 0.95},
    {"t": 11200, "type": "phase_start", "phase": "synthesis"},
    {"t": 13500, "type": "synthesis_complete", "answer": "...", "confidence": 0.91},
    {"t": 14000, "type": "complete"}
  ]
}
```

### State Types

```rust
// Trace event types for DSPy demo playback
pub enum RlmTraceEventType {
    PhaseStart { phase: String },
    RoutingResult { sections: String },
    ChunkCreated { id: usize, section: String, preview: String },
    ExtractionStart { chunk_id: usize },
    ExtractionComplete { chunk_id: usize, findings: String, relevance: f32 },
    SynthesisComplete { answer: String, confidence: f32 },
    Complete,
}

pub struct RlmTraceEvent {
    pub t: u64,  // ms from start
    pub event: RlmTraceEventType,
}

pub struct RlmDemoTrace {
    pub query: String,
    pub document_preview: String,
    pub events: Vec<RlmTraceEvent>,
}

// Updated RlmVizState fields
pub struct RlmVizState {
    // ... existing fields ...

    // Trace playback state
    pub trace: Option<RlmDemoTrace>,
    pub trace_start_time: u64,
    pub trace_event_idx: usize,
    pub auto_restart: bool,
}
```

---

## File Structure

```
crates/web/
├── worker/src/routes/
│   └── rlm.rs              # Route handler (serves HTML)
├── client/
│   ├── assets/
│   │   └── rlm-demo-trace.json  # Pre-recorded DSPy trace
│   └── src/
│       ├── state.rs            # RlmVizState + trace types
│       ├── app.rs              # Event wiring
│       └── views/
│           └── rlm.rs          # Rendering + trace playback
└── docs/
    └── rlm-visualization.md    # This file
```

---

## Event Handling

| Event | Handler |
|-------|---------|
| MouseMove | `handle_rlm_mouse_move()` + `state.rlm.handle_event()` |
| MouseDown | `state.rlm.handle_event()` |
| KeyDown | `state.rlm.handle_event()` |
| Ctrl+V | `state.rlm.paste_text()` |
| Click on RUN | `handle_rlm_click()` → starts/stops trace playback |
| Scroll | Updates `scroll_offset` |
| Frame tick | `tick_demo()` → processes trace events at correct time |

---

## Future: Live DSPy Backend

To make the demo interactive with real document input:

### Option A: Backend DSPy Service

```
POST /api/rlm/run
  { query: string, document: string }
  → Returns run_id

GET /api/rlm/stream/:run_id (SSE)
  → Streams trace events in real-time
```

**Required:**
- External service running `crates/rlm` with DSPy feature
- SSE endpoint streaming `DspyOrchestrator` events
- `rlm_runtime.rs` for EventSource connection

### Option B: WebAssembly DSPy

Compile DSPy orchestrator to WASM (challenging due to async/network dependencies).

### Option C: Multiple Pre-recorded Traces

Add more trace files for different documents/queries:
- Research paper analysis
- Codebase exploration
- Multi-document synthesis

---

## Generating New Traces

To generate a new trace from a real DSPy execution:

```rust
// crates/rlm/examples/generate_demo_trace.rs
use rlm::{DspyOrchestrator, DspyOrchestratorConfig, configure_dspy_lm};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    configure_dspy_lm("openai:gpt-4o-mini", None, None).await?;

    let document = fs::read_to_string("docs/rlm/git.md")?;
    let query = "What are the main components?";

    let config = DspyOrchestratorConfig {
        verbose: true,
        use_cot_extraction: true,
        max_chunks: 8,
        ..Default::default()
    };

    let orchestrator = DspyOrchestrator::with_config(config);
    let result = orchestrator.analyze(query, &document).await?;

    // Build and save trace JSON
    let trace = build_trace(query, &document, &result);
    fs::write("rlm-demo-trace.json", serde_json::to_string_pretty(&trace)?)?;

    Ok(())
}
```

---

## Related Documentation

- [RLM DSPy Integration](../../rlm/docs/DSPY.md) - DSPy orchestrator usage
- [RLM Architecture](../../rlm/docs/ARCHITECTURE.md) - Crate structure
- [DSPy in Rust](../../../docs/dspy/rust.md) - DSRs (dspy-rs) overview
