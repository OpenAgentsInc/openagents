# Plan: Dioxus + WGPUI Hybrid Architecture

## Summary

Replace ~150+ Zed/GPUI crates with a lean architecture using:
- **Dioxus** for standard web UI with SSR + hydration
- **wgpui** for canvas-based rich graphics (chat thread rendering, visualizations)
- **Hybrid deployment**: Dioxus pages with embedded WebGL/WebGPU canvases
- **First goal**: Rebuild MechaCoder chat UI

---

## Decision: Dioxus

**Why Dioxus:**
- Cross-platform potential (web + desktop + mobile) if needed later
- React-like DX - familiar hooks and component model
- Experimental WGPU renderer could eventually unify with wgpui
- Hot reload with `dx serve` for fast iteration
- SSR + hydration support
- 20k+ GitHub stars, active development

---

## Proposed Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    OpenAgents Web Platform                       │
├─────────────────────────────────────────────────────────────────┤
│  Dioxus App (SSR + Hydration)                                   │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐            │
│  │ Login/Auth   │ │  Dashboard   │ │  Settings    │            │
│  │ (standard)   │ │ (standard)   │ │ (standard)   │            │
│  └──────────────┘ └──────────────┘ └──────────────┘            │
│  ┌─────────────────────────────────────────────────┐            │
│  │            MechaCoder Page (hybrid)              │            │
│  │  ┌─────────────────────────────────────────┐    │            │
│  │  │     <WgpuiCanvas> Component             │    │            │
│  │  │  - Chat thread rendering (GPU)          │    │            │
│  │  │  - Code visualization                   │    │            │
│  │  │  - Rich text with cosmic-text           │    │            │
│  │  └─────────────────────────────────────────┘    │            │
│  └─────────────────────────────────────────────────┘            │
├─────────────────────────────────────────────────────────────────┤
│  Server (Axum + Dioxus Fullstack)                               │
│  - SSR rendering                                                │
│  - GitHub OAuth (server functions)                              │
│  - Claude API proxy (server functions)                          │
│  - Database (SQLx + PostgreSQL)                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Key Integration Point: `<WgpuiCanvas>` Dioxus Component

```rust
// crates/openagents-web/src/components/wgpui_canvas.rs
use dioxus::prelude::*;

#[component]
fn WgpuiCanvas(class: String) -> Element {
    let mut wgpui_handle = use_signal(|| None::<wgpui::Handle>);

    // Initialize wgpui when canvas is mounted
    use_effect(move || {
        spawn(async move {
            // Get canvas element and initialize wgpui
            let window = web_sys::window().unwrap();
            let document = window.document().unwrap();
            let canvas = document.get_element_by_id("wgpui-canvas").unwrap();
            let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into().unwrap();

            let handle = wgpui::WebPlatform::init_on_canvas(&canvas).await;
            wgpui_handle.set(Some(handle));
        });
    });

    rsx! {
        canvas {
            id: "wgpui-canvas",
            class: "{class}",
        }
    }
}
```

---

## New Crate Structure

### CREATE

| Crate | Purpose |
|-------|---------|
| `crates/openagents-web/` | Main Leptos app (SSR + hydration) |
| `crates/api-server/` | Axum server (auth, AI proxy, SSR) - could merge with openagents-web |

### KEEP (Core Agent System)

```
crates/
├── agent/          # Core agent implementation
├── anthropic/      # Claude API client
├── atif/           # ATIF data format
├── atif-store/     # ATIF storage
├── config/         # Configuration
├── db/             # Database layer
├── deepseek/       # DeepSeek client
├── fm-bridge/      # Foundation models bridge
├── google_ai/      # Google AI client
├── guardrails/     # Safety constraints
├── healer/         # Error correction
├── hillclimber/    # Optimization
├── llm/            # LLM abstraction
├── mistral/        # Mistral client
├── nostr/          # Nostr protocol
├── nostr-chat/     # Nostr chat
├── nostr-client/   # Nostr client
├── nostr-relay/    # Nostr relay
├── oanix/          # Plan 9 agent environment
├── ollama/         # Ollama client
├── open_ai/        # OpenAI client
├── orchestrator/   # Orchestration
├── parallel/       # Parallel execution
├── reflexion/      # Reflection
├── sandbox/        # Sandboxed execution
├── sessions/       # Session management
├── sqlez/          # SQLite utilities
├── sqlez_macros/   # SQLite macros
├── tasks/          # Task management
├── terminalbench/  # Terminal benchmarking
├── testgen/        # Test generation
├── tools/          # Tool definitions
├── unit/           # MIMO state machines
├── wgpui/          # Canvas-based GPU UI (KEEP & enhance)
├── cloudflare/     # Cloudflare workers
└── pi/             # Pi infrastructure
```

### EVALUATE (May Keep)

```
crates/
├── collections/    # Utility collections (likely keep)
├── http_client/    # HTTP client wrapper (likely keep)
├── paths/          # Path utilities (likely keep)
├── util/           # General utilities (likely keep)
└── json_schema_store/  # JSON schemas (keep if needed)
```

