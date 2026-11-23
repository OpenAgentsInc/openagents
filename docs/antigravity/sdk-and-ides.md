# Antigravity Python SDK, IDE Extensions & Release Drift Reference

Complete reference for the official `google-antigravity` Python SDK (`v0.1.24`), all Antigravity IDE/editor extensions, migration paths, and a quantitative changelog & version-drift analysis across all 226+ releases in `antigravity.google/docs/changelog`.

---

## 1. Google Antigravity Python SDK (`google-antigravity`)

### 1.1 Installation, Platform Wheels & Optional Extras
- **Package:** `pip install google-antigravity` (Python **3.10+**, including Python 3.14 deferred annotations since `v0.1.5`–`v0.1.24`).
- **GitHub Examples:** <https://github.com/google-antigravity/antigravity-sdk-python/tree/main/examples/getting_started>
- **Wheels:** macOS (`darwin-arm64` Metal, `darwin-x86_64`), Linux glibc & Alpine `musllinux_1_1` (`x86_64`, `aarch64`, `v0.1.15+`), Windows (`x86_64`, `ARM64`).
- **Optional dependencies:** `litert-lm >= 0.19.1` (for on-device `.litertlm` vision with SDK `>= 0.1.24`), `certifi` (macOS SSL cert workaround for `litert-lm import`), `mcp >= 1.0, < 3.0` (FastMCP), `watchfiles` (`on_file_change` trigger), `opentelemetry.sdk` (optional GenAI spans), `pydantic`.
- **Cloud Managed REST/gRPC API (separate from local Python SDK):** <https://ai.google.dev/gemini-api/docs/antigravity-agent>

### 1.2 Runtime Architecture & Connection Strategies
1. **`Agent`**: Async context manager (`async with Agent(config) as agent:`) owning binary discovery, session lifecycle, tool/hook/policy/trigger registration (static at init since `v0.1.4`), and transport selection.
2. **`Conversation`** (`agent.conversation`): Stateful trajectory container (`chat()`, `history`, `total_usage`, `last_turn_stop_reason`, `compaction_indices`, `receive_chunks()`, `receive_steps()`, `get_last_structured_output()`).
3. **`ConnectionStrategy` implementations:**
   - **`LocalConnectionStrategy`**: Spawns bundled `localharness` subprocess (overridable per-connection via `ANTIGRAVITY_HARNESS_PATH` without mutating `os.environ` since `v0.1.15`), speaking protobufs over stdio + local WebSocket (`StepUpdate`, `TrajectoryStateUpdate`, `ActionCompaction`, `ActionGenerateImage`, `UsageUpdate`, `Content`). Inference routes to Gemini Developer API or Vertex AI (`aiplatform.googleapis.com`).
   - **`LiteRTConnectionStrategy`**: Launches an ephemeral local OpenAI-compatible HTTP server (`port=0`) backed by `litert_lm.Engine` over a `.litertlm` checkpoint with multi-turn KV prefix caching (`v0.1.23`–`v0.1.24`).
   - **`LocalOpenAIConnectionStrategy`**: Connects `localharness` to external OpenAI-compatible HTTP/SSE endpoints (Ollama, LM Studio, vLLM) supporting Chat Completions & Responses APIs with SSE usage/latency metadata and `Retry-After` handling (`v0.1.24`).

*(Note: No public `RemoteAgentConfig` exists in the Python SDK; public config hierarchy is `AgentConfig` -> `BaseLocalAgentConfig` -> `LocalAgentConfig`, `LiteRTAgentConfig`, `LocalOpenAIAgentConfig`.)*

### 1.3 Root Package Exports (`google.antigravity`)
`Agent`, `LocalAgentConfig`, `LiteRTAgentConfig`, `LiteRTBackend`, `LocalOpenAIAgentConfig`, `CapabilitiesConfig`, `GeminiAPIEndpoint`, `VertexEndpoint`, `ToolContext`, `AntigravityCancelledError`, `Content`, `Image`, `Document`, `Audio`, `Video`, `from_file`, `from_bytes`, `BuiltinTools`, `SystemInstructions`, `ServiceTier`, `InlineSkill`, `SandboxStatus`, `types`, `hooks`, `beta`.

