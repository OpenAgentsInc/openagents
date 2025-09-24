# Core: Config Internals

File: `codex-rs/core/src/config.rs`

Describes how the `Config` object is constructed, merged, and consumed by the
core runtime.

## Construction and precedence

- `Config::load_with_cli_overrides(cli_overrides, overrides)` reads
  `~/.codex/config.toml`, applies generic `-c key=value` overrides, then applies
  strongly‑typed `ConfigOverrides` from the frontend.
- Built‑in provider defaults are merged with user‑defined `model_providers`.
- Paths like `codex_home` and `cwd` are resolved early and stored.

## Key fields used by the runtime

- `model`, `review_model`, `model_family`
- `model_context_window`, `model_max_output_tokens`, `model_auto_compact_token_limit`
- `approval_policy`, `sandbox_policy`, `shell_environment_policy`
- `hide_agent_reasoning`, `show_raw_agent_reasoning`
- `notify` (external notifier command) and `tui_notifications`
- `mcp_servers`, `model_providers`, `model_provider`
- `project_doc_max_bytes`, `history`
- `use_experimental_unified_exec_tool`, `include_view_image_tool`, `tools_web_search_request`

## Provider selection

- A resolved `ModelProviderInfo` is embedded into the `Config` for the current
  session based on `model_provider_id`.