### DELETE (~150 crates)

**Core GPUI:**
- `gpui`, `gpui_macros`, `gpui_tokio`

**All Zed UI crates:**
- `ui`, `ui_macros`, `ui_input`, `ui_prompt`, `ui_oa`
- `theme`, `theme_oa`, `theme_extension`, `theme_importer`, `theme_selector`
- `storybook`, `story`

**Zed Editor crates:**
- `editor`, `vim`, `vim_mode_setting`
- `language`, `languages`, `language_extension`, `language_tools`
- `language_model`, `language_models`, `language_onboarding`, `language_selector`
- `lsp`, `dap`, `dap_adapters`, `debug_adapter_extension`, `debugger_tools`, `debugger_ui`
- `snippet`, `snippet_provider`, `snippets_ui`
- `outline`, `outline_panel`, `project_symbols`
- `breadcrumbs`, `go_to_line`, `file_finder`, `search`, `fuzzy`, `picker`
- `command_palette`, `command_palette_hooks`, `tab_switcher`
- `repl`, `eval`, `eval_utils`

**Zed Project/Workspace:**
- `project`, `workspace`, `project_panel`, `explorer_command_injector`
- `recent_projects`, `component`, `buffer_diff`, `streaming_diff`
- `worktree`, `worktree_benchmarks`

**Zed Panels:**
- `terminal`, `terminal_view`
- `markdown`, `markdown_preview`, `html_to_markdown`
- `rich_text`, `image_viewer`, `svg_preview`
- `file_icons`, `icons`, `assets`
- `activity_indicator`, `menu`, `title_bar`, `notifications`
- `feedback`, `inspector_ui`, `miniprofiler_ui`, `onboarding`
- `panel`, `hud`

**Zed Settings:**
- `settings`, `settings_json`, `settings_macros`, `settings_ui`, `settings_profile_selector`
- `release_channel`, `feature_flags`
- `keymap_editor`, `line_ending_selector`, `toolchain_selector`

**Zed Git:**
- `git`, `git_ui`, `git_hosting_providers`

**Zed System:**
- `auto_update`, `auto_update_helper`, `auto_update_ui`
- `system_specs`, `credentials_provider`, `askpass`
- `telemetry`, `telemetry_events`, `crashes`, `migrator`
- `journal`, `zlog`, `zlog_settings`, `ztracing`, `ztracing_macro`
- `time_format`, `clock`, `vercel`

**Zed Extensions:**
- `extension`, `extension_api`, `extension_host`, `extensions_ui`, `extension_cli`

**Zed Collaboration:**
- `collab_ui`, `remote`, `remote_server`, `client`
- `rpc`, `proto`, `channel`, `call`
- `livekit_api`, `livekit_client`, `harbor`, `nc`

**Zed AI UI:**
- `assistant_slash_command`, `assistant_slash_commands`, `assistant_text_thread`
- `ai_onboarding`, `action_log`, `agent_settings`, `agent_ui`, `agent_servers`
- `copilot`, `supermaven`, `supermaven_api`

**Zed Tasks:**
- `tasks_ui`, `task`

**Zed ACP (GPUI-dependent):**
- `acp`, `acp_thread`, `acp_tools`

**OpenAgents GPUI apps:**
- `commander`, `marketplace`, `gym`, `vibe`, `mechacoder`

**Zed Main:**
- `zed`, `zed_actions`, `zed_env_vars`

**Other GPUI-dependent:**
- `rope`, `text`, `multi_buffer`, `fs`
- `diagnostics`, `prettier`
- `edit_prediction`, `edit_prediction_context`, `edit_prediction_types`, `edit_prediction_ui`, `edit_prediction_cli`
- `denoise`, `rules_library`, `schema_generator`, `scheduler`
- `audio`, `node_runtime`, `context_server`
- `net`, `fsevent`, `watch`, `fs_benchmarks`, `project_benchmarks`
- `cloud_api_client`, `cloud_api_types`, `cloud_zeta2_prompt`, `bedrock`
- `web_search`, `web_search_providers`
- `prompt_store`, `archivist`

---

## Implementation Steps

### Phase 1: Delete ~150 Zed/GPUI Crates (First)

1. Remove all crates listed in DELETE section from `Cargo.toml` workspace members
2. Delete the crate directories from `crates/`
3. Fix any remaining dependency issues in kept crates

### Phase 2: Setup Dioxus in openagents-web

1. Update `crates/openagents-web/Cargo.toml`:
   ```toml
   [dependencies]
   dioxus = { version = "0.6", features = ["fullstack", "router"] }
   dioxus-web = "0.6"
   axum = "0.8"
   tokio = { version = "1", features = ["full"] }
   sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"] }

   # Keep existing wgpu deps for canvas integration
   wgpu = { version = "24.0", features = ["webgpu", "webgl"] }
   ```

2. Install Dioxus CLI: `cargo install dioxus-cli`