### 1.4 Configuration Parameters & Presets

#### `LocalAgentConfig` (and common `BaseLocalAgentConfig` fields)
| Parameter | Type / Default | Notes |
| --- | --- | --- |
| `model` | `str \| ModelTarget \| None` | Default **`"gemini-4-hydrogen"`** in `v0.1.24` (rotated through `3.6-flash` -> `3.7-flash` -> `3.8-flash` -> `4-hydrogen`). |
| `models` | `list[ModelTarget]` | Multi-model routing/fallback list. |
| `endpoint` | `GeminiAPIEndpoint \| VertexEndpoint` | Supports custom `base_url`, HTTP `headers`, `GeminiModelOptions(service_tier=ServiceTier.PRIORITY|STANDARD, max_output_tokens=...)`. `ThinkingLevel.EXTRA_HIGH` maps to `"xhigh"` (`v0.1.24`). |
| `api_key` | `str \| None` | Gemini Developer API key or Vertex Express Mode key (`vertex=True`); defaults to `GEMINI_API_KEY`. |
| `vertex`, `project`, `location` | `bool` (`False`), `str \| None` | Vertex AI Standard Mode (ADC; reads `GOOGLE_GENAI_USE_VERTEXAI`, `GOOGLE_CLOUD_PROJECT`, `GOOGLE_CLOUD_LOCATION`). Omit `project`/`location` for Express Mode. |
| `system_instructions` | `str \| TemplatedSystemInstructions \| CustomSystemInstructions` | Plain `str` appends to default scaffolding (`v0.1.10`); `TemplatedSystemInstructions(identity, sections)` preserves scaffolding; `CustomSystemInstructions(text, title)` replaces scaffolding completely. |
| `capabilities` | `CapabilitiesConfig \| None` | `enabled_tools`, `disabled_tools`, `agent_behavior` (`AUTONOMOUS` [default `v0.1.11+`], `INTERACTIVE`, `MINIMAL`), `enable_subagents`, `max_subagent_depth`, `allowed_subagents`, `run_command_config: RunCommandConfig(enable_sandbox, enable_daemons)`, `mcp_max_output_bytes`. |
| `tools` | `Sequence[Callable]` | Plain sync/async Python functions, `functools.partial`, functors, dataclass methods. No decorator required. |
| `mcp_servers` | `Sequence[BaseMcpServerConfig]` | `McpStdioServer` and `McpStreamableHttpServer` (**`McpSseServer` removed in `v0.1.2`**). |
| `policies` | `Sequence[Policy]` | Defaults to `[policy.confirm_run_command()]`; auto-prepends `policy.workspace_only(workspaces)` if `workspaces` is set. |
| `hooks` | `Sequence[Hook]` | Lifecycle hooks (sync or async; bulk sequences supported `v0.1.21+`). |
| `triggers` | `Sequence[Trigger]` | `every(seconds, cb)`, `on_file_change(path, cb)` (requires `watchfiles`), custom async triggers (`TriggerDelivery` removed `v0.1.15`). |
| `subagents` | `Sequence[SubagentConfig]` | Static custom subagent definitions (with dedicated `model`, subagent-scoped `tools`, `SubagentSkillsConfig`). |
| `skills_paths` / `inline_skills` | `Sequence[str]` / `Sequence[InlineSkill]` | **Mutually exclusive** (`v0.1.22`). |
| `workspaces` | `Sequence[str]` | Workspace roots (expands `~` and relative paths against `os.getcwd()`). |
| `save_dir`, `conversation_id`, `app_data_dir` | `str \| None` | Persistence & resume. `conversation_id`: >=32 chars, alphanumeric + hyphens only (**no underscores**). `app_data_dir`: **must be absolute path**. |
| `response_schema` | `type[pydantic.BaseModel] \| None` | Enforces typed JSON structured output (`await response.structured_output()`). |
| `env` | `dict[str, str] \| None` | Env overrides for `localharness` & `run_command`. |
| `budget_config` | `BudgetConfig \| None` | `max_model_calls`, `max_tool_calls`, `max_input_tokens` (net uncached), `max_output_tokens` (candidates+thinking), `max_total_tokens`, `scope=BudgetScope.LIFETIME\|FORWARD_LOOKING`. |
| `compaction_config` | `CompactionConfig \| None` | `CompactionConfig(token_threshold=50_000)` (`v0.1.18`). |
| `retry_config` | `RetryConfig \| None` | `ModelAPIRetryConfig` + `ModelOutputRetryConfig` + `RetryConfig.benchmark()`. |
| `tool_output_truncation_config`, `debug_config` | config objects | Output truncation & connection diagnostics. |

