# Mana Tap: DSPy Chain Visualizer Spec

## Overview

Mana Tap is a visual demonstration of DSPy signature chains executing in real-time. It renders a flowing visualization of signatures as they process a user prompt, showing inputs, outputs, and execution state.

**Demo Prompt:** `"Summarize the markdown files in the root level of this repository."`

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              MANA TAP                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐                   │
│   │   wgpui     │────▶│   dsrs      │────▶│   gpt-oss   │                   │
│   │  (render)   │     │  (chain)    │     │  (llama)    │                   │
│   └─────────────┘     └─────────────┘     └─────────────┘                   │
│         │                   │                   │                            │
│         │                   │                   │                            │
│         ▼                   ▼                   ▼                            │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                     CHAIN VISUALIZATION                              │   │
│   │                                                                      │   │
│   │   [TaskAnalysis] ──▶ [FileDiscovery] ──▶ [ContentSummarizer] ──▶ ...│   │
│   │        │                   │                    │                    │   │
│   │   {inputs}            {inputs}             {inputs}                  │   │
│   │   {outputs}           {outputs}            {outputs}                 │   │
│   │                                                                      │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## LLM Backend Setup

Following the pattern from `crates/arrow/`, Mana Tap auto-starts a local llama-server:

### Initialization Flow

```rust
use gpt_oss::{GptOssClient, LlamaServerManager};

async fn init_llm() -> Result<(GptOssClient, Option<LlamaServerManager>)> {
    let client = GptOssClient::new();

    // Check if server already running
    if client.health().await.is_ok() {
        return Ok((client, None));
    }

    // Auto-start llama-server
    let mut manager = LlamaServerManager::builder()
        .port(8000)
        .build()?;

    manager.start()?;
    manager.wait_ready_timeout(Duration::from_secs(30)).await?;

    Ok((client, Some(manager)))
}
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `LLAMA_MODEL_PATH` | Path to GGUF model | Auto-discover |
| `GPTOSS_BASE_URL` | llama-server URL | `http://localhost:8000` |
| `MANATAP_AUTO_START` | Auto-start server | `true` |

## Signature Chain for Markdown Summarization

The demo task `"Summarize the markdown files in the root level of this repository."` executes through 5 signatures:

```
┌────────────────┐     ┌────────────────┐     ┌────────────────┐
│ TaskAnalysis   │────▶│ FileDiscovery  │────▶│ ContentReader  │
│                │     │                │     │                │
│ IN:  prompt    │     │ IN:  pattern   │     │ IN:  paths     │
│ OUT: intent,   │     │      repo_root │     │ OUT: contents  │
│      file_pat  │     │ OUT: paths     │     │      metadata  │
└────────────────┘     └────────────────┘     └────────────────┘
                                                      │
                       ┌──────────────────────────────┘
                       ▼
┌────────────────┐     ┌────────────────┐
│ ChunkSummarize │────▶│ Aggregate      │
│                │     │                │
│ IN:  content   │     │ IN:  summaries │
│      filename  │     │      intent    │
│ OUT: summary   │     │ OUT: final     │
│      key_pts   │     │      structure │
└────────────────┘     └────────────────┘
```

---

## Signature Definitions

### 1. TaskAnalysisSignature (EXISTING - adapt from ChunkTaskSelector)

Parses the user prompt to understand intent and extract parameters.

```rust
#[Signature]
pub struct TaskAnalysisSignature {
    /// The user's natural language request
    #[input]
    pub prompt: String,

    /// Detected task type: "summarize" | "search" | "analyze" | "modify"
    #[output]
    pub task_type: String,

    /// File pattern to target (e.g., "*.md", "src/**/*.rs")
    #[output]
    pub file_pattern: String,

    /// Scope: "root" | "recursive" | "specific_path"
    #[output]
    pub scope: String,

    /// What to do with results: "summarize" | "list" | "aggregate"
    #[output]
    pub output_action: String,

    /// Confidence in interpretation (0.0-1.0)
    #[output]
    pub confidence: f32,
}

impl TaskAnalysisSignature {
    pub fn instruction() -> &'static str {
        r#"Analyze the user's request to understand:
1. What type of task they want (summarize, search, analyze, modify)
2. Which files they're targeting (extract glob pattern)
3. The scope (root level only, recursive, or specific path)
4. What to do with results

Be precise about file patterns. "root level" means no subdirectories."#
    }
}
```

