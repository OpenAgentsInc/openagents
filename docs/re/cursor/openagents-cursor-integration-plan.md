# OpenAgents × Cursor: 10x Better AI Coding Assistant Integration Plan

**Version:** 2.1
**Date:** 2025-01-13
**Status:** Planning
**Goal:** Build a 10x better coding assistant than Cursor through hybrid local/swarm/cloud inference, desktop+mobile sync, and open marketplace

**v2.1 Update:** Replaced libp2p/Kademlia P2P networking with Nostr DVMs (Data Vending Machines) for decentralized compute marketplace. Uses NIP-90 for job requests, NIP-89 for provider discovery, and NIP-57 for Lightning payments (Zaps).

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [The 10x Vision](#the-10x-vision)
3. [Feature Mapping: Cursor → OpenAgents](#feature-mapping-cursor--openagents)
4. [Architecture Overview](#architecture-overview)
5. [Core Systems Design](#core-systems-design)
6. [Hybrid Inference System](#hybrid-inference-system)
7. [Technology Stack](#technology-stack)
8. [Implementation Roadmap](#implementation-roadmap)
9. [Integration with Existing Systems](#integration-with-existing-systems)
10. [Compute Marketplace & Revenue Sharing](#compute-marketplace--revenue-sharing)
11. [Privacy & Security](#privacy--security)
12. [Performance Targets](#performance-targets)
13. [Open Questions & Decisions](#open-questions--decisions)

---

## Executive Summary

### Vision

Build a **10x better coding assistant** than Cursor by combining:
- **Cursor's best features**: shadow workspace, codebase indexing, intelligent context, fast apply
- **Hybrid inference**: Local (llama.cpp/MLX) + Swarm (peer compute) + Cloud (Codex, Claude Code, Groq)
- **Cross-platform**: Desktop (Tauri) + Mobile (iOS/Android) with real-time sync
- **Agent orchestration**: Delegate to sub-agents (Codex, Claude Code) in single conversation
- **Open marketplace**: Revenue sharing for agent plugins, tools, and compute
- **Desktop-first UX**: ChatGPT-style sidebar, no janky TUI, proper history

### Key Differentiators from Cursor

| Aspect | Cursor | OpenAgents (10x Vision) |
|--------|--------|----------------------|
| **Inference** | Cloud-only (Fireworks, OpenAI) | **Hybrid: Local + Swarm + Cloud** |
| **Platforms** | Desktop only | **Desktop + Mobile (sync)** |
| **Agent Model** | Single cloud agent | **Orchestrator + sub-agents (Codex, Claude Code, etc.)** |
| **Compute** | Rent from Cursor | **Use local, sell excess, buy from swarm** |
| **Extensibility** | MCP (complex setup) | **Marketplace with revenue sharing** |
| **Privacy** | Privacy mode (still cloud) | **Flexible: local-only OR cloud opt-in** |
| **Cost** | $20/mo subscription | **Free local + pay-as-you-go swarm/cloud** |
| **Open Source** | Closed | **Fully open + community-driven** |
| **Long-running tasks** | Manual "continue" spam | **Scheduled prompts, overnight coding** |
| **History** | Basic | **SQLite-backed, semantic search, cross-chat retrieval** |

### Success Criteria

- **10x better UX**: Desktop + mobile, ChatGPT-style sidebar, no TUI jank
- **10x more flexible**: Hybrid inference (local/swarm/cloud), user chooses cost/quality trade-off
- **10x more extensible**: Open marketplace with revenue sharing, anyone can contribute
- **10x more powerful**: Agent orchestration (delegate to Codex, Claude Code, etc.)
- **10x more efficient**: Use local compute when possible, sell excess compute
- **10x more private**: Local-first option, but cloud available when user wants it
- **Better than Cursor** on ALL dimensions: speed, quality, cost, privacy, extensibility

---

## The 10x Vision

### 1. Ditch the TUI ✅

**Problem:** Janky terminal UIs with screen flicker, broken copy/paste, fake terminal aesthetics

**Solution:** ChatGPT-style desktop app
- Sidebar with conversation history
- Rich UI components (not fake terminal)
- Proper widgets for long-running agents
- Monaco editor for code diffs
- Real-time streaming updates

**Already Built:** ✅ Tauri desktop app with sidebar, assistant-ui components

### 2. Go Mobile 📱

**Problem:** Can't code from phone, cloud-based editors suck on mobile

**Solution:** Native mobile apps (iOS/Android) synced to desktop
- Same UI components (React Native or Tauri mobile when ready)
- Real-time sync via WebSocket (already have tinyvex-ws)
- Full feature parity: completions, chat, diffs, approvals
- Work from coffee shop, code from couch

**Status:** iOS bridge already exists, needs completion UI

### 3. Code Overnight 🌙

**Problem:** Can't let agents work while you sleep (Codex requires manual "continue" spam)

**Solution:** Scheduled prompts + autonomous agents
- `schedule("in 3 hours", "add tests to auth module")`
- `schedule("after rate limit resets", "continue refactor")`
- Agent discretion: check what's done, advance toward objective
- Cron-like orchestration with sub-agents

**Implementation:**
```rust
pub struct ScheduledPrompt {
    id: Uuid,
    trigger: Trigger,
    prompt: String,
    agent: AgentSpec,
    dependencies: Vec<Uuid>,  // wait for other prompts
}

pub enum Trigger {
    Delay(Duration),          // "in 3 hours"
    Timestamp(DateTime),      // "at 2am"
    Conditional(Box<dyn Fn() -> bool>),  // "when tests pass"
    RateLimitReset(Provider), // "after OpenAI limit resets"
}
```

### 4. CLI Agents as Sub-Agents 🤖

**Problem:** Can't mix different agents in one conversation

**Solution:** Orchestrator model delegates to specialized agents
- Main chat is a router/orchestrator
- Delegate to Codex for Python tasks
- Delegate to Claude Code for Rust
- Delegate to local model for simple completions
- All in ONE conversation thread with shared context

**Architecture:**
```
User: "Refactor this Python module to use async/await"
  ↓
Orchestrator (local Foundation Model):
  → Tool call: delegate_to_codex(task: "refactor to async", context: [...])
  ↓
Codex Agent (cloud):
  → Generates refactor plan
  → Asks orchestrator: search_codebase("current async patterns")
  ↓
Orchestrator:
  → Uses local indexer → returns results
  ↓
Codex:
  → Completes refactor
  ↓
Orchestrator:
  → Validates in shadow workspace (local)
  → Shows diff to user
```

**Already Built:** ✅ ACP session manager can route to multiple agents

### 5. History and Memory 🧠

**Problem:** Can't search past conversations, no persistent memory

**Solution:** SQLite-backed conversation store with semantic search
- All conversations in tinyvex (already exists)
- Cross-chat retrieval: "In that conversation about auth, we decided..."
- Semantic search over history (embed conversations)
- Timeline view, tag system, favorites

**Enhancement:**
```sql
-- Extend tinyvex
CREATE TABLE conversation_embeddings (
    thread_id TEXT PRIMARY KEY,
    title TEXT,
    summary TEXT,
    embedding BLOB,  -- via sqlite-vec
    created_at INTEGER,
    tags JSON
);

-- Semantic search
SELECT thread_id, title, similarity
FROM conversation_embeddings
WHERE vec_search(embedding, query_vec, k=10);
```

### 6. Hassle-Free Interop 🔌

**Problem:** MCP setup is painful (tool registry too long, need API wrapper scripts)

**Solution:** Built-in marketplace with one-click installs
- Browse agent plugins in UI
- One-click install (downloads, configures, ready to use)
- No manual JSON editing, no script writing
- Revenue sharing incentivizes quality plugins

**Example:**
```
User clicks "Install Figma Plugin"
  ↓
1. Download plugin code from marketplace
2. Verify signature (prevent malware)
3. Auto-configure tool definitions for ACP
4. Add to available tools in orchestrator
5. Ready to use: "Generate React components from this Figma design"
```

### 7. Embrace Open Source 🌍

**Problem:** Closed-source tools lack community innovation

**Solution:** Fully open-source like Codex, but with community-first approach
- All code on GitHub (already is)
- Easy contribution guidelines
- Plugin SDK for developers
- Revenue sharing creates incentive to contribute
- Community decides on features (not just maintainers)

**Already On Track:** ✅ OpenAgents is open-source

### 8. Local + Swarm + Cloud Inference ⚡

**Problem:** Cursor forces cloud-only (expensive, privacy concerns)

**Solution:** Hybrid inference with automatic or manual selection

**Three Tiers:**

**A) Local Inference** (free, private, fast for simple tasks)
- Run on your M2/M3/RTX GPU
- Models: Qwen2.5-Coder 1.5B-7B, DeepSeek-Coder
- Use cases: Completions, simple edits, summaries
- Latency: <200ms first token

**B) Swarm Inference** (cheap, distributed, scales)
- Buy compute from peers with idle GPUs
- Pay tiny fractions of Bitcoin per request
- Models: Larger models (32B, 70B) run by swarm
- Use cases: Complex refactors, heavy reasoning
- Cost: 90% cheaper than cloud providers

**C) Cloud Inference** (highest quality, bleeding edge)
- Codex, Claude Code, Groq, OpenAI, Anthropic
- Latest frontier models (GPT-4, Claude Opus)
- Use cases: Most complex tasks, need best quality
- Cost: Standard API pricing

**User Controls:**
```toml
[inference]
default_tier = "auto"  # or "local", "swarm", "cloud"
budget_limit = 10.00  # USD per day
prefer_privacy = true  # prefer local when possible

[tier_thresholds]
local_max_complexity = 7    # 1-10 scale
swarm_max_cost = 0.50       # per request
cloud_min_quality = 9       # only use cloud for hardest tasks
```

### 9. Compute Fracking 🛢️

**Problem:** Billions of devices with idle compute (M2 Macs, RTX GPUs just sitting there)

**Solution:** Compute marketplace - sell your unused GPU time

**How It Works:**
1. User installs OpenAgents
2. Enables "Sell Compute" in settings
3. Downloads worker node software
4. When GPU idle, accepts inference jobs from swarm
5. Gets paid in Bitcoin (Lightning Network for instant microtransactions)

**Economics:**
- Average user makes $5-20/month passive income
- Buyers pay 90% less than AWS/Azure
- Network effect: More sellers → cheaper inference → more buyers → more demand

**Implementation:**
```rust
pub struct ComputeWorker {
    gpu_spec: GpuSpec,
    available_models: Vec<ModelSpec>,
    min_price_per_token: f64,  // in satoshis
    lightning_node: LightningClient,
}

impl ComputeWorker {
    pub async fn accept_job(&self, job: InferenceJob) -> Result<()> {
        // 1. Verify payment (Lightning invoice)
        // 2. Load model if not cached
        // 3. Run inference
        // 4. Stream results back
        // 5. Claim payment on completion
    }
}
```

**Already Built:** Episode 140 compute marketplace prototype exists

### 10. Revenue Sharing 💰

**Problem:** No incentive to build/maintain MCP tools → they suck

**Solution:** Marketplace with built-in payments

**How It Works:**
1. Developer builds agent plugin (e.g., "Figma → React")
2. Publishes to OpenAgents marketplace
3. Sets price: $0.01 per use OR free with tip jar
4. Users discover and install plugin
5. Every use triggers micropayment (Lightning Network)
6. Developer earns passive income

**Example Plugin:**
```typescript
// my-awesome-plugin/manifest.json
{
  "name": "Advanced Code Analyzer",
  "version": "1.0.0",
  "author": "developer123",
  "lightning_address": "developer123@getalby.com",
  "pricing": {
    "per_use": 100,  // 100 satoshis (~$0.01)
    "subscription": 10000  // 10,000 sats/month (~$10)
  },
  "tools": [...]
}
```

**Marketplace Revenue:**
- 90% to plugin author
- 10% to OpenAgents (fund development)

**Already Built:** ✅ Built-in Bitcoin wallet (Lightning)

---

## Feature Mapping: Cursor → OpenAgents

### Priority 1: Core Intelligence (MVP)

#### 1.1 Codebase Indexing & Semantic Search

**Cursor's Approach:**
- Background indexing of entire repo
- Tree-sitter-based intelligent chunking (split at function/class boundaries)
- Cloud embeddings via OpenAI API
- Vector DB (Turbopuffer) for semantic search
- Merkle tree for incremental updates (3-min polling)
- `.cursorignore` for exclusions

**OpenAgents Implementation:**
```rust
// New Rust module: crates/oa-indexer/
pub struct CodebaseIndexer {
    watcher: notify::RecommendedWatcher,
    chunker: TreeSitterChunker,
    embedder: LocalEmbedder,      // ONNX Runtime
    vector_db: SqliteVec,          // sqlite-vec extension
    merkle_state: MerkleTree,
}

impl CodebaseIndexer {
    pub async fn index_workspace(&self, root: PathBuf) -> Result<IndexStats> {
        // 1. Walk files (respect .gitignore + .openagentsignore)
        // 2. Parse with tree-sitter → extract functions/classes/modules
        // 3. Generate embeddings locally (ONNX model: bge-small-en-v1.5)
        // 4. Store in sqlite-vec with metadata (file, line range, language)
        // 5. Build Merkle tree of file hashes
        // 6. Emit progress via WebSocket
    }

    pub async fn search(&self, query: &str, top_k: usize) -> Vec<CodeChunk> {
        // 1. Embed query locally
        // 2. KNN search in sqlite-vec
        // 3. Return chunks with context (surrounding lines)
    }
}
```

**Storage Schema (extend tinyvex):**
```sql
CREATE TABLE code_chunks (
    id INTEGER PRIMARY KEY,
    file_path TEXT NOT NULL,
    start_line INTEGER,
    end_line INTEGER,
    chunk_type TEXT,  -- 'function', 'class', 'module', etc.
    chunk_name TEXT,  -- function/class name
    language TEXT,
    content_hash BLOB,
    embedding BLOB,   -- stored via sqlite-vec
    indexed_at INTEGER
);

CREATE VIRTUAL TABLE vec_chunks USING vec0(
    chunk_id INTEGER PRIMARY KEY,
    embedding FLOAT[384]  -- bge-small-en-v1.5 dimension
);

CREATE TABLE merkle_state (
    workspace_root TEXT PRIMARY KEY,
    merkle_root BLOB,
    last_updated INTEGER,
    file_hashes JSON  -- map of path → hash
);
```

**Integration:**
- Trigger indexing on workspace open (background task)
- Incremental updates via file watcher (notify crate)
- Expose search via Tauri command: `search_codebase(query: String) -> Vec<CodeChunk>`
- Agents can request context via ACP extension: `workspace.search()`

#### 1.2 Shadow Workspace Validation

**Cursor's Approach:**
- Spin up hidden VS Code window
- Apply AI-suggested edits in shadow
- Run LSPs (TypeScript, Python, etc.) to catch errors
- Feed diagnostics back to AI for refinement
- Only apply to real workspace when validated

**OpenAgents Implementation:**
```rust
// New Rust module: crates/oa-shadow/
pub struct ShadowWorkspace {
    temp_dir: TempDir,              // OS-specific temp location
    original_root: PathBuf,
    lsp_clients: HashMap<String, LspClient>,  // lang → client
}

impl ShadowWorkspace {
    pub async fn create_from(workspace: &Path) -> Result<Self> {
        // Platform-specific copy-on-write:
        // - macOS: APFS clonefile() via libc
        // - Linux: reflink via ioctl (XFS, Btrfs)
        // - Windows: fallback to hardlinks + sparse copy

        #[cfg(target_os = "macos")]
        unsafe {
            libc::clonefile(
                original_path.as_ptr(),
                shadow_path.as_ptr(),
                0
            );
        }

        // Start LSP servers for detected languages
        self.start_lsp_servers().await?;
        Ok(self)
    }

    pub async fn apply_edits(&mut self, edits: Vec<FileEdit>) -> ValidationResult {
        // 1. Apply edits to shadow files
        // 2. Trigger LSP diagnostics
        // 3. Collect errors/warnings
        // 4. Return validation report

        for edit in edits {
            self.write_file(&edit.path, &edit.content).await?;
        }

        let diagnostics = self.collect_diagnostics().await?;

        ValidationResult {
            success: diagnostics.errors.is_empty(),
            errors: diagnostics.errors,
            warnings: diagnostics.warnings,
            suggested_fixes: self.lsp_code_actions(&diagnostics).await?,
        }
    }
}
```

**LSP Integration:**
```rust
// Use tower-lsp for LSP client
use tower_lsp::lsp_types::*;
use tower_lsp::Client;

pub struct LspClient {
    process: Child,
    client: Client,
    language: String,
}

impl LspClient {
    pub async fn new(language: &str, root: &Path) -> Result<Self> {
        // Spawn LSP server (typescript-language-server, pyright, rust-analyzer, etc.)
        let process = match language {
            "typescript" | "javascript" => {
                Command::new("typescript-language-server")
                    .arg("--stdio")
                    .spawn()?
            },
            "python" => {
                Command::new("pyright-langserver")
                    .arg("--stdio")
                    .spawn()?
            },
            // ... other languages
        };

        // Initialize LSP connection
        let (client, server) = Client::new_stdio(process.stdin, process.stdout);
        client.initialize(root).await?;

        Ok(Self { process, client, language })
    }

    pub async fn get_diagnostics(&self, uri: &Url) -> Vec<Diagnostic> {
        self.client.text_document_publish_diagnostics(uri).await
    }
}
```

**Workflow:**
1. User asks AI to refactor code
2. AI generates edits (via ACP agent)
3. OpenAgents creates shadow workspace
4. Apply edits in shadow
5. LSPs report errors
6. If errors → send back to AI with error context
7. AI refines edits
8. Repeat until clean
9. Show diff to user, apply to real workspace on approval

#### 1.3 Local Code Completion

**Cursor's Approach:**
- Custom "Copilot++" model (70B Llama2-based)
- Cloud inference via Fireworks (1000 tok/s)
- Speculative decoding for speed
- Tab trigger, inline streaming

**OpenAgents Implementation:**
```rust
// New Rust module: crates/oa-completion/
pub struct CompletionEngine {
    draft_model: LlamaCppModel,   // 1-3B for speed
    verify_model: LlamaCppModel,  // 7-8B for quality
    context_builder: ContextBuilder,
}

impl CompletionEngine {
    pub async fn complete(&self, request: CompletionRequest) -> CompletionStream {
        // 1. Build context (file content + RAG snippets)
        let context = self.context_builder.build(
            &request.file_path,
            request.cursor_position,
            &request.recent_edits,
        ).await?;

        // 2. Speculative decoding:
        //    - Draft model generates N tokens fast
        //    - Verify model checks in parallel
        //    - Accept longest valid prefix

        let draft_stream = self.draft_model.generate(&context);
        let verify_stream = self.verify_model.verify(draft_stream);

        CompletionStream::new(verify_stream)
    }
}
```

**Model Selection:**
- **Draft model**: Qwen2.5-Coder-1.5B-Instruct (Q5_K_M)
- **Verify model**: Qwen2.5-Coder-7B-Instruct (Q4_K_M) or DeepSeek-Coder-6.7B
- **Runtime**: llama.cpp with Metal backend (macOS), CUDA (Linux/Windows)
- **Alternative**: MLX for Apple Silicon (faster, native Swift/Python bindings)

**Performance Target:**
- **First token latency**: <100ms (draft model)
- **Throughput**: 15-30 tok/s (combined speculative decoding)
- **Memory**: <8GB VRAM for both models loaded

#### 1.4 Fast Apply (Multi-File Edits)

**Cursor's Approach:**
- Specialized "Fast Apply" model
- Fine-tuned on (instruction, diff) pairs
- Generates unified diffs
- Applies changes across multiple files atomically

**OpenAgents Implementation:**
```rust
pub struct FastApplyEngine {
    model: LlamaCppModel,  // Qwen2.5-Coder-7B fine-tuned
    diff_parser: UnifiedDiffParser,
    safety_checker: EditSafetyChecker,
}

impl FastApplyEngine {
    pub async fn apply(&self, instruction: &str, files: Vec<PathBuf>) -> ApplyResult {
        // 1. Build prompt with instruction + file contents
        let prompt = self.build_apply_prompt(instruction, files)?;

        // 2. Generate unified diff
        let diff = self.model.generate(&prompt).await?;

        // 3. Parse and validate diff
        let edits = self.diff_parser.parse(&diff)?;
        self.safety_checker.validate(&edits)?;

        // 4. Create shadow workspace for validation
        let shadow = ShadowWorkspace::create_from(&workspace_root).await?;
        let validation = shadow.apply_edits(edits.clone()).await?;

        // 5. Show diff UI, await user approval
        ApplyResult {
            edits,
            validation,
            preview_url: self.generate_preview(&edits)?,
        }
    }
}
```

**Prompt Template:**
```
You are an expert code editor. Generate a unified diff to implement the following:

Instruction: {instruction}

Files:
{files_with_line_numbers}

Generate ONLY a valid unified diff. No explanations.
```

### Priority 2: Advanced Features (Quality Phase)

#### 2.1 Intelligent Context Management

**Cursor's @-Commands:**
- `@File` - include specific file
- `@Folder` - include all files in folder
- `@Code` - include symbol definition
- `@Docs` - include documentation
- `@Git` - include git history

**OpenAgents Implementation:**
```rust
pub enum ContextReference {
    File(PathBuf),
    Folder(PathBuf),
    Symbol { name: String, workspace_search: bool },
    Git { commit: String, file: Option<PathBuf> },
    Web(Url),  // fetch docs
}

pub struct ContextResolver {
    indexer: Arc<CodebaseIndexer>,
    lsp_clients: HashMap<String, LspClient>,
    git_repo: Repository,
}

impl ContextResolver {
    pub async fn resolve(&self, refs: Vec<ContextReference>) -> ResolvedContext {
        let mut chunks = Vec::new();

        for ref in refs {
            match ref {
                ContextReference::Symbol { name, .. } => {
                    // Use LSP "go to definition" + RAG search
                    let definitions = self.find_symbol_definitions(&name).await?;
                    chunks.extend(definitions);
                },
                ContextReference::File(path) => {
                    chunks.push(self.read_file_with_metadata(&path)?);
                },
                // ... handle other ref types
            }
        }

        ResolvedContext { chunks, total_tokens: self.count_tokens(&chunks) }
    }
}
```

#### 2.2 Agentic Composer Mode

**Cursor's Composer:**
- Multi-step autonomous edits
- Can search codebase, read files, apply changes
- User gives high-level instruction
- Agent breaks down into steps

**OpenAgents Implementation:**
```rust
// Integrate with existing ACP agents (Claude Code, Codex)
pub struct ComposerAgent {
    acp_client: ACPClient,
    tools: ComposerTools,
}

pub struct ComposerTools {
    codebase_search: Arc<CodebaseIndexer>,
    file_ops: FileOperations,
    shadow_workspace: Arc<ShadowWorkspace>,
    lsp: Arc<LspClientManager>,
}

impl ComposerAgent {
    pub async fn execute_task(&self, task: &str) -> TaskResult {
        // 1. Send task to ACP agent with tool access
        let session = self.acp_client.create_session_with_tools(
            task,
            vec![
                Tool::CodebaseSearch,
                Tool::ReadFile,
                Tool::WriteFile,
                Tool::RunLSP,
                Tool::ValidateInShadow,
            ]
        ).await?;

        // 2. Agent autonomously uses tools (via ACP requests)
        // 3. Each step validated in shadow workspace
        // 4. Collect all proposed changes
        // 5. Show unified diff for approval

        self.stream_progress(session).await
    }
}
```

**Tool Definitions (exposed to ACP agent):**
```json
{
  "tools": [
    {
      "name": "codebase_search",
      "description": "Semantic search across entire codebase",
      "parameters": {
        "query": "string",
        "top_k": "number"
      }
    },
    {
      "name": "get_symbol_definition",
      "description": "Get definition of function/class via LSP",
      "parameters": {
        "symbol": "string",
        "file": "string"
      }
    },
    {
      "name": "validate_edits",
      "description": "Validate edits in shadow workspace",
      "parameters": {
        "edits": "array"
      }
    }
  ]
}
```

#### 2.3 Tree-Sitter Integration

**Purpose:**
- Syntax-aware code chunking
- Structural code understanding
- Better context boundaries
- Enables advanced refactoring

**Implementation:**
```rust
use tree_sitter::{Parser, Language};

pub struct TreeSitterChunker {
    parsers: HashMap<String, Parser>,
}

impl TreeSitterChunker {
    pub fn chunk_file(&self, path: &Path, content: &str) -> Vec<CodeChunk> {
        let lang = detect_language(path);
        let parser = self.parsers.get(lang)?;
        let tree = parser.parse(content, None)?;

        let mut chunks = Vec::new();

        // Extract top-level constructs
        for node in tree.root_node().children() {
            match node.kind() {
                "function_declaration" | "method_definition" => {
                    chunks.push(CodeChunk {
                        chunk_type: ChunkType::Function,
                        name: extract_name(&node),
                        range: node.range(),
                        content: content[node.byte_range()].to_string(),
                        language: lang.to_string(),
                    });
                },
                "class_declaration" => {
                    // Include class + all methods
                    chunks.push(self.extract_class(&node, content));
                },
                // ... handle imports, types, etc.
            }
        }

        chunks
    }
}
```

**Supported Languages (via tree-sitter grammars):**
- Rust, TypeScript, JavaScript, Python, Go, C/C++, Java, Swift, Kotlin
- Auto-download grammars on first use

### Priority 3: Polish & Enterprise (Production)

#### 3.1 Model Management

**Features:**
- Local model registry
- Download/update models
- Multiple model profiles (fast, balanced, quality)
- Quantization options (Q4, Q5, Q8)

```rust
pub struct ModelRegistry {
    models_dir: PathBuf,  // ~/.openagents/models/
    index: ModelIndex,
}

pub struct ModelProfile {
    name: String,
    completion_model: ModelSpec,
    chat_model: ModelSpec,
    embedding_model: ModelSpec,
}

impl ModelRegistry {
    pub async fn download_model(&self, spec: &ModelSpec) -> Progress {
        // Download from HuggingFace
        let url = format!("https://huggingface.co/{}/{}", spec.repo, spec.file);
        self.download_with_resume(&url, &self.models_dir).await
    }

    pub fn list_available_profiles() -> Vec<ModelProfile> {
        vec![
            ModelProfile {
                name: "fast".to_string(),
                completion_model: "Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF".into(),
                chat_model: "Qwen/Qwen2.5-Coder-7B-Instruct-GGUF".into(),
                embedding_model: "BAAI/bge-small-en-v1.5".into(),
            },
            ModelProfile {
                name: "quality".to_string(),
                completion_model: "deepseek-ai/DeepSeek-Coder-V2-Lite-Instruct".into(),
                chat_model: "deepseek-ai/DeepSeek-Coder-V2-Instruct".into(),
                embedding_model: "BAAI/bge-large-en-v1.5".into(),
            },
        ]
    }
}
```

#### 3.2 Hybrid Cloud Mode (Optional)

**For users who want best of both worlds:**
- Local indexing + completions (default)
- Optional cloud agent for complex reasoning (Claude/GPT-4)
- User controls when to use cloud

```rust
pub enum InferenceBackend {
    Local(LocalModel),
    Cloud { provider: CloudProvider, api_key: String },
    Hybrid { local: LocalModel, cloud: CloudProvider },
}

impl InferenceBackend {
    pub async fn complete(&self, context: &Context) -> Completion {
        match self {
            Self::Local(model) => model.generate(context).await,
            Self::Cloud { provider, .. } => provider.complete(context).await,
            Self::Hybrid { local, cloud } => {
                // Try local first, fallback to cloud if needed
                match local.generate(context).await {
                    Ok(result) if result.confidence > 0.8 => result,
                    _ => cloud.complete(context).await?,
                }
            }
        }
    }
}
```

---

## Architecture Overview

### System Architecture Diagram (Hybrid Local/Swarm/Cloud)

```
┌──────────────────────────────────────────────────────────────┐
│              Frontend (Desktop + Mobile)                     │
│  ┌────────────┐  ┌────────────┐  ┌───────────┐  ┌────────┐ │
│  │ Chat UI    │  │ History    │  │ Diff View │  │ Market │ │
│  │ (sidebar)  │  │ (semantic) │  │ (Monaco)  │  │ -place │ │
│  └──────┬─────┘  └──────┬─────┘  └─────┬─────┘  └────┬───┘ │
│         │               │               │             │      │
│         └───────────────┴───────────────┴─────────────┘      │
│                              │                                │
│                     ┌────────▼────────┐                       │
│                     │ Tauri IPC/WS    │                       │
│                     └────────┬────────┘                       │
└──────────────────────────────┼───────────────────────────────┘
                               │
┌──────────────────────────────▼───────────────────────────────┐
│                  Tauri Backend (Rust)                         │
│                                                                │
│  ┌────────────────────────────────────────────────────────┐  │
│  │         Agent Orchestrator & Router                    │  │
│  │  ┌──────────────────────────────────────────────────┐ │  │
│  │  │  Route tasks based on:                           │ │  │
│  │  │  • Complexity (simple→local, hard→cloud)         │ │  │
│  │  │  • Privacy (sensitive→local)                     │ │  │
│  │  │  • Cost (budget→swarm, quality→cloud)            │ │  │
│  │  │  • Speed (urgent→local/swarm, batch→cloud)       │ │  │
│  │  └──────────────────────────────────────────────────┘ │  │
│  │         │              │              │                │  │
│  │    ┌────▼───┐     ┌───▼────┐    ┌───▼─────┐          │  │
│  │    │ Local  │     │ Swarm  │    │  Cloud  │          │  │
│  │    │ Tier   │     │  Tier  │    │  Tier   │          │  │
│  │    └────┬───┘     └───┬────┘    └───┬─────┘          │  │
│  └─────────┼─────────────┼─────────────┼────────────────┘  │
│            │             │             │                     │
│  ┌─────────▼─────────────▼─────────────▼────────────────┐  │
│  │          Inference Execution Layer                    │  │
│  │                                                        │  │
│  │  LOCAL INFERENCE          SWARM INFERENCE             │  │
│  │  ┌─────────────┐          ┌──────────────┐           │  │
│  │  │ llama.cpp/  │          │ Nostr DVMs   │           │  │
│  │  │ MLX Runtime │          │ (NIP-90)     │           │  │
│  │  │             │          │              │           │  │
│  │  │ • 1-7B      │          │ • Find DVMs  │           │  │
│  │  │   models    │          │ • Pay Zaps   │           │  │
│  │  │ • Metal/    │          │ • Stream     │           │  │
│  │  │   CUDA      │          │   results    │           │  │
│  │  └─────────────┘          └──────────────┘           │  │
│  │                                                        │  │
│  │  CLOUD AGENTS (via ACP)                               │  │
│  │  ┌──────────┐  ┌──────────┐  ┌────────┐  ┌────────┐ │  │
│  │  │ Codex    │  │ Claude   │  │ Groq   │  │ Custom │ │  │
│  │  │ (ACP)    │  │ Code(ACP)│  │ (API)  │  │ (MCP)  │ │  │
│  │  └──────────┘  └──────────┘  └────────┘  └────────┘ │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                                │
│  ┌────────────────────────────────────────────────────────┐  │
│  │          Code Intelligence Layer                       │  │
│  │  ┌───────────┐  ┌──────────┐  ┌─────────┐  ┌───────┐ │  │
│  │  │ Indexer   │  │ Shadow   │  │ LSP Mgr │  │ Git   │ │  │
│  │  │ (local)   │  │ (local)  │  │ (local) │  │ (all) │ │  │
│  │  └───────────┘  └──────────┘  └─────────┘  └───────┘ │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                                │
│  ┌────────────────────────────────────────────────────────┐  │
│  │          Storage & State Layer                         │  │
│  │  ┌───────────┐  ┌──────────┐  ┌──────────┐  ┌──────┐ │  │
│  │  │ Tinyvex   │  │ sqlite-  │  │ Schedule │  │ File │ │  │
│  │  │ (convos)  │  │ vec(RAG) │  │ Queue    │  │ Watch│ │  │
│  │  └───────────┘  └──────────┘  └──────────┘  └──────┘ │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                                │
│  ┌────────────────────────────────────────────────────────┐  │
│  │          Marketplace & Payments Layer                  │  │
│  │  ┌───────────┐  ┌──────────┐  ┌──────────┐            │  │
│  │  │ Plugin    │  │ Lightning│  │ Compute  │            │  │
│  │  │ Registry  │  │ Node     │  │ Worker   │            │  │
│  │  └───────────┘  └──────────┘  └──────────┘            │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

### Data Flow: Completion Request

```
1. User types, triggers completion (Tab key)
   │
   ├→ Frontend: Capture cursor position + file context
   │
   ├→ Tauri IPC: request_completion({ file, position, recent_edits })
   │
   ├→ Context Builder:
   │  ├─ Read surrounding code (±50 lines)
   │  ├─ RAG search for relevant chunks (codebase indexer)
   │  ├─ Fetch imports/definitions (LSP)
   │  └─ Assemble prompt (max 4096 tokens)
   │
   ├→ Completion Engine:
   │  ├─ Draft model generates 20 tokens (50ms)
   │  ├─ Verify model validates in parallel
   │  └─ Stream accepted tokens to frontend
   │
   └→ Frontend: Display inline completion (Monaco/CodeMirror)
```

### Data Flow: Multi-File Refactor

```
1. User asks in chat: "Rename UserService to AccountService everywhere"
   │
   ├→ Composer Agent (ACP):
   │  ├─ Tool call: codebase_search("UserService")
   │  ├─ Finds 15 occurrences across 8 files
   │  ├─ Tool call: validate_edits([...])
   │  │
   │  └→ Shadow Workspace:
   │     ├─ Clone workspace (APFS copy-on-write)
   │     ├─ Apply all renames in shadow
   │     ├─ Start LSP servers in shadow
   │     ├─ Collect diagnostics (TypeScript compiler errors, etc.)
   │     ├─ If errors: return to agent with context
   │     └─ If clean: generate unified diff
   │
   ├→ Fast Apply Engine:
   │  └─ Parse diff, validate safety, generate preview
   │
   └→ Frontend:
      ├─ Show diff viewer (8 files changed)
      ├─ User reviews, clicks "Apply"
      └─ Apply to real workspace atomically
```

---

## Core Systems Design

### 1. Codebase Indexer

**Module:** `crates/oa-indexer/`

**Responsibilities:**
- Watch file system for changes
- Parse files with tree-sitter
- Extract semantic chunks (functions, classes, modules)
- Generate embeddings locally
- Store in vector database
- Maintain Merkle tree for incremental updates

**Key Components:**

```rust
// src/watcher.rs
pub struct FileWatcher {
    watcher: RecommendedWatcher,
    debouncer: Debouncer,
    exclude_patterns: Vec<Pattern>,
}

impl FileWatcher {
    pub fn watch(&self, root: PathBuf) -> Receiver<FileEvent> {
        // Use notify crate, respect .gitignore + .openagentsignore
        // Debounce events (300ms)
        // Filter out ignored paths
    }
}

// src/chunker.rs
pub struct TreeSitterChunker {
    parsers: HashMap<Language, Parser>,
    chunk_strategy: ChunkStrategy,
}

pub enum ChunkStrategy {
    TopLevel,      // Only top-level functions/classes
    Hierarchical,  // Include nested structures
    FixedSize(usize),  // Fallback for unsupported languages
}

// src/embedder.rs
pub struct LocalEmbedder {
    model: OnnxModel,  // bge-small-en-v1.5
    tokenizer: Tokenizer,
    executor: ExecutionProvider,  // CPU/Metal/CUDA
}

impl LocalEmbedder {
    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        // Batch encoding for efficiency
        // Normalize embeddings (L2 norm)
    }
}

// src/vector_db.rs
pub struct SqliteVecDb {
    conn: Connection,
}

impl SqliteVecDb {
    pub fn insert(&self, chunk: &CodeChunk, embedding: &[f32]) -> Result<()> {
        // Store in both code_chunks and vec_chunks tables
    }

    pub fn knn_search(&self, query_vec: &[f32], k: usize) -> Result<Vec<SearchResult>> {
        // Use sqlite-vec's KNN search
        // Return chunks with similarity scores
    }
}

// src/merkle.rs
pub struct MerkleTree {
    root: Option<Hash>,
    file_hashes: HashMap<PathBuf, Hash>,
}

impl MerkleTree {
    pub fn update(&mut self, path: &Path, content: &[u8]) -> bool {
        // Hash file content (BLAKE3)
        // Update tree structure
        // Return true if root changed
    }

    pub fn diff(&self, other: &MerkleTree) -> Vec<PathBuf> {
        // Find files with changed hashes
    }
}
```

**Configuration:**

```toml
# openagents.toml
[indexer]
enabled = true
exclude = [
    "**/node_modules/**",
    "**/target/**",
    "**/.git/**",
    "**/*.min.js",
]
max_file_size = 1048576  # 1MB
chunk_size_target = 512  # tokens
embedding_model = "BAAI/bge-small-en-v1.5"
update_interval = 180  # seconds (3 min like Cursor)
```

### 2. Shadow Workspace

**Module:** `crates/oa-shadow/`

**Responsibilities:**
- Create copy-on-write workspace clones
- Apply edits in isolation
- Run LSP servers for validation
- Collect diagnostics
- Clean up temporary files

**Platform-Specific Implementation:**

```rust
// src/clone.rs
pub fn clone_workspace(src: &Path, dst: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        // Use APFS clonefile for instant CoW copy
        unsafe {
            let src_c = CString::new(src.as_os_str().as_bytes())?;
            let dst_c = CString::new(dst.as_os_str().as_bytes())?;

            let ret = libc::clonefile(
                src_c.as_ptr(),
                dst_c.as_ptr(),
                0,  // flags
            );

            if ret != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Use reflink on supporting filesystems (Btrfs, XFS)
        std::process::Command::new("cp")
            .arg("--reflink=auto")
            .arg("-r")
            .arg(src)
            .arg(dst)
            .status()?;
    }

    #[cfg(target_os = "windows")]
    {
        // Fallback: hardlinks + sparse file copy
        reflink_copy::reflink_or_copy(src, dst)?;
    }

    Ok(())
}

// src/lsp_manager.rs
pub struct LspManager {
    clients: HashMap<String, LspClient>,
    workspace_root: PathBuf,
}

impl LspManager {
    pub async fn start_server(&mut self, language: &str) -> Result<()> {
        let config = self.get_lsp_config(language)?;

        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.current_dir(&self.workspace_root);

        let child = cmd.spawn()?;

        let client = LspClient::new(
            child,
            language.to_string(),
            self.workspace_root.clone(),
        ).await?;

        self.clients.insert(language.to_string(), client);
        Ok(())
    }

    pub async fn get_diagnostics(&self, file: &Path) -> Result<Vec<Diagnostic>> {
        let lang = detect_language(file);
        let client = self.clients.get(lang).ok_or(anyhow!("LSP not started"))?;

        client.get_diagnostics(file).await
    }
}

// LSP configurations
static LSP_CONFIGS: LazyLock<HashMap<&str, LspConfig>> = LazyLock::new(|| {
    hashmap! {
        "typescript" => LspConfig {
            command: "typescript-language-server",
            args: vec!["--stdio"],
        },
        "python" => LspConfig {
            command: "pyright-langserver",
            args: vec!["--stdio"],
        },
        "rust" => LspConfig {
            command: "rust-analyzer",
            args: vec![],
        },
        "go" => LspConfig {
            command: "gopls",
            args: vec![],
        },
        // ... more languages
    }
});
```

**Validation Workflow:**

```rust
pub struct Validator {
    shadow: ShadowWorkspace,
    lsp_manager: LspManager,
}

impl Validator {
    pub async fn validate(&mut self, edits: Vec<FileEdit>) -> ValidationResult {
        // 1. Apply edits to shadow files
        for edit in &edits {
            self.shadow.write_file(&edit.path, &edit.content).await?;
        }

        // 2. Notify LSP servers of changes
        for edit in &edits {
            let lang = detect_language(&edit.path);
            if let Some(client) = self.lsp_manager.clients.get(lang) {
                client.did_change(&edit.path, &edit.content).await?;
            }
        }

        // 3. Wait for diagnostics to settle (500ms debounce)
        tokio::time::sleep(Duration::from_millis(500)).await;

        // 4. Collect all diagnostics
        let mut all_diagnostics = Vec::new();
        for edit in &edits {
            let diags = self.lsp_manager.get_diagnostics(&edit.path).await?;
            all_diagnostics.extend(diags);
        }

        // 5. Categorize by severity
        let errors: Vec<_> = all_diagnostics.iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::Error))
            .collect();

        let warnings: Vec<_> = all_diagnostics.iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::Warning))
            .collect();

        ValidationResult {
            success: errors.is_empty(),
            errors,
            warnings,
            files_checked: edits.len(),
        }
    }
}
```

### 3. Local Inference Engine

**Module:** `crates/oa-inference/`

**Responsibilities:**
- Load and manage GGUF models
- Run inference via llama.cpp or MLX
- Implement speculative decoding
- Manage KV cache
- Stream tokens

**Model Loading:**

```rust
// src/model_loader.rs
pub struct ModelLoader {
    models_dir: PathBuf,
    registry: ModelRegistry,
}

impl ModelLoader {
    pub async fn load(&self, spec: &ModelSpec) -> Result<LlamaCppModel> {
        let model_path = self.models_dir.join(&spec.filename);

        if !model_path.exists() {
            // Download from HuggingFace
            self.download_model(spec).await?;
        }

        // Load with llama.cpp
        let params = LlamaParams {
            n_ctx: spec.context_length,
            n_gpu_layers: self.detect_gpu_layers(),
            use_mmap: true,
            use_mlock: false,
            n_threads: num_cpus::get_physical(),
            ..Default::default()
        };

        LlamaCppModel::load(&model_path, params)
    }

    fn detect_gpu_layers(&self) -> i32 {
        #[cfg(target_os = "macos")]
        return 999;  // Use Metal for all layers

        #[cfg(all(target_os = "linux", feature = "cuda"))]
        return 999;  // Use CUDA

        #[cfg(all(target_os = "windows", feature = "cuda"))]
        return 999;

        0  // CPU only
    }
}

// src/llama_cpp.rs
pub struct LlamaCppModel {
    ctx: *mut llama_context,
    model: *mut llama_model,
    spec: ModelSpec,
}

impl LlamaCppModel {
    pub fn generate(&self, prompt: &str, params: GenParams) -> TokenStream {
        // Tokenize prompt
        let tokens = self.tokenize(prompt);

        // Create KV cache
        let mut kv_cache = self.create_kv_cache();

        // Generate tokens
        let stream = async_stream::stream! {
            let mut generated = 0;
            while generated < params.max_tokens {
                let token = unsafe {
                    llama_sample_token(
                        self.ctx,
                        &params.sampling,
                    )
                };

                if token == self.eos_token() {
                    break;
                }

                yield token;
                generated += 1;
            }
        };

        TokenStream::new(stream)
    }
}
```

**Speculative Decoding:**

```rust
pub struct SpeculativeDecoder {
    draft_model: LlamaCppModel,   // Fast 1-3B model
    verify_model: LlamaCppModel,  // Accurate 7B model
}

impl SpeculativeDecoder {
    pub async fn generate(&self, prompt: &str) -> TokenStream {
        let stream = async_stream::stream! {
            let mut accepted_tokens = Vec::new();

            loop {
                // Draft model generates K tokens (K=4-8)
                let draft_tokens = self.draft_model
                    .generate_n(prompt, K, &accepted_tokens)
                    .await?;

                // Verify model checks all K tokens in parallel
                let verified = self.verify_model
                    .verify_batch(&draft_tokens, prompt, &accepted_tokens)
                    .await?;

                // Accept longest correct prefix
                let accept_count = verified.longest_prefix();

                for i in 0..accept_count {
                    yield draft_tokens[i];
                    accepted_tokens.push(draft_tokens[i]);
                }

                // If draft was wrong, use verify's next token
                if accept_count < draft_tokens.len() {
                    let correction = verified.correction_token();
                    yield correction;
                    accepted_tokens.push(correction);
                }

                if accepted_tokens.len() >= max_tokens {
                    break;
                }
            }
        };

        TokenStream::new(stream)
    }
}
```

**MLX Alternative (Apple Silicon):**

```rust
// For macOS, optionally use MLX for better performance
#[cfg(target_os = "macos")]
pub struct MlxModel {
    model: mlx_rs::Model,
    tokenizer: mlx_rs::Tokenizer,
}

#[cfg(target_os = "macos")]
impl MlxModel {
    pub fn load(path: &Path) -> Result<Self> {
        // Load MLX model (converted from GGUF or native)
        let model = mlx_rs::Model::load(path)?;
        let tokenizer = mlx_rs::Tokenizer::from_pretrained("Qwen/Qwen2.5-Coder-7B")?;

        Ok(Self { model, tokenizer })
    }

    pub async fn generate(&self, prompt: &str, max_tokens: usize) -> TokenStream {
        // MLX has native async support and better Metal optimization
        self.model.generate_stream(prompt, max_tokens).await
    }
}
```

---

## Hybrid Inference System

**Overview:** OpenAgents uses a three-tier hybrid inference system:
- **Local**: Run models on your device (llama.cpp/MLX) - free, private, fast for simple tasks
- **Swarm**: Buy compute from peers via Nostr DVMs (Data Vending Machines) - 90% cheaper than cloud, decentralized
- **Cloud**: Use frontier models (Codex, Claude Code, GPT-4) via ACP - highest quality

The swarm tier is powered by **Nostr DVMs** (NIP-90), allowing decentralized discovery of compute providers via Nostr relays. Payments are handled through **Zaps** (NIP-57) - Lightning payments over Nostr.

### Orchestrator & Router

**Core Concept:** Every request goes through an intelligent router that decides: local, swarm, or cloud?

```rust
pub struct InferenceOrchestrator {
    local_engine: Option<LocalInference>,
    swarm_client: Option<SwarmClient>,
    cloud_agents: HashMap<String, ACPClient>,
    preferences: UserPreferences,
    cost_tracker: CostTracker,
}

pub struct RoutingDecision {
    tier: InferenceTier,
    reasoning: String,
    estimated_cost: f64,
    estimated_latency: Duration,
}

impl InferenceOrchestrator {
    pub async fn route_request(&self, request: &Request) -> RoutingDecision {
        // 1. Analyze request complexity
        let complexity = self.analyze_complexity(request);

        // 2. Check privacy requirements
        if request.contains_sensitive_data() {
            return RoutingDecision {
                tier: InferenceTier::Local,
                reasoning: "Request contains sensitive code".into(),
                estimated_cost: 0.0,
                estimated_latency: Duration::from_millis(200),
            };
        }

        // 3. Check user budget
        if self.cost_tracker.daily_spend >= self.preferences.budget_limit {
            return RoutingDecision {
                tier: InferenceTier::Local,
                reasoning: "Daily budget exhausted".into(),
                estimated_cost: 0.0,
                estimated_latency: Duration::from_millis(250),
            };
        }

        // 4. Route based on complexity and preferences
        match (complexity, self.preferences.default_tier) {
            (1..=3, _) => {
                // Simple tasks → always local
                RoutingDecision {
                    tier: InferenceTier::Local,
                    reasoning: "Simple task, local is sufficient".into(),
                    estimated_cost: 0.0,
                    estimated_latency: Duration::from_millis(150),
                }
            },
            (4..=7, InferenceTier::Auto) => {
                // Medium tasks → swarm if available, else local
                if self.swarm_client.is_some() {
                    RoutingDecision {
                        tier: InferenceTier::Swarm,
                        reasoning: "Medium complexity, swarm offers good balance".into(),
                        estimated_cost: 0.05,  // $0.05
                        estimated_latency: Duration::from_millis(500),
                    }
                } else {
                    RoutingDecision {
                        tier: InferenceTier::Local,
                        reasoning: "Medium task, swarm unavailable".into(),
                        estimated_cost: 0.0,
                        estimated_latency: Duration::from_millis(300),
                    }
                }
            },
            (8..=10, _) => {
                // Hard tasks → cloud (best quality)
                RoutingDecision {
                    tier: InferenceTier::Cloud("codex".into()),
                    reasoning: "High complexity, need frontier model".into(),
                    estimated_cost: 0.50,  // $0.50
                    estimated_latency: Duration::from_secs(3),
                }
            },
            (_, tier) => {
                // Respect user override
                RoutingDecision {
                    tier,
                    reasoning: "User preference override".into(),
                    estimated_cost: Self::estimate_cost(tier),
                    estimated_latency: Self::estimate_latency(tier),
                }
            }
        }
    }

    fn analyze_complexity(&self, request: &Request) -> u8 {
        // Score 1-10 based on:
        // - Token count
        // - Number of files involved
        // - Whether it needs multi-step reasoning
        // - Past performance (learn from failures)

        let mut score = 1u8;

        // Context size
        if request.context_tokens > 8000 {
            score += 3;
        } else if request.context_tokens > 2000 {
            score += 1;
        }

        // Multi-file?
        if request.files.len() > 5 {
            score += 2;
        } else if request.files.len() > 1 {
            score += 1;
        }

        // Keywords suggesting complexity
        let complex_keywords = ["refactor", "redesign", "architecture", "migrate"];
        if complex_keywords.iter().any(|k| request.prompt.contains(k)) {
            score += 2;
        }

        score.min(10)
    }
}
```

### Local Inference Tier

**When to Use:**
- Simple completions (1-3 lines)
- Quick edits
- Sensitive code (privacy required)
- Offline mode
- User is over budget

**Implementation:**
```rust
pub struct LocalInference {
    completion_engine: SpeculativeDecoder,
    chat_model: LlamaCppModel,
    embedding_model: OnnxModel,
}

impl LocalInference {
    pub async fn complete(&self, context: &Context) -> Result<Completion> {
        // Use speculative decoding (1.5B draft + 7B verify)
        self.completion_engine.generate(context).await
    }

    pub async fn chat(&self, messages: &[Message]) -> Result<Response> {
        // Use 7B chat model with RAG
        let relevant_chunks = self.search_codebase(&messages.last().content).await?;
        let augmented_context = self.merge_context(messages, relevant_chunks);

        self.chat_model.generate(&augmented_context).await
    }
}
```

**Performance:**
- First token: <200ms
- Throughput: 20-35 tok/s (with speculation)
- Cost: $0 (free)
- Privacy: 100% local

### Swarm Inference Tier

**When to Use:**
- Medium-complexity tasks (multi-file edits)
- Need bigger models (32B, 70B)
- Want to save money vs. cloud
- Local compute insufficient

**Implementation:**
```rust
pub struct SwarmClient {
    nostr_client: NostrClient,
    relays: Vec<String>,
    lightning: LightningClient,
    provider_ratings: HashMap<PublicKey, Rating>,
}

impl SwarmClient {
    pub async fn request_inference(&self, request: InferenceRequest) -> Result<Response> {
        // 1. Find suitable DVMs (has model, good rating, fair price)
        let providers = self.find_dvms(&request.model_spec, request.max_cost_per_token).await?;

        // 2. Select best provider (lowest latency + price + highest rating)
        let provider = self.select_provider(&providers).await?;

        // 3. Create DVM job request (NIP-90) with custom OpenAgents kind
        let job_event = Event::new(
            6500,  // Custom OpenAgents DVM kind for code generation
            vec![
                Tag::new(&["i", &request.prompt, "text"]),
                Tag::new(&["param", "model", &request.model_spec.to_string()]),
                Tag::new(&["param", "max_tokens", &request.estimated_tokens.to_string()]),
                Tag::new(&["param", "language", &request.language]),
                Tag::new(&["output", "text/plain"]),
                Tag::new(&["p", &provider.pubkey.to_hex()]),  // Tag provider
            ],
        );

        // 4. Publish job request to relays
        self.nostr_client.publish_event(job_event).await?;

        // 5. Subscribe to job results (kind 6501 = result event)
        let mut subscription = self.nostr_client.subscribe(vec![
            Filter::new()
                .kind(6501)
                .event(job_event.id)
                .author(provider.pubkey)
        ]).await?;

        // 6. Stream results while paying incrementally via Zaps (NIP-57)
        let mut response = String::new();
        let mut tokens_consumed = 0;

        while let Some(result_event) = subscription.next().await {
            let chunk = self.parse_result_chunk(&result_event)?;
            response.push_str(&chunk.text);
            tokens_consumed += chunk.tokens;

            // Zap payment every 100 tokens
            if tokens_consumed % 100 == 0 {
                let zap_amount = tokens_consumed as f64 * provider.price_per_token;
                self.send_zap(&provider.pubkey, zap_amount, &job_event.id).await?;
            }
        }

        // 7. Final Zap payment
        let final_amount = tokens_consumed as f64 * provider.price_per_token;
        self.send_zap(&provider.pubkey, final_amount, &job_event.id).await?;

        // 8. Rate provider (publish kind 6502 rating event)
        self.rate_provider(provider.pubkey, &response, &job_event.id).await?;

        Ok(Response { text: response })
    }

    async fn find_dvms(&self, model: &ModelSpec, max_cost: f64) -> Result<Vec<DvmProvider>> {
        // Query relays for NIP-89 handler announcements
        let handlers = self.nostr_client.query_handlers(6500).await?;

        // Parse handler metadata for model availability and pricing
        let mut providers = Vec::new();
        for handler in handlers {
            if let Some(provider) = self.parse_dvm_handler(&handler, model).await? {
                if provider.price_per_token <= max_cost {
                    providers.push(provider);
                }
            }
        }

        // Sort by rating * latency
        providers.sort_by_key(|p| {
            let rating_score = self.provider_ratings.get(&p.pubkey).map(|r| r.score).unwrap_or(5.0);
            (p.avg_latency.as_millis() as f64 / rating_score) as u64
        });

        Ok(providers)
    }
}
```

**Nostr DVM Network:**
- Protocol: Nostr (NIP-01, NIP-90, NIP-89, NIP-57)
- Discovery: Relay queries for NIP-89 handlers
- Transport: WebSocket to Nostr relays
- Payment: Zaps (Lightning over Nostr via NIP-57)

**Performance:**
- First token: 300-800ms (network latency)
- Throughput: 30-60 tok/s (depends on peer GPU)
- Cost: ~$0.01-0.10 per request (90% cheaper than cloud)
- Privacy: Moderate (peer sees prompt, but not identity)

### Cloud Inference Tier

**When to Use:**
- Highest complexity tasks
- Need frontier models (GPT-4, Claude Opus)
- Quality > cost
- User explicitly requests

**Implementation:**
```rust
pub struct CloudAgents {
    codex_client: ACPClient,        // Codex via ACP
    claude_code_client: ACPClient,  // Claude Code via ACP
    groq_client: ApiClient,         // Direct API
    openai_client: ApiClient,       // Direct API
    anthropic_client: ApiClient,    // Direct API
}

impl CloudAgents {
    pub async fn delegate(&self, task: &Task, agent: &str) -> Result<Response> {
        match agent {
            "codex" => {
                // Delegate to Codex via ACP
                self.codex_client.send_prompt(&task.prompt).await
            },
            "claude-code" => {
                // Delegate to Claude Code via ACP
                self.claude_code_client.send_prompt(&task.prompt).await
            },
            "groq" => {
                // Use Groq API directly (for their fast inference)
                self.groq_client.complete(&task.prompt, "llama-3.3-70b").await
            },
            "gpt-4" => {
                // OpenAI API
                self.openai_client.complete(&task.prompt, "gpt-4-turbo").await
            },
            _ => Err(anyhow!("Unknown agent")),
        }
    }
}
```

**Sub-Agent Orchestration:**
```
User: "Refactor this entire module to use dependency injection"
  ↓
Orchestrator:
  1. Analyze complexity → 9/10 (high)
  2. Route to Cloud tier
  3. Choose agent: Codex (good at refactoring)
  ↓
Codex (via ACP):
  1. Requests: search_codebase("dependency injection patterns")
  2. Orchestrator provides context from local RAG
  3. Codex generates refactor plan
  4. Requests: validate_edits([...edits...])
  5. Orchestrator validates in shadow workspace (local)
  6. If errors → Codex refines → validate again
  7. If clean → return to user
  ↓
User sees: Diff with confidence score, applies
```

**Performance:**
- Latency: 1-5 seconds (API call + processing)
- Throughput: Varies (GPT-4: ~30 tok/s, Groq: ~300 tok/s)
- Cost: $0.10-$5.00 per request (standard API pricing)
- Quality: Highest (frontier models)

### Routing Examples

**Example 1: Simple Completion**
```
Task: User types "function calculateTotal" and hits Tab
Complexity: 2/10 (single-line completion)
→ Route: LOCAL (1.5B draft model)
→ Latency: 80ms
→ Cost: $0
```

**Example 2: Medium Refactor**
```
Task: "Rename UserService to AccountService across 8 files"
Complexity: 6/10 (multi-file, but straightforward)
→ Route: SWARM (find peer with Qwen 32B)
→ Latency: 600ms
→ Cost: $0.08
```

**Example 3: Complex Architecture**
```
Task: "Design a new event-driven architecture for this module"
Complexity: 10/10 (requires deep reasoning)
→ Route: CLOUD (Codex)
→ Latency: 4s
→ Cost: $1.20
```

**Example 4: Sensitive Code**
```
Task: "Fix this authentication bug" (contains API keys in context)
Complexity: 7/10
Privacy: HIGH (secrets detected)
→ Route: LOCAL (forced, despite complexity)
→ Latency: 300ms
→ Cost: $0
→ User notified: "Using local model due to sensitive data"
```

### User Controls

**Settings UI:**
```toml
[inference.routing]
default = "auto"  # auto | local | swarm | cloud

# Auto-routing thresholds
[inference.thresholds]
local_max_complexity = 5       # 1-10 scale
swarm_max_cost = 0.25          # $ per request
cloud_min_quality = 8          # only use cloud for hardest tasks
force_local_for_secrets = true # never send secrets to cloud/swarm

# Budget controls
[inference.budget]
daily_limit = 10.00  # USD
warn_at = 7.50       # warn when 75% spent
pause_at = 10.00     # pause cloud/swarm when exhausted

# Privacy controls
[inference.privacy]
prefer_local = true        # prefer local when possible
allow_swarm = true         # enable P2P inference
redact_secrets = true      # auto-redact API keys, etc.
audit_log = true           # log all requests
```

**Runtime Dashboard:**
```
┌─────────────────────────────────────┐
│ Inference Usage (Today)             │
├─────────────────────────────────────┤
│ Local:  142 requests ($0.00)        │
│ Swarm:   28 requests ($2.45)        │
│ Cloud:    5 requests ($6.80)        │
├─────────────────────────────────────┤
│ Total: $9.25 / $10.00 budget        │
│                                     │
│ [●●●●●●●●●○] 92% used               │
│                                     │
│ Estimated savings vs. all-cloud:    │
│ $47.60 (84% cheaper)                │
└─────────────────────────────────────┘
```

---

## Technology Stack

### Rust Crates (Backend)

| Purpose | Crate | Version | Notes |
|---------|-------|---------|-------|
| **Code Intelligence** |
| Tree-sitter | `tree-sitter` | 0.24 | Parsing + AST |
| Tree-sitter grammars | `tree-sitter-{rust,typescript,python,...}` | latest | Language grammars |
| LSP client | `tower-lsp` | 0.20 | Language Server Protocol |
| Git operations | `git2` | 0.19 | Repository access |
| **Indexing & Search** |
| File watcher | `notify` | 7.0 | Cross-platform file watching |
| Vector DB | `sqlite-vec` (extension) | 0.1 | SQLite vector extension |
| Embeddings | `ort` (ONNX Runtime) | 2.0 | Local embedding inference |
| Merkle trees | `merkle_hash` | 3.7 | Incremental diff tracking |
| **Inference** |
| llama.cpp bindings | `llama-cpp-rs` | 0.16 | Local LLM inference |
| MLX bindings (macOS) | `mlx-rs` | 0.1 | Apple Silicon optimization |
| Tokenizers | `tokenizers` | 0.20 | HuggingFace tokenizers |
| **Nostr & DVM Integration** |
| Nostr client | `nostr-sdk` | 0.36 | Nostr protocol client |
| Nostr types | `nostr` | 0.36 | Core Nostr types (Event, Keys, etc.) |
| WebSocket client | `tokio-tungstenite` | 0.24 | WebSocket for relay connections |
| NIP-57 (Zaps) | Built into `nostr-sdk` | - | Lightning payments over Nostr |
| **Payments** |
| Lightning | `ldk` (Lightning Dev Kit) | 0.0.124 | Lightning Network node |
| Bitcoin wallet | `bdk` (Bitcoin Dev Kit) | 1.0 | On-chain wallet |
| **Existing Stack** |
| Tauri framework | `tauri` | 2.0 | Desktop app |
| ACP protocol | `agent-client-protocol` | 0.7 | Agent communication |
| Database | `rusqlite` | 0.36 | SQLite access |
| Async runtime | `tokio` | 1.0 | Async execution |
| WebSocket | `axum` | 0.8 | Real-time server |
| Serialization | `serde` + `serde_json` | 1.0 | JSON handling |
| **Scheduling & Orchestration** |
| Cron scheduling | `tokio-cron-scheduler` | 0.13 | Scheduled prompts |
| Async channels | `tokio::sync::mpsc` | (builtin) | Task queues |

### Frontend (TypeScript/React)

| Purpose | Package | Version | Notes |
|---------|---------|---------|-------|
| **UI Components** |
| Chat interface | `@assistant-ui/react` | 0.11 | Existing chat UI |
| Code editor | `@monaco-editor/react` | 4.6 | VS Code editor component |
| Diff viewer | `react-diff-viewer-continued` | 3.3 | Side-by-side diffs |
| **Utilities** |
| Markdown | `react-markdown` | 9.0 | Render markdown |
| Syntax highlighting | `prism-react-renderer` | 2.4 | Code highlighting |
| Icons | `lucide-react` | 0.460 | Icon library |

### AI Models

#### Completion Models

**Tier 1: Fast (Draft)**
- **Qwen2.5-Coder-1.5B-Instruct** (Q5_K_M)
  - Size: ~1.2GB
  - Speed: 40-60 tok/s on M2
  - Use: Real-time completions, draft in speculative decoding

**Tier 2: Quality (Verify/Chat)**
- **Qwen2.5-Coder-7B-Instruct** (Q4_K_M)
  - Size: ~4.8GB
  - Speed: 15-25 tok/s on M2
  - Use: Verify in speculative decoding, chat, edits

- **DeepSeek-Coder-V2-Lite-Instruct** (Q4_K_M)
  - Size: ~8.5GB
  - Speed: 12-20 tok/s on M2
  - Use: Alternative quality model, better at reasoning

**Tier 3: Heavy (Optional)**
- **Qwen2.5-Coder-32B-Instruct** (Q4_K_M)
  - Size: ~18GB
  - Speed: 5-10 tok/s on M2 Ultra
  - Use: Complex refactors, high-end hardware only

#### Embedding Models

- **BAAI/bge-small-en-v1.5** (ONNX)
  - Size: 133MB
  - Dim: 384
  - Speed: 500 chunks/sec
  - Use: Default for fast indexing

- **BAAI/bge-large-en-v1.5** (ONNX)
  - Size: 1.3GB
  - Dim: 1024
  - Speed: 150 chunks/sec
  - Use: Higher quality retrieval

### Hardware Requirements

**Minimum (Basic Completions):**
- Apple M1/M2 or AMD Ryzen 5 / Intel i5 (8 cores)
- 16GB RAM
- 10GB disk space (models + index)

**Recommended (Full Features):**
- Apple M2 Pro/Max or AMD Ryzen 7 / Intel i7
- 32GB RAM
- 20GB disk space

**Optimal (Fast Performance):**
- Apple M3 Max or NVIDIA RTX 4070+
- 64GB RAM
- 50GB SSD space

---

## Implementation Roadmap

### Phase 1: MVP (Weeks 1-6)

**Goal:** Local completions + basic indexing

#### Week 1-2: Foundation
- [ ] Set up `oa-indexer` crate structure
- [ ] Implement file watcher (notify crate)
- [ ] Integrate tree-sitter parsers (Rust, TS, Python, Go)
- [ ] Basic chunking logic (top-level functions only)
- [ ] Extend tinyvex schema for code_chunks table

#### Week 3-4: Embeddings & Search
- [ ] Integrate ONNX Runtime for local embeddings
- [ ] Download and bundle bge-small-en-v1.5 model
- [ ] Implement sqlite-vec integration
- [ ] Build indexing pipeline (watch → parse → embed → store)
- [ ] Expose Tauri command: `search_codebase(query)`
- [ ] Basic UI for index status (frontend)

#### Week 5-6: Completions
- [ ] Set up `oa-inference` crate
- [ ] Integrate llama.cpp bindings
- [ ] Model download/management system
- [ ] Implement basic completion engine (single model, no speculation)
- [ ] Context builder (file content + RAG top-3 chunks)
- [ ] Tauri command: `request_completion(...)`
- [ ] Frontend: Inline completion UI (Monaco editor plugin)

**MVP Demo:**
- Type in editor → get local completions (1-3 seconds latency)
- Ask in chat: "Where is the login function?" → RAG search works
- Index status shows in UI (X files indexed)

### Phase 2: Quality & Validation (Weeks 7-14)

#### Week 7-8: Speculative Decoding
- [ ] Load two models (draft + verify)
- [ ] Implement speculative decoding algorithm
- [ ] KV cache sharing between models
- [ ] Benchmark and tune acceptance rate
- [ ] **Target: <200ms first token, 20+ tok/s sustained**

#### Week 9-10: Shadow Workspace
- [ ] Implement APFS clonefile (macOS)
- [ ] Implement reflink fallback (Linux/Windows)
- [ ] LSP client manager with tower-lsp
- [ ] Auto-detect and start LSP servers
- [ ] Validation workflow (apply → check → report)

#### Week 11-12: Fast Apply
- [ ] Multi-file edit prompt engineering
- [ ] Unified diff parser
- [ ] Integration with shadow workspace
- [ ] Diff UI in frontend (react-diff-viewer)
- [ ] Apply/reject workflow

#### Week 13-14: Composer Integration
- [ ] Extend ACP protocol with new tools:
  - `codebase_search`
  - `get_symbol_definition`
  - `validate_in_shadow`
- [ ] Agent can orchestrate multi-step tasks
- [ ] Progress UI (show agent's thinking)
- [ ] Rollback mechanism

**Phase 2 Demo:**
- Fast completions (<200ms, feels instant)
- Multi-file refactor: "Rename class X to Y" → agent proposes changes → validates → user approves
- Shadow workspace catches type errors before applying

### Phase 3: Polish & Enterprise (Weeks 15-20)

#### Week 15-16: Advanced Indexing
- [ ] Hierarchical chunking (include nested functions)
- [ ] Symbol table extraction (all definitions)
- [ ] Cross-file reference tracking
- [ ] Import graph analysis
- [ ] Smarter context retrieval (use import graph)

#### Week 17-18: Model Management UI
- [ ] Model browser/download UI
- [ ] Profile system (fast/balanced/quality)
- [ ] Quantization options (Q4/Q5/Q8)
- [ ] Model update notifications
- [ ] Disk usage management

#### Week 19: Performance & Optimization
- [ ] Benchmark all systems
- [ ] Profile memory usage
- [ ] Optimize KV cache reuse
- [ ] Reduce index storage size
- [ ] Parallelize embedding batches

#### Week 20: Documentation & Testing
- [ ] User documentation
- [ ] API documentation
- [ ] Integration tests
- [ ] Performance regression tests
- [ ] Example videos/tutorials

**Phase 3 Deliverables:**
- Production-ready local AI coding assistant
- Complete documentation
- Model marketplace/registry
- Benchmarks published

---

## Integration with Existing Systems

### ACP Protocol Extensions

**New Tool Definitions:**

```json
{
  "tools": [
    {
      "name": "codebase_search",
      "description": "Search the entire codebase semantically",
      "input_schema": {
        "type": "object",
        "properties": {
          "query": { "type": "string" },
          "top_k": { "type": "number", "default": 5 },
          "file_filter": { "type": "string", "optional": true }
        }
      }
    },
    {
      "name": "get_symbol_definition",
      "description": "Get the definition of a symbol using LSP",
      "input_schema": {
        "type": "object",
        "properties": {
          "symbol": { "type": "string" },
          "file": { "type": "string" }
        }
      }
    },
    {
      "name": "validate_edits",
      "description": "Validate code edits in shadow workspace",
      "input_schema": {
        "type": "object",
        "properties": {
          "edits": {
            "type": "array",
            "items": {
              "type": "object",
              "properties": {
                "file": { "type": "string" },
                "content": { "type": "string" }
              }
            }
          }
        }
      }
    }
  ]
}
```

**Agent Request Handling:**

```rust
// In oa_acp/client.rs
impl ACPClient {
    async fn handle_agent_request(&self, request: AgentRequest) -> Result<Response> {
        match request {
            AgentRequest::ReadFile(path) => {
                // Existing: read file from workspace
                self.read_file(&path).await
            },

            AgentRequest::WriteFile { path, content } => {
                // Existing: write file to workspace
                self.write_file(&path, &content).await
            },

            // NEW: Codebase search
            AgentRequest::ToolCall { name: "codebase_search", args } => {
                let query = args.get("query").unwrap();
                let top_k = args.get("top_k").unwrap_or(&5);

                let indexer = self.indexer.lock().await;
                let results = indexer.search(query, *top_k).await?;

                Response::ToolResult {
                    results: serde_json::to_value(results)?,
                }
            },

            // NEW: LSP symbol definition
            AgentRequest::ToolCall { name: "get_symbol_definition", args } => {
                let symbol = args.get("symbol").unwrap();
                let file = args.get("file").unwrap();

                let lsp = self.lsp_manager.lock().await;
                let definition = lsp.get_definition(file, symbol).await?;

                Response::ToolResult {
                    results: serde_json::to_value(definition)?,
                }
            },

            // NEW: Shadow validation
            AgentRequest::ToolCall { name: "validate_edits", args } => {
                let edits = args.get("edits").unwrap();

                let mut shadow = ShadowWorkspace::create_from(&self.workspace_root).await?;
                let validation = shadow.apply_edits(edits).await?;

                Response::ToolResult {
                    results: serde_json::to_value(validation)?,
                }
            },
        }
    }
}
```

### Tinyvex Database Extensions

**New Tables:**

```sql
-- Code chunks (indexed content)
CREATE TABLE code_chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_root TEXT NOT NULL,
    file_path TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    chunk_type TEXT NOT NULL,  -- function, class, module, etc.
    chunk_name TEXT,
    language TEXT NOT NULL,
    content TEXT NOT NULL,
    content_hash BLOB NOT NULL,
    indexed_at INTEGER NOT NULL,
    UNIQUE(workspace_root, file_path, start_line, end_line)
);

CREATE INDEX idx_chunks_workspace ON code_chunks(workspace_root);
CREATE INDEX idx_chunks_file ON code_chunks(file_path);
CREATE INDEX idx_chunks_type ON code_chunks(chunk_type);

-- Vector embeddings (via sqlite-vec extension)
CREATE VIRTUAL TABLE vec_chunks USING vec0(
    chunk_id INTEGER PRIMARY KEY,
    embedding FLOAT[384]
);

-- Merkle state for incremental indexing
CREATE TABLE merkle_state (
    workspace_root TEXT PRIMARY KEY,
    merkle_root BLOB NOT NULL,
    last_updated INTEGER NOT NULL,
    file_hashes TEXT NOT NULL  -- JSON map
);

-- Symbol index (for LSP-less goto-definition)
CREATE TABLE symbols (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_root TEXT NOT NULL,
    symbol_name TEXT NOT NULL,
    symbol_type TEXT NOT NULL,  -- function, class, variable, etc.
    file_path TEXT NOT NULL,
    line INTEGER NOT NULL,
    definition_text TEXT,
    UNIQUE(workspace_root, symbol_name, file_path, line)
);

CREATE INDEX idx_symbols_name ON symbols(symbol_name);
CREATE INDEX idx_symbols_workspace ON symbols(workspace_root);
```

### WebSocket Events

**New Event Types:**

```typescript
// Frontend receives these events
type IndexingEvent = {
  type: 'indexing.progress';
  workspace: string;
  progress: {
    files_indexed: number;
    total_files: number;
    current_file: string;
  };
};

type CompletionEvent = {
  type: 'completion.token';
  request_id: string;
  token: string;
  is_final: boolean;
};

type ValidationEvent = {
  type: 'validation.result';
  request_id: string;
  result: {
    success: boolean;
    errors: Diagnostic[];
    warnings: Diagnostic[];
  };
};
```

**Frontend Subscription:**

```typescript
// In useAcpRuntime.tsx or new useCodeIntelligence.tsx
const ws = tinyvexWebSocketSingleton;

ws.subscribe('indexing.progress', (event: IndexingEvent) => {
  setIndexingProgress(event.progress);
});

ws.subscribe('completion.token', (event: CompletionEvent) => {
  appendCompletionToken(event.request_id, event.token);
});
```

### Nostr Infrastructure (Already Exists!)

**OpenAgents already has Nostr integration** (currently macOS-only via SwiftPM). We're extending it for the DVM marketplace:

**Existing Nostr Components:**
- **Nostr SDK**: `/Users/christopherdavid/code/nostr-sdk-ios` (integrated via SwiftPM)
- **NIP-28 Chat**: Chat channels already working (see Episode 177)
- **DVM Awareness**: Custom kinds 6500-6599 already defined for code tasks
- **Event Types**:
  - 6500: Code generation
  - 6501: Code review
  - 6502: Code refactoring
  - 6503: Code Q&A
  - etc. (full range 6500-6599 reserved for OpenAgents)

**New DVM Integration (Rust/Tauri):**

```rust
// Add to tauri/src-tauri/Cargo.toml
[dependencies]
nostr-sdk = "0.36"
nostr = "0.36"

// New module: crates/oa-nostr/
pub struct NostrDvmClient {
    client: NostrClient,
    relays: Vec<String>,
    keys: Keys,
}

impl NostrDvmClient {
    pub fn new(relays: Vec<String>) -> Result<Self> {
        let keys = Keys::generate();  // or load from secure storage
        let client = NostrClient::new(&keys);

        // Connect to relays
        for relay in &relays {
            client.add_relay(relay).await?;
        }

        Ok(Self { client, relays, keys })
    }

    pub async fn publish_handler(&self, kinds: Vec<u16>) -> Result<()> {
        // NIP-89: Announce as DVM handler
        let event = Event::new(
            31990,
            kinds.iter().map(|k| Tag::new(&["k", &k.to_string()])).collect(),
            self.build_handler_metadata(),
        );

        self.client.publish_event(event).await?;
        Ok(())
    }

    pub async fn query_handlers(&self, kind: u16) -> Result<Vec<Event>> {
        // Find DVMs that support this kind
        let filter = Filter::new()
            .kind(31990)
            .custom_tag(Tag::new(&["k", &kind.to_string()]));

        self.client.query(vec![filter]).await
    }
}
```

**Why Nostr DVMs are Perfect:**
- ✅ **Already integrated**: Nostr SDK exists, just need Rust bindings
- ✅ **Decentralized discovery**: No central registry, anyone can be a DVM
- ✅ **Built-in payments**: NIP-57 Zaps = Lightning over Nostr
- ✅ **Custom kinds**: 6500-6599 range already reserved for OpenAgents
- ✅ **Open protocol**: NIPs are public specs, no vendor lock-in
- ✅ **Relay network**: Existing infrastructure (wss://relay.damus.io, etc.)

**Default Relays:**
```toml
[nostr]
relays = [
    "wss://relay.damus.io",
    "wss://relay.nostr.band",
    "wss://nos.lol",
    "wss://relay.openagents.com"  # Our own relay for high availability
]
```

---

## Compute Marketplace & Revenue Sharing

### Marketplace Architecture

**Three Markets in One:**

1. **Compute Market** - Buy/sell GPU time for inference
2. **Plugin Market** - Buy/sell agent tools and extensions
3. **Model Market** - Discover and download open models

### 1. Compute Marketplace

**Sell Compute:**

```rust
pub struct ComputeProvider {
    nostr_keys: Keys,  // Nostr identity (public/private keypair)
    gpu_spec: GpuSpec,
    available_models: Vec<ModelSpec>,
    pricing: PricingConfig,
    lightning_address: String,
    uptime_stats: UptimeStats,
    earnings: Arc<Mutex<EarningsTracker>>,
    nostr_client: NostrClient,
}

impl ComputeProvider {
    pub async fn start_selling(&self) -> Result<()> {
        // 1. Publish NIP-89 handler announcement
        self.publish_handler_announcement().await?;

        // 2. Subscribe to DVM job requests (kind 6500-6599)
        let mut subscription = self.nostr_client.subscribe(vec![
            Filter::new()
                .kinds(vec![6500, 6501, 6502, 6503])  // OpenAgents DVM kinds
                .pubkey(self.nostr_keys.public_key())
        ]).await?;

        // 3. Listen for job requests
        while let Some(event) = subscription.next().await {
            match event.kind {
                6500 => {  // Code generation job
                    // Validate payment (check for pending Zap)
                    if !self.verify_zap_intent(&event).await? {
                        continue;
                    }

                    // Run inference in background
                    let provider = self.clone();
                    tokio::spawn(async move {
                        let result = provider.run_inference(&event).await;
                        provider.publish_result(&event, result).await;
                    });
                },
                _ => {}
            }
        }

        Ok(())
    }

    async fn publish_handler_announcement(&self) -> Result<()> {
        // NIP-89 handler announcement
        let handler_event = Event::new(
            31990,  // Application handler
            vec![
                Tag::new(&["k", "6500"]),  // Supports kind 6500 (code generation)
                Tag::new(&["k", "6501"]),  // Supports kind 6501 (code review)
                Tag::new(&["web", &format!("https://openagents.com/dvm/{}", self.nostr_keys.public_key().to_hex())]),
            ],
            json!({
                "name": "OpenAgents DVM",
                "about": "Code generation and inference",
                "picture": "https://openagents.com/logo.png",
                "models": self.available_models,
                "pricing": self.pricing,
                "gpu": self.gpu_spec,
                "lightning": self.lightning_address,
            }).to_string(),
        );

        self.nostr_client.publish_event(handler_event).await?;
        Ok(())
    }

    pub async fn run_inference(&self, job: InferenceJob) -> Result<InferenceResult> {
        // Load model (use cache if available)
        let model = self.get_or_load_model(&job.model_spec).await?;

        // Run inference with streaming
        let stream = model.generate(&job.prompt, job.params).await?;

        // Track earnings
        let mut earnings = self.earnings.lock().await;
        earnings.add_job(job.payment_amount);

        Ok(InferenceResult { stream })
    }
}
```

**Buy Compute:**

Already covered in Swarm Inference section above.

**Economics:**

```
Provider earns: ~$0.01-0.10 per request
  (90% cheaper than AWS for buyer)

Monthly passive income potential:
  - Low usage (10 req/day): $3-$30/month
  - Medium (50 req/day): $15-$150/month
  - High (200 req/day): $60-$600/month

Network takes: 0% initially (bootstrap network)
  → Future: 2-5% to fund development
```

**Reputation System:**

```rust
pub struct Reputation {
    provider_pubkey: PublicKey,  // Nostr public key
    total_jobs: u64,
    successful_jobs: u64,
    average_latency_ms: f64,
    average_quality_score: f64,  // based on user ratings
    uptime_percentage: f64,
    disputes: Vec<Dispute>,
}

impl Reputation {
    pub fn compute_score(&self) -> f64 {
        let success_rate = self.successful_jobs as f64 / self.total_jobs as f64;
        let latency_score = (5000.0 - self.average_latency_ms).max(0.0) / 5000.0;
        let uptime_score = self.uptime_percentage / 100.0;

        // Weighted average
        success_rate * 0.4
            + latency_score * 0.2
            + self.average_quality_score * 0.3
            + uptime_score * 0.1
    }
}
```

### 2. Plugin Marketplace

**Publish Plugin:**

```typescript
// plugin.manifest.json
{
  "id": "figma-to-react",
  "name": "Figma → React Components",
  "version": "1.2.0",
  "author": "developer123",
  "description": "Generate React components from Figma designs",
  "pricing": {
    "model": "per-use",
    "amount": 100,  // satoshis (~$0.01)
    "currency": "sats"
  },
  "lightning_address": "dev123@getalby.com",
  "tools": [
    {
      "name": "figma_to_component",
      "description": "Convert Figma frame to React component",
      "parameters": {
        "figma_url": "string",
        "component_name": "string",
        "typescript": "boolean"
      }
    }
  ],
  "permissions": [
    "network:figma.com",
    "filesystem:write:src/components/"
  ],
  "signature": "..."  // GPG signature for verification
}
```

**Install Plugin (User Flow):**

```
1. User browses marketplace in app
2. Finds "Figma → React" plugin
3. Clicks "Install"
   ↓
4. App downloads manifest + code
5. Verifies GPG signature (prevent malware)
6. Shows permissions request
7. User approves
   ↓
8. Plugin added to available tools
9. Can use in chat: "Convert this Figma design to React"
   ↓
10. Tool executes → Lightning micropayment sent to author
11. Author earns $0.01
```

**Marketplace Backend:**

```rust
pub struct PluginMarketplace {
    registry: Arc<RwLock<HashMap<PluginId, PluginMetadata>>>,
    lightning: LightningNode,
    ipfs: IpfsClient,  // Store plugin code on IPFS
}

impl PluginMarketplace {
    pub async fn publish_plugin(&self, plugin: PluginBundle) -> Result<PluginId> {
        // 1. Verify signature
        self.verify_signature(&plugin)?;

        // 2. Scan for malware (basic static analysis)
        self.scan_plugin(&plugin).await?;

        // 3. Upload to IPFS
        let cid = self.ipfs.add(&plugin.code).await?;

        // 4. Add to registry
        let id = PluginId::new();
        let metadata = PluginMetadata {
            id,
            manifest: plugin.manifest,
            ipfs_cid: cid,
            published_at: Utc::now(),
        };

        self.registry.write().await.insert(id, metadata.clone());

        // 5. Announce on gossipsub
        self.announce_plugin(&metadata).await?;

        Ok(id)
    }

    pub async fn install_plugin(&self, plugin_id: PluginId) -> Result<InstalledPlugin> {
        // 1. Fetch metadata
        let metadata = self.registry.read().await.get(&plugin_id).cloned()
            .ok_or(anyhow!("Plugin not found"))?;

        // 2. Download from IPFS
        let code = self.ipfs.get(&metadata.ipfs_cid).await?;

        // 3. Verify signature again
        self.verify_plugin_code(&code, &metadata)?;

        // 4. Install locally
        let installed = InstalledPlugin {
            metadata,
            code,
            install_path: self.get_install_path(&plugin_id),
        };

        Ok(installed)
    }

    pub async fn handle_tool_use(&self, plugin_id: PluginId, usage: ToolUsage) -> Result<()> {
        // 1. Get plugin pricing
        let metadata = self.registry.read().await.get(&plugin_id).cloned()
            .ok_or(anyhow!("Plugin not found"))?;

        let price = metadata.manifest.pricing.amount;

        // 2. Pay author via Lightning
        self.lightning.pay(
            &metadata.manifest.lightning_address,
            price,
            format!("Plugin use: {}", metadata.manifest.name),
        ).await?;

        // 3. Track usage stats
        self.record_usage(&plugin_id, &usage).await?;

        Ok(())
    }
}
```

**Revenue Split:**

```
Per-use: $0.01 per tool execution
  → Plugin author: $0.009 (90%)
  → OpenAgents: $0.001 (10% - fund development)

Subscription: $10/month
  → Plugin author: $9.50 (95%)
  → OpenAgents: $0.50 (5%)
```

**Discovery & Ratings:**

```sql
CREATE TABLE plugins (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    author TEXT NOT NULL,
    description TEXT,
    category TEXT,
    pricing_model TEXT,
    price_sats INTEGER,
    downloads INTEGER DEFAULT 0,
    rating REAL DEFAULT 0.0,
    num_ratings INTEGER DEFAULT 0,
    published_at INTEGER,
    ipfs_cid TEXT,
    verified BOOLEAN DEFAULT 0  -- verified by OpenAgents team
);

CREATE TABLE plugin_ratings (
    plugin_id TEXT,
    user_id TEXT,
    rating INTEGER CHECK (rating >= 1 AND rating <= 5),
    review TEXT,
    created_at INTEGER,
    FOREIGN KEY (plugin_id) REFERENCES plugins(id)
);

-- Discovery query
SELECT * FROM plugins
WHERE category = 'code-generation'
ORDER BY (rating * 0.7 + ln(downloads + 1) * 0.3) DESC
LIMIT 20;
```

### 3. Model Marketplace

**Discover Models:**

```toml
[models.registry]
# Curated registry of open models
models = [
    { name = "Qwen2.5-Coder-1.5B", repo = "Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF", quantization = "Q5_K_M", size_mb = 1200 },
    { name = "Qwen2.5-Coder-7B", repo = "Qwen/Qwen2.5-Coder-7B-Instruct-GGUF", quantization = "Q4_K_M", size_mb = 4800 },
    { name = "DeepSeek-Coder-6.7B", repo = "deepseek-ai/deepseek-coder-6.7b-instruct-GGUF", quantization = "Q5_K_M", size_mb = 5200 },
    # ... more models
]
```

**One-Click Download:**

```rust
pub async fn download_model(&self, spec: &ModelSpec) -> Result<PathBuf> {
    let url = format!(
        "https://huggingface.co/{}/resolve/main/{}",
        spec.repo, spec.filename
    );

    let dest = self.models_dir.join(&spec.filename);

    // Resume-capable download with progress
    let mut downloader = Downloader::new(&url, &dest);

    while let Some(progress) = downloader.next().await {
        // Emit progress via WebSocket
        self.emit_progress(DownloadProgress {
            model: spec.name.clone(),
            bytes_downloaded: progress.downloaded,
            total_bytes: progress.total,
            speed_mbps: progress.speed,
        }).await?;
    }

    Ok(dest)
}
```

### Integration with OpenAgents

**Unified UI:**

```
┌─────────────────────────────────────────┐
│ Marketplace                             │
├─────────────────────────────────────────┤
│ Tabs: [ Compute | Plugins | Models ]   │
├─────────────────────────────────────────┤
│                                         │
│  🖥️  Compute Providers (42 online)     │
│  ┌──────────────────────────────────┐  │
│  │ peer-abc123  ●  Qwen 32B         │  │
│  │ Rating: ⭐⭐⭐⭐⭐ (4.9/5)        │  │
│  │ Price: 0.0002 sats/token         │  │
│  │ Latency: 350ms avg               │  │
│  │ [Select]                         │  │
│  └──────────────────────────────────┘  │
│                                         │
│  🔧 Top Plugins                         │
│  ┌──────────────────────────────────┐  │
│  │ Figma → React      ⭐ 4.8/5      │  │
│  │ $0.01/use    1.2k downloads      │  │
│  │ [Install]                        │  │
│  └──────────────────────────────────┘  │
│                                         │
│  🤖 Available Models                    │
│  ┌──────────────────────────────────┐  │
│  │ Qwen2.5-Coder-7B                 │  │
│  │ 4.8GB  Q4_K_M  ✅ Downloaded     │  │
│  └──────────────────────────────────┘  │
│                                         │
│  💰 My Earnings (This Month)            │
│  Compute sold: $47.20                   │
│  Plugins: $12.80                        │
│  Total: $60.00                          │
└─────────────────────────────────────────┘
```

---

## Privacy & Security

### Flexible Privacy Model

**What Stays Local:**
- ✅ All source code (never leaves device)
- ✅ Embeddings generation (ONNX local inference)
- ✅ Vector database (sqlite-vec on disk)
- ✅ Completions (llama.cpp local inference)
- ✅ Shadow workspace validation (local LSP servers)
- ✅ File indexing metadata

**What Can Go to Cloud (User Opt-In):**
- ❌ Nothing by default
- ⚠️ Optional: Send prompts to ACP cloud agents (Claude, GPT-4)
  - User explicitly chooses cloud agent
  - Clear UI indication when cloud is used
  - Can switch to local-only mode

### Data Handling

**Storage Locations:**
```
~/.openagents/
├── models/              # GGUF/ONNX models
│   ├── Qwen2.5-Coder-1.5B-Q5_K_M.gguf
│   ├── Qwen2.5-Coder-7B-Q4_K_M.gguf
│   └── bge-small-en-v1.5.onnx
├── cache/               # Temporary caches
│   └── embeddings/      # Embedding cache (if enabled)
└── config/
    └── settings.toml

~/Library/Application Support/openagents/  # macOS
└── tinyvex.db          # Database with code chunks + vectors

<workspace>/.openagents/
├── .openagentsignore   # Files to exclude
└── index/
    └── merkle.json     # Merkle tree state
```

**Exclusion Patterns (.openagentsignore):**
```gitignore
# Automatically respect .gitignore
# Plus additional exclusions:

node_modules/
target/
.git/
*.min.js
*.bundle.js
dist/
build/
.env
.env.*
*.key
*.pem
credentials.json
```

### Secret Detection

**Built-in scanner:**
```rust
pub struct SecretScanner {
    patterns: Vec<Regex>,
}

impl SecretScanner {
    pub fn scan(&self, content: &str) -> Vec<SecretMatch> {
        // Detect common secrets
        let patterns = vec![
            r"(?i)api[_-]?key['\"]?\s*[:=]\s*['\"]([a-z0-9]{32,})['\"]",
            r"(?i)secret[_-]?key['\"]?\s*[:=]\s*['\"]([a-z0-9]{32,})['\"]",
            r"sk-[a-zA-Z0-9]{48}",  // OpenAI keys
            r"ghp_[a-zA-Z0-9]{36}",  // GitHub tokens
            r"AKIA[0-9A-Z]{16}",     // AWS keys
            // ... more patterns
        ];

        // Return matches with redacted versions
    }

    pub fn redact(&self, content: &str) -> String {
        // Replace secrets with [REDACTED]
    }
}
```

### Sandboxing

**Model Execution:**
- Models run in main process (for GPU access)
- No network access for model inference
- Read-only access to workspace files
- No execution of arbitrary code

**Shadow Workspace:**
- Isolated temporary directory
- LSP servers run in subprocess with limited permissions
- Auto-cleanup on validation complete
- Cannot modify real workspace

---

## Performance Targets

### Indexing Performance

| Metric | Target | Notes |
|--------|--------|-------|
| Initial indexing (10k LOC) | <30s | Background, non-blocking |
| Initial indexing (100k LOC) | <5min | Large repo |
| Incremental update (1 file) | <500ms | Real-time |
| Incremental update (10 files) | <3s | Git pull |
| Search latency (RAG) | <100ms | KNN in sqlite-vec |
| Embedding throughput | 500 chunks/s | Batch processing |

### Completion Performance

| Model Size | First Token | Sustained Throughput | Memory |
|------------|-------------|---------------------|---------|
| 1.5B (draft) | <80ms | 40-60 tok/s | 2GB |
| 7B (verify) | <150ms | 15-25 tok/s | 6GB |
| Speculative (combined) | <100ms | 25-35 tok/s | 8GB |

**Hardware: M2 MacBook Pro (16GB RAM)**

### Shadow Validation Performance

| Operation | Target | Notes |
|-----------|--------|-------|
| Create shadow (100 files) | <1s | APFS clone instant |
| Apply 1 edit | <50ms | File write |
| Apply 10 edits | <300ms | Batch write |
| LSP startup (TypeScript) | <2s | One-time per validation |
| LSP diagnostics (1 file) | <500ms | After edit |
| Total validation (10 files) | <5s | Clone + apply + LSP |

---

## Open Questions & Decisions

### 1. Model Selection Strategy

**Question:** Which models should we bundle by default?

**Options:**
- **A) Minimal**: Only 1.5B draft + 7B verify (~6GB total)
- **B) Balanced**: Add alternative 7B model (~11GB total)
- **C) Complete**: Include 32B for high-end users (~25GB total)

**Recommendation:** Start with **A** (minimal), allow opt-in download for B/C

### 2. MLX vs llama.cpp

**Question:** Use MLX or llama.cpp as primary inference engine on macOS?

**Trade-offs:**

| Aspect | llama.cpp | MLX |
|--------|-----------|-----|
| **Performance** | Good (Metal backend) | Excellent (native Metal) |
| **Model support** | Wide (GGUF standard) | Growing (need conversion) |
| **Cross-platform** | ✅ Yes | ❌ macOS only |
| **Maturity** | Very mature | Newer, improving fast |
| **Integration** | Rust bindings available | Rust bindings experimental |

**Recommendation:** **llama.cpp primary, MLX optional** (feature flag)

### 3. Embedding Model Size

**Question:** Default to bge-small (133MB, 384-dim) or bge-large (1.3GB, 1024-dim)?

**Trade-offs:**
- **bge-small**: 5x faster, much smaller, slightly lower retrieval quality
- **bge-large**: Better retrieval, slower indexing, uses more disk

**Recommendation:** **bge-small default**, allow switching to large in settings

### 4. LSP Server Distribution

**Question:** How do we ensure LSP servers are available?

**Options:**
- **A) User installs**: Document required LSPs, user installs globally
- **B) Bundle LSPs**: Ship typescript-language-server, pyright, etc.
- **C) Hybrid**: Auto-detect installed, offer to download missing

**Recommendation:** **C (hybrid)** - check PATH, offer guided install for missing

### 5. Shadow Workspace Frequency

**Question:** When should we create shadow workspaces?

**Options:**
- **A) Every edit**: Maximum safety, slower UX
- **B) Multi-file only**: Fast for single-file, validate complex changes
- **C) User opt-in**: Manual validation toggle

**Recommendation:** **B (multi-file only)** - balance safety and speed

### 6. Offline ACP Agents

**Question:** Should we create a local-only ACP agent that uses local models directly?

**Benefits:**
- True zero-cloud operation
- No ACP subprocess overhead
- Tighter integration

**Challenges:**
- Duplicate agent logic (vs. Claude Code/Codex)
- Maintain prompt engineering
- Tool calling implementation

**Recommendation:** **Phase 4** - start with external ACP, add local agent later

---

## Success Metrics

### Technical Metrics

- ✅ **Completion latency**: <200ms first token (90th percentile)
- ✅ **Completion quality**: Accept rate >30% (user keeps suggestion)
- ✅ **Search relevance**: Top-3 RAG hits relevant >80% of time
- ✅ **Validation accuracy**: Shadow workspace catches >95% of type errors
- ✅ **Index freshness**: <5s lag from file save to index update
- ✅ **Memory usage**: <4GB idle, <10GB during heavy use

### User Experience Metrics

- ✅ **Time to first completion**: <30s after opening project
- ✅ **Index build time**: <2min for 50k LOC project
- ✅ **Crash rate**: <0.1% of sessions
- ✅ **Response time**: All UI interactions <100ms

### Competitive Benchmarks

**vs. GitHub Copilot:**
- ⚡ **Faster** for single-line completions (local vs. cloud latency)
- 🎯 **Comparable** quality on common patterns
- ⚠️ **Weaker** on rare/obscure libraries (smaller training data)

**vs. Cursor:**
- ⚡ **Faster** completions (local inference)
- 🎯 **Comparable** multi-file refactoring (with shadow workspace)
- ⚠️ **Weaker** on very complex reasoning (no GPT-4 level model locally)
- 🔒 **Better** privacy (fully local)

---

## Next Steps

### Immediate Actions (Week 1)

1. **Set up project structure:**
   ```bash
   mkdir -p tauri/src-tauri/crates/{oa-indexer,oa-shadow,oa-inference}
   cargo new --lib tauri/src-tauri/crates/oa-indexer
   cargo new --lib tauri/src-tauri/crates/oa-shadow
   cargo new --lib tauri/src-tauri/crates/oa-inference
   ```

2. **Add dependencies** to `Cargo.toml`:
   ```toml
   [dependencies]
   notify = "7.0"
   tree-sitter = "0.24"
   tree-sitter-rust = "0.23"
   tree-sitter-typescript = "0.23"
   ort = "2.0"  # ONNX Runtime
   llama-cpp-rs = "0.16"
   tower-lsp = "0.20"
   ```

3. **Download initial models:**
   - Create model download script
   - Fetch bge-small-en-v1.5 (ONNX)
   - Fetch Qwen2.5-Coder-1.5B (GGUF Q5_K_M)

4. **Prototype indexer:**
   - Implement file watcher
   - Parse single Rust file with tree-sitter
   - Extract functions
   - Print chunks (no embedding yet)

### Questions for User

Before proceeding, please confirm:

1. **Model size budget**: Is 6-10GB of models acceptable for default install?
2. **Platform priority**: Should we optimize for macOS first, or cross-platform from day 1?
3. **ACP integration**: Keep existing ACP agents or replace with local-only agent?
4. **UI changes**: Can we embed Monaco editor for better code editing, or keep current text-only UI?
5. **Licensing**: MIT/Apache-2.0 for all new code? What about model licenses (Qwen/DeepSeek)?

---

## Conclusion

This plan outlines a comprehensive path to build a **10x better coding assistant than Cursor** by combining:

### The Hybrid Advantage

**Not Just Local, Not Just Cloud — The Best of Everything:**

- **Local Tier**: Free, fast, private (completions, simple tasks)
- **Swarm Tier**: Cheap, distributed, scales (medium tasks, 90% cheaper than cloud)
- **Cloud Tier**: Highest quality, frontier models (complex tasks, delegate to Codex/Claude Code)

**Intelligent routing** ensures you get the best cost/quality trade-off automatically, or you choose manually.

### The 10x Features

1. ✅ **Better UX**: Desktop + Mobile (no TUI jank)
2. ✅ **Code Overnight**: Scheduled prompts, autonomous agents
3. ✅ **Agent Orchestration**: Delegate to Codex, Claude Code, local models in ONE conversation
4. ✅ **History & Memory**: SQLite-backed, semantic search across past chats
5. ✅ **Hassle-Free Plugins**: One-click install marketplace with revenue sharing
6. ✅ **Open Source**: Community-driven, extensible, transparent
7. ✅ **Hybrid Inference**: Local when private/free, swarm when cheap, cloud when quality matters
8. ✅ **Compute Fracking**: Sell your idle GPU time, earn passive income
9. ✅ **Marketplace**: Developers earn from plugins, create network effects
10. ✅ **All Cursor Features**: Shadow workspace, codebase indexing, fast apply, RAG

### Technical Foundation

**Built on Rust + Tauri + ACP:**
- **Rust crates**: nostr-sdk (DVMs), ldk (Lightning), llama-cpp-rs (local inference), tree-sitter (parsing)
- **Existing strengths**: Tauri desktop app, ACP agent protocol, tinyvex database, WebSocket sync, Nostr integration
- **New systems**: Orchestrator/router, DVM swarm client, compute worker, marketplace

**Performance Targets:**
- Local completions: <200ms first token, 20-35 tok/s
- Swarm inference: <800ms first token, 30-60 tok/s, 90% cheaper than cloud
- Cloud agents: Full Codex/Claude Code quality when needed
- Indexing: <5s lag from file save to index update

### Economic Model

**For Users:**
- Free: Use local tier exclusively ($0/month)
- Cheap: Mix local + swarm ($2-10/month for heavy use, vs. $20+ for Cursor)
- Premium: Use cloud when needed (pay-as-you-go)

**For Contributors:**
- **Plugin Developers**: Earn $0.01-$1 per use, passive income from tools
- **Compute Providers**: Earn $5-$600/month selling idle GPU time
- **Model Creators**: Get downloads, attribution (no revenue yet, but builds community)

**For OpenAgents:**
- 10% of plugin sales (fund development)
- 0-5% of compute market (future, bootstrapping with 0%)
- 100% open-source, sustainable through marketplace fees

### Roadmap

**Phase 1: Core Features (8 weeks)**
- Hybrid orchestrator (local/swarm/cloud routing)
- Local completions (llama.cpp + speculative decoding)
- Codebase indexing (tree-sitter + embeddings + sqlite-vec)
- Shadow workspace validation (APFS clone + LSP)
- Agent delegation to Codex/Claude Code
- Scheduled prompts (overnight coding)

**Phase 2: Marketplace (6 weeks)**
- Nostr DVM network (NIP-90 job requests + NIP-89 discovery)
- Compute marketplace (buy/sell swarm inference via DVMs)
- Plugin marketplace (install, pay, earn)
- Model registry (download, manage)
- Zap payments (NIP-57 Lightning over Nostr)

**Phase 3: Mobile + Polish (6 weeks)**
- iOS/Android apps (synced to desktop)
- Conversation embeddings (semantic history search)
- Advanced analytics (cost tracking, savings dashboard)
- Performance optimization (KV cache reuse, parallel indexing)
- Documentation + tutorials

**Total: ~20 weeks to production-ready 10x product**

### Why This Will Win

**vs. Cursor:**
- ✅ **10x cheaper**: Free local + cheap swarm vs. $20/mo subscription
- ✅ **10x more flexible**: Choose your own trade-off (privacy, cost, quality)
- ✅ **10x more open**: Full source code, community-driven, marketplace
- ✅ **10x more powerful**: Agent orchestration, overnight coding, swarm compute

**vs. Copilot:**
- ✅ **Codebase awareness**: RAG-powered completions vs. local-only context
- ✅ **Multi-file intelligence**: Shadow workspace validation
- ✅ **Cost**: Free local option vs. $10-$19/mo

**vs. Everyone:**
- ✅ **Network effects**: Marketplace creates flywheel (more users → more sellers → cheaper inference → more users)
- ✅ **Compute fracking**: Unlock billions in idle GPU value
- ✅ **Mobile-first**: Code from anywhere (desktop, phone, tablet)
- ✅ **Revenue sharing**: Align incentives with developers

### Let's Ship It! 🚀

The pieces are in place:
- ✅ Tauri desktop app (shipping)
- ✅ ACP protocol (supports Codex, Claude Code)
- ✅ Tinyvex database (conversation storage)
- ✅ WebSocket sync (real-time updates)
- ✅ Bitcoin wallet (Lightning ready)

We just need to:
1. Build the orchestrator (hybrid routing)
2. Integrate local inference (llama.cpp)
3. Add codebase indexing (tree-sitter + sqlite-vec)
4. Launch DVM swarm network (Nostr relays + NIP-90)
5. Build marketplace UI

**6 weeks to MVP. 20 weeks to 10x better than Cursor.**

**Let's build the future of AI coding! 💪🔥**