- **Presets:** `.lightweight()` (minimal toolset + `AgentBehavior.MINIMAL` + disables subagents; auto-applied on `LiteRTAgentConfig`) and `.eval()` (`v0.1.18`, disables image gen & subagents, enables daemons, applies `RetryConfig.benchmark()`).

#### Local Model Configs
- **`LiteRTAgentConfig`**: `model_path` (required absolute path; `~` not expanded), `backend` (`gpu|npu|cpu`), `vision_backend`, `audio_backend`, `enable_speculative_decoding`, `cache_dir` (must exist), `port` (`0` ephemeral), `compaction_config` (default `40960` for 64k KV cache), `download_if_missing`. Emits thoughts via `reasoning`/`reasoning_content` SSE fields (`v0.1.24`).
- **`LocalOpenAIAgentConfig`**: `model`, `base_url` (Ollama `http://localhost:11434/v1`, LM Studio `http://localhost:1234/v1`, vLLM). Call `.lightweight()` explicitly; do not use with `litert-lm serve`.

### 1.5 Runtime Streaming, Built-in Tools & Custom Tools
- **`ChatResponse`:** `await response.text()`, `async for token in response`, `async for thought in response.thoughts`, `await response.structured_output()`, `await response.cancel()`, `response.usage_metadata`, `response.stop_reason` (`UNSPECIFIED`, `MAX_MODEL_CALLS_EXCEEDED`, `MAX_TOOL_CALLS_EXCEEDED`, `MAX_INPUT_TOKENS_EXCEEDED`, `MAX_OUTPUT_TOKENS_EXCEEDED`, `MAX_TOTAL_TOKENS_EXCEEDED`, `RESOURCE_EXHAUSTED`, `BUDGET_EXCEEDED`, `TURN_LIMIT`, `USER_CANCELLED`).
- **`BuiltinTools`:** `LIST_DIR`, `SEARCH_DIR`, `FIND_FILE`, `VIEW_FILE`, `CREATE_FILE`, `EDIT_FILE`, `RUN_COMMAND`, `ASK_QUESTION` (**excluded from `BuiltinTools.default()` since `v0.1.18`**), `INVOKE_SUBAGENT` (canonical `v0.1.22+`, deprecating `START_SUBAGENT`), `GENERATE_IMAGE` (`DEFAULT_IMAGE_GENERATION_MODEL = "gemini-3.1-flash-lite-image"`), `SEARCH_WEB`, `READ_URL_CONTENT`, `FINISH`, `schedule`, `manage_task`, `RUN_WORKFLOW`. Groups: `.default()`, `.read_only()`, `.minimal()`.
- **Custom tools:** Automatic docstring & JSON Schema Draft 7/2020-12 normalization + Pydantic `TypeAdapter` coercion; inject `ctx: ToolContext` for thread-safe `get_state`/`set_state`; return text, Pydantic models, or `Image`/`Audio`/`Video`/`Document`; override built-in tools by defining a function with the exact built-in name.
- **MCP servers:** `McpStdioServer` / `McpStreamableHttpServer` (`name` must be valid Python identifier; `enabled_tools` XOR `disabled_tools`; `timeout_seconds`; `force_all_tools_eager`).

