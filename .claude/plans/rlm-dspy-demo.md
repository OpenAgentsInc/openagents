# Plan: Replace RLM Demo with Real DSPy Demo

## Current State

The `/rlm` visualization page is a **UI mockup** that:
- Ignores query and document inputs
- Shows 12 fake chunks with "Lorem ipsum" placeholder text
- Advances through phases on a 2-second timer
- Displays hardcoded findings and canned final answer

**Location**: `crates/web/client/src/views/rlm.rs` (`tick_demo()` function)

## Goal

Replace with a **real RLM+DSPy demo** that:
1. Runs automatically in a loop (non-interactive)
2. Shows real execution of a representative task
3. Uses actual DSPy orchestrator output
4. Demonstrates the Route → Extract → Reduce → Verify pipeline

## Approach: Pre-recorded Trace Playback

Create a trace file from a **real DSPy execution** and play it back with realistic timing.

### Why Pre-recorded?

1. **No backend needed** - Works on Cloudflare Workers (no Python/WASM limitations)
2. **Deterministic** - Same demo every time, tunable timing
3. **Real data** - Actual findings, not lorem ipsum
4. **Cost-free** - No LLM API calls during demo

## Representative Task: "Repo-RLM Spec Analysis"

Use the `docs/rlm/git.md` document as the demo subject. It's ~430 lines describing:
- External contract (string in/out)
- Environment (repo + tools)
- Tool API (list/read/grep/symbols)
- Program graph (Route → Read → Extract → Reduce → Verify)
- Budgets & stopping
- Tracing & provenance

**Query**: "What are the main components of the Repo-RLM runtime spec?"

This is perfect because:
- It's self-referential (analyzing RLM spec with RLM)
- Has clear structure (numbered sections)
- Produces meaningful findings
- Demonstrates real extraction quality

## Implementation Steps

### Phase 1: Generate Real Trace (Offline)

Create a CLI tool or script that:

1. **Runs DspyOrchestrator** on `docs/rlm/git.md`
2. **Captures all events** as JSON:
   - Phase transitions
   - Chunk creation (with real section titles)
   - Extraction results (real findings)
   - Synthesis output (real answer)
3. **Saves trace file** to `crates/web/client/assets/rlm-demo-trace.json`

**Trace format**:
```json
{
  "query": "What are the main components of the Repo-RLM runtime spec?",
  "document_preview": "## 0) What we're building...",
  "events": [
    {"t": 0, "type": "phase_start", "phase": "routing"},
    {"t": 500, "type": "routing_result", "sections": "Environment, Tool API, Program Graph..."},
    {"t": 800, "type": "phase_start", "phase": "chunking", "total_chunks": 8},
    {"t": 1000, "type": "chunk_created", "id": 0, "section": "External Contract", "preview": "answer = repo_rlm(prompt)..."},
    {"t": 1200, "type": "chunk_created", "id": 1, "section": "Repository Environment", "preview": "Repo handle + pinning..."},
    ...
    {"t": 2000, "type": "phase_start", "phase": "extraction"},
    {"t": 2500, "type": "extraction_start", "chunk_id": 0},
    {"t": 3500, "type": "extraction_complete", "chunk_id": 0, "findings": "The external API is string-in/string-out...", "relevance": 0.9},
    ...
    {"t": 12000, "type": "phase_start", "phase": "synthesis"},
    {"t": 14000, "type": "synthesis_complete", "answer": "The Repo-RLM runtime spec consists of...", "confidence": 0.85},
    {"t": 14500, "type": "complete"}
  ]
}
```

### Phase 2: Update Frontend State

Modify `crates/web/client/src/state.rs`:

1. **Add trace loading**:
```rust
pub(crate) struct RlmDemoTrace {
    pub query: String,
    pub document_preview: String,
    pub events: Vec<RlmTraceEvent>,
}

pub(crate) struct RlmTraceEvent {
    pub t: u64,  // ms from start
    pub event_type: RlmEventType,
    pub data: serde_json::Value,
}
```

2. **Update RlmVizState**:
```rust
// Replace demo_mode fields with:
pub(crate) trace: Option<RlmDemoTrace>,
pub(crate) trace_start_time: u64,
pub(crate) trace_event_idx: usize,
pub(crate) auto_restart: bool,  // true = loop forever
```

### Phase 3: Update Demo Logic

Rewrite `tick_demo()` in `crates/web/client/src/views/rlm.rs`:

