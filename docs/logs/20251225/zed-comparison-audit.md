# Zed Comparison Audit - 2025-12-25

## Scope and Method
- Compared OpenAgents workspace structure to `/home/christopherdavid/code/zed`.
- Reviewed Zed workspace crate list, key architecture docs, and representative crates:
  - `crates/gpui/README.md`
  - `crates/component/src/component.rs`
  - `crates/component_preview/src/component_preview.rs`
  - `crates/storybook/src/storybook.rs`
  - `crates/acp_thread/src/acp_thread.rs`
  - `crates/acp_tools/src/acp_tools.rs`
  - `crates/telemetry/src/telemetry.rs`
  - `docs/src/worktree-trust.md`
- This is a structural and feature audit, not a line-by-line code review of all Zed crates.

## High-Level Takeaways
- Zed splits UI, editor core, and platform services into many narrowly scoped crates with explicit boundaries.
- Their UI system (GPUI + UI crate + component registry + storybook) is far more formalized than our current WGPUI usage.
- Zed has a mature settings, actions, command palette, and telemetry pipeline that we can adapt to Autopilot UX.
- Zed’s ACP/agent integration is deeper: ACP thread, ACP tools UI, tool logging, and model registry handling.
- Collaboration and remote development are first-class systems; we have early pieces (autopilot + marketplace + compute) but less unified orchestration.

## Zed Systems and Crates Relevant to OpenAgents

### UI Framework and Rendering
- `gpui`, `gpui_macros`, `gpui_tokio`
  - GPUI is a hybrid immediate/retained-mode GPU UI framework.
  - Core concepts: `Application`, `Entity` state, `Render` trait, and low-level `Element` rendering.
  - Built-in async executor tied to the event loop and a dedicated `gpui::test` macro.
  - What to adapt: entity-based state management model, rendering pipeline design, test harness patterns.
- `ui`, `ui_macros`, `ui_input`, `component`, `component_preview`, `storybook`
  - `ui` provides primitives and components with styling helpers; `ui_input` for input fields.
  - `component` crate uses inventory-based registration and metadata for previews.
  - `component_preview` integrates a searchable component browser in the workspace UI.
  - `storybook` app selects stories and theme variants.
  - What to adapt: component registry + previewing pipeline, inventory-style registration, searchable UI story index.

### Theming, Icons, and Style System
- `theme`, `theme_selector`, `theme_extension`, `theme_importer`, `file_icons`, `icons`
  - Theme registration and selector UI; icon theme support.
  - What to adapt: theme registry abstraction, theme selector patterns, icon packs and asset pipelines.

### Actions, Command Palette, and Keybindings
- `command_palette`, `command_palette_hooks`, `which_key`, `keymap_editor`, `menu`, `zed_actions`
  - Actions are first-class; command palette is central to navigation and workflow.
  - What to adapt: action registry, command palette with dynamic filtering, keymap editor UX.

### Editor Core and Text Model
- `editor`, `text`, `rope`, `sum_tree`, `multi_buffer`, `buffer_diff`, `streaming_diff`, `rich_text`
  - Sophisticated text data structures (rope, sum_tree) and multi-buffer editing.
  - What to adapt: text data structures for log viewers, chat buffers, terminal output, and streaming diffs.

### Language and LSP Systems
- `language`, `languages`, `language_tools`, `language_extension`, `lsp`, `diagnostics`, `outline`, `project_symbols`
  - Full language registry, LSP clients, diagnostics pipeline, symbol index.
  - What to adapt: language registry patterns for syntax highlighting and structured views in WGPUI.

### Project / Workspace / Worktree
- `project`, `workspace`, `worktree`, `project_panel`, `recent_projects`, `session`, `paths`
  - Worktree is a core unit; workspace handles project state and UI integration.
  - `docs/src/worktree-trust.md` describes restricted mode and trust escalation.
  - What to adapt: explicit worktree model, trust/restriction modes for tool and MCP execution.

### Search and Navigation
- `search`, `file_finder`, `fuzzy`, `breadcrumbs`, `tab_switcher`, `outline_panel`
  - Robust search and fuzzy navigation infrastructure.
  - What to adapt: fuzzy search utilities for command palette, components, or issue browsing.

### Terminal and REPL
- `terminal`, `terminal_view`, `repl`
  - Integrated terminal and REPL view.
  - What to adapt: consistent terminal UI interactions for Autopilot tool output.

### Tasks and Scheduling
- `task`, `tasks_ui`, `scheduler`
  - Shell task runner abstractions and UI for tasks.
  - What to adapt: background task scheduling for Autopilot runs and queues.

### Git and VCS
- `git`, `git_ui`, `git_hosting_providers`
  - Git core + UI integration.
  - What to adapt: worktree-aware git status and diffs for Autopilot GUI panes.

### Extensions / Plugin System
- `extension`, `extension_api`, `extension_host`, `extension_cli`, `extensions_ui`, `rules_library`
  - Extension manifests, host integration, and UI surfaces.
  - What to adapt: extension architecture patterns for Skills marketplace and ACP tool packs.

### Collaboration and Remote Development
- `collab`, `collab_ui`, `call`, `livekit_api`, `livekit_client`, `remote`, `remote_server`, `rpc`, `channel`, `net`, `cloud_api_client`, `cloud_api_types`
  - Collaborative editing and remote execution stack.
  - What to adapt: multi-agent session orchestration and remote worker management patterns.

