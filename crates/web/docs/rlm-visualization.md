# RLM Execution Visualization

Interactive visualization page showing Recursive Language Model (RLM) execution as an "execution movie" at `/rlm`.

## Overview

The RLM visualization page displays the four-phase execution pipeline of Recursive Language Models:

1. **Structure Discovery** - Analyzing document structure to understand hierarchy and sections
2. **Chunking** - Splitting the document into semantic chunks based on structure
3. **Extraction** - Processing each chunk through LLM to extract relevant information
4. **Synthesis** - Combining extracted information into a final answer

The page provides real-time visualization of this pipeline with a timeline, phase indicators, chunk grid, and streaming text display.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ RLM EXECUTION VISUALIZER                                        [READY] │
├─────────────────────────────────────────────────────────────────────────────┤
│ Query: [_______________________________________]                    [RUN] │
│ Document: [multiline textarea for document context...]                    │
├─────────────────────────────────────────────────────────────────────────────┤
│ TIMELINE: [Structure]──[Chunking]──[Extraction]──[Synthesis]              │
│           [====●====]─────────────────────────────────────────            │
│ Phase: Structure Discovery                        Chunks: 0/12            │
├────────────────────────────┬────────────────────────────────────────────────┤
│ EXECUTION PHASES           │ DETAIL VIEW                                    │
│                            │                                                │
│ > Structure Discovery      │ CHUNK 5 DETAIL                                 │
│   Analyzing document...    │                                                │
│                            │ Section: ## Methods                            │
│ ○ Chunking (12)            │ "The model uses attention mechanisms..."       │
│                            │                                                │
│ ○ Extraction (0/12)        │ Findings:                                      │
│   [□□□□□□□□□□□□]           │ "Uses multi-head attention with 8 heads..."    │
│                            │                                                │
│ ○ Synthesis                │ LLM Response:                                  │
│                            │ "Based on this section, the key insight is..." │
├────────────────────────────┴────────────────────────────────────────────────┤
│ FINAL ANSWER (when complete)                                               │
│ "Based on analysis of all 12 sections, the document describes..."          │
└─────────────────────────────────────────────────────────────────────────────┘
```

## File Structure

```
crates/web/
├── worker/src/routes/
│   ├── mod.rs              # Route module declarations
│   └── rlm.rs              # Route handler serving /rlm page
├── client/src/
│   ├── state.rs            # RlmVizState and related types
│   ├── app.rs              # Flag detection, event handling, render dispatch
│   └── views/
│       ├── mod.rs          # View module exports
│       └── rlm.rs          # Main visualization rendering
```

## State Management

### RlmVizState

Located in `crates/web/client/src/state.rs`:

```rust
pub(crate) struct RlmVizState {
    // Frame/scroll (standard WGPUI pattern)
    pub(crate) frame_animator: FrameAnimator,
    pub(crate) frame_started: bool,
    pub(crate) scroll_offset: f32,
    pub(crate) content_bounds: Bounds,
    pub(crate) content_height: f32,

    // Connection status
    pub(crate) connection_status: RlmConnectionStatus,
    pub(crate) run_id: Option<String>,

    // Input fields
    pub(crate) query_input: TextInput,
    pub(crate) query_input_bounds: Bounds,
    pub(crate) context_input: TextInput,
    pub(crate) context_input_bounds: Bounds,
    pub(crate) run_button_bounds: Bounds,
    pub(crate) run_button_hovered: bool,

    // Execution state
    pub(crate) current_phase: RlmPhase,
    pub(crate) iterations: Vec<RlmIteration>,
    pub(crate) chunks: Vec<RlmChunkState>,
    pub(crate) total_chunks: usize,
    pub(crate) processed_chunks: usize,
    pub(crate) active_chunk_id: Option<usize>,
    pub(crate) final_answer: Option<String>,
    pub(crate) streaming_text: String,

    // Timeline
    pub(crate) timeline_events: Vec<RlmTimelineEvent>,
    pub(crate) timeline_position: f32,
    pub(crate) timeline_slider_bounds: Bounds,