**Example I/O:**
```
Input:
  prompt: "Summarize the markdown files in the root level of this repository."

Output:
  task_type: "summarize"
  file_pattern: "*.md"
  scope: "root"
  output_action: "summarize"
  confidence: 0.95
```

---

### 2. FileDiscoverySignature (NEW)

Discovers files matching a pattern within scope.

```rust
#[Signature]
pub struct FileDiscoverySignature {
    /// Glob pattern for files (e.g., "*.md", "src/**/*.rs")
    #[input]
    pub pattern: String,

    /// Scope constraint: "root" | "recursive" | path
    #[input]
    pub scope: String,

    /// Repository or directory root path
    #[input]
    pub root_path: String,

    /// JSON array of discovered file paths
    #[output]
    pub paths: String,

    /// Number of files found
    #[output]
    pub count: i32,

    /// Any issues encountered (empty if none)
    #[output]
    pub issues: String,
}

impl FileDiscoverySignature {
    pub fn instruction() -> &'static str {
        r#"Given a file pattern and scope, determine which files exist.

For scope="root", only include files directly in the root directory.
For scope="recursive", include all matching files in subdirectories.

Output paths as a JSON array of relative paths from root."#
    }
}
```

**Note:** This signature is typically executed via tool call (glob/ripgrep), but the signature defines the contract for LLM-based discovery when tools aren't available.

**Example I/O:**
```
Input:
  pattern: "*.md"
  scope: "root"
  root_path: "/home/user/code/openagents"

Output:
  paths: '["README.md", "CHANGELOG.md", "CONTRIBUTING.md", "LICENSE.md"]'
  count: 4
  issues: ""
```

---

### 3. ContentReaderSignature (NEW)

Reads and prepares file contents for processing.

```rust
#[Signature]
pub struct ContentReaderSignature {
    /// JSON array of file paths to read
    #[input]
    pub paths: String,

    /// Maximum characters per file (0 = unlimited)
    #[input]
    pub max_chars_per_file: i32,

    /// JSON array of {path, content, size, truncated} objects
    #[output]
    pub contents: String,

    /// Total size in characters
    #[output]
    pub total_size: i32,

    /// Files that couldn't be read
    #[output]
    pub failed_paths: String,
}

impl ContentReaderSignature {
    pub fn instruction() -> &'static str {
        r#"Read the contents of the specified files.

For each file, return an object with:
- path: the file path
- content: the file contents (truncated if exceeds max_chars_per_file)
- size: character count
- truncated: boolean indicating if content was cut

Return as JSON array."#
    }
}
```

**Note:** In practice, this is executed via file_read tool. The signature exists for tracing and contract definition.

---

### 4. ContentSummarizerSignature (NEW)

Summarizes a single piece of content.

```rust
#[Signature]
pub struct ContentSummarizerSignature {
    /// The content to summarize
    #[input]
    pub content: String,

    /// Source filename for context
    #[input]
    pub filename: String,

    /// Content type hint: "markdown" | "code" | "text" | "config"
    #[input]
    pub content_type: String,

    /// Target summary length: "brief" | "moderate" | "detailed"
    #[input]
    pub length: String,

    /// One-paragraph summary
    #[output]
    pub summary: String,

    /// JSON array of key points (3-5 items)
    #[output]
    pub key_points: String,

    /// Primary topic/theme
    #[output]
    pub topic: String,

    /// Detected sections/headings
    #[output]
    pub sections: String,
}

impl ContentSummarizerSignature {
    pub fn instruction() -> &'static str {
        r#"Summarize the provided content concisely.

For markdown files:
- Extract the main purpose/topic
- Identify key sections from headings
- Pull out 3-5 most important points
- Write a coherent one-paragraph summary

Adjust detail level based on the 'length' parameter:
- brief: 1-2 sentences, top 3 points
- moderate: 3-4 sentences, top 5 points
- detailed: full paragraph, all significant points"#
    }
}
```