### 1.6 Safety Policies, 10 Lifecycle Hooks, Beta Workflows, Errors & Observability
- **Policies (`google.antigravity.hooks.policy`):** `allow`, `deny`, `ask_user(handler=...)`, `allow_all`, `deny_all`, `confirm_run_command`, `workspace_only`, `safe_defaults`. **9-level precedence:** Specific (`Deny > Ask > Allow`) > Prefix Wildcard (`Deny > Ask > Allow`) > Global `*` (`Deny > Ask > Allow`). `when=` predicates **fail closed** on exception.
- **OS Sandbox:** `RunCommandConfig(enable_sandbox=True)` -> `SandboxStatus` / `is_sandboxed` (`v0.1.22+`); warns if OS sandbox unavailable (`v0.1.18+`).
- **All 10 Lifecycle Hooks (`google.antigravity.hooks` — `DecideHook`, `InspectHook`, `TransformHook`):**
  1. `on_session_start`
  2. `on_session_end`
  3. `pre_turn` -> `HookResult(allow=...)`
  4. `post_turn`
  5. `pre_tool_call_decide` (`ToolCall` includes normalized `canonical_path`, `call_id`, `trajectory_id`, `step_index`) -> `HookResult(allow=..., modified_args=...)` (`modified_arguments_json` removed in `v0.1.18`)
  6. `post_tool_call` (`ToolResult` with `id`, `call_id`, `step_id`, `server_name`)
  7. `on_tool_error` -> optional fallback `str` for self-recovery
  8. `stop` (`StopArgs`) -> `StopHookResult(decision=CONTINUE|ALLOW_STOP, reason=...)` (`CONTINUE` requires non-empty `reason` or raises `ValueError`)
  9. `on_interaction` (`AskQuestionInteractionSpec`) -> `QuestionHookResult(responses=[...])`
  10. `on_compaction` (`Step`)
- **Beta Dynamic Workflows (`google.antigravity.beta.workflows`, `v0.1.21`–`v0.1.23`):** `@beta.workflows.define` (AST rejects `print()`), `phase`, `log`, `agent` (`WorkflowAgent`), `parallel`, `pipeline`, `call_tool`, `args`; invoked via `agent.beta.run_workflow(...)`.
- **Error Hierarchy:**
  - `AntigravityValidationError` (fatal: bad config, relative `app_data_dir`, empty prompt)
  - `AntigravityConnectionError` (retryable for transient 429/5xx/WS drop; fatal on 4xx/auth)
  - `AntigravityExecutionError` (fatal: terminal `localharness` error or non-zero subprocess teardown exit)
  - `AntigravityCancelledError` (**inherits from `asyncio.CancelledError` (`BaseException`), NOT `Exception`** — catch before `CancelledError`)
  - `ToolExecutionError` (carries `step_id`; catchable in `OnToolErrorHook`)
  - `ValueError` (empty `StopHookResult.reason`, uninferable MIME type)
- **Observability:** `UsageMetadata` (`prompt_token_count`, `cached_content_token_count`, `candidates_token_count`, `thoughts_token_count`, `total_token_count`, `service_tier`; supports arithmetic operators), Python `logging.getLogger("google.antigravity")`, and native OpenTelemetry GenAI semantic spans (`v0.1.5+`).

---

## 2. IDE Extensions Matrix & Backend Integration Mechanics

