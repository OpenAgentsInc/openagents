# Vibe Technical Architecture

Deep technical specification for Vibe's architecture, implementation, and system design.

---

## Table of Contents

1. [System Overview](#system-overview)
2. [OANIX Integration](#oanix-integration)
3. [Agent System](#agent-system)
4. [IDE Architecture](#ide-architecture)
5. [Backend Runtime](#backend-runtime)
6. [Frontend Dev Runtime](#frontend-dev-runtime)
7. [Database Layer](#database-layer)
8. [Deployment Pipeline](#deployment-pipeline)
9. [Real-Time Systems](#real-time-systems)
10. [Security Architecture](#security-architecture)
11. [Native to Browser Extraction](#native-to-browser-extraction)

---

## System Overview

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Vibe Application                             │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Presentation Layer                        │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │   │
│  │  │  Editor  │ │  Design  │ │ Terminal │ │ Preview  │       │   │
│  │  │  Panel   │ │  Mode    │ │  Panel   │ │  Frame   │       │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Application Layer                         │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │   │
│  │  │  Agent   │ │  Project │ │Collabora-│ │   Build  │       │   │
│  │  │ Manager  │ │ Manager  │ │   tion   │ │  System  │       │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                      OANIX Kernel                            │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │   │
│  │  │Namespace │ │FileServ- │ │   WASI   │ │Capability│       │   │
│  │  │ Manager  │ │   ices   │ │ Runtime  │ │ Manager  │       │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Platform Layer                            │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │   │
│  │  │   GPUI   │ │ Wasmtime │ │  SQLite  │ │   HTTP   │       │   │
│  │  │(Renderer)│ │ (WASI)   │ │   (DB)   │ │ (Client) │       │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │   │
│  └─────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
                               │
                    ┌──────────┴──────────┐
                    │                     │
              ┌─────▼─────┐         ┌─────▼─────┐
              │  Native   │         │  Browser  │
              │  (macOS)  │         │  (WASM)   │
              └───────────┘         └───────────┘
```

### Component Summary

| Component | Responsibility | Technology |
|-----------|---------------|------------|
| **Presentation** | User interface rendering | GPUI (native), DOM (browser) |
| **Application** | Business logic, state management | Rust |
| **OANIX Kernel** | Filesystem abstraction, sandboxing | Rust |
| **Platform** | System primitives | Platform-specific |

---

## OANIX Integration

### Namespace Model

Every Vibe project runs inside an OANIX namespace:

```
/                           (root)
├── workspace/              (WorkspaceFs - project files)
│   ├── src/
│   │   ├── App.tsx
│   │   └── components/
│   ├── public/
│   ├── package.json
│   └── vibe.toml
├── logs/                   (LogsFs - build logs, ATIF)
│   ├── build.log
│   ├── preview.log
│   └── atif/
│       └── trajectory.jsonl
├── cap/                    (Capabilities)
│   ├── agents/             (AgentFs - AI agent access)
│   ├── net/                (NetFs - network access)
│   ├── payments/           (PaymentsFs - Bitcoin/Lightning)
│   └── nostr/              (NostrFs - identity/pubsub)
├── db/                     (DatabaseFs - SQLite access)
│   └── main.db
└── preview/                (PreviewFs - built assets)
    └── dist/
```

### FileService Implementations

```rust
// Core trait from OANIX
pub trait FileService: Send + Sync {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError>;
    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError>;
    fn stat(&self, path: &str) -> Result<Metadata, FsError>;
    fn watch(&self, path: &str) -> Result<Box<dyn Watcher>, FsError>;
}

// Vibe-specific implementations
pub struct WorkspaceFs {
    root: PathBuf,
    git: Option<GitRepo>,
}

pub struct LogsFs {
    sessions: HashMap<SessionId, LogSession>,
}

pub struct AgentFs {
    agent_pool: AgentPool,
    current_job: Option<JobId>,
}

pub struct PreviewFs {
    dist_path: PathBuf,
    hot_reload: HotReloadServer,
}
```

### Namespace Configuration

```rust
// Creating a Vibe project namespace
let namespace = Namespace::builder()
    .mount("/workspace", WorkspaceFs::new(project_path))
    .mount("/logs", LogsFs::new())
    .mount("/cap/agents", AgentFs::new(agent_config))
    .mount("/cap/net", NetFs::new(network_policy))
    .mount("/cap/payments", PaymentsFs::new(wallet))
    .mount("/cap/nostr", NostrFs::new(keypair))
    .mount("/db", DatabaseFs::new(db_path))
    .mount("/preview", PreviewFs::new(dist_path))
    .build();

let env = OanixEnv::new(namespace, wasi_runtime);
```

---

## Agent System

### Agent Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Agent System                                 │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │                    Agent Manager                               │ │
│  │  - Job queue management                                        │ │
│  │  - Agent lifecycle                                             │ │
│  │  - Mode switching (Agent/Chat)                                 │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                              │                                      │
│         ┌────────────────────┼────────────────────┐                │
│         ▼                    ▼                    ▼                │
│  ┌─────────────┐      ┌─────────────┐      ┌─────────────┐        │
│  │    Task     │      │    LLM      │      │    Tool     │        │
│  │  Decomposer │      │   Router    │      │  Executor   │        │
│  └─────────────┘      └─────────────┘      └─────────────┘        │
│         │                    │                    │                │
│         └────────────────────┼────────────────────┘                │
│                              ▼                                      │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │                    ATIF Logger                                 │ │
│  │  - Step recording                                              │ │
│  │  - Tool call capture                                           │ │
│  │  - Result logging                                              │ │
│  └───────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

### Agent Job Types

```rust
pub enum AgentJob {
    // Full project scaffolding
    Scaffold {
        prompt: String,
        template: Option<TemplateId>,
    },

    // Single component/page creation
    Create {
        prompt: String,
        target_type: CreateTarget, // Component, Page, Function, etc.
    },

    // Modify existing code
    Modify {
        prompt: String,
        target_files: Vec<PathBuf>,
    },

    // Select element and edit
    SelectAndEdit {
        element_selector: String,
        prompt: String,
    },

    // Debug and fix
    Debug {
        error: ErrorInfo,
        context: Vec<PathBuf>,
    },

    // Chat mode (no code changes)
    Chat {
        message: String,
        history: Vec<ChatMessage>,
    },
}
```

### Agent Tools

```rust
pub trait AgentTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> serde_json::Value;
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError>;
}

// File operations
pub struct FileReadTool;
pub struct FileWriteTool;
pub struct FileDeleteTool;
pub struct FilePatchTool;

// Code intelligence
pub struct CodeSearchTool;
pub struct SymbolLookupTool;
pub struct TypeCheckTool;

// Build/run
pub struct ShellExecTool;
pub struct PreviewRefreshTool;
pub struct BuildTool;

// Database
pub struct DatabaseQueryTool;
pub struct SchemaMigrateTool;

// External
pub struct WebSearchTool;
pub struct ImageGenerateTool;
pub struct ApiCallTool;
```

### LLM Provider Abstraction

```rust
pub trait LLMProvider: Send + Sync {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LLMError>;
    async fn stream(&self, request: CompletionRequest) -> Result<CompletionStream, LLMError>;
}

pub struct MultiProvider {
    providers: Vec<Box<dyn LLMProvider>>,
    router: ProviderRouter,
}

// Supported providers
pub struct AnthropicProvider;    // Claude
pub struct OpenAIProvider;       // GPT-4
pub struct GeminiProvider;       // Google
pub struct OllamaProvider;       // Local models
pub struct FMBridgeProvider;     // Apple Foundation Model
```

### Task Feed System

```rust
pub struct TaskFeed {
    tasks: Vec<TaskItem>,
    subscribers: Vec<TaskFeedSubscriber>,
}

pub struct TaskItem {
    id: TaskId,
    description: String,
    status: TaskStatus,
    started_at: Option<Instant>,
    completed_at: Option<Instant>,
    subtasks: Vec<TaskItem>,
    files_changed: Vec<FileChange>,
}

pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

impl TaskFeed {
    pub fn start_task(&mut self, description: &str) -> TaskId;
    pub fn complete_task(&mut self, id: TaskId);
    pub fn fail_task(&mut self, id: TaskId, error: &str);
    pub fn add_file_change(&mut self, id: TaskId, change: FileChange);
    pub fn subscribe(&mut self) -> TaskFeedReceiver;
}
```

---

## IDE Architecture

### Editor Component

```rust
pub struct VibeEditor {
    // Document management
    open_documents: HashMap<PathBuf, Document>,
    active_document: Option<PathBuf>,

    // Editor state
    tabs: TabBar,
    split_panes: SplitLayout,

    // Features
    syntax_highlighter: SyntaxHighlighter,
    intellisense: IntelliSenseProvider,
    search: SearchEngine,

    // Integration
    file_watcher: FileWatcher,
    git: Option<GitIntegration>,
}

pub struct Document {
    path: PathBuf,
    content: Rope,
    language: LanguageId,
    dirty: bool,
    version: u64,
    selections: Vec<Selection>,
    undo_stack: UndoStack,
}
```

### Design Mode System

```rust
pub struct DesignMode {
    // Selection
    selected_element: Option<ElementSelector>,
    hover_element: Option<ElementSelector>,

    // Panels
    properties_panel: PropertiesPanel,
    theme_panel: ThemePanel,
    layout_panel: LayoutPanel,

    // Live preview connection
    preview_bridge: PreviewBridge,
}

pub struct ElementSelector {
    // CSS selector to locate element
    selector: String,
    // Component location in source
    source_location: SourceLocation,
    // Current computed styles
    computed_styles: ComputedStyles,
}

pub struct PropertiesPanel {
    // Dynamic properties based on element type
    properties: Vec<PropertyEditor>,
}

pub enum PropertyEditor {
    Spacing(SpacingEditor),      // Margin, padding
    Typography(TypographyEditor), // Font, size, weight
    Color(ColorEditor),           // Text, background, border
    Layout(LayoutEditor),         // Flex, grid, position
    Custom(CustomEditor),         // Component-specific props
}
```

### File Tree

```rust
pub struct FileTree {
    root: TreeNode,
    expanded: HashSet<PathBuf>,
    selected: Option<PathBuf>,
    git_status: HashMap<PathBuf, GitStatus>,
}

pub struct TreeNode {
    path: PathBuf,
    name: String,
    kind: NodeKind,
    children: Vec<TreeNode>,
}

pub enum NodeKind {
    Directory,
    File { language: Option<LanguageId> },
}

impl FileTree {
    pub fn refresh(&mut self);
    pub fn expand(&mut self, path: &Path);
    pub fn collapse(&mut self, path: &Path);
    pub fn select(&mut self, path: &Path);
    pub fn create_file(&mut self, path: &Path) -> Result<(), FileError>;
    pub fn create_directory(&mut self, path: &Path) -> Result<(), FileError>;
    pub fn rename(&mut self, from: &Path, to: &Path) -> Result<(), FileError>;
    pub fn delete(&mut self, path: &Path) -> Result<(), FileError>;
}
```

### Terminal Integration

```rust
pub struct VibeTerminal {
    pty: PtyHandle,
    buffer: TerminalBuffer,
    history: CommandHistory,
}

impl VibeTerminal {
    // PTY connects through OANIX
    pub fn new(namespace: &Namespace) -> Self {
        let pty = namespace.open("/dev/pty0", OpenFlags::RDWR).unwrap();
        // ...
    }

    pub fn write(&mut self, data: &[u8]);
    pub fn read(&mut self) -> Vec<u8>;
    pub fn resize(&mut self, cols: u16, rows: u16);
}
```

---

## Backend Runtime

### Rust Backend Framework

```rust
// crates/vibe-backend/src/lib.rs

pub struct Request {
    method: Method,
    path: String,
    headers: HeaderMap,
    body: Option<Bytes>,
    params: HashMap<String, String>,
}

pub struct Response {
    status: StatusCode,
    headers: HeaderMap,
    body: Option<Bytes>,
}

impl Response {
    pub fn json<T: Serialize>(data: T) -> Self;
    pub fn text(text: &str) -> Self;
    pub fn html(html: &str) -> Self;
    pub fn redirect(url: &str) -> Self;
    pub fn status(code: StatusCode) -> Self;
}

pub struct Router {
    routes: Vec<Route>,
}

impl Router {
    pub fn new() -> Self;
    pub fn get(self, path: &str, handler: impl Handler) -> Self;
    pub fn post(self, path: &str, handler: impl Handler) -> Self;
    pub fn put(self, path: &str, handler: impl Handler) -> Self;
    pub fn delete(self, path: &str, handler: impl Handler) -> Self;
}

pub trait Handler: Send + Sync {
    async fn handle(&self, req: Request) -> Response;
}
```

### Backend Build Pipeline

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Backend Build Pipeline                            │
│                                                                     │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐             │
│  │   Source    │    │   Compile   │    │   Output    │             │
│  │   Files     │───▶│   (cargo)   │───▶│   WASM      │             │
│  │  (Rust)     │    │             │    │             │             │
│  └─────────────┘    └─────────────┘    └─────────────┘             │
│                                               │                     │
│                                               ▼                     │
│                     ┌─────────────────────────────────────┐        │
│                     │          OANIX Runtime              │        │
│                     │                                     │        │
│                     │  ┌─────────┐     ┌─────────┐       │        │
│                     │  │  WASI   │     │ Request │       │        │
│                     │  │ Module  │◀───▶│ Router  │       │        │
│                     │  └─────────┘     └─────────┘       │        │
│                     │                                     │        │
│                     └─────────────────────────────────────┘        │
└─────────────────────────────────────────────────────────────────────┘
```

### Request Routing

```rust
pub struct BackendRuntime {
    router: Router,
    wasm_module: wasmtime::Module,
    instance: wasmtime::Instance,
}

impl BackendRuntime {
    pub async fn handle_request(&self, req: Request) -> Response {
        // Find matching route
        let route = self.router.match_route(&req.method, &req.path);

        // Extract path parameters
        let params = route.extract_params(&req.path);
        let req = req.with_params(params);

        // Execute handler in WASM
        let result = self.instance
            .get_func(&route.handler_name)
            .call(req.serialize())
            .await;

        Response::deserialize(result)
    }
}
```

---

## Frontend Dev Runtime

### Build System

```
┌─────────────────────────────────────────────────────────────────────┐
│                   Frontend Build System                              │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Module Graph                               │   │
│  │                                                               │   │
│  │         index.html                                            │   │
│  │             │                                                 │   │
│  │         main.tsx                                              │   │
│  │         /      \                                              │   │
│  │    App.tsx    styles.css                                      │   │
│  │    /     \                                                    │   │
│  │ Header  Footer                                                │   │
│  │                                                               │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│  ┌─────────────┐  ┌─────────▼─────────┐  ┌─────────────┐          │
│  │  File       │  │   Transformer     │  │   Bundler   │          │
│  │  Watcher    │──│   (SWC/ESBuild)   │──│             │          │
│  └─────────────┘  └───────────────────┘  └─────────────┘          │
│                                                   │                 │
│                                          ┌───────▼───────┐         │
│                                          │  HMR Server   │         │
│                                          └───────────────┘         │
└─────────────────────────────────────────────────────────────────────┘
```

### Module Resolution

```rust
pub struct ModuleGraph {
    modules: HashMap<ModuleId, Module>,
    dependencies: HashMap<ModuleId, Vec<ModuleId>>,
    entry_points: Vec<ModuleId>,
}

pub struct Module {
    id: ModuleId,
    path: PathBuf,
    source: String,
    transformed: Option<String>,
    module_type: ModuleType,
    dependencies: Vec<Dependency>,
}

pub enum ModuleType {
    JavaScript,
    TypeScript,
    JSX,
    TSX,
    CSS,
    JSON,
    Asset,
}

impl ModuleGraph {
    pub fn add_entry_point(&mut self, path: &Path) -> Result<ModuleId, ResolveError>;
    pub fn resolve_import(&self, from: ModuleId, specifier: &str) -> Result<ModuleId, ResolveError>;
    pub fn invalidate(&mut self, path: &Path);
    pub fn get_affected(&self, module: ModuleId) -> Vec<ModuleId>;
}
```

### Transform Pipeline

```rust
pub struct TransformPipeline {
    typescript: TypeScriptTransformer,
    jsx: JsxTransformer,
    css: CssTransformer,
}

impl TransformPipeline {
    pub fn transform(&self, module: &Module) -> Result<TransformResult, TransformError> {
        match module.module_type {
            ModuleType::TypeScript | ModuleType::TSX => {
                let js = self.typescript.transform(&module.source)?;
                let js = self.jsx.transform(&js)?;
                Ok(TransformResult::JavaScript(js))
            }
            ModuleType::CSS => {
                let css = self.css.transform(&module.source)?;
                Ok(TransformResult::CSS(css))
            }
            // ...
        }
    }
}
```

### Hot Module Replacement

```rust
pub struct HMRServer {
    clients: Vec<WebSocket>,
    module_graph: Arc<Mutex<ModuleGraph>>,
}

impl HMRServer {
    pub fn notify_change(&self, changed_modules: Vec<ModuleId>) {
        let update = HMRUpdate {
            modules: changed_modules.iter()
                .map(|id| self.get_module_update(*id))
                .collect(),
        };

        for client in &self.clients {
            client.send(update.serialize());
        }
    }
}

// Client-side HMR runtime
pub struct HMRUpdate {
    modules: Vec<ModuleUpdate>,
}

pub struct ModuleUpdate {
    id: String,
    code: String,
    dependencies: Vec<String>,
}
```

---

## Database Layer

### SQLite Integration

```rust
pub struct VibeDatabase {
    conn: rusqlite::Connection,
    migrations: MigrationRunner,
}

impl VibeDatabase {
    pub fn new(path: &Path) -> Result<Self, DatabaseError>;

    // Schema management
    pub fn run_migrations(&self, migrations: &[Migration]) -> Result<(), MigrationError>;
    pub fn get_schema(&self) -> Schema;

    // Query interface
    pub fn query<T: FromRow>(&self, sql: &str, params: &[Value]) -> Result<Vec<T>, QueryError>;
    pub fn execute(&self, sql: &str, params: &[Value]) -> Result<usize, QueryError>;

    // Table operations
    pub fn create_table(&self, table: &TableDef) -> Result<(), SchemaError>;
    pub fn alter_table(&self, name: &str, changes: &[ColumnChange]) -> Result<(), SchemaError>;
    pub fn drop_table(&self, name: &str) -> Result<(), SchemaError>;
}
```

### Schema Definition

```rust
pub struct Schema {
    tables: Vec<Table>,
    indexes: Vec<Index>,
    foreign_keys: Vec<ForeignKey>,
}

pub struct Table {
    name: String,
    columns: Vec<Column>,
}

pub struct Column {
    name: String,
    data_type: DataType,
    nullable: bool,
    default: Option<DefaultValue>,
    primary_key: bool,
    unique: bool,
}

pub enum DataType {
    Integer,
    Real,
    Text,
    Blob,
    Boolean,
    Timestamp,
    Json,
}
```

### Migration System

```rust
pub struct Migration {
    version: u64,
    name: String,
    up: String,    // SQL to apply
    down: String,  // SQL to rollback
}

pub struct MigrationRunner {
    db: Arc<VibeDatabase>,
    applied: Vec<u64>,
}

impl MigrationRunner {
    pub fn pending(&self, migrations: &[Migration]) -> Vec<&Migration>;
    pub fn apply(&self, migration: &Migration) -> Result<(), MigrationError>;
    pub fn rollback(&self, migration: &Migration) -> Result<(), MigrationError>;
    pub fn generate(&self, name: &str, from: &Schema, to: &Schema) -> Migration;
}
```

---

## Deployment Pipeline

### Build Process

```
┌─────────────────────────────────────────────────────────────────────┐
│                     Deployment Pipeline                              │
│                                                                     │
│  ┌─────────────┐                                                    │
│  │   Trigger   │  (Publish button click)                            │
│  └──────┬──────┘                                                    │
│         │                                                           │
│         ▼                                                           │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Build Stage                               │   │
│  │  1. TypeScript compilation                                   │   │
│  │  2. Asset optimization (images, fonts)                       │   │
│  │  3. CSS minification                                         │   │
│  │  4. JavaScript bundling & minification                       │   │
│  │  5. Backend WASM compilation                                 │   │
│  │  6. Generate manifest                                        │   │
│  └─────────────────────────────────────────────────────────────┘   │
│         │                                                           │
│         ▼                                                           │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                   Security Scan                              │   │
│  │  - Dependency vulnerabilities                                │   │
│  │  - Hardcoded secrets                                         │   │
│  │  - Security anti-patterns                                    │   │
│  └─────────────────────────────────────────────────────────────┘   │
│         │                                                           │
│         ▼                                                           │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Deploy Stage                              │   │
│  │  1. Upload to edge storage                                   │   │
│  │  2. Deploy backend functions                                 │   │
│  │  3. Run database migrations                                  │   │
│  │  4. Invalidate CDN cache                                     │   │
│  │  5. Update DNS (if needed)                                   │   │
│  │  6. Health check                                             │   │
│  └─────────────────────────────────────────────────────────────┘   │
│         │                                                           │
│         ▼                                                           │
│  ┌─────────────┐                                                    │
│  │   Live!     │                                                    │
│  └─────────────┘                                                    │
└─────────────────────────────────────────────────────────────────────┘
```

### Hosting Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                      Hosting Infrastructure                          │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                         CDN Edge                             │   │
│  │  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐          │   │
│  │  │ NYC │ │ SFO │ │ AMS │ │ TYO │ │ SYD │ │ ... │          │   │
│  │  └─────┘ └─────┘ └─────┘ └─────┘ └─────┘ └─────┘          │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│                              ▼                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                      Origin Server                           │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │   │
│  │  │    Static    │  │   Function   │  │   Database   │      │   │
│  │  │    Assets    │  │   Runtime    │  │   (SQLite)   │      │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘      │   │
│  └─────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Real-Time Systems

### Collaboration Protocol

```rust
pub enum CollaborationMessage {
    // Presence
    UserJoined { user_id: UserId, cursor: Option<Position> },
    UserLeft { user_id: UserId },
    CursorMoved { user_id: UserId, position: Position },

    // Document changes
    DocumentEdit { document_id: DocumentId, operations: Vec<Operation> },
    DocumentSaved { document_id: DocumentId },

    // Comments
    CommentAdded { comment: Comment },
    CommentResolved { comment_id: CommentId },

    // Agent activity
    AgentStarted { job_id: JobId },
    AgentProgress { job_id: JobId, task: TaskItem },
    AgentCompleted { job_id: JobId, result: JobResult },
}

pub struct CollaborationHub {
    sessions: HashMap<SessionId, Session>,
    documents: HashMap<DocumentId, SharedDocument>,
}

impl CollaborationHub {
    pub fn broadcast(&self, project_id: ProjectId, message: CollaborationMessage);
    pub fn sync_document(&self, document_id: DocumentId, from_version: u64) -> Vec<Operation>;
}
```

### CRDT for Concurrent Editing

```rust
// Using Yjs-style CRDT for document sync
pub struct SharedDocument {
    id: DocumentId,
    content: YText,      // CRDT text type
    awareness: Awareness, // Cursor positions
    history: UndoManager,
}

impl SharedDocument {
    pub fn apply_local(&mut self, operation: Operation) -> Update;
    pub fn apply_remote(&mut self, update: Update);
    pub fn get_state_vector(&self) -> StateVector;
    pub fn encode_diff(&self, from: &StateVector) -> Update;
}
```

---

## Security Architecture

### Authentication Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Authentication Flow                               │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Nostr Identity                            │   │
│  │                                                               │   │
│  │  1. Generate keypair locally                                  │   │
│  │  2. Store private key securely                                │   │
│  │  3. Public key = identity                                     │   │
│  │                                                               │   │
│  │  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐      │   │
│  │  │   Local     │    │   Sign      │    │  Verify on  │      │   │
│  │  │   Keypair   │───▶│   Request   │───▶│   Server    │      │   │
│  │  └─────────────┘    └─────────────┘    └─────────────┘      │   │
│  │                                                               │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                  Optional: Account Linking                   │   │
│  │                                                               │   │
│  │  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐      │   │
│  │  │   Nostr     │    │   GitHub    │    │   Cross-    │      │   │
│  │  │   Identity  │───▶│   OAuth     │───▶│   Device    │      │   │
│  │  └─────────────┘    └─────────────┘    └─────────────┘      │   │
│  │                                                               │   │
│  └─────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

### Capability-Based Security

```rust
// All capabilities granted through namespace mounts
pub struct SecurityPolicy {
    // Network access
    allowed_domains: Vec<String>,
    blocked_domains: Vec<String>,

    // File access
    writable_paths: Vec<PathBuf>,
    readable_paths: Vec<PathBuf>,

    // Execution
    allowed_commands: Vec<String>,
    max_execution_time: Duration,

    // Resource limits
    max_memory: usize,
    max_cpu_time: Duration,
    max_file_size: usize,
}

impl OanixEnv {
    pub fn with_policy(mut self, policy: SecurityPolicy) -> Self {
        // Network capability
        if !policy.allowed_domains.is_empty() {
            self.namespace.mount("/cap/net",
                NetFs::new(policy.allowed_domains, policy.blocked_domains));
        }

        // Restrict workspace
        self.namespace.mount("/workspace",
            WorkspaceFs::new(policy.writable_paths, policy.readable_paths));

        self
    }
}
```

---

## Native to Browser Extraction

### Abstraction Boundaries

```
┌─────────────────────────────────────────────────────────────────────┐
│                   Platform-Agnostic Core                             │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                 Business Logic (Rust)                        │   │
│  │  - Agent system                                               │   │
│  │  - Project management                                         │   │
│  │  - Build pipeline                                             │   │
│  │  - Collaboration                                              │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                 OANIX Abstractions                           │   │
│  │  - FileService trait                                          │   │
│  │  - Namespace model                                            │   │
│  │  - Capability system                                          │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
└──────────────────────────────┼──────────────────────────────────────┘
                               │
              ┌────────────────┴────────────────┐
              │                                 │
    ┌─────────▼─────────┐           ┌─────────▼─────────┐
    │  Native Backend   │           │  Browser Backend  │
    │                   │           │                   │
    │  - std::fs        │           │  - IndexedDB      │
    │  - tokio          │           │  - Web APIs       │
    │  - Wasmtime       │           │  - Browser WASM   │
    │  - GPUI (Metal)   │           │  - DOM            │
    └───────────────────┘           └───────────────────┘
```

### Platform Trait Implementations

```rust
// Platform-agnostic trait
pub trait PlatformStorage {
    async fn read(&self, key: &str) -> Result<Vec<u8>, StorageError>;
    async fn write(&self, key: &str, data: &[u8]) -> Result<(), StorageError>;
    async fn delete(&self, key: &str) -> Result<(), StorageError>;
    async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError>;
}

// Native implementation
#[cfg(not(target_arch = "wasm32"))]
pub struct NativeStorage {
    root: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl PlatformStorage for NativeStorage {
    async fn read(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        tokio::fs::read(self.root.join(key)).await.map_err(Into::into)
    }
    // ...
}

// Browser implementation
#[cfg(target_arch = "wasm32")]
pub struct BrowserStorage {
    db: web_sys::IdbDatabase,
}

#[cfg(target_arch = "wasm32")]
impl PlatformStorage for BrowserStorage {
    async fn read(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        // IndexedDB operations
    }
    // ...
}
```

### UI Layer Abstraction

```rust
// GPUI compiles to both native and WASM
// Native: Metal/Vulkan rendering
// Browser: DOM rendering (future)

pub trait VibeUI {
    fn render_editor(&self, state: &EditorState) -> impl Element;
    fn render_file_tree(&self, state: &FileTreeState) -> impl Element;
    fn render_terminal(&self, state: &TerminalState) -> impl Element;
    fn render_preview(&self, state: &PreviewState) -> impl Element;
}

// Same component code works in both contexts
pub struct EditorComponent {
    state: EditorState,
}

impl Render for EditorComponent {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        // This code compiles to native GPUI or browser DOM
        div()
            .flex()
            .flex_col()
            .child(self.render_tabs(cx))
            .child(self.render_content(cx))
            .child(self.render_status_bar(cx))
    }
}
```

### Build Targets

```toml
# Cargo.toml

[lib]
crate-type = ["cdylib", "rlib"]

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
tokio = { version = "1", features = ["full"] }
wasmtime = "29"

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
web-sys = { version = "0.3", features = ["Window", "Document", "IdbDatabase"] }
js-sys = "0.3"
```

```bash
# Build for native
cargo build --release

# Build for browser
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen target/wasm32-unknown-unknown/release/vibe.wasm --out-dir pkg
```

---

## Crate Structure

```
crates/vibe/
├── Cargo.toml
├── README.md
├── docs/
│   ├── PRODUCT.md
│   ├── FEATURES.md
│   └── ARCHITECTURE.md
└── src/
    ├── lib.rs
    ├── error.rs
    ├── config.rs
    ├── traits.rs
    │
    ├── agent/
    │   ├── mod.rs
    │   ├── manager.rs
    │   ├── job.rs
    │   ├── tools/
    │   │   ├── mod.rs
    │   │   ├── file_ops.rs
    │   │   ├── code_intel.rs
    │   │   ├── build.rs
    │   │   └── external.rs
    │   └── feed.rs
    │
    ├── ide/
    │   ├── mod.rs
    │   ├── editor.rs
    │   ├── tree.rs
    │   ├── tabs.rs
    │   ├── terminal.rs
    │   ├── preview.rs
    │   └── atif_viewer.rs
    │
    ├── design/
    │   ├── mod.rs
    │   ├── mode.rs
    │   ├── properties.rs
    │   ├── theme.rs
    │   └── layout.rs
    │
    ├── backend/
    │   ├── mod.rs
    │   ├── runtime.rs
    │   ├── router.rs
    │   └── builder.rs
    │
    ├── devrt/
    │   ├── mod.rs
    │   ├── graph.rs
    │   ├── transform.rs
    │   ├── bundler.rs
    │   └── hmr.rs
    │
    ├── db/
    │   ├── mod.rs
    │   ├── schema.rs
    │   ├── migration.rs
    │   └── query.rs
    │
    ├── deploy/
    │   ├── mod.rs
    │   ├── build.rs
    │   ├── security.rs
    │   └── hosting.rs
    │
    ├── collab/
    │   ├── mod.rs
    │   ├── presence.rs
    │   ├── sync.rs
    │   └── comments.rs
    │
    └── platform/
        ├── mod.rs
        ├── native.rs
        └── browser.rs

crates/vibe-backend/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── request.rs
    ├── response.rs
    ├── router.rs
    └── json.rs
```

---

*This architecture document provides the technical foundation for implementing Vibe. Components should be built incrementally, starting with the OANIX integration and core IDE functionality.*