**Example I/O:**
```
Input:
  content: "# OpenAgents\n\nOpenAgents is a framework for building..."
  filename: "README.md"
  content_type: "markdown"
  length: "moderate"

Output:
  summary: "OpenAgents is a Rust-based framework for building autonomous coding agents. It provides DSPy integration for declarative AI programming, multi-provider LLM support, and tools for code analysis and modification."
  key_points: '["Rust-based agent framework", "DSPy compiler layer", "Multi-provider LLM support", "Autonomous coding workflows"]'
  topic: "Agent Framework Documentation"
  sections: '["Overview", "Installation", "Quick Start", "Architecture"]'
```

---

### 5. SummaryAggregatorSignature (NEW - similar to ChunkAnalysisToActionSignature)

Combines multiple summaries into a coherent final output.

```rust
#[Signature]
pub struct SummaryAggregatorSignature {
    /// JSON array of {filename, summary, key_points, topic} objects
    #[input]
    pub summaries: String,

    /// Original user intent/request
    #[input]
    pub original_request: String,

    /// Output format: "narrative" | "bullet_points" | "structured"
    #[input]
    pub format: String,

    /// Combined summary addressing the user's request
    #[output]
    pub final_summary: String,

    /// Grouped themes across all files
    #[output]
    pub themes: String,

    /// Overall key takeaways (5-7 points)
    #[output]
    pub key_takeaways: String,

    /// Suggested next steps or related files
    #[output]
    pub suggestions: String,
}

impl SummaryAggregatorSignature {
    pub fn instruction() -> &'static str {
        r#"Synthesize multiple file summaries into a coherent response.

1. Identify common themes across files
2. Group related information logically
3. Write a final summary that directly addresses the user's original request
4. Extract 5-7 key takeaways spanning all files
5. Suggest logical next steps

Format the output according to the 'format' parameter:
- narrative: flowing prose paragraphs
- bullet_points: hierarchical bullet list
- structured: JSON with clear sections"#
    }
}
```

**Example I/O:**
```
Input:
  summaries: '[{"filename": "README.md", "summary": "...", ...}, ...]'
  original_request: "Summarize the markdown files in the root level"
  format: "narrative"

Output:
  final_summary: "The root-level markdown files document the OpenAgents project..."
  themes: '["Project Overview", "Contributing Guidelines", "Licensing", "Change History"]'
  key_takeaways: '["OpenAgents is a Rust agent framework", "MIT licensed", ...]'
  suggestions: '["Explore crates/dsrs for DSPy details", "See docs/ for architecture"]'
```

---

## Visual Chain Representation

### Node States

Each signature node displays one of these states:

| State | Color | Description |
|-------|-------|-------------|
| `pending` | Gray | Not yet started |
| `running` | Blue (pulsing) | Currently executing |
| `complete` | Green | Finished successfully |
| `failed` | Red | Error occurred |
| `skipped` | Yellow | Bypassed (cached/unnecessary) |

### Node Layout

```
┌─────────────────────────────────────────┐
│  TaskAnalysisSignature          [state] │
├─────────────────────────────────────────┤
│  INPUTS:                                │
│    prompt: "Summarize the markdown..."  │
├─────────────────────────────────────────┤
│  OUTPUTS:                               │
│    task_type: "summarize"               │
│    file_pattern: "*.md"                 │
│    scope: "root"                        │
├─────────────────────────────────────────┤
│  tokens: 127 | cost: 0 msats | 1.2s     │
└─────────────────────────────────────────┘
          │
          ▼
```

### Animation Flow

1. **Initial State**: All nodes gray, prompt shown at top
2. **Execution Start**: First node pulses blue
3. **Node Complete**: Node turns green, outputs populate, arrow animates to next
4. **Chain Progress**: Each node transitions in sequence
5. **Final State**: All nodes green, final output highlighted

---

## Implementation Phases

### Phase 1: Static Chain Display
- Render 5 signature nodes in a vertical chain
- Show mock inputs/outputs
- Static state colors

### Phase 2: DSPy Integration
- Wire up dsrs with GPT-OSS provider
- Execute real signatures
- Use DspyCallback to update node states

### Phase 3: Animation
- Pulse animation for running nodes
- Fade-in for outputs as they arrive
- Connecting line animations

### Phase 4: Interactivity
- Click nodes to expand full I/O
- Hover for token/cost details
- Replay chain from any point

---

## Dependencies