| Target Surface | Latest Version / Min IDE | Backend Transport | Auth | Key Features & Constraints |
| --- | --- | --- | --- | --- |
| **Antigravity IDE** (standalone) | `v2.8.0` (`v2.6.0+` embeds VS Code ext) | `agy --hub` / Language Server + WebSocket | Personal OAuth, Enterprise/BYOID, ADC *(standalone IDE does NOT support Enterprise accounts per `getting-started.md`)* | **Exclusive Tab completions (Supercomplete + Tab-to-Jump)**; inline diffs; `/browser` via Chrome DevTools MCP (`npx`). |
| **VS Code** (`Google.google-antigravity`) | `v1.13.0` (VS Code `1.90+`) | Auto-downloads `~/.gemini/bin/agy` (or reuses `/usr/bin/agy`) -> `agy --hub` / LS + WS | OAuth, Enterprise, Cloud Workstations ambient auth | Inline diffs, `@terminal` (8 KB), gutter comments, voice dictation (`v1.13.0`), Remote-SSH & VS Code Web (`AGY_ALLOWED_DOMAINS`), `/browser` (local Chrome 144+ only). |
| **Visual Studio** (`Google.GoogleAntigravity`) | `v1.0.261116.0` (VS 2022 `17.14+` / 2026) | Auto-downloads `~/.gemini/bin/agy` -> LS + multiplexed WS | OAuth, Enterprise | Solution sync, hierarchical inline diffs, `@terminal` + Output panes, Send to Terminal, voice dictation. |
| **Eclipse** | `v1.2.0` (Eclipse `2026-06+`) | Auto-managed local server + WebView (WebKitGTK / WebView2) | OAuth, Enterprise | Compare editor diffs + Undo, Workspace Trust synced with CLI, Maven/Gradle import. |
| **JetBrains IDEs** (IntelliJ, PyCharm, WebStorm, GoLand, CLion, Rider, Air) | `v1.5.0` (JB `2026.2.1+`) or `~/.jetbrains/acp.json` | **ACP stdio** (`antigravity-acp` Registry or `agy acp`) | 4 ACP `auth.type` JSON modes | Native AI Chat, diffs, permission cards, context usage (`v1.5.0`); requires absolute `which agy` path. |
| **Android Studio** | ACP Registry (BYOA) | **ACP stdio** (`antigravity-acp`) | 4 ACP modes (Enterprise Preview) | Native Agent tool window, 3rd-party models (`v1.4.1+`). |
| **Zed** | `v1.5.0` (Zed `0.140.0+`) or `settings.json` `agent_servers` | **ACP stdio** (`antigravity-acp` or `agy acp`) | 4 ACP modes + `GEMINI_API_KEY` | Multi-root workspaces, inline diffs, permission cards. |
| **Xcode** | `v1.5.0` (Xcode `27 beta 6+`) | **ACP stdio** (`antigravity-acp` or `agy acp`) | 4 ACP modes (`_meta` API key) | Requires absolute path (no `~` or shell `$PATH`); MCP `rawInput` flattened + nested. |
| **Other ACP Editors** | Any ACP v1 client (e.g. VS Code `formulahendry.acp-client`) | **`agy acp`** stdio | Cached CLI creds + 5 ACP methods (incl. `terminal-login`) | `--process-per-session` for HTTP MCP; `--ide-mcp-port` (requires `nc`); `--sandbox`. |

### Key Patterns Used by Google's Official IDE Extensions for Binary Sync
Google's VS Code, Visual Studio, and Eclipse extensions all converged on a shared binary lifecycle design:
- **Shared cache & lock:** `~/.gemini/bin/agy` + `~/.gemini/bin/agy.version.json` + `.agy-install.lock` preventing concurrent download corruption and **cross-window binary downgrades** (a real bug fixed in VS Code `v1.12.0` / IDE `v2.8.0`).
- **Warm start + async update:** Launch cached binary immediately; fetch manifest asynchronously; re-fetch manifest if a release rolls mid-download; preserve offline fallback; never wipe metadata on transient timeout.
- **Health & crash recovery:** `/healthz` heartbeat (15s with OS sleep/suspend drift detection), max 3 restarts in 10 min with jittered backoff, and **non-retriable detection** for `SIGILL` (missing `SSE4.1`/`SSE4.2` on Linux VMs), incompatible CLI flags, and Windows logoff loops.
- **Windows locked-binary upgrades:** Stage running binary aside before atomic swap + 24h cleanup of abandoned staging directories.

### Migrations
- **Workflows -> Skills:** Legacy `.agents/workflows/<name>.md` (12K-char cap) is deprecated in favor of `.agents/skills/<name>/SKILL.md` (unbounded, progressive disclosure). `/migrate-workflows` exists only in App `v2.12.0+`. Dynamic Workflows (`run_workflow`) is the new orchestration engine.
- **Firebase Studio -> Antigravity:** Automated `@fbs-to-agy-export` or zero-token `npx firebase-tools@latest studio:export <path>` (Node 20+, Firebase CLI `15.10.0+`).

---

## 3. Versioning, Release Cadence & Drift Analysis (`changelog.md`)