### Agent / ACP / AI Stack
- `agent`, `agent_servers`, `agent_settings`, `agent_ui`, `agent_ui_v2`
  - Agent thread model and ACP integration, plus UI for agent controls.
- `acp_thread`, `acp_tools`
  - ACP thread handles messages, diffs, terminal I/O, and formatting triggers.
  - ACP tools UI monitors ACP traffic and messages.
  - What to adapt: ACP threading model and structured tool log UI.
- LLM providers: `anthropic`, `open_ai`, `google_ai`, `bedrock`, `deepseek`, `mistral`, `x_ai`, `ollama`, `lmstudio`, `cloud_llm_client`
  - Provider abstraction and model selection patterns.
  - What to adapt: model registry and provider selection for OpenAgents, with explicit capability metadata.

### Telemetry and Logging
- `telemetry`, `telemetry_events`, `action_log`, `zlog`, `ztracing`, `ztracing_macro`
  - Event macro and unified telemetry pipeline with structured events.
  - What to adapt: event macro API and log pipeline for Autopilot + GUI usage analytics.

### Storage, Migrations, and Persistence
- `db`, `sqlez`, `sqlez_macros`, `migrator`, `prompt_store`
  - SQLite abstraction and migration tooling.
  - What to adapt: migration tooling and typed statements for our autopilot and marketplace DBs.

### Updates, Install, and Distribution
- `auto_update`, `auto_update_ui`, `auto_update_helper`, `install_cli`, `release_channel`, `system_specs`
  - Formal update pipeline and system capabilities checks.
  - What to adapt: update process for OpenAgents desktop shell and autopilot daemon.

### Miscellaneous Infrastructure
- `fs`, `fsevent`, `watch`, `paths` for filesystem abstraction and watching.
- `credentials_provider` for managed credentials.
- `crashes` for crash reporting.
- `docs_preprocessor` for documentation pipelines.

## Recommended Adaptations for OpenAgents

### 1) Formal UI Component Registry and Preview
- Zed’s `component` + `component_preview` system is a strong model for our ACP component storybook.
- Use inventory registration + metadata, so component permutations are discoverable and searchable.
- Add a lightweight searchable component preview in our Autopilot UI or WGPUI storybook.

### 2) GPUI-Style Entity State and Render Model
- GPUI’s `Entity` and `Render` pattern is clean for state updates and drawing.
- Our WGPUI can adopt the same conceptual model even if implementations differ.

### 3) Action Registry + Command Palette
- Zed’s action system and command palette are core UX primitives; we should adopt them as baseline.
- This fits Autopilot control (start/stop, run prompt, jump to session, open logs).

### 4) Worktree Trust and Restricted Mode
- Zed’s worktree trust model is a good blueprint for guardrails around tool execution.
- We can implement a similar concept for MCP, autopilot tools, or marketplace agent tasks.

### 5) ACP Threading and Tool Telemetry
- `acp_thread` and `acp_tools` show how ACP streams should be modeled and inspected.
- Adapt the ACP log viewer UI and diff/terminal integration design for Autopilot.

### 6) Provider Registry and Model Grouping
- Zed groups and filters models by provider and authentication status.
- Adapt this for `openagents` to manage multiple LLM providers and local models.

### 7) Telemetry Macro Pattern
- Zed’s `telemetry::event!` macro is a clean pattern for structured events.
- We can apply this to autopilot metrics, UI events, and performance reporting.

### 8) Database and Migration Tools
- `sqlez` + `migrator` demonstrates typed statement and migration layers.
- Adopt similar patterns to reduce raw SQL usage and schema drift across autopilot/marketplace DBs.

### 9) Extension Architecture for Skills Marketplace
- Zed’s extension host / API / UI provide a blueprint for externalizing tools.
- Map this to OpenAgents Skills and tool packs, with a manifest + host protocol.

### 10) Editor/Buffer Data Structures for Streaming Views
- Rope + sum_tree + multi-buffer patterns are ideal for large streaming logs.
- We can adapt these patterns to efficiently render long Autopilot output in WGPUI.

## Systems That Are Likely Not Directly Relevant
- Low-level editor-specific crates that do not map to Autopilot use cases (for now):
  - `dap`, `dap_adapters`, `debugger_ui`, `debugger_tools`
  - `snippets_ui`, `snippet_provider` (unless we build scripting UX)
  - `vim`, `vim_mode_setting` (unless we want vi mode)

## License and Reuse Considerations
- Zed is multi-licensed (AGPL/GPL/Apache). Many crates are GPL.
- Direct code reuse or forking requires license compliance. Prefer architectural inspiration or reimplementation where needed.

## Gap Analysis vs OpenAgents
- UI system maturity: Zed is far ahead in component registry, storybook, theme system, and action infrastructure.
- Agent integration: Zed has a cohesive ACP thread + UI tool viewer; we have ACP components but need stronger wiring.
- Settings and trust: Zed has robust settings and worktree trust; OpenAgents has less formalized trust and preferences.
- Telemetry: Zed has a dedicated telemetry event macro and event routing; we should formalize our event pipeline.

## Suggested Next Steps
1) Add component registry metadata and searchable previews to WGPUI storybook.
2) Implement an action registry and command palette in Autopilot GUI.
3) Define worktree trust/restriction policies for tools and MCP.
4) Introduce a model registry abstraction for OpenAgents providers.
5) Design a small telemetry event macro API and event queue.