```toml
[dependencies]
wgpui = { path = "../wgpui", default-features = false, features = ["desktop"] }
dsrs = { path = "../dsrs" }
gpt-oss = { path = "../gpt-oss" }
wgpu = { workspace = true }
winit = { workspace = true }
pollster = { workspace = true }
tokio = { workspace = true, features = ["rt-multi-thread", "sync", "time"] }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
```

---

## Chain Execution Code

```rust
use dsrs::prelude::*;
use dsrs::predictors::Predict;

pub struct MarkdownSummarizationChain {
    lm: Arc<LM>,
    callback: Arc<dyn DspyCallback>,
}

impl MarkdownSummarizationChain {
    pub async fn execute(&self, prompt: &str, repo_root: &str) -> Result<ChainResult> {
        // Stage 1: Task Analysis
        let task = self.run_task_analysis(prompt).await?;

        // Stage 2: File Discovery (via tool or LLM)
        let files = self.run_file_discovery(
            &task.file_pattern,
            &task.scope,
            repo_root,
        ).await?;

        // Stage 3: Content Reading (via tool)
        let contents = self.run_content_reader(&files.paths).await?;

        // Stage 4: Summarize each file
        let mut summaries = Vec::new();
        for file in contents.files {
            let summary = self.run_content_summarizer(
                &file.content,
                &file.path,
                "markdown",
                "moderate",
            ).await?;
            summaries.push(summary);
        }

        // Stage 5: Aggregate
        let final_result = self.run_summary_aggregator(
            &summaries,
            prompt,
            "narrative",
        ).await?;

        Ok(final_result)
    }
}
```

---

## Callback Integration for Visualization

```rust
pub struct VisualizerCallback {
    sender: mpsc::Sender<ChainEvent>,
}

impl DspyCallback for VisualizerCallback {
    fn on_module_start(&self, call_id: Uuid, module_name: &str, inputs: &Example) {
        let _ = self.sender.try_send(ChainEvent::NodeStarted {
            id: call_id,
            name: module_name.to_string(),
            inputs: inputs.clone(),
        });
    }

    fn on_module_end(&self, call_id: Uuid, result: Result<&Prediction, &Error>) {
        let _ = self.sender.try_send(ChainEvent::NodeCompleted {
            id: call_id,
            success: result.is_ok(),
            outputs: result.ok().cloned(),
            usage: result.ok().map(|p| p.lm_usage.clone()),
        });
    }
}
```

---

## File Locations

```
crates/manatap/
├── Cargo.toml
├── docs/
│   └── CHAIN_VISUALIZER_SPEC.md    # This file
└── src/
    ├── main.rs                      # Window + event loop
    ├── chain/
    │   ├── mod.rs                   # Chain orchestration
    │   ├── signatures.rs            # Signature definitions
    │   └── executor.rs              # DSPy execution wrapper
    ├── render/
    │   ├── mod.rs                   # Rendering coordination
    │   ├── node.rs                  # Signature node rendering
    │   ├── connector.rs             # Arrow/line rendering
    │   └── animation.rs             # State transitions
    └── llm/
        ├── mod.rs                   # LLM setup
        └── server.rs                # llama-server management
```

---

## Summary of Signatures

| # | Signature | Status | Description |
|---|-----------|--------|-------------|
| 1 | TaskAnalysisSignature | NEW | Parse user intent, extract file pattern/scope |
| 2 | FileDiscoverySignature | NEW | Find files matching pattern in scope |
| 3 | ContentReaderSignature | NEW | Read file contents (usually tool-based) |
| 4 | ContentSummarizerSignature | NEW | Summarize single file content |
| 5 | SummaryAggregatorSignature | NEW | Combine summaries into final output |

**Existing signatures that could be reused/adapted:**
- `ChunkTaskSelectorSignature` → basis for TaskAnalysisSignature
- `ChunkAnalysisToActionSignature` → basis for SummaryAggregatorSignature
- `ToolCallSignature` → for file_read/glob tool invocation
- `ToolResultSignature` → for interpreting tool outputs

---

## Future Extensions

1. **Multi-Chain Comparison**: Run same prompt through different signature orderings
2. **Optimization Visualization**: Show MIPROv2/GEPA improving signatures over time
3. **Cost Dashboard**: Real-time token/msat accumulation display
4. **Branch Visualization**: Show parallel signature execution (e.g., summarize multiple files simultaneously)
5. **Trace Export**: Save chain execution to Nostr for sharing/replay
