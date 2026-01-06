# RLM Execution Visualization

Interactive visualization page at `/rlm` showing Recursive Language Model execution.

## Current State: UI Demo Only

**The page is a visual mockup.** When you click RUN, it plays a canned animation that:
- Ignores your query and document inputs completely
- Shows 12 fake chunks with placeholder text
- Advances through phases on a 2-second timer
- Displays hardcoded "findings" and a canned final answer

Nothing actually runs. No LLM is called. No document is processed.

---

## What's On The Page

### Header Section
- Title: "RLM EXECUTION VISUALIZER"
- Status badge showing connection state (READY/STREAMING/COMPLETE/ERROR)
- Subtitle describing the page

### Input Section
- **Query field**: Text input for user's question (currently ignored)
- **Document field**: Textarea for pasting source document (currently ignored)
- **RUN button**: Starts demo animation / STOP to halt it

### Timeline Bar
- Horizontal progress showing 4 phases:
  1. Structure Discovery
  2. Chunking
  3. Extraction
  4. Synthesis
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
  - Section title
  - Content preview
  - Extracted findings
- Streaming LLM response text
- Final answer display when complete

---

## What's Fake (Demo Mode)

All in `tick_demo()` function in `views/rlm.rs`:

```rust
// Fake chunk data
section_title: Some(format!("Section {}", i + 1)),
content_preview: Some("Lorem ipsum dolor sit amet...".to_string()),
findings: Some("Extracted key information from this section.".to_string()),

// Fake streaming text
streaming_text = format!("Processing chunk {}...", chunk_idx + 1);

// Fake final answer
final_answer = Some("Based on the analysis of all 12 document sections,
the key findings are: [summary of extracted information]. The document
primarily discusses [main topics] with emphasis on [key themes].".to_string());
```

The demo:
1. Sets `total_chunks = 12` (hardcoded)
2. Every 2 seconds advances `demo_phase_idx`
3. Populates chunks with fake data
4. Marks chunks complete one by one
5. Shows canned final answer

---

## What's Needed To Make It Real

### Option A: Backend SSE Service

Create a backend service that:
1. Receives POST to `/api/rlm/run` with `{ query, document }`
2. Runs actual RLM pipeline from `crates/rlm/`
3. Streams SSE events back to browser

**Required files:**

```
crates/web/worker/src/routes/rlm.rs
  - Add POST /api/rlm/run endpoint
  - Add GET /api/rlm/stream/:run_id SSE endpoint

crates/web/client/src/rlm_runtime.rs (NEW)
  - EventSource connection management
  - Parse SSE events → update RlmVizState
  - Handle reconnection/errors
```

**SSE Event Schema:**

```json
{"type": "phase_start", "phase": "structure_discovery"}
{"type": "phase_start", "phase": "chunking", "total_chunks": 45}
{"type": "chunk_start", "chunk_id": 0, "section_title": "## Introduction"}
{"type": "chunk_content", "chunk_id": 0, "preview": "This paper presents..."}
{"type": "token", "text": "The"}
{"type": "token", "text": " key"}
{"type": "token", "text": " finding"}
{"type": "chunk_complete", "chunk_id": 0, "findings": "Introduces the RLM concept..."}
{"type": "phase_start", "phase": "synthesis"}
{"type": "token", "text": "Based"}
{"type": "final", "answer": "...", "processing_time_ms": 12400}
{"type": "error", "message": "Rate limit exceeded"}
```

**Challenge:** RLM requires Python execution environment for code interpreter. Cloudflare Workers can't run Python. Options:
- External RLM service (separate server/container)
- Modal/Fly.io serverless function
- WebSocket proxy to a GPU box

### Option B: Simplified WASM-Only Mode

Strip RLM down to work without Python code execution:
1. Structure discovery via regex/heuristics in Rust
2. Chunking via semantic boundaries (headers, paragraphs)
3. Extraction via direct LLM API calls (Anthropic/OpenAI)
4. Synthesis via final LLM call

This loses the "recursive code execution" aspect but could run entirely client-side or in Workers.

### Option C: Pre-recorded Traces

Record real RLM executions as JSON trace files:
```json
{
  "query": "What are the main contributions?",
  "document_hash": "abc123",
  "events": [
    {"t": 0, "type": "phase_start", "phase": "structure_discovery"},
    {"t": 150, "type": "phase_start", "phase": "chunking", "total_chunks": 8},
    ...
  ]
}
```

Play back traces with realistic timing. Good for demos, not for real use.

---

## File Structure

```
crates/web/
├── worker/src/routes/
│   └── rlm.rs              # Route handler (currently just serves HTML)
├── client/src/
│   ├── state.rs            # RlmVizState struct and enums
│   ├── app.rs              # Event wiring (mouse, keyboard, paste)
│   └── views/
│       └── rlm.rs          # Rendering + demo mode logic
└── docs/
    └── rlm-visualization.md  # This file
```

---

## State Types

```rust
pub enum RlmConnectionStatus { Idle, Connecting, Streaming, Complete, Error }
pub enum RlmPhase { Idle, StructureDiscovery, Chunking, Extraction, Synthesis, Complete }
pub enum RlmStepStatus { Pending, Processing, Complete, Error }

pub struct RlmChunkState {
    pub chunk_id: usize,
    pub section_title: Option<String>,
    pub content_preview: Option<String>,
    pub findings: Option<String>,
    pub status: RlmStepStatus,
}

pub struct RlmVizState {
    // Inputs
    pub query_input: TextInput,
    pub context_input: TextInput,

    // Execution state
    pub current_phase: RlmPhase,
    pub chunks: Vec<RlmChunkState>,
    pub total_chunks: usize,
    pub processed_chunks: usize,
    pub streaming_text: String,
    pub final_answer: Option<String>,

    // Demo mode
    pub demo_mode: bool,
    pub demo_phase_idx: usize,
    pub demo_last_tick: u64,
}
```

---

## Event Handling

Currently wired up in `app.rs`:

| Event | Handler |
|-------|---------|
| MouseMove | `handle_rlm_mouse_move()` + `state.rlm.handle_event()` |
| MouseDown | `state.rlm.handle_event()` |
| KeyDown | `state.rlm.handle_event()` |
| Ctrl+V | `state.rlm.paste_text()` |
| Click on RUN | `handle_rlm_click()` → toggles `demo_mode` |
| Scroll | Updates `scroll_offset` |

---

## To Actually Run RLM

The `crates/rlm/` crate contains the real implementation:

- `engine.rs` - RlmEngine with code execution loop
- `orchestrator.rs` - EngineOrchestrator with 4-phase pipeline
- `chunking.rs` - Semantic document chunking
- `subquery.rs` - Parallel chunk processing

Key integration points:

```rust
// From crates/rlm/src/orchestrator.rs
pub struct EngineOrchestrator {
    pub async fn run_full_pipeline(&mut self, query: &str, context: &str) -> Result<String>
}

// From crates/rlm/src/engine.rs
pub struct ExecutionLogEntry {
    pub iteration: u32,
    pub llm_response: String,
    pub command_type: String,
    pub executed: String,
    pub result: String,
}
```

These would need to emit events that the visualization can consume.

---

## Next Steps

1. **Decide on backend approach** (external service vs. simplified WASM)
2. **Create `rlm_runtime.rs`** for SSE/WebSocket handling
3. **Add API endpoints** in `routes/rlm.rs`
4. **Wire up real inputs** - send query+document to backend
5. **Replace demo mode** with real event processing
6. **Add error handling** - connection drops, timeouts, rate limits
7. **Add timeline scrubbing** - replay past executions