    // Error handling
    pub(crate) error: Option<String>,

    // Demo mode
    pub(crate) demo_mode: bool,
    pub(crate) demo_phase_idx: usize,
    pub(crate) demo_chunk_idx: usize,
    pub(crate) demo_last_tick: u64,
}
```

### Supporting Types

```rust
pub(crate) enum RlmConnectionStatus {
    Idle,       // Ready to start
    Connecting, // Establishing SSE connection
    Streaming,  // Receiving events
    Complete,   // Execution finished
    Error,      // Error occurred
}

pub(crate) enum RlmPhase {
    Idle,               // Not started
    StructureDiscovery, // Analyzing document
    Chunking,           // Splitting into chunks
    Extraction,         // Processing chunks
    Synthesis,          // Combining results
    Complete,           // Finished
}

pub(crate) enum RlmStepStatus {
    Pending,    // Not yet processed
    Processing, // Currently being processed
    Complete,   // Finished successfully
    Error,      // Failed
}

pub(crate) struct RlmChunkState {
    pub(crate) chunk_id: usize,
    pub(crate) section_title: Option<String>,
    pub(crate) content_preview: Option<String>,
    pub(crate) findings: Option<String>,
    pub(crate) status: RlmStepStatus,
}
```

## View Components

### Header (`draw_header`)
- Title: "RLM EXECUTION VISUALIZER"
- Status badge showing connection state with color coding
- Subtitle with description

### Input Section (`draw_input_section`)
- Query text input for user questions
- Document context textarea for pasting source material
- RUN/STOP button with hover effects

### Timeline (`draw_timeline`)
- Horizontal progress bar showing all four phases
- Phase dots indicating current position
- Current phase label and chunk progress counter

### Phases Panel (`draw_phases_panel`)
- Vertical list of all phases with status icons
- `>` for current phase (orange)
- `=` for completed phases (green)
- ` ` for pending phases (gray)
- Chunk grid visualization during Extraction phase

### Chunk Grid (`draw_chunk_grid`)
- Grid of small squares representing each chunk
- Color-coded by status:
  - Gray: Pending
  - Orange: Processing
  - Green: Complete
  - Red: Error

### Detail Panel (`draw_detail_panel`)
- Shows current chunk information during extraction
- Section title and content preview
- Extracted findings
- Streaming LLM response
- Final answer display when complete

## Event Handling

### Mouse Events

In `app.rs`:

```rust
// Mouse move - hover detection
if state.view == AppView::RlmPage {
    handle_rlm_mouse_move(&mut state, x, y);
}

// Mouse click - button handling
AppView::RlmPage => {
    handle_rlm_click(&mut state, click_pos.x, click_pos.y);
}
```

### Scroll Events

```rust
if state.view == AppView::RlmPage {
    if state.rlm.content_bounds.contains(point) {
        state.rlm.scroll_offset += delta;
        let max_scroll = (state.rlm.content_height - state.rlm.content_bounds.size.height).max(0.0);
        state.rlm.scroll_offset = state.rlm.scroll_offset.clamp(0.0, max_scroll);
        return;
    }
}
```

### Input Events

Input handling is delegated to `RlmVizState::handle_event()`:

```rust
pub(crate) fn handle_event(&mut self, event: &InputEvent) -> EventResult {
    let mut handled = EventResult::Ignored;
    handled = merge_event_result(
        handled,
        self.query_input.event(event, self.query_input_bounds, &mut self.input_event_ctx),
    );
    handled = merge_event_result(
        handled,
        self.context_input.event(event, self.context_input_bounds, &mut self.input_event_ctx),
    );
    handled
}
```

## Demo Mode

The visualization includes a demo mode for testing without a backend connection. When the RUN button is clicked and no SSE endpoint is configured, it enters demo mode:

```rust
pub(crate) fn handle_rlm_click(state: &mut AppState, x: f32, y: f32) -> bool {
    if state.rlm.run_button_bounds.contains(point) {
        if state.rlm.connection_status == RlmConnectionStatus::Streaming {
            // Stop execution
            state.rlm.connection_status = RlmConnectionStatus::Idle;
            state.rlm.demo_mode = false;
        } else {
            // Start demo mode
            state.rlm.reset_execution();
            state.rlm.demo_mode = true;
            state.rlm.demo_last_tick = 0;
        }
        return true;
    }
    false
}
```

Demo mode progresses through phases every 2 seconds:
1. Starts at Structure Discovery
2. Moves to Chunking, populates 12 demo chunks
3. Processes chunks one by one in Extraction phase
4. Moves to Synthesis
5. Displays final answer
6. Resets and can restart

## SSE Event Schema (Future Backend Integration)

When connected to a real RLM backend, the page expects Server-Sent Events with this JSON schema:

```json
// Phase start
{"type": "phase_start", "phase": "chunking", "total_chunks": 45}