3. Create app structure:
   ```
   crates/openagents-web/
   ├── Cargo.toml
   ├── Dioxus.toml        # Dioxus config (replaces Trunk.toml)
   ├── src/
   │   ├── main.rs        # Entry point (fullstack)
   │   ├── app.rs         # Root component + routes
   │   ├── routes/
   │   │   ├── mod.rs
   │   │   ├── home.rs
   │   │   ├── auth.rs
   │   │   └── mechacoder.rs
   │   ├── components/
   │   │   ├── mod.rs
   │   │   ├── wgpui_canvas.rs
   │   │   ├── chat_thread.rs
   │   │   ├── message_input.rs
   │   │   └── sidebar.rs
   │   └── server/
   │       ├── mod.rs
   │       ├── auth.rs     # GitHub OAuth server functions
   │       └── ai.rs       # Claude API proxy server functions
   └── assets/
       └── main.css
   ```

### Phase 3: Build MechaCoder Chat UI

1. **Chat Thread Component** (uses wgpui canvas for GPU-accelerated rendering):
   ```rust
   #[component]
   fn ChatThread(messages: Signal<Vec<Message>>) -> Element {
       rsx! {
           div { class: "chat-container",
               WgpuiCanvas {
                   class: "chat-canvas",
                   messages: messages,
               }
           }
       }
   }
   ```

2. **Message Input** (standard Dioxus component):
   ```rust
   #[component]
   fn MessageInput(on_send: EventHandler<String>) -> Element {
       let mut input = use_signal(|| String::new());
       rsx! {
           form { onsubmit: move |_| { on_send.call(input()); input.set(String::new()); },
               textarea { value: "{input}", oninput: move |e| input.set(e.value()) }
               button { "Send" }
           }
       }
   }
   ```

3. **Server Functions for Claude API**:
   ```rust
   #[server(SendMessage)]
   async fn send_message(message: String) -> Result<String, ServerFnError> {
       // Server-side only - API key is safe
       let client = anthropic::Client::new(std::env::var("ANTHROPIC_API_KEY")?);
       let response = client.messages().create(...).await?;
       Ok(response.content)
   }
   ```

### Phase 4: WGPUI Integration

1. Modify wgpui to accept external canvas:
   ```rust
   // crates/wgpui/src/platform/web.rs
   impl WebPlatform {
       pub async fn init_on_canvas(canvas: &HtmlCanvasElement) -> Self { ... }
   }
   ```

2. Create message bridge between Dioxus signals and wgpui:
   ```rust
   // When Dioxus signal changes, update wgpui scene
   use_effect(move || {
       if let Some(handle) = wgpui_handle() {
           handle.update_messages(messages());
       }
   });
   ```

### Phase 5: Auth & Deploy

1. **GitHub OAuth** (server functions):
   ```rust
   #[server(InitiateOAuth)]
   async fn initiate_oauth() -> Result<String, ServerFnError> {
       // Return GitHub OAuth URL
   }

   #[server(HandleCallback)]
   async fn handle_callback(code: String) -> Result<User, ServerFnError> {
       // Exchange code for token, create session
   }
   ```

2. **Build & Run**:
   ```bash
   dx serve                    # Development with hot reload
   dx build --release          # Production build
   ```

3. **Deploy**: Single binary + WASM bundle to fly.io, railway, or AWS

---

## Key Files to Modify

| File | Changes |
|------|---------|
| `Cargo.toml` | Remove ~150 workspace members |
| `crates/openagents-web/Cargo.toml` | Add dioxus, remove trunk deps |
| `crates/wgpui/src/platform/web.rs` | Add `init_on_canvas()` method |

## Key Files to Create

| File | Purpose |
|------|---------|
| `crates/openagents-web/Dioxus.toml` | Dioxus build configuration |
| `crates/openagents-web/src/main.rs` | Fullstack entry point |
| `crates/openagents-web/src/app.rs` | Root component + router |
| `crates/openagents-web/src/routes/mechacoder.rs` | MechaCoder page |
| `crates/openagents-web/src/components/wgpui_canvas.rs` | wgpui bridge |
| `crates/openagents-web/src/components/chat_thread.rs` | Chat display |
| `crates/openagents-web/src/components/message_input.rs` | Input component |
| `crates/openagents-web/src/server/auth.rs` | OAuth server functions |
| `crates/openagents-web/src/server/ai.rs` | Claude proxy server functions |

## Files/Dirs to Delete (~150 crates)

All crates listed in the DELETE section above. The full list is in `docs/architecture/gpui-removal-crates.md`.

---

## Execution Order

1. **Delete crates** - Remove ~150 Zed/GPUI crates from workspace
2. **Setup Dioxus** - Convert openagents-web to Dioxus fullstack
3. **Build chat UI** - MechaCoder page with message thread + input
4. **Integrate wgpui** - Canvas component for GPU-rendered chat
5. **Add auth** - GitHub OAuth via server functions
6. **Add Claude proxy** - Server function to call Anthropic API
7. **Deploy** - Build and deploy to hosting platform