```rust
fn tick_trace_playback(state: &mut AppState) {
    let now = performance_now();
    let Some(trace) = &state.rlm.trace else { return };

    let elapsed = now - state.rlm.trace_start_time;

    // Process all events up to current time
    while state.rlm.trace_event_idx < trace.events.len() {
        let event = &trace.events[state.rlm.trace_event_idx];
        if event.t > elapsed { break; }

        apply_trace_event(state, event);
        state.rlm.trace_event_idx += 1;
    }

    // Auto-restart after completion + delay
    if state.rlm.current_phase == RlmPhase::Complete
       && state.rlm.auto_restart
       && elapsed > trace.events.last().map(|e| e.t).unwrap_or(0) + 5000
    {
        reset_and_restart_trace(state);
    }
}

fn apply_trace_event(state: &mut AppState, event: &RlmTraceEvent) {
    match &event.event_type {
        RlmEventType::PhaseStart { phase } => {
            state.rlm.current_phase = *phase;
            state.rlm.connection_status = RlmConnectionStatus::Streaming;
        }
        RlmEventType::ChunkCreated { id, section, preview } => {
            state.rlm.chunks.push(RlmChunkState {
                chunk_id: *id,
                section_title: Some(section.clone()),
                content_preview: Some(preview.clone()),
                findings: None,
                status: RlmStepStatus::Pending,
            });
            state.rlm.total_chunks = state.rlm.chunks.len();
        }
        RlmEventType::ExtractionComplete { chunk_id, findings, relevance } => {
            if let Some(chunk) = state.rlm.chunks.get_mut(*chunk_id) {
                chunk.findings = Some(findings.clone());
                chunk.status = RlmStepStatus::Complete;
            }
            state.rlm.processed_chunks += 1;
            state.rlm.streaming_text = format!("Extracted: {}", findings.chars().take(80).collect::<String>());
        }
        RlmEventType::SynthesisComplete { answer, confidence } => {
            state.rlm.final_answer = Some(answer.clone());
            state.rlm.streaming_text.clear();
        }
        RlmEventType::Complete => {
            state.rlm.current_phase = RlmPhase::Complete;
            state.rlm.connection_status = RlmConnectionStatus::Complete;
        }
    }
}
```

### Phase 4: Load Trace on Page Init

In `crates/web/client/src/app.rs` or route handler:

```rust
// On /rlm page load, fetch and parse the trace
let trace_json = include_str!("../assets/rlm-demo-trace.json");
let trace: RlmDemoTrace = serde_json::from_str(trace_json).unwrap();
state.rlm.trace = Some(trace);
state.rlm.auto_restart = true;
// Start playback
state.rlm.trace_start_time = performance_now();
```

### Phase 5: Update UI Labels

Update phase labels to match DSPy terminology:

| Old | New |
|-----|-----|
| Structure Discovery | Routing |
| Chunking | Chunking |
| Extraction | Extraction (CoT) |
| Synthesis | Reduction + Verification |

### Phase 6: Generate the Trace File

Create `crates/rlm/examples/generate_demo_trace.rs`:

```rust
use rlm::{DspyOrchestrator, DspyOrchestratorConfig, configure_dspy_lm};
use serde_json::json;
use std::fs;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure LM
    configure_dspy_lm("openai:gpt-4o-mini", None, None).await?;

    // Load document
    let document = fs::read_to_string("docs/rlm/git.md")?;
    let query = "What are the main components of the Repo-RLM runtime spec?";

    // Run with verbose to capture events
    let config = DspyOrchestratorConfig {
        verbose: true,
        use_cot_extraction: true,
        verify_answer: false,
        max_chunks: 8,
        ..Default::default()
    };

    let orchestrator = DspyOrchestrator::with_config(config);
    let result = orchestrator.analyze(query, &document).await?;

    // Build trace from result
    let trace = build_trace(query, &document, &result);

    // Save
    fs::write(
        "crates/web/client/assets/rlm-demo-trace.json",
        serde_json::to_string_pretty(&trace)?
    )?;

    println!("Trace saved with {} chunks", result.chunks_processed);
    Ok(())
}
```

## File Changes Summary

| File | Change |
|------|--------|
| `crates/web/client/src/state.rs` | Add trace types, update RlmVizState |
| `crates/web/client/src/views/rlm.rs` | Replace `tick_demo()` with trace playback |
| `crates/web/client/src/app.rs` | Load trace on page init |
| `crates/web/client/assets/rlm-demo-trace.json` | **NEW** - Pre-recorded trace |
| `crates/rlm/examples/generate_demo_trace.rs` | **NEW** - Trace generator |
| `crates/web/docs/rlm-visualization.md` | Update docs |

## Timeline Estimates

This plan requires ~500 lines of code changes across 5 files.

## Alternatives Considered

### A. Live Backend Service
- **Pros**: Real-time, interactive
- **Cons**: Needs external server, Python/GPU, API costs
- **Verdict**: Too complex for demo, save for production

### B. WASM-only DSPy
- **Pros**: Runs in browser
- **Cons**: dspy-rs has async/network deps, heavy WASM
- **Verdict**: Not feasible without major work

### C. Multiple Pre-recorded Traces
- **Pros**: Show variety
- **Cons**: More generation work
- **Verdict**: Start with one, add more later

## Success Criteria

1. Demo auto-starts on `/rlm` page load
2. Shows 6-8 real chunks from git.md analysis
3. Displays actual extracted findings (not lorem ipsum)
4. Shows real synthesized answer
5. Loops continuously without user interaction
6. Timing feels natural (~15-20 seconds per cycle)
