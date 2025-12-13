# Crates to Delete if Removing GPUI

This document identifies crates that should be deleted if OpenAgents removes its dependence on GPUI and related Zed packages, based on the WGPUI migration plan.

## Overview

The WGPUI plan (see `docs/logs/20251212/2329-wgpu-plan.md`) proposes replacing GPUI with a new `wgpui` framework for web-first deployment. This would eliminate the need for many Zed-specific crates that are tightly coupled to GPUI or the desktop editor architecture.

## Categories

### 1. Core GPUI Framework (DELETE)

These are the GPUI framework itself and its direct dependencies:

- **`gpui`** - The core GPUI framework (Zed's GPU-accelerated UI framework)
- **`gpui_macros`** - Procedural macros for GPUI (`#[derive(Render)]`, `#[derive(IntoElement)]`, etc.)
- **`gpui_tokio`** - Tokio integration for GPUI async runtime

**Rationale:** These are the core framework being replaced by `wgpui`.

---

### 2. GPUI-Dependent UI Crates (DELETE)

These crates directly depend on GPUI for rendering and UI functionality:

#### Core UI Components
- **`ui`** - Shared GPUI components
- **`ui_macros`** - Macros for UI components
- **`ui_input`** - Input components (text input, form controls)
- **`ui_prompt`** - Prompt/dialog components
- **`ui_oa`** - OpenAgents-specific UI components (GPUI-based)
- **`theme`** - Centralized UI theme colors (GPUI-specific)
- **`theme_oa`** - OpenAgents theme (GPUI-based)
- **`theme_extension`** - Theme extension system
- **`theme_importer`** - Theme import utilities
- **`theme_selector`** - Theme selection UI
- **`storybook`** - Visual storybook for GPUI components
- **`story`** - Story-based UI component system

#### Application UIs
- **`commander`** - Desktop app UI built with GPUI (main OpenAgents desktop app)
- **`marketplace`** - Marketplace UI (GPUI-based)
- **`gym`** - Training & benchmarking UI (GPUI-based)
- **`hud`** - GPUI visualization layer for Unit dataflow graphs
- **`vibe`** - Agentic development environment (GPUI-based)
- **`mechacoder`** - MechaCoder agent implementation (uses GPUI)

#### Editor & Language Support
- **`editor`** - Text editor core (Zed, GPUI-based)
- **`vim`** - Vim emulation mode (Zed, GPUI-based)
- **`vim_mode_setting`** - Vim mode settings
- **`language`** - Language support framework (Zed, GPUI-based)
- **`languages`** - Language definitions (Zed)
- **`language_extension`** - Language extension system
- **`language_tools`** - Language tooling utilities
- **`language_model`** - Language model integration
- **`language_models`** - Language models abstraction
- **`language_onboarding`** - Language onboarding UI
- **`language_selector`** - Language selection UI
- **`lsp`** - Language Server Protocol client (Zed, GPUI-based)
- **`dap`** - Debug Adapter Protocol (Zed, GPUI-based)
- **`dap_adapters`** - DAP adapter implementations
- **`debug_adapter_extension`** - Debug adapter extension system
- **`debugger_tools`** - Debugger utilities
- **`debugger_ui`** - Debugger UI components
- **`snippet`** - Code snippet system (Zed, GPUI-based)
- **`snippet_provider`** - Snippet provider abstraction
- **`snippets_ui`** - Snippets UI
- **`outline`** - Code outline (Zed, GPUI-based)
- **`outline_panel`** - Outline panel UI
- **`project_symbols`** - Project symbol indexing
- **`breadcrumbs`** - Breadcrumb navigation
- **`go_to_line`** - Go to line feature
- **`file_finder`** - File finder (Zed, GPUI-based)
- **`search`** - Search functionality (Zed, GPUI-based)
- **`fuzzy`** - Fuzzy matching
- **`picker`** - Picker UI component
- **`command_palette`** - Command palette (Zed, GPUI-based)
- **`command_palette_hooks`** - Command palette hooks
- **`tab_switcher`** - Tab switcher
- **`repl`** - REPL integration
- **`eval`** - Code evaluation
- **`eval_utils`** - Evaluation utilities

#### Project & Workspace
- **`project`** - Project management (Zed, GPUI-based)
- **`workspace`** - Workspace management (Zed, GPUI-based)
- **`project_panel`** - Project panel UI
- **`explorer_command_injector`** - Explorer command injection
- **`recent_projects`** - Recent projects
- **`component`** - Component system
- **`buffer_diff`** - Buffer diff visualization
- **`streaming_diff`** - Streaming diff algorithm

#### Panels & Views
- **`panel`** - Panel system (Zed, GPUI-based)
- **`terminal`** - Terminal emulator (Zed, GPUI-based)
- **`terminal_view`** - Terminal view UI
- **`markdown`** - Markdown rendering (Zed, GPUI-based)
- **`markdown_preview`** - Markdown preview panel
- **`html_to_markdown`** - HTML to Markdown conversion
- **`rich_text`** - Rich text rendering
- **`image_viewer`** - Image viewer
- **`svg_preview`** - SVG preview
- **`file_icons`** - File icon system
- **`icons`** - Icon library
- **`assets`** - Asset management
- **`activity_indicator`** - Activity indicator
- **`menu`** - Menu system
- **`title_bar`** - Title bar customization
- **`notifications`** - Notification system
- **`feedback`** - Feedback UI
- **`inspector_ui`** - Inspector UI
- **`miniprofiler_ui`** - Mini profiler UI
- **`onboarding`** - Onboarding flow

#### Settings & Configuration
- **`settings`** - Settings system (Zed, GPUI-based)
- **`settings_json`** - Settings JSON schema
- **`settings_macros`** - Settings macros
- **`settings_ui`** - Settings UI
- **`settings_profile_selector`** - Settings profile selector
- **`release_channel`** - Release channel management
- **`feature_flags`** - Feature flags
- **`keymap_editor`** - Keymap editor
- **`line_ending_selector`** - Line ending selector
- **`toolchain_selector`** - Toolchain selector

#### Git & Version Control
- **`git`** - Git integration (Zed, GPUI-based)
- **`git_ui`** - Git UI components
- **`git_hosting_providers`** - Git hosting provider integration

#### Diagnostics & Development
- **`diagnostics`** - Diagnostics display
- **`prettier`** - Prettier integration
- **`edit_prediction`** - Edit prediction
- **`edit_prediction_context`** - Edit prediction context
- **`edit_prediction_types`** - Edit prediction types
- **`edit_prediction_ui`** - Edit prediction UI
- **`edit_prediction_cli`** - Edit prediction CLI
- **`denoise`** - Code denoising
- **`rules_library`** - Rules library
- **`schema_generator`** - Schema generator
- **`scheduler`** - Task scheduler (Zed, GPUI-based)

#### System Integration
- **`auto_update`** - Auto-update system (GPUI-based)
- **`auto_update_helper`** - Auto-update helper
- **`auto_update_ui`** - Auto-update UI
- **`system_specs`** - System specifications
- **`credentials_provider`** - Credentials provider
- **`askpass`** - Askpass utility
- **`telemetry`** - Telemetry system
- **`telemetry_events`** - Telemetry event definitions
- **`crashes`** - Crash reporting
- **`migrator`** - Data migration
- **`journal`** - Journal system
- **`zlog`** - Zed logging
- **`zlog_settings`** - Zed log settings
- **`ztracing`** - Zed tracing
- **`ztracing_macro`** - Tracing macros
- **`time_format`** - Time formatting
- **`clock`** - Clock utilities
- **`vercel`** - Vercel integration

#### Extensions & Plugins
- **`extension`** - Extension system (Zed, GPUI-based)
- **`extension_api`** - Extension API
- **`extension_host`** - Extension host
- **`extensions_ui`** - Extensions UI

#### Collaboration & Remote
- **`collab`** - Collaboration core (Zed, GPUI-based)
- **`collab_ui`** - Collaboration UI
- **`remote`** - Remote development (Zed, GPUI-based)
- **`remote_server`** - Remote server management
- **`client`** - Client utilities
- **`rpc`** - RPC framework (Zed, GPUI-based)
- **`proto`** - Shared protocol for communication
- **`channel`** - Channel communication
- **`call`** - Call management
- **`livekit_api`** - LiveKit API client
- **`livekit_client`** - LiveKit client SDK
- **`harbor`** - Harbor collaboration
- **`nc`** - Network communication

#### AI & LLM Integration (UI Components)
- **`assistant_slash_command`** - Assistant slash commands
- **`assistant_slash_commands`** - Assistant slash commands collection
- **`assistant_text_thread`** - Assistant text thread
- **`ai_onboarding`** - AI onboarding UI
- **`action_log`** - Action logging
- **`agent_settings`** - Agent settings
- **`agent_ui`** - Agent UI components
- **`agent_servers`** - Agent server management
- **`copilot`** - GitHub Copilot integration
- **`supermaven`** - Supermaven integration
- **`supermaven_api`** - Supermaven API client

#### Task Management
- **`tasks_ui`** - Task management UI
- **`task`** - Task types and utilities (Zed, GPUI-based)

#### ACP (Agent Client Protocol)
- **`acp`** - ACP connection layer (uses GPUI)
- **`acp_thread`** - ACP thread management (GPUI-based)
- **`acp_tools`** - ACP tools implementation

#### Zed Main Application
- **`zed`** - Zed editor main application
- **`zed_actions`** - Zed actions system
- **`zed_env_vars`** - Zed environment variables

**Rationale:** All of these crates either directly depend on GPUI or are part of the Zed editor architecture that assumes GPUI rendering.

---

### 3. GPUI Infrastructure Crates (EVALUATE)

These crates support GPUI but might have reusable concepts:

#### Potentially Reusable (KEEP IF USEFUL)
- **`collections`** - Standard collection type re-exports (used by GPUI, but might be useful standalone)
- **`util`** - Utilities (used by GPUI, but general-purpose)
- **`util_macros`** - Utility macros
- **`refineable`** - Refineable trait (used by GPUI for efficient updates, but concept might be reusable)
- **`sum_tree`** - Sum tree data structure (used by GPUI for text editing, but might be reusable)
- **`media`** - Media handling (might be reusable for non-GPUI contexts)
- **`perf`** - Performance utilities (general-purpose)
- **`http_client`** - HTTP client (reqwest wrapper, general-purpose)
- **`http_client_tls`** - TLS configuration for HTTP client
- **`reqwest_client`** - Reqwest client wrapper
- **`aws_http_client`** - AWS HTTP client
- **`paths`** - Path utilities (general-purpose)
- **`net`** - Network utilities (general-purpose)
- **`fsevent`** - Filesystem event monitoring (general-purpose)
- **`watch`** - File watching (general-purpose, but GPUI-based)
- **`worktree`** - Git worktree utilities (Zed, GPUI-based, but concept might be reusable)
- **`worktree_benchmarks`** - Worktree performance benchmarks
- **`fs_benchmarks`** - Filesystem performance benchmarks
- **`project_benchmarks`** - Project operations benchmarks

**Rationale:** These might have reusable concepts, but many are still GPUI-dependent. Evaluate each based on whether the concept is needed in the new architecture.

#### GPUI-Specific (DELETE)
- **`rope`** - Rope data structure (Zed, GPUI-based, used for editor buffers)
- **`text`** - Text utilities (Zed, GPUI-based)
- **`multi_buffer`** - Multi-buffer management (Zed, GPUI-based)
- **`fs`** - Filesystem utilities (Zed, GPUI-based)

---

### 4. Non-GPUI Crates (KEEP)

These crates do NOT depend on GPUI and should be kept:

#### Core Agent System
- `agent`
- `orchestrator`
- `parallel`
- `tools`
- `llm`
- `sandbox`

#### Terminal-Bench & Training
- `hillclimber`
- `testgen`
- `terminalbench`

#### Foundation Models
- `fm-bridge`

#### Data & Storage
- `atif`
- `atif-store`
- `tasks`
- `sessions`
- `config`
- `db`
- `sqlez`
- `sqlez_macros`
- `json_schema_store`
- `prompt_store`

#### Nostr & Decentralized
- `nostr`
- `nostr-chat`
- `nostr-client`
- `nostr-relay`

#### Infrastructure
- `unit` (MIMO finite state machines - not GPUI-dependent)
- `cloudflare`
- `oanix`
- `pi`

#### Learning & Research
- `archivist`
- `guardrails`
- `healer`
- `reflexion`

#### CLI & Tools
- `cli`
- `install_cli`
- `extension_cli`

#### AI & LLM Integration (Non-UI)
- `anthropic`
- `open_ai`
- `google_ai`
- `deepseek`
- `mistral`
- `x_ai`
- `codestral`
- `ollama`
- `lmstudio`
- `open_router`
- `cloud_llm_client`
- `cloud_zeta2_prompt`
- `bedrock`
- `web_search`
- `web_search_providers`

#### Language-Specific Tools (Non-UI)
- `node_runtime`
- `context_server`

#### Other
- `openagents-web` (if this is the web version, might be replaced by wgpui)

---

## Summary Statistics

- **Total crates to DELETE:** ~150+ crates
- **Total crates to KEEP:** ~50+ crates
- **Total crates to EVALUATE:** ~20 crates

---

## Migration Considerations

### What Needs to be Rebuilt

If removing GPUI, the following functionality needs to be rebuilt with WGPUI or alternative approaches:

1. **Desktop Applications**
   - `commander` - Main desktop app
   - `gym` - Training UI
   - `marketplace` - Marketplace UI
   - `vibe` - Development environment
   - `mechacoder` - Agent UI

2. **Core UI Components**
   - All basic UI elements (buttons, inputs, panels, etc.)
   - Theme system
   - Layout system
   - Text rendering
   - Event handling

3. **Editor Functionality** (if needed)
   - Text editing
   - Language support
   - LSP integration
   - Debugging support

4. **Visualization**
   - `hud` - Unit dataflow visualization

### What Can Be Reused

- Core agent logic (no UI dependencies)
- LLM clients (no UI dependencies)
- Data storage (no UI dependencies)
- Terminal-Bench (no UI dependencies)
- Foundation Models bridge (no UI dependencies)
- Nostr protocol (no UI dependencies)

---

## Action Items

1. **Audit GPUI dependencies** - Verify which crates actually depend on GPUI vs. just being in the Zed ecosystem
2. **Identify reusable concepts** - Some crates might have concepts worth porting (e.g., `refineable`, `sum_tree`)
3. **Plan WGPUI migration** - Determine which UI functionality needs to be rebuilt
4. **Update workspace** - Remove deleted crates from `Cargo.toml` workspace members
5. **Update dependencies** - Remove GPUI dependencies from remaining crates

---

## Notes

- This analysis is based on `Cargo.toml` dependency declarations and the crates README
- Some crates might have optional GPUI dependencies that could be removed
- The `wgpui` plan suggests web-first deployment, so desktop-specific crates are likely unnecessary
- Editor functionality (Zed crates) might not be needed if the focus is on web deployment