// Chunk processing start
{"type": "chunk_start", "chunk_id": 12, "section_title": "## Methods"}

// Chunk result
{"type": "chunk_result", "chunk_id": 12, "findings": "Uses attention..."}

// Streaming token
{"type": "token", "token": "Based"}

// Final answer
{"type": "final", "answer": "...", "processing_time_ms": 12400}

// Error
{"type": "error", "error": "Connection lost"}
```

## Color Scheme

Consistent with other visualization pages:

| State | Color | Hsla |
|-------|-------|------|
| Pending | Gray | `Hsla::new(0.0, 0.0, 0.4, 1.0)` |
| Processing | Orange | `#FF9900` / `Hsla::from_hex(0xff9900)` |
| Complete | Green | `#00FF88` / `Hsla::from_hex(0x00ff88)` |
| Error | Red | `#FF4444` / `Hsla::from_hex(0xff4444)` |
| Accent | Cyan | `#7FD3E5` / `Hsla::from_hex(0x7fd3e5)` |

## Route Configuration

The route is configured in `crates/web/worker/src/lib.rs`:

```rust
(Method::Get, "/rlm") => routes::rlm::view_rlm(env).await,
```

The route handler in `routes/rlm.rs` serves HTML with the `window.RLM_PAGE = true` flag, which is detected in `app.rs`:

```rust
let is_rlm_page = web_sys::window()
    .and_then(|w| js_sys::Reflect::get(&w, &"RLM_PAGE".into()).ok())
    .map(|v| v.is_truthy())
    .unwrap_or(false);

if is_rlm_page {
    let mut state_guard = state.borrow_mut();
    state_guard.loading = false;
    state_guard.view = AppView::RlmPage;
    drop(state_guard);
}
```

## Future Enhancements

1. **Real SSE Backend Integration**
   - Create `rlm_runtime.rs` module for EventSource handling
   - Connect to `/api/rlm/run` endpoint
   - Parse and dispatch SSE events to state updates

2. **Timeline Scrubbing**
   - Enable dragging on timeline to replay past events
   - Store all events with timestamps for playback

3. **Chunk Hover Details**
   - Show tooltip on chunk grid hover
   - Quick preview of chunk content and findings

4. **Export Results**
   - Copy final answer to clipboard
   - Export full execution trace as JSON

5. **Multiple Documents**
   - Support for comparing RLM execution across documents
   - Side-by-side visualization

## Usage

1. Navigate to `https://openagents.com/rlm`
2. Enter a query in the Query field (e.g., "What are the main contributions?")
3. Paste document content in the Document textarea
4. Click RUN to start (currently triggers demo mode)
5. Watch the execution progress through all phases
6. View the final answer when synthesis completes

## Related Documentation

- [RLM Paper Synopsis](/docs/frlm/RLM_PAPER_SYNOPSIS.md) - Overview of Recursive Language Models
- [FRLM Paper](/docs/frlm/paper.md) - Federated RLM extension
- [Client UI Architecture](/crates/web/docs/client-ui.md) - General UI patterns
- [WGPUI Components](/crates/wgpui/README.md) - GPU rendering framework