### 3.1 Measured Release Velocity Across All 8 Surfaces
| Surface | Version Series | Total Releases | Date Span | Overall Cadence | Recent Cadence (Last 15) |
| --- | --- | ---: | --- | --- | --- |
| **Antigravity CLI (`agy`)** | `v1.0.0` -> `v1.5.12` *(live `v1.5.14`)* | 97+ | Jan 1 – Nov 2026 | 3.35 days/release | **1.43 days/release** |
| **Antigravity App** | `v2.0.1` -> `v2.34.0` | 49 | May 19 – Nov 20, 2026 | 3.85 days/release | **1.79 days/release** |
| **VS Code Extension** | `v1.0.0` -> `v1.13.0` | 15 | Aug 18 – Nov 16, 2026 | 6.43 days/release | Weekly (Mon) |
| **Visual Studio VSIX** | `v1.0.260818.0` -> `v1.0.261116.0` | 14 | Aug 18 – Nov 16, 2026 | 6.92 days/release | Weekly (Mon) |
| **Python SDK** | `v0.1.1` -> `v0.1.24` | 22 | May 29 – Nov 16, 2026 | 8.14 days/release | 8.43 days/release |
| **ACP Registry Server** | `v1.0.0` -> `v1.5.0` | 10 | Aug 20 – Nov 10, 2026 | 9.11 days/release | ~10–14 days |
| **Antigravity IDE** | `1.11.2`–`1.23.2`, `2.0.1` -> `v2.8.0` | 33 | Nov 18, 2025 – Nov 19, 2026 | 11.44 days/release | ~17 days (lags VS Code ~10d) |
| **Eclipse Extension** | `v1.0.0` -> `v1.2.0` | 3 | Oct 27 – Nov 19, 2026 | 11.50 days/release | ~11 days |

### 3.2 Model Default Churn (6 Default Shifts in 6 Months)
`Gemini 3.5 Flash` (May 19) -> `3.6 Flash` (Jul 21) -> `3.7 Flash` (Aug 13) -> `3.8 Flash` (Sep 2) -> `4 Argon` (Oct 13) -> `4 Carbon` (Nov 10) -> `4 Hydrogen` + `Orchestrator` (Nov 12) -> `4 Neon` + `Orchestrator (Neon)` (Nov 18). **Default model slugs rotate every 2–5 weeks.**

### 3.3 High-Impact Breaking & Behavioral Shifts Wrappers Must Track
1. **Default `Agent Mode` flipped in a patch release (`v1.5.7`):** Unset `agentMode` switched from `default` to `auto` (even in `-p`). Wrappers must pass `--mode` explicitly.
2. **Exit codes changed across patch/minor versions:** `--print-timeout` changed from exit `0` (`v1.1.28`) to exit `3` + `AGY_ERROR` (`v1.5.0`); streaming errors changed from `0` to `3` (`v1.2.10`); `GEMINI_API_KEY` unsupported features changed to exit `2` (`v1.5.11`–`v1.5.12`).
3. **Sandbox & permission tightening:** `--dangerously-skip-permissions` stopped auto-approving `/plan` (`v1.3.2`) and stopped bypassing protected commands & Enterprise admin reviews (`v1.5.2`); sandbox blocked `~/.docker`, `~/.kube`, `~/_netrc`, `~/Library/Application Support` (`v1.3.3`/`v1.4.6`), and `~/.config`, `AppData\Roaming`, `~/.git-credentials` (`v1.5.10`).
4. **Config relocation:** App `v2.17.0` moved per-project config from `.agents/settings.json` (no longer read!) to `<project>/.gemini/config.json`.
5. **Diff & ACP protocol fixes:** `v1.5.9` removed stray trailing newlines from `oldText`/`newText` ACP diffs; `v1.5.10` enabled immediate post-turn wakeup streaming and cleanup of orphan subagents/daemons on cancel/close; `v1.5.14` added `showThinking` (`/thinking`), `plan` mode on `--remote-agent`, per-role `orchestrator` config in `settings.json`/`--settings`, and changed `auto` mode to allow out-of-workspace reads without prompting.
6. **SDK `0.1.x` breaking changes:** Removed `McpSseServer` (`v0.1.2`), removed post-init dynamic hooks (`v0.1.4`), flipped `AgentBehavior` default to `AUTONOMOUS` (`v0.1.11`), removed `ASK_QUESTION` from defaults & `modified_arguments_json` (`v0.1.18`), deprecated `START_SUBAGENT` -> `INVOKE_SUBAGENT` (`v0.1.22`), changed `DEFAULT_MODEL` to `gemini-4-hydrogen` & mapped `EXTRA_HIGH` -> `xhigh` (`v0.1.24`).
