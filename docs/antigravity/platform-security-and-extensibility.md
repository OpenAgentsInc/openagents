# Google Antigravity — Platform, Security, Extensibility & Orchestration Reference

> **Scope:** Exhaustive reference for Antigravity's permission model, OS terminal sandbox, lifecycle hooks, MCP & MCP Apps (`ui://`), customization discovery hierarchy (skills, rules, custom agents, plugins, sidecars), multi-agent orchestration (subagents, swarms, teamwork, boost, dynamic workflows, schedules, automations), first-party App/remote surfaces (Remote Control, Remote Agents, SSH, Voice/`mic-serve`, Browser Use, Artifacts, Generative UI, Review Changes), and Enterprise/Workspace/Plans/Quotas/ToS constraints.
>
> **Sources:** Official Antigravity documentation corpus (`antigravity.google/docs/*`, 119 pages) and built-in Antigravity skills (`agy-customizations`, `mcp-app-development`, `plugin`, `run-workflow`, `swarm_monitor`, `ui-plugin-navigation`).

---

## 1. Permissions & Terminal Sandbox

### 1.1 Permission Modes & Presets Across Surfaces

Antigravity separates **surface-level permission presets** from **CLI execution modes (`--mode`)**, **tool permission baselines (`toolPermission`)**, and **explicit `action(target)` rules**.

| Surface | Mode / Preset | Terminal Sandbox | Terminal Commands | File Access | MCP & Web |
|---|---|---|---|---|---|
| **App (Updated: personal & non-admin business)** | **Request Review** | Off | Always ask | Workspace only | Ask |
| | **Default** *(default if sandbox available)* | On | Allowed in sandbox; ask outside | Workspace + temp dirs | Ask |
| | **Auto** *(App `v2.23.0+` macOS/Linux, `v2.26.0+` Win; unavailable on admin-managed/Enterprise)* | On | Allowed in sandbox; auto-reviewed outside | Workspace + temp dirs | Ask |
| | **Turbo** | Off | Allowed unrestricted | Full filesystem | Allowed (except uploads, `data:`/`blob:` downloads, Deny list, IdP/payment, blocked download types) |
| **App (Previous: business/enterprise)** | **Default** (`Require Review` / Outside-of-folders: `Always Ask`), **Full machine** (`Require Review` / `Allow`), **Turbo mode** (`Always Proceed` / `Allow`), **Custom** (enables **Enable Sandbox Mode (Preview)** + **Proceed in Sandbox**) | Off in presets; On in Custom | `Require Review`, `Proceed in Sandbox`, `Always Proceed` | Workspace + `~/.gemini/antigravity/`; outside-of-folder: `Always Ask`, `Allow`, `Always Deny` | Ask |
| **CLI (`agentMode` / `--mode`, cycled via `Shift+Tab`)** | `default` (`""` in `settings.json`) | Per `enableTerminalSandbox` (`false` default) | Follows `toolPermission` (`request-review` default) | Prompts interactive diff review on edits/creates | Follows `toolPermission` |
| | `auto` *(default in `v1.5.7+` where available: `v1.3.5+` macOS/Linux, `v1.4.4+` Win; personal only)* | Auto-enabled | Allowed in sandbox; auto-reviewed outside | Auto-approves workspace edits/creates (incl. subagents) | Ask |
| | `accept-edits` | Per `enableTerminalSandbox` | Follows `toolPermission` | Auto-approves workspace edits/creates (incl. subagents) | Follows `toolPermission` |
| | `plan` | Per `enableTerminalSandbox` | Prepends `/plan` read-only analysis | Read-only until approved | Follows `toolPermission` |
| **CLI (`toolPermission` baseline/overrides)** | `request-review` (default), `proceed-in-sandbox` (falls back to `request-review` if sandbox unavailable in `v1.4.4+`), `strict` (prompts ALL actions incl. reads; overrides `auto` & allow rules), `always-proceed` (`--dangerously-skip-permissions`; overrides `auto`, still honors `ask`/`deny` & built-in protected command list) | `--sandbox` / `enableTerminalSandbox` | Controls baseline outside `auto`; `strict`/`always-proceed` override `auto` | `allowNonWorkspaceAccess` (`false` default) | Controls baseline |
| **IDE** | **Strict mode** vs normal (`Request Review` / `Always Proceed`, `Enable Terminal Sandbox`, `Sandbox Allow Network`) | Strict auto-enables sandbox + denies network | Strict forces `Request Review` & ignores allowlist | Strict enforces `.gitignore` + disables non-workspace (normal allows `~/.gemini/antigravity-ide/`) | Strict gates Markdown images, `read_url`, browser JS, artifact review |
| **Remote Agents** | `always-proceed (turbo)` (default), `default`, `request review` | Cloud-isolated | Per preset | Remote project resources | Plan Review Policy: `always ask` / `always proceed` |

- **App preset scopes:** Global (**Settings > General > Global Permissions > Permission Preset**, or **Permissions > This Computer** when Remote Agents enabled), per-project override (**Settings > Projects**, default `Inherit Global`), or composer picker (`v2.23.0+`; **Use Global Settings** clears override). Standalone conversations have their own scoped settings and scratch directory.
- **CLI rule scopes in `/permissions`:** **Project**, **Shared** (shared across all Antigravity products), and **Global** (`~/.gemini/antigravity-cli/settings.json` under `permissions.{allow,ask,deny}`).

### 1.2 Rule Syntax, Matching & Precedence

- **Format:** `action(target)` (no space before `(`), evaluated across three lists: `deny`, `ask`, `allow`.
- **Strict Precedence:** **`Deny > Ask > Allow`**. Explicit rules always override presets and defaults (e.g., `ask: command(rm)` prompts even in sandbox; `deny` overrides `Turbo`).
- **Implicit Rules:**
  - `allow write_file(P)` implicitly grants `read_file(P)`.
  - `deny read_file(P)` implicitly denies `write_file(P)`.
- **Global wildcard:** `*` works in every action namespace (`read_file(*)`, `command(*)`, `mcp(*)`, etc.).
- **Supported Actions:**
  - `read_file(/path | dir | *)`: Absolute or workspace-relative recursive match. Default: Ask (allowed inside workspace).
  - `write_file(/path | *)`: Same as `read_file`; implies read. Default: Ask (allowed inside workspace).
  - `read_url(domain | *)`: Matches hostname & subdomains, ignores path. Also populates sandbox `AllowedDomains` outbound allowlist. Default: Ask.
  - `execute_url(domain | *)`: Browser subagent navigation/redirects. Ignores path; isolates loopback (`localhost`, `127.0.0.1`, `[::1]`) by `:port` (`localhost` without port allows all ports); requires explicit rules for sensitive Google subdomains (`accounts.google.com`, `myaccount.google.com`, `mail.google.com`, `docs.google.com`, `drive.google.com`, `calendar.google.com`, `chat.google.com`, `colab.research.google.com`, `console.cloud.google.com`). Not covered by `read_url`. Default: Ask.
  - `download_url(domain | *)`: Built-in browser agent downloads (App `v2.30.0+` rules, `v2.31.0+` prompts). Same domain/`localhost:port`/sensitive-Google matching as `execute_url`. `data:` and `blob:` always ask once (even in Turbo). Default: Ask.
  - `command(prefix | regex:pattern | *)`: Word/token prefix match by default; `regex:` evaluates each whitespace-separated token as anchored `^(?:pattern)$` (or full raw line when exact-match fallback triggers). Covers commands both inside and outside the sandbox.
  - `unsandboxed(prefix | regex:pattern | *)`: **Business/Enterprise accounts & previous permission settings only** (ignored with warning on consumer accounts). Permits running outside sandbox.
  - `mcp(server/tool | server/* | *)`: Exact tool or server wildcard (local & remote). Default: Ask.
- **When Commands Disable Prefix Matching (Exact-Match Fallback):**
  - Triggered by: `$(...)`, backticks, `<(...)`, `$((...))`, brace expansion `{a,b}`, non-literal command names, network/FD redirections, subcommand-executing flags (e.g., `git -c core.pager=<cmd>`, `tar --to-command`), or unparseable syntax (including Windows PowerShell/CMD). Requires full-line character-for-character match (or full raw line `regex:`), else falls back to **Ask**.
  - Normal prefix matching is preserved for: pipelines `|`, chains `&&`/`||`/`;`, quoted literals, plain `$VAR`, simple file redirects, and transparent wrappers (`timeout`, `nohup`, `nice`, `env` — inner command evaluated).
- **Cross-Platform Path Normalization:** Windows strips drive letters (`C:`) and normalizes `\` to `/` before matching.
- **Interactive Scope Editing:** Prompt cards allow editing target scope (e.g., widening a file rule to its parent directory) before clicking Allow for file, URL, and MCP permissions (validates that the target covers the operation; applies for the rest of the turn). **Not supported for terminal commands.**

### 1.3 Managed Admin Policies & Protected Files

- **Admin Policy File (`admin_settings.json`):** Device management policies lock sign-in and project settings, enforce permission ceilings, disable remote control, configure enterprise LLM gateway routing (`AGY_LLM_GATEWAY_*`, `AGY_LLM_GATEWAY_MODELS`, `AGY_LLM_GATEWAY_PROXY_URL`, `caCertPath`, `models.list[].supportsThoughtCirculation`), and set `pluginControls` (`pluginsDisabled`, `marketplaceDisabled`). *(Note: exact OS filesystem path(s) for `admin_settings.json` are not publicly documented in the official docs corpus.)*
- **Admin Enforcement Integrity:** Hooks and wildcard rules cannot bypass admin-enforced command or outside-project file review; org browser allowed domains appear as **Organization Allowed Domains (Read-Only)**.
- **Protected Files & Blocked Surfaces:**
  - Sandbox blocks sensitive files such as `~/.ssh` and `.env`; on macOS/Linux anything not explicitly mounted is invisible. On Windows, other files in the user profile and other users' profiles are hidden; folders outside the user profile are readable but non-writable.
  - `--dangerously-skip-permissions` still honors `ask`/`deny` rules and excludes a built-in list of protected commands (which go through standard review).
  - Browser agent blocks executable/active-markup downloads: `.exe`, `.dmg`, `.pkg`, `.msi`, `.bat`, `.cmd`, `.ps1`, `.sh`, `.dll`, `.so`, `.dylib`, `.html`, `.svg`, `.xml`, `.wasm`, plus IdP/payment domains and local/private/cloud-metadata redirects.
  - Voice mode blocks voice approval of destructive commands (`rm`, `sudo`, git force-push); the user must click Allow on screen (voice Deny still works).

### 1.4 Terminal Sandbox Internals & Network Proxy

| OS | Isolation Primitive | Details / Requirements |
|---|---|---|
| **macOS** | `sandbox-exec` (Seatbelt SBPL) | Restricts filesystem mounts & socket connections. |
| **Linux** | Kernel namespaces (unprivileged user/mount/PID/net namespaces; enterprise docs also cite `bubblewrap`) | Isolates filesystem, hides host processes, cuts off networking. Unavailable in hosts/containers blocking unprivileged user namespaces. |
| **Windows** | AppContainer inside Process Security Environments | Requires Windows 11 24H2+ with August 2026 update or later. Outside-user-profile dirs remain read-only. |

- **Default Mounts:** Workspace/project folders (RW), temp dirs (RW), common build caches (RW), system read dirs like `/usr`, `/etc` (RO). Dynamic additions: `read_file` grants become RO mounts; `write_file` grants become RW mounts; `deny` rules block paths.
- **Network Allowlist & Proxy:**
  - No outbound network by default inside the sandbox.
  - `read_url(...)` grants compile into an outbound allowlist (`AllowedDomains`) enforced by the built-in proxy.
  - **Custom CLI Proxy (`sandboxProxy` in `~/.gemini/antigravity-cli/settings.json`):**
    ```json
    {
      "sandboxProxy": {
        "http": {
          "address": "host:port",
          "certFile": "/path/ca.pem",
          "username": "...",
          "password": "..."
        }
      }
    }
    ```
    Replaces the built-in proxy while sandbox is active; custom proxy must enforce `read_url` rules itself. Requires both `address` + `certFile` (and both `username` + `password` if auth is used) or it is ignored.
  - **Plugin OAuth Token Protection in Sandbox:** Placeholder tokens are replaced with real OAuth tokens **only in HTTP `Authorization` headers**, preventing exfiltration via URLs or echo headers.
- **Graceful Fallback (`v1.4.4+`):** If the OS sandbox is unavailable, `enableTerminalSandbox`/`--sandbox` are no-ops, `proceed-in-sandbox` behaves as `request-review`, `auto` is hidden/not offered, and saved settings remain untouched.

### 1.5 Auto Mode Reviewer Behavior

- Commands run inside the terminal sandbox by default (zero prompts).
- When a command requests to run **outside** the sandbox (network access, Docker, host services, or sandbox retry), a **separate reviewer agent** evaluates the command (applies to main agent, subagents, and custom agents):
  - **Auto-approves** safe, reversible commands (e.g., workspace format, lint, fix, build, test commands).
  - **Prompts / blocks** risky, irreversible commands or commands that would upload sensitive data.
  - Explicit `command(...)` allow/ask/deny rules still take precedence over Auto review.
- **Wrapper Implication:** Mapping `auto` mode to `--dangerously-skip-permissions` breaks this entire security model by turning off sandboxing and bypassing the reviewer agent.

---

## 2. Hooks System

### 2.1 File Locations & Merging

- **Workspace:** `<workspace>/.agents/hooks.json`
- **Global:** `~/.gemini/config/hooks.json` (CLI `/hooks` writes here; any `hooks` key inside `~/.gemini/antigravity-cli/settings.json` is **ignored**)
- **Plugin:** `<plugin-root>/hooks.json` (**must** be at plugin root for native plugins; `hooks/hooks.json` is ignored except during Gemini CLI / Claude Code extension import staging). UTF-8 BOM is supported.
- **Merging:** Multiple named hook groups across configs and plugins are merged and executed **sequentially**.

### 2.2 Schema & Handler Config

Top-level JSON object mapping arbitrary hook group names to event arrays plus optional `"enabled": false` (default `true`). *(Note: a Claude Code-style top-level `"hooks"` key is parsed as a single hook group named `"hooks"`.)*

- **Handler object:**
  - `type`: `"command"` (optional, default `"command"` — **only `"command"` is supported**; no HTTP or prompt hooks).
  - `command`: string (required; executed via `sh -c` on Unix, `cmd /c` on Windows; `~` expanded; process `cwd` is the directory containing `hooks.json`). Plugin command hooks receive `PLUGIN_ROOT` and `PLUGIN_DATA` as **environment variables** (not interpolated into the `command` string — use relative `./scripts/...`).
  - `timeout`: integer seconds (optional, default `30`).
- **Execution Model:** **Strictly synchronous** — blocks the agent execution loop (no async hooks).
- **Exit Code Contract:** Must exit **`0`** and print valid JSON to `stdout`. **Any non-zero exit code is logged as a hook execution error and ignored (fails open).**

### 2.3 Lifecycle Events & Matchers

| Event | Structure | Matcher Target | Fires When |
|---|---|---|---|
| `SessionStart` | Flat handler array | N/A (ignored) | Once when a new conversation session starts, before first model invocation. |
| `PreToolUse` | Grouped `{ "matcher": "<regex>", "hooks": [...] }` | Tool name regex (`""` or `"*"` = all; e.g. `"run_command\|view_file"`, `"browser_.*"`; derived by lowercasing step type & stripping `CORTEX_STEP_TYPE_`) | Before a tool executes. |
| `PostToolUse` | Grouped `{ "matcher": "<regex>", "hooks": [...] }` | Tool name regex | After a tool completes. |
| `PreInvocation` | Flat handler array | N/A (ignored) | Before each model invocation. |
| `PostInvocation` | Flat handler array | N/A (ignored) | Immediately after each model invocation / tool calls finish. |
| `Stop` | Flat handler array | N/A (ignored) | When the execution loop terminates. |

**Documented Standard Tool Names (for matchers & `toolCall.name`/`args`):**
- **File/Dir:** `view_file` (`AbsolutePath`, `StartLine`, `EndLine`, `IsSkillFile`), `write_to_file` (`TargetFile`, `Overwrite`, `CodeContent`, `Description`, `IsArtifact`, `ArtifactMetadata`), `replace_file_content` (`TargetFile`, `Instruction`, `Description`, `AllowMultiple`, `TargetContent`, `ReplacementContent`, `StartLine`, `EndLine`, `TargetLintErrorIds`), `multi_replace_file_content` (`TargetFile`, `Instruction`, `Description`, `ReplacementChunks`, `TargetLintErrorIds`, `ArtifactMetadata`), `list_dir` (`DirectoryPath`), `find_by_name` (`SearchDirectory`, `Pattern`, `Type`, `Excludes`, `Extensions`, `FullPath`, `MaxDepth`).
- **Search/Web:** `grep_search` (`SearchPath`, `Query`, `IsRegex`, `CaseInsensitive`, `Includes`, `MatchPerLine`), `search_web` (`query`, `domain`), `read_url_content` (`Url`).
- **Execution/System:** `run_command` (`CommandLine`, `Cwd`, `WaitMsBeforeAsync`, `RunPersistent`, `RequestedTerminalID`), `manage_task` (`Action`, `TaskId`, `Input`), `schedule` (`DurationSeconds`, `CronExpression`, `MaxIterations`, `Prompt`), `list_permissions`, `ask_permission` (`Action`, `Target`, `Reason`).
- **Collaboration:** `invoke_subagent` (`Subagents`), `define_subagent` (`name`, `description`, `system_prompt`, `enable_mcp_tools`, `enable_write_tools`, `enable_subagent_tools`), `send_message` (`Recipient`, `Message`), `manage_subagents` (`Action`, `ConversationIds`).
- **UI/Media:** `ask_question` (`questions`), `generate_image` (`Prompt`, `ImageName`, `ImagePaths`).

### 2.4 Input (`stdin`) & Output (`stdout`) JSON Contracts (camelCase / protojson)

**Common `stdin` fields (all 6 events):**
- `conversationId` (`string` UUID)
- `workspacePaths` (`string[]`)
- `transcriptPath` (`string` — `<app_data_dir>/brain/<conversationId>/.system_generated/logs/transcript.jsonl`, where `<app_data_dir>` is `~/.gemini/antigravity` [App], `~/.gemini/antigravity-cli` [CLI], `~/.gemini/antigravity-ide` [IDE], or `~/.gemini/antigravity-acp` [ACP])
- `artifactDirectoryPath` (`string` — `<app_data_dir>/brain/<conversationId>`)
- `modelName` (`string`)
- `lastUserInput` (`string`)
- `hookEventName` (`string`, exact event key from `hooks.json`) & `cwd` (`string`, agent working dir — documented in builtin `agy-customizations` hooks spec)

**Per-Event Contracts:**
- **`SessionStart`**
  - `stdin`: Common fields only.
  - `stdout`: `{ "injectSteps": [...] }` (optional; same schema as `PreInvocation`).
- **`PreToolUse`**
  - `stdin`: `toolCall: { id?, name, args }`, `stepIdx` (`int`) + common.
  - `stdout`:
    - `decision` (**required**): `"allow"` (proceed/delegate to next hook or policy), `"auto_approve"` (auto-approve if default policy would prompt, while honoring explicit user rules), `"deny"` (hard block), `"ask"` (prompt, respects Always Allow cache), `"force_ask"` (always prompt, ignores cache), `"deny_unless_prior_grant"` (deny unless previously granted).
    - `reason` (optional `string`).
    - `overwrite` (optional `object`): **shallow top-level merge** into `toolCall.args` before execution; prepends a notice naming changed keys to the tool result.
    - `permissionOverrides` (optional `string[]`, e.g., `["command(npm test)"]`).
- **`PostToolUse`**
  - `stdin`: `toolCall: { id?, name, args }` (same `id` as `PreToolUse`), `stepIdx` (`int`), `error` (`string`, empty on success) + common. *(Does NOT receive tool output.)*
  - `stdout`: `{}`.
- **`PreInvocation`**
  - `stdin`: `invocationNum` (0-indexed `int`), `initialNumSteps` (`int`) + common.
  - `stdout`:
    - `injectSteps` (optional array of single-key step objects):
      - `{"systemMessage": {"systemMessage": "...", "metadata": {"user_markdown"?: "..."}}}` (`metadata.user_markdown` renders inline Markdown to user without sending it to the model).
      - `{"userMessage": "..."}`
      - `{"ephemeralMessage": "..."}` *(deprecated; converted to hidden system message)*
      - `{"errorMessage": {"userMessage": "..."}}` (displayed without sending to model; sets non-zero exit code in `-p` headless mode; requires `"terminationBehavior": "terminate"` to stop execution).
    - `terminationBehavior` (optional): `"terminate"` | `""`.
- **`PostInvocation`**
  - `stdin`: Same as `PreInvocation`.
  - `stdout`: `injectSteps` (same) + `terminationBehavior`: `"force_continue"` | `"terminate"` | `""`.
- **`Stop`**
  - `stdin`: `executionNum` (`int`), `terminationReason` (`string` — docs show `"model_stop"`, `"max_steps_exceeded"`, `"error"`; builtin spec lists enum names stripped of `EXECUTOR_TERMINATION_REASON_`: `"NO_TOOL_CALL"`, `"MAX_INVOCATIONS"`, `"MAX_FORCED_INVOCATIONS"`, `"MAX_TOKEN_BUDGET_EXCEEDED"`, `"ERROR"`, `"USER_CANCELED"`, `"TERMINAL_STEP_TYPE"`, `"TERMINAL_CUSTOM_HOOK"`, `"INJECTED_RESPONSE"`), `error` (`string`), `fullyIdle` (`bool`, required — `false` if background tasks still running), `continuationActive` (`bool` — `true` after a `Stop` hook kept the agent going this turn) + common.
  - `stdout`: `decision` (**required**: `"continue"` blocks stop & re-enters loop; any other value allows stop), `reason` (optional `string` injected as system message when continuing).

---

## 3. MCP & MCP Apps (`ui://`)

### 3.1 `mcp_config.json` Paths, Transports & Schema

- **Config Locations:**
  - Global: `~/.gemini/config/mcp_config.json`
  - Workspace: `<workspace>/.agents/mcp_config.json` (if global & workspace reuse a server key, the already-running server keeps it)
  - Plugin: `<plugin-root>/mcp_config.json` (namespaced at runtime as `<plugin-name>_<server-key>`)
  - Refresh RPC (internal Language Server): `POST http://${ANTIGRAVITY_LS_ADDRESS:-localhost:5387}/exa.language_server_pb.LanguageServerService/RefreshMcpServers` with header `x-codeium-csrf-token: ${ANTIGRAVITY_CSRF_TOKEN}` and body `{"serverName": "..."}`.
- **Transports:**
  - **Local `stdio`:** `command` (falls back to bundled Node.js when `"command": "node"`), `args`, `env`, `cwd`.
  - **Remote Streamable HTTP:** `serverUrl` or `url`. **Legacy HTTP+SSE (`2024-11-05`) and `httpUrl` are NOT supported** in App/CLI/IDE (use `mcp-remote` over `stdio` as bridge; the Python SDK separately supports `stdio`, `SSE`, and `HTTP`).
- **Full `mcpServers.<id>` Properties:**
  - `command` (`string`), `serverUrl` / `url` (`string`), `args` (`string[]`), `env` (`object`), `cwd` (`string`), `headers` (`object` — not forwarded across cross-origin redirects).
  - `authProviderType`: `"google_credentials"` (attaches ADC; requires `gcloud auth application-default login` + `set-quota-project`).
  - `oauth`: `{ "clientId": "...", "clientSecret"?: "..." }`.
  - `disabled`: `boolean` (default `false`).
  - `enabledTools`: `string[]` (allowlist; hides all other tools when non-empty).
  - `disabledTools`: `string[]` (denylist; default `[]`).
  - `tools`: `{ "<toolName>": { "eager"?: boolean, "background"?: "off" | "always" } }`.
  - `timeoutSeconds`: `integer` (defaults: `300`s connect/list, `180`s per tool call; positive replaces both; `-1` disables both; `0`/unset keeps defaults).
  - `variables` *(plugin `mcp_config.json` only, inside `mcpServers.<server>.variables`)*: array of `{ name (req), title?, description?, link? }`. Plugins with `variables` install disabled via CLI until the user supplies values in UI or `/plugin`.
- **Env / Variable Expansion (Plugin MCP):**
  - `${PLUGIN_ROOT}` (read-only plugin dir) and `${PLUGIN_DATA}` (persistent writable dir outside workspace) expand in `args`, `cwd`, custom `env`, and are auto-exported as env vars `PLUGIN_ROOT` and `PLUGIN_DATA`.
  - `${VARIABLE_NAME}` from `variables` expands in `command`, `args`, `env`, `url`, `headers`, and `oauth`.

### 3.2 OAuth 2.0 Flow & Token Storage

- Auto-discovers `/.well-known/oauth-protected-resource` (`resource` must match MCP server URL) and first `authorization_servers` entry (`/.well-known/oauth-authorization-server` or `/.well-known/openid-configuration`).
- Credential resolution order:
  1. Manual `oauth.clientId` / `clientSecret`.
  2. **CIMD** (`client_id_metadata_document_supported: true`, `code_challenge_methods_supported: ["S256"]`, non-empty `issuer` — uses `client_id=https://antigravity.google/oauth/client-metadata.json`, `redirect_uri=https://antigravity.google/oauth-callback`, `token_endpoint_auth_method: "none"`, PKCE `S256`, RFC 8707 `resource`; IDE requires `v2.6.0+`).
  3. **DCR** (`registration_endpoint`).
- Tokens stored in `~/.gemini/antigravity/mcp_oauth_tokens.json` (and OS keyring); auto-refreshed (including public clients without `clientSecret`); callback falls back to IPv6 `[::1]` if `127.0.0.1` is unavailable; supports auth codes > 1024 chars.

### 3.3 Tool Filtering & Lazy vs. Eager Loading

Evaluated after `enabledTools` / `disabledTools`:
- **Lazy (default):** Writes tool schemas to `<app-data-dir>/mcp/<serverName>/<toolName>.json` + optional `instructions.md`. System prompt lists dir path & tool names grouped under `Eager`/`Lazy`. The agent reads schema files on demand and invokes built-in `call_mcp_tool(ServerName, ToolName, Arguments)`. Falls back to eager if disk write fails.
- **Eager (`"eager": true` or ACP `_meta.antigravity.eagerMcpServers`):** Registers native tool declaration `mcp_<serverName>_<toolName>` directly in model context on every turn.

### 3.4 MCP Apps UI Protocol (Antigravity App `v2.34.0+`)

- Implements **SEP-1865** (`io.modelcontextprotocol/ui`, 2026-01-26) + Antigravity extension **`com.google.antigravity/ui`** (`{"entrypoints": ["aux_pane", "global", "file"]}`) advertised during MCP `initialize`. Non-UI clients (CLI, IDE) run tools normally via plain-text `content` (CLI ignores `_meta.ui.visibility`).
- **UI Resource:** `ui://` URI, MIME `text/html;profile=mcp-app`, single self-contained HTML doc, <= **10 MiB**, fetched fresh on every mount (never persisted in trajectory). Resource `_meta.ui`: `prefersBorder` (`bool`), `csp: { connectDomains, resourceDomains, frameDomains, baseUriDomains }` (bare `http(s)`/`ws(s)` origins only, optional `*.` or `:port`; paths/keywords silently dropped).
- **Tool `_meta` & Entrypoint Surfaces:**
  - `_meta.ui.resourceUri` (legacy `_meta["ui/resourceUri"]`): must start with `ui://`.
  - `_meta.ui.visibility`: `["model"]`, `["app"]`, or both (default). `["app"]` hides tool from the model (helper tool).
  - `_meta["com.google.antigravity/ui"].entrypoints[]`:
    - **Inline in chat** (no `aux_pane`/`global`/`file` entrypoint): replaces collapsed tool card; auto-resizes up to `800 px` (`150 px` fallback if no `size-changed` within `100 ms`).
    - **`aux_pane`** (`title?`): Side panel tab (one tab per `(server, tool)`). Auto-opens when agent calls tool; manual launches pass `{}`; repeat agent calls update open tab in place; does not render inline.
    - **`global`** (`title?`): Full-width view from left sidebar **Plugins**. Manual launch passes `{}`. Omits conversation capabilities (`message`, `updateModelContext`).
    - **`file`** (`title?`, **required** `extensions: string[]` normalized lowercase with leading `.`): Custom file viewer/editor; **unconditionally hidden from model** regardless of `visibility`; first enabled server claiming extension wins; uses plugin `logo` as tab icon. Invoked with opaque `{"file": {"name": "...", "resourceUri": "antigravity-file://<uuid>/<name>"}}` (hides raw host path). Read via `resources/read` (answered directly by host with base64 `blob`, `application/octet-stream`, <= 10 MiB); Write (local files only, gated on `hostContext["com.google.antigravity/resources"]?.write`) via `com.google.antigravity/resources/write` with `{ uri, text }` or `{ uri, blob }` (<= 10 MiB).
- **Iframe Bridge (`@modelcontextprotocol/ext-apps` JSON-RPC over `postMessage`):**
  - Handshake: `ui/initialize` -> `ui/notifications/initialized`.
  - Host -> App events: `ontoolinput`, `ontoolresult` (persisted `CallToolResult` capped at **1 MiB** on trajectory step), `ontoolcancelled`, `onhostcontextchanged` (`theme`, `styles.variables`, `platform: "web"`, `displayMode: "inline"`, `locale`, `timeZone`, `containerDimensions.maxHeight`).
  - App -> Host calls:
    - `app.callServerTool()` (`serverTools`): own server only, enabled + `"app"` visibility, max **4** concurrent per view, aborts on close.
    - `app.sendMessage()` (`message`): text only; `_meta: {"com.google.antigravity/message": {"target": "new"}}` starts background conversation in same project returning `conversationId` in `_meta`.
    - `app.updateModelContext()` (`updateModelContext`): attaches `text` / `resource_link` pill to next user message.
    - `app.openLink()` (`openLinks`): `http`/`https` external open.
    - `com.google.antigravity/toggle-aux-pane`: params `{"tabType": "fileView", "uri": "file:///..."}` (not in `hostCapabilities`; returns JSON-RPC `-32601` on other hosts).
  - **Limits & Security:** Isolated fresh/opaque origin (no persistent `localStorage`/`IndexedDB`/cookies); default CSP `default-src 'none'` (inline scripts/styles + `data:`/`blob:` images/fonts/media allowed); max **5** hidden background `aux_pane`/`global` frames (LRU); **NO `ui/resource-teardown`** sent on close (persist eagerly); host CSS vars `--color-{background,text,border,ring}-*` & `--font-sans`.

---

## 4. Customization Discovery Hierarchy

### 4.1 Discovery Roots & Traversal Rules

- **Two primary scopes** sharing identical internal layout (`.agents/` also recognizes `.agent/`, `_agents/`, `_agent/`):
  - **Workspace:** Each workspace folder (App project folders; CLI startup dir + `--add-dir`/`/add-dir`; ACP `additionalDirectories`) and **every parent directory up to the repository root (`.git`)**. Stops at repo root; **never scans subdirectories below the workspace folder**.
  - **Personal / Global:** `~/.gemini/config/` + `~/.gemini/AGENTS.md` + `~/.gemini/GEMINI.md`.
  - **Surface-specific extra paths:** CLI also loads `~/.gemini/antigravity-cli/skills/` (**Global**) and `~/.gemini/skills/` (**Shared**) — App ignores these. IDE supports legacy `~/.gemini/antigravity/skills/`.
  - **Shared-Repo Personal Dir:** `<repo-root>/.gemini/config.json` with `{"personal_customization_dir": "people/{user}"}` (`{user}` from `USER`/`USERNAME`/OS user). Reads `<resolved>/.agents/`; manifests inside resolve relative paths from repo root.
- **One-Level Scan Depth:** Scans only immediate children (`skills/<name>/SKILL.md`, `rules/*.md`, `agents/<name>.md` or `agents/<name>/agent.md`, `plugins/<dir>/`). Deeper nesting is skipped unless listed in a discovery manifest.
- **Discovery Manifests (`skills.json`, `rules.json`, `agents.json`, `plugins.json`, plus plugin `sidecars.json`):**
  - Placed in `.agents/` (honored at every level from workspace to repo root) or `~/.gemini/config/`. Additive to standard folders.
  - Top-level fields: `entries[]` (`{ path, include_only?, exclude? }`), `inherits[]` (`{ path, include_only?, exclude? }`), `exclude[]` (hides matching items across all locations except CLI-specific skill dirs).
  - Matching: exact folder/file name (with/without `.md`) or relative path for nested items — **no wildcards**. Project manifest relative paths resolve against the folder containing `.agents/` (fallback: repo root); personal manifest `entries` resolve from `~/.gemini/config/` (use absolute paths for `inherits`; `~` is not expanded).

### 4.2 Exact Priority & Conflict Resolution

1. **Skills & Custom Agents** (highest -> lowest):
   - *(CLI only, above all & immune to manifest `exclude`)*: `~/.gemini/antigravity-cli/skills/` & `~/.gemini/skills/`
   - Repo personal dir (`personal_customization_dir` from `.gemini/config.json`)
   - Workspace (walking CWD -> repo root + workspace manifests)
   - Personal `~/.gemini/config/` (+ personal manifests)
   - Built-in skills & agents
   - Plugin skills are namespaced `<plugin>:<skill>` and do not collide.
2. **Rules:** **Cumulative** (never override each other); deduplicated by resolved file path.
3. **Plugins:** Single global `name` namespace — first discovered wins; duplicate `name` dropped **silently**. Enable/disable stored in `~/.gemini/config/config.json` under `plugins.<install-dir>.enabled` (keyed by **directory name**, NOT `plugin.json` `name`; overrides `plugin.json` `disabled`).
4. **Refresh:** Rules and skills are reread before each turn; CLI needs `/skills reload` or a new conversation to refresh `/skills` list & slash commands.

### 4.3 Component Formats & Limits

- **Skills (`skills/<name>/SKILL.md`):**
  - Optional subdirs: `scripts/` (`0755`), `examples/`, `resources/`, `references/`, `assets/`.
  - YAML frontmatter: `name` (kebab-case `1–64` chars, required in plugins & must match parent folder name; optional standalone -> defaults to folder name), `description` (**required**, `1–1,024` chars), `disable-model-invocation` (`bool`, default `false` — slash-only when `true`), `license`, `compatibility`, `disable-slash-command` (internal built-in skills).
  - Progressive disclosure (name + description on every turn; body read on demand); auto-exposed as `/<skill-name>`.
- **Rules (`AGENTS.md`, `GEMINI.md`, `rules/*.md`):**
  - `AGENTS.md` / `GEMINI.md` (at `<dir>/`, `<dir>/.agents/`, `~/.gemini/`, `~/.gemini/config/`, `<plugin>/rules/`): **no frontmatter**, always `always_on`.
  - `rules/*.md`: **must** have YAML frontmatter with valid `trigger`: `always_on` | `model_decision` (requires `description`) | `glob` (requires `globs` or `glob` as **comma-separated string**, not YAML array) | `manual` (explicit `@rules:` mention in App; unsupported in CLI). Missing frontmatter or invalid trigger (e.g., camelCase) is **silently discarded**.
  - File includes inside rules: `@[label](path)` inlines target content before size check (resolves relative to rule file, expands `~/`, strips target frontmatter); `@filename` does **not** inline and rewrites to canonical `@/workspace/path`.
  - Budgets: **24 KB (`24,000` bytes)** per-file truncation (post-includes); **20,000-token** aggregate budget for global + `always_on` rules (largest files auto-demoted to `- <path>: <description>` pointers).
- **Custom Agents / Subagents (`agents/<name>.md` or `agents/<name>/agent.md`):**
  - YAML frontmatter: `name` (req, kebab-case), `description` (req), `tools` (allowlist; `send_message` always included; **known issue: misspelled tool name can hang subagent**), `mainAgent` (default `true`; selectable in App, CLI `/agents`, `agy --agent <name>`), `subagent` (default `true`; callable via `invoke_subagent`), `model` (`inherit`|`flash_lite`|`flash`|`pro`), `commandExecutionPolicy` (`off`|`auto`|`eager`|`sandbox`, default `sandbox`), `mcpServers`, `skills`, `plugins`, `inheritMcp` (`true`), `inheritCustomizations` (`true`).
- **Plugins (`plugins/<dir>/`):**
  - Manifest: `<plugin-root>/.antigravity-plugin/plugin.json` (takes precedence over root `plugin.json`; `.gemini-plugin/plugin.json` also recognized). Parsed as JSONC. Official schema `https://antigravity.google/schemas/v1/plugin.json` (`additionalProperties: false`): `$schema`, `name` (`^[a-z0-9]+(-[a-z0-9]+)*$`), `displayName`, `description`, `version`, `disabled` (`false`), `logo` (relative 1:1 `>=128x128` `.svg|.png|.jpg|.jpeg|.webp`), `suggestedPrompts` (max 3), `skills` (extra relative dirs), `author` (string or `{name, email, url}`), `homepage`, `repository`, `license`, `keywords`, `extensions`.
  - Multi-tool repo files safely ignored/warned by `agy plugin validate`: `.mcp.json`, `mcp.json`, root `AGENTS.md`/`GEMINI.md`, `hooks/hooks.json`, `commands/`, `workflows/`, shadowed root `plugin.json`.
  - CLI management: `agy plugin install [<name>[@antigravity-plugins-official] | <github-url> | <local-path>]`, `list [--json]`, `enable`, `disable`, `uninstall`, `validate [--strict] [--json]`, `import [gemini|claude]` (from `~/.gemini/extensions/` or `~/.claude/plugins/`). App <-> CLI auto-sync. Built-in plugins (`/builtin/plugins/`) can only be disabled, not uninstalled.
  - **Plugin Auth Providers & Multi-Account (`AGY_ACCOUNT`):** Installed plugins can declare auth providers queried via `list_plugin_accounts` (returns providers, sign-in status, identity, user labels like `"personal"`/`"work"`, and powered CLIs). When multiple accounts are connected for a provider, sandboxed CLI commands select one via prefix `AGY_ACCOUNT=<account-name>` (passing the account's name, **not** the label; omitted uses default account).
- **Sidecars (`sidecar.json` & `sidecars.json`):**
  - Locations: `~/.gemini/config/sidecars/<id>/sidecar.json` (ID `<id>`), `~/.gemini/config/plugins/<plugin>/sidecars/<id>/sidecar.json` or external dirs referenced by `<plugin>/sidecars.json` (ID `<plugin>/<id>`), plus builtin `~/.gemini/<app-name>/builtin/plugins/<plugin>/sidecars/`.
  - Enabled in `~/.gemini/config/config.json`: `"sidecars": { "<id>": { "enabled": true, "projectId": "..." } }`.
  - Schema: `command` XOR `builtin` (`"schedule"`), `args[]`, `restart_policy` (`always` [default, required for `schedule`] | `on-failure` | `never`), `description`, `env`, `display_name`, `arguments[]` (`key`, `description`, `string_arg: {default_value?}` | `workspace_arg: {}`), `agent_permissions: { workspace_uris[], access_grants[] }`. UI panels add `has_web_ui: true` and `ui_config.views[]` with `entrypoint: "SIDECAR_UI_ENTRYPOINT_AUX_PANE"`, `path`, `title` (opened via `[Label](sidecar://<sidecar-id>/<view-path>)` pill link).
  - Runtime data: `~/.gemini/antigravity/sidecar_data/<sidecarId>/{data, logs, events}`; `ANTIGRAVITY_EXECUTABLE_DATA_DIR` points to `data/`; `agentapi` on `PATH` supports `agentapi new-conversation <prompt>` (needs `projectId`) and `agentapi send-message <conversation_id> <prompt>`.

---

## 5. Multi-Agent Orchestration

### 5.1 Subagents

- **Tools:** `invoke_subagent` (`TypeName`, `Role`, `Prompt`, `Model`, `Workspace`), `define_subagent` (`Name`, `Description`, `SystemPrompt`, `EnableMCPTools`, `EnableWriteTools`, `EnableSubagentTools`), `send_message` (`Recipient`, `Message`), `manage_subagents` (`list`, `kill`, `kill_all`).
- **Built-in types:** `research` (read-only sandbox, no network), `browser`, `self` (clone of parent), `image-generator`.
- **Workspace modes:** `inherit` (parent workspace), `branch` (isolated Git worktree/clone), `share` (new working copy sharing underlying repo storage).
- **Model overrides:** `inherit`, `flash_lite`, `flash`, `pro`.
- **Lifecycle:** `Running` -> `Idle` (retains context, auto-wakes on `send_message` or direct user message) -> `Killed` (permanent, deletes temporary worktrees, keeps JSONL transcript).
- **Limits & Security:** Max nesting depth **10 levels**. Inherits parent's command prefixes, file scopes, and sandbox; parent retains access to subagent worktrees; permission requests bubble up (`Alt+J` teleport / `Ctrl+K` fast-path approve in CLI; `@<type>:<role> <msg>` direct messaging in CLI).

### 5.2 Swarms (`/swarm`)

- **Availability:** All plans (preview; enterprise subject to admin settings). App & CLI.
- **Architecture:** Current conversation becomes `coordinator` (delegates only, no technical work). Spawns named workers (`@name`, pools `<name>-1..k`), typically one `@verifier` per 2–4 workers, and `@monitor` (`swarm_monitor` skill). All team coordination happens publicly on one shared message board via `@name`, `@group`, `@all` (or channels: `main` + private member channels).
- **`swarm` Tool Actions:**
  - `add`: spawns worker(s) (`name` lowercase `[a-z0-9-_]`, max 24 chars, not `all`/`coordinator`/`coordinator-*`; optional `n` homogeneous copies or `tasks[]` sharded slices; `brief` posted publicly; `fork: "<member>|coordinator"` optional history fork; `model`; `workspace: inherit|share|branch`). Workers can also spawn helpers.
  - `status`: lists members, model, state (`working`, `waiting` [0 tokens until mentioned], `stopped` [revived by post], `killed` [permanent]).
  - `models`: lists available models (`all: true` for full list).
  - `model`: switches a `waiting` worker (or self) to another model **keeping conversation history** (used on quota exhaustion or user request).
  - `kill`: permanently removes worker and deletes its `share`/`branch` workspace (land changes first).
  - `title`: sets swarm UI title (coordinator only).
- **Monitor (`@monitor`):** Uses `read_board{since, status}`, `swarm_wait{since, mentions_only}`, and daemon cron `schedule{CronExpression: "*/3 * * * *", IsDaemon: true}`. Detects stalls when all live members except monitor are `waiting`/`stopped` and work isn't done; posts single targeted `@member [stall]` citing `#seq` (max 3 alerts per stall, spaced >= 3 ticks); goes dormant on `[done]`, no workers left, or 5 idle ticks (~15 min).
- **Quota Behavior:** Out-of-quota member pauses once (no retry loop, does not auto-resume on refresh); unstuck by switching its model, a new board post, or a direct message.
- **Monitoring UI:** App **Swarm** side pane (**Board** with `New` divider/Search/Filters/Follow, **Members** grouped Active/Standing by/Retired, **Graph** with `@mention` weights & replay); CLI status row (`Alt+X` dismiss) + full-screen board (`Alt+S` or `/swarm board`).

### 5.3 Teamwork (`/teamwork`), Boost (`/boost`) & Orchestrator Model

- **Teamwork (`/teamwork`, Paid plans only):**
  - Phase 1 scoping interview ("specify what, not how") -> reviewable **prompt artifact**. Phase 2 autonomous execution in dedicated `~/teamwork_projects/{PROJECT_NAME}` with exclusive file ownership, per-agent scratch dirs, and structured Request / Project Plan / Progress artifacts.
  - Roles: `Sentinel`, `Project Orchestrator` (hands off to fresh successor between milestones), `Explorers` (read-only), `Workers` (implementation); Adversarial gates: `Critic`, `Challenger`, `Auditor`, `Success Auditor`.
  - Execution paths: General (Distributed coding), Iterative coding, Document Review, Math / Proof, Math / Proof (Large Team).
  - Integrity modes: `development` (default, lenient), `demo` (no OSS core copy / external delegation / reading test source), `benchmark` (from-scratch stdlib only, strict real-execution verification).
- **Boost (`/boost`, Paid plans only):**
  - 3-phase pipeline: Orchestrator routes to `DeepCoder` (implementation) or `DeepInvestigator` (root-cause) -> isolated initial worker + adversarial improvement workers trying to break/fix prior attempts -> Orchestrator independent spot-check & re-delegation. Runs in current workspace across isolated contexts.
- **Orchestrator Model (`gemini-pro-orchestrator`):**
  - Single-conversation planner+executor handoff (Pro/Ultra: planner Neon/Carbon/Argon/Claude Opus 5.5 + executor Hydrogen/Claude Sonnet 5.5; Enterprise/ADC: fixed Carbon High + Hydrogen High). `--effort`/`/effort` disabled for Orchestrator in CLI.

### 5.4 Dynamic Workflows (`/run-workflow` / `run_workflow` tool)

- **Availability:** All plans (App & CLI).
- **Runtime:** Separate Python 3.8+ process (bundled Python unpacked on first run; override via `ANTIGRAVITY_PYTHON`; fallback to `PATH`/Homebrew/python.org/`py` launcher). Tool args: `Script` XOR `ScriptPath`, `Args` (validated against `workflow.json` if saved workflow), `Cwd`, `Description`, `WaitMsBeforeAsync` (0–10000 ms).
- **Python Primitives:**
  - `phase(name)` — groups logs, tool calls, and agent cards under a named phase.
  - `log(*args)` — writes to phase logs & main agent tool result (`Logs:`).
  - `agent(prompt=None, schema=None, role=None, type_name="self", workspace="inherit", model=None)` — lazy definition; `await agent(...)` single-use; `a = agent(...); await a(prompt, schema)` multi-turn persistent conversation (`del a` ends it). Requires JSON `schema`; agent returns parsed value via `finish` tool (ending without `finish` fails workflow).
  - `parallel(items, fn)` — concurrent execution in item order; **fail-fast** (one failure cancels siblings).
  - `pipeline(items, fn)` — sequential execution in item order.
  - `await trajectory(last=None, types=None, tools=None, min_index=None, max_index=None)` — reads steps prior to workflow start (`Step` dict+attr: `index`, `type`, `status`, `text`, `tool`, `args`, `output` capped at 200k chars; evicted steps have `status == "CLEARED"`).
  - `await call_tool(name, **args)` — direct tool invocation returning `str`; raises `ToolError(.tool, .message, .denied)`; cannot prompt user (ungranted tools auto-deny).
  - Constants: `args`, `CONVERSATION_ID`, `STEP_INDEX`, `WORKSPACES` (tuple, primary first).
- **Permissions:** Prompts approval for the exact script (command prefix rules do not cover Python scripts); child workflow agents ask in their own conversations (hidden from main conversation list, opened from agent cards / CLI `/tasks` or `Ctrl+O`). Stopping the parent stops all workflow agents.

### 5.5 Scheduled Tasks (`/schedule`) & Automations (`/automation`)

- **`/schedule` (`schedule` tool — App, CLI, IDE):** Conversation-scoped background task (non-blocking, wakes agent with high-priority prompt):
  - One-shot timer: `DurationSeconds`, `Prompt`, `TimerCondition` (`"never"` | `"any"` | `<senderId>` early cancel).
  - Recurring cron: 5-field `CronExpression`, `Prompt` (suffixed with iteration #), `MaxIterations`, `IsDaemon` (`true` keeps firing after task ends), `ExactTiming` (`false` default jitter).
  - Managed via `manage_task` (`list`/`status`/`kill`/`send_input`) or CLI `/tasks`.
- **`/automation` (App only):** Guided Q&A creating persistent `~/.gemini/config/sidecars/<id>/sidecar.json` (`builtin: "schedule"`, `CRON_TZ=<IANA zone>`, `agentapi new-conversation`). Optional test run records scoped `agent_permissions` (unattended runs **auto-deny** unapproved operations). Starts **Paused** in Automations tab; Flash default (selectable Flash Lite / Flash / Pro); jitter up to 10% of interval (max 15 min); supports **Run Now**.

---

## 6. App & Remote Features Relevant to a Wrapper App

### 6.1 Remote Control (First-Party Architecture vs. OpenAgents Tailnet)

- **Relay / Dashboard Architecture:** Host (App or CLI) opens an outbound **reverse tunnel** (WebRTC/WebChannel) to Google's relay; remote clients connect via web browser at `https://antigravity.google.com` (PWA installable on mobile for push notifications) signed into the **same Google Account**, switching hosts via the instance switcher.
- **Antigravity App:** Settings > Application > **Enable Remote Control** + **Device Name** (`remoteControlHostname` in `~/.gemini/config/config.json`). Auto-reconnects on network drops; running agents continue uninterrupted.
- **CLI Interactive Mode:**
  - Enable mid-session with `/remote-control` (or `/remote-control on` / `off`) or at launch with `agy --remote-control`.
  - Allocates dynamic free local port, generates per-session hostname (e.g., `devbox-swift-falcon`), prints direct conversation handoff URL: `https://antigravity.google.com/r/<instance-id>?p=c%2F<conversation-id>`.
  - Lifecycle strictly tied to `agy` process (tears down on `/exit`, `Ctrl+C`/`Ctrl+D`, or `/remote-control off`). Supports multiple concurrent terminal sessions per host.
  - **Bidirectional live sync:** prompts, streaming thoughts, tool executions, diffs, tool approvals, interactive questions, and browser Settings modal (syncs back to `settings.json`).
- **CLI Headless Background Daemon:**
  - Commands: `agy remote-control start [--name "<label>"] [--session]`, `agy remote-control status`, `agy remote-control stop` (runs `agy remote-control serve` using saved `settings.json`).
  - Registers OS service: **Linux** `systemd --user` unit `antigravity-cli-daemon` (starts at boot, persists after logout; logs `journalctl --user -u antigravity-cli-daemon -n 50`); **macOS** LaunchAgent (starts at login, stops on logout; log `~/Library/Logs/antigravity-cli-daemon.log`); **Windows** Scheduled Task (starts at boot if Admin, else at login; up to 3 crash restarts; logs `%USERPROFILE%\.gemini\antigravity-cli\log\cli-*.log`).
  - Machine name stored in `~/.gemini/config/config.json` as `cliRemoteControlHostname` (separate from editor's `remoteControlHostname`); reload requires re-running `agy remote-control start`. Uses CLI sign-in credentials (independent of App).
- **Incompatibility:** `--remote-control` exits with code **`2`** when authenticating with `GEMINI_API_KEY`; disabled if org admin policy blocks remote control (`Remote control is not enabled for your account.`).

### 6.2 Remote Agents (Cloud Execution)

- **Availability:** Personal **Google AI Pro** & **Ultra** only (no Free, Plus, Workspace/Cloud business, or `GEMINI_API_KEY` [exits code `2`]).
- **Invocation:** App (`+ New Remote Project` or `Remote > GitHub Repositories`), Web (`antigravity.google.com`), CLI (`v1.4.12+`: `agy --remote-agent [--project=<name|id>] [--new-project] [-c] [--conversation=<id>]`, default project `CLI Project (<hostname>)`). Local and remote conversation histories are strictly segregated.
- **Resources** (instead of local folders): GitHub repos via OAuth (private repos in one project must belong to the same owner), Google Drive files/folders, Uploaded files staged at `/home/antigravity/upload` (limits: **8 MiB/file**, **50 files & 100 MiB/project**, **2 GiB/account**).
- **Plugins:** Cloud OAuth plugins (GitHub, Drive, Docs, Sheets, Slides, Calendar); tokens encrypted at rest & injected remotely.
- **CLI Limitations in Remote Agent Mode:** Default preset is `always-proceed (turbo)`; only `default` execution mode (no `plan`, `accept-edits`, `auto`); unavailable: `--print`, `/codesearch`, `/diff`, `/fork`, `/rewind`, `/btw`, `/voice`, `!`, `@subagent`, local MCP/plugins/skills.

### 6.3 SSH (`Connect over SSH`, App `v2.33.0+`)

- Dedicated `(SSH: <host>)` window launched via Sidebar `+ > SSH Host…`, Command Palette **Connect to SSH Host…**, or `--ssh-host=<dest>` (`~/.ssh/config`, custom `-F` config, keys, `ssh-agent`, `ProxyJump`; uses `ssh`/`ssh.exe` or `ANTIGRAVITY_SSH_BIN`).
- **Host Requirements:** 64-bit Linux (`x86_64`/`arm64`), `glibc >= 2.26` (no macOS, Windows, or `musl`/Alpine), `AllowTcpForwarding yes|local`, outbound HTTPS, writable+executable `$HOME` (`~/.antigravity-hub-server/` for server + logs `data/ls-*.log`, `~/.gemini/` for data/customizations).
- **SSH Overrides Forced by Client:** `ForwardAgent=no`, `ForwardX11=no`, `ControlMaster=no`, `ControlPath=none`, `RemoteCommand=none`, `ServerAliveInterval=15`, `ServerAliveCountMax=3`.
- **Env & Lifecycle:** Resolves shell env via `$SHELL -l -i` (10s timeout, sets `ANTIGRAVITY_RESOLVING_ENVIRONMENT=1`). Closing the window disconnects SSH while keeping the remote server & agents alive; **Stop SSH Server…** shuts down host server/agents/sidecars.
- **Error Codes (12):** `INVALID_DESTINATION`, `SSH_CONFIG_INVALID`, `AUTH_FAILED`, `HOSTKEY_REJECTED`, `CONNECTION_FAILED`, `CONNECTION_CLOSED`, `FORWARDING_PROHIBITED`, `UNSUPPORTED_HOST`, `SHARED_HOME_UNSUPPORTED`, `INSTALL_FAILED`, `LS_START_FAILED`, `LS_STOPPED`.
- **Feature Delta:** Built-in side-panel browser hidden (links open in local default browser); `/browser` targets remote Chrome; voice prompts per-session mic permission (no camera/screen); local IDE/Finder actions hidden; OAuth & sidecar panels relay over SSH.

### 6.4 Voice Mode, Dictation & `agy mic-serve`

- **Voice Mode (Gemini Live two-way speech):** App empty-composer round button; CLI `v1.5.0+` `/voice live` or `F6` (end: `F7`, `/voice stop`, or `Esc` when idle; mute: App `Opt/Alt+M`, mute agent `Shift+Opt/Alt+M`, CLI `F6`/`Shift+F6`/`/voice mute`). 9 selectable voices (e.g. Fola, Arlo), speed (App `0.75x–2x`, CLI `0.5–3x`), volume (`0–100`). Quiet by default; spoken progress updates on request (~30s interval). App shows live panel + floating dock across conversations (max 1 active call) + local saved **Voice transcript** card. CLI prints turns into transcript; half-duplex default (`/voice duplex on` for barge-in).
- **Voice Dictation:** App mic button / `Ctrl+M` (works in composer & artifact comments; context-aware); CLI `/voice dictate`, `/record`, or `F5` (`Enter`/`F5` insert, `Esc` discard, `Ctrl+_` single undo; appends to prompt, never auto-submits). Requires fresh OAuth scope (`/logout` + re-login). Unavailable on business/enterprise accounts & `--print`.
- **SSH Loopback Bridge (`agy mic-serve`):**
  1. Local machine: `agy mic-serve [--addr 127.0.0.1:<port>]` (default **`127.0.0.1:4713`**; MUST stay on loopback; streams raw uncompressed PCM ~32 kB/s from default input AND plays back Live agent voice locally).
  2. Reverse tunnel: `ssh -f -N -R 24713:127.0.0.1:4713 <remote-machine>`.
  3. Remote CLI: `ANTIGRAVITY_MIC=127.0.0.1:24713 agy`.
- **Restrictions:** Voice mode works only in local conversations (no Remote Control, Remote Agents, subagents, `/btw`, CLI `--print`, or `GEMINI_API_KEY`). Destructive actions cannot be approved by voice.

### 6.5 Browser Use

- **Engine 1 — Built-in Side-Panel Browser (App `v2.30.0+`, personal accounts):** Zero Chrome/Node setup; auto-invoked from plain language or `/browser`; parallel isolated tabs per subagent; animated live cursor; zero focus theft; foreground tab protection; red **Stop** takeover; isolated persistent cookie profile per Antigravity account with pause-and-resume SSO/MFA/CAPTCHA handoff; structured actions only (no arbitrary JS eval); scrubbed network/console logs; manual toolbar (`Back`/`Forward`/`Reload`, TLS/cookie clear, Downloads, **Add screenshot to conversation**, WebUSB/HID/Serial, right-click `Comment` `C` / `Quote` `Cmd/Ctrl+L`).
- **Engine 2 — Chrome DevTools MCP (`/browser` in CLI, IDE `v2.6.0+`, VS Code `v1.9.0+`, App business/enterprise):** Drives running local Chrome `v144+` with `chrome://inspect/#remote-debugging` enabled (60s Chrome **Allow** dialog; `npx` / Node.js in CLI/IDE, bundled runtime in App; single connection). Adds DevTools performance traces, Lighthouse, heap snapshots, CPU/network throttling, `evaluate_script` (governed by **Browser Javascript Execution Policy**: `Request Review` [default], `Always Proceed`, `Disabled`), and optional interaction recordings (**MP4** in CLI `v1.5.9+`; **WebM** `recording.webm` in IDE & earlier CLI).
- **Subagent Model:** Defaults to Gemini Flash; stops & asks on quota exhaustion rather than silently switching models.
- **Guardrails:** `execute_url` / `download_url` in **Browser Actuation Rules**; per-port `localhost` isolation; sensitive Google subdomains require explicit rules; every cross-origin redirect checked; uploads always require one-time approval even in Turbo (staged files only: <= 20 files, <= 50 MiB each, <= 100 MiB total); `data:`/`blob:` downloads always ask once; blocked executable/active-markup extensions; IdP/payment pages blocked & cross-origin/payment/sign-in iframes masked from screenshots/snapshots; no fullscreen, pointer/keyboard lock, or private-LAN/metadata redirects.

### 6.6 Artifacts, Review Changes, Generative UI, Notebooks & Side Panel

- **Artifacts:**
  - **Implementation Plan:** Triggered by `/plan` or Planning mode; **Proceed** button; inline text comments batched as `N Comments` chip on next message; `/plan` always pauses for approval regardless of Plan Review Policy (`Always Ask` vs `Always Proceed`; CLI `artifactReviewPolicy`: `asks-for-review` | `agent-decides` | `always-proceed`).
  - **Walkthrough:** Post-implementation summary + validation outputs + embedded screenshots/recordings.
  - **Screenshots:** Saved to artifacts on request -> region drag-select commenting (held until next message or **Review** header button).
  - **Generative UI (`/generative_ui`, App only):** Single-file HTML artifact; standalone side panel (default) or compact inline widget (<= `500 px` height). Sandboxed iframe; matches host light/dark theme; Tailwind CSS script only (no external scripts, file access, or embeds); point-in-time snapshot; **Export Artifact** (strips app theme vars); non-conversational interactive controls.
  - **Task Groups (IDE):** Overview, edited file pills, collapsible subtask updates, dedicated pending-steps section.
  - **Learning Proposal:** `learning_proposal.md` generated by `/learn` requesting feedback before editing rules/skills.
- **Review Changes:**
  - **App Review Pane:** Views: **Uncommitted** (Staged Changes / Changes; stage `+`, unstage `-`, immediate no-confirm discard/delete untracked), **Branch** (vs base branch), **Agent Edits**, **For Turn** (single-turn filter). Status letters: `M`, `A`, `U`, `D`, `R`. Split/Stacked diff, hide whitespace, expand `+N`/`+10` lines, search, inline line/multi-line comments (`1 Comment` chip) & `Quote` (`Cmd/Ctrl+L` -> `file #L13-17`). Git actions: **Commit** (empty msg auto-generates), **Amend**, **Push** (no force-push).
  - **CLI `/diff`:** Full-screen views (`←`/`→`): **All Changes**, **Per Turn**, **Commit Tree**; press `C` for line comments (prompts **Unsent Comments** `Y`/`N` on exit).
- **Side Panel, Overview, Layouts & Viewers:**
  - **Overview panel** (`Cmd+Opt+O` / `Ctrl+Alt+O`): rows (**Review**, **Files**, **Terminal**, **Plugins**) + customizable sections (**Subagents**, **Background Tasks**, **Goals**, **Documents**, **Artifacts**, **Links**, **Modified Files**, **Uploads**, **Side Questions**).
  - **Side panel** (`Cmd/Ctrl+Shift+B`): preview tabs (italic, double-click to pin), `Cmd/Ctrl+T` New Tab search/URL, `Cmd/Ctrl+Shift+T` reopen, `Cmd/Ctrl+Shift+M` maximize, **Split View** (`Cmd/Ctrl+D` right, `Cmd/Ctrl+Shift+D` down).
  - **Interface Layouts (App `v2.30.0+`, personal accounts):** **Software Engineering** vs **Knowledge Work** (hides Git/worktrees/Review/Terminal/diffs/advanced rules; same underlying agent/tools).
  - **PDF Viewer** (thumbnails, zoom, search, Comment `C`, Quote `Cmd/Ctrl+L`) & **Jupyter Notebooks (`.ipynb`)** (`nbformat` v3/v4, Preview/Raw toggle, cell inline comments, cell outputs stripped from turn diffs).

### 6.7 Projects, Worktrees, Mentions (`@`), Search & Slash Commands

- **Projects & Worktrees:** Multi-folder local projects with isolated settings/permissions; new conversations choose **Local mode** vs **New worktree mode** (isolated Git worktree per Git checkout + existing non-Git folders). CLI uses `default-cli-project`, `--project=<id>`, `--new-project`, `/fork <project_id>`.
- **Mentions (`@`) & Quoting:**
  - App: `@` fuzzy files/folders/artifacts/plugins (passes full path), `@rules:` (inlines full rule text, triggers `manual` rules), `@conversation:` (full-text search; passes title+ID so agent reads saved transcript), **Quote** (`Cmd/Ctrl+L` or `I`) on responses (full text), files/diffs (path + line range), artifacts (selected text), terminal (up to last 8 KB), PDF, browser pages.
  - CLI: `@` workspace or external (`~/` or `/`) paths (doesn't grant permission); leading `@<type>:<role> <msg>` messages a subagent directly.
- **Search & Palette:** App `Cmd/Ctrl+K` (Conversations full-text over title, project, first folder, user prompts, agent responses — excludes thinking, artifacts, command outputs, read files; sort Last Updated/Last Prompt; filter Unread/Archived/Automations), `Cmd/Ctrl+P` (Files), `Cmd/Ctrl+Shift+K` (Projects), `Cmd/Ctrl+Shift+P` (Commands), `Cmd/Ctrl+F` (Find). CLI `/resume` (title/first line/ID + **Other** tab to import App conversations), `/codesearch`.
- **Key Slash Commands:**
  - Planning: `/plan`, `/grill-me` (1 question at a time, `(Recommended)` first, explores code first; stackable `/plan /grill-me`).
  - Execution/Delegation: `/goal` (goal stop hook blocks premature stops with elapsed time + continuation count + evidence checklist; 1 active goal per conversation; waits for subagents/background tasks), `/boost`, `/swarm`, `/teamwork`, `/run-workflow`, `/browser`, `/automation`, `/schedule`.
  - Side Questions & Context:
    - `/btw`: forks conversation read-only (instruction-level, same permissions/sandbox); runs alongside main agent without polluting history. App `v2.32.0+` opens **Side Question** tab (supports follow-ups, tool approvals, rename, **Promote to Conversation**, delete); CLI `v1.5.0+` opens `/btw` card (one at a time, no follow-ups, auto-denies tools requiring approval, `/copy btw`, bare `/btw` reopens last).
    - `/compact [msg]`: leading-only; bare compacts & stops, with `<msg>` guides summary & continues in same turn. Replaces older steps with structured continuation summary + deterministic snapshots (attachments, background tasks, live subagents, artifacts). Works in headless `-p` unless `--disable-slash-commands`.
  - Customization & Help: `/learn` (analyzes corrections -> `learning_proposal.md` -> applies on approval), `/generative_ui`, `/mcp-app-development`, `/agy-customizations`, `/antigravity-guide`, `/plugin`.
  - CLI Session/Config: `/diff`, `/context`, `/resume`, `/agents`, `/tasks`, `/codesearch`, `/voice`, `/permissions`, `/effort`, `/model`, `/config`, `/statusline`, `/title`, `/usage`, `/credits`, `/changelog`, `/rewind`, `/fork`, `/clear`, `/logout`, `/keybindings`.

---

## 7. Enterprise, Workspace, Plans, Quotas, Credits & Wrapper Compliance

### 7.1 Tiers, Model Access & Billing Models

| Account / Tier | Terms & Telemetry | Rate Limits & Overages | Key Model / Feature Access |
|---|---|---|---|
| **Personal — Free** | Antigravity ToS (18+, geo list); telemetry opt-out | Weekly quota & refresh; no AI credits | `Gemini 3.8 Flash` (+ `Claude 4.6` & `GPT-OSS-120b` retiring Nov 30, 2026); `/swarm`, `/run-workflow`, built-in browser; NO `/boost`, `/teamwork`, Remote Agents |
| **Personal — Google AI Plus** | Antigravity ToS | Higher weekly limits | Same model tier as Free; NO Remote Agents |
| **Personal — Google AI Pro** | Antigravity ToS | High 5-hour refresh up to weekly limit + optional **AI Credit Overages** (`Never`/`Always`; CLI `useG1Credits`) | Adds `Gemini 4 Neon` (512K ctx, `low`/`medium`/`high`/`xhigh`), `Gemini 4 Argon`, `Gemini 4 Hydrogen` (non-trial), customizable `Orchestrator`, `Claude Sonnet/Opus 5.5` (non-trial), `/boost`, `/teamwork`, Remote Agents |
| **Personal — Google AI Ultra** | Antigravity ToS | Highest 5-hour & weekly limits + AI Credit Overages | Adds `Gemini 4 Carbon` + all Pro features & 3P models |
| **Google Workspace (without Cloud/Enterprise)** | Workspace plans do **NOT** include Antigravity plans, higher limits, or AI credits | Limited **Antigravity Starter Quota** (or blocked/ineligible if Workspace admin disabled individual access) | Upgrade via Personal Account, GCP Pay As You Go, or Gemini Enterprise |
| **GCP Pay As You Go** *(App `v2.31.0+`, CLI `v1.4.11+`)* | Google Cloud ToS; telemetry always off | Per-token Agent Platform API pricing; no included quotas | Needs `aiplatform.googleapis.com` + Cloud Billing + IAM `roles/aiplatform.user` (`aiplatform.endpoints.predict`). Claude models require Model Garden enablement |
| **Gemini Enterprise — Standard / Plus** | Google Cloud ToS; telemetry -> GCP project (0 training) | Pooled seat credits (**\$10/mo Standard**, **\$15/mo Plus**) + managed overages; 3P Claude billed per token | 1P Gemini (`4 Neon`, `4 Carbon`, `4 Argon`, `4 Hydrogen`, fixed `Orchestrator` Carbon+Hydrogen High, `3.8/3.7/3.6 Flash`, `3.1 Pro`) + admin-opted-in `Claude 5.5` (`low`/`medium`/`high`/`max`). **Hybrid execution:** Claude primary still uses 1P Gemini background models (incurs 1P token cost). App, CLI, IDE extensions (**NOT standalone Antigravity IDE**) |
| **Gemini Enterprise — Pay-as-you-go edition** | Google Cloud ToS | No seat license; invoiced or prepaid-credit Cloud Billing; selected under **Subscription** | Requires IAM `roles/discoveryengine.agentspaceUser` (admins: `roles/discoveryengine.agentspaceAdmin`) + region **AI developer tools** toggle (`discoveryengine.googleapis.com`, `businessaicode.googleapis.com`) + `aiplatform.googleapis.com` enabled **before** purchasing licenses (wait ~5 min) |
| **CLI `GEMINI_API_KEY` (`modelProvider: "gemini"`)** | Gemini API pricing (AI Studio) | API billing; supports `customModelsConfig` | Disables Remote Control, `--remote-agent`, and `/voice` (exit code `2`) |

### 7.2 Enterprise Endpoints, Data Residency, VPC-SC, PSC & Auth

- **Locations:** `global` (text, code, multimodal, **image gen**), `us` (multi-region, no image gen), `eu` (multi-region, no image gen). Third-party inference runs only in `global`/`us`/`eu` (no in-country regions).
- **Firewall / Proxy Allowlist (TCP 443):**
  - AI Dev Tools Orchestration: `businessaicode.googleapis.com`, `businessaicode.us.rep.googleapis.com`, `businessaicode.eu.rep.googleapis.com`, mTLS `businessaicode.mtls.googleapis.com`
  - Vertex AI / Gemini Enterprise: `aiplatform.googleapis.com`, `aiplatform.us.rep.googleapis.com`, `aiplatform.eu.rep.googleapis.com`, mTLS `aiplatform.mtls.googleapis.com`
  - Discovery Engine: `discoveryengine.googleapis.com`, `{us,eu}-discoveryengine.googleapis.com`, mTLS `{,us-,eu-}discoveryengine.mtls.googleapis.com`
  - Cloud Code Telemetry/Services: `cloudcode-pa.googleapis.com`
  - **Mandatory Global Reachability:** Even in `us`/`eu`, sign-in requires global `businessaicode.googleapis.com` + `cloudcode-pa.googleapis.com` for license discovery & model catalog.
- **VPC-SC & PSC:**
  - Add `businessaicode.googleapis.com` + `aiplatform.googleapis.com` to the VPC-SC perimeter.
  - **PSC `x509` Pitfall:** Global PGA VIPs (`private.googleapis.com` / `restricted.googleapis.com`) & global PSC serve a single-level `*.googleapis.com` cert that fails on `*.<loc>.rep.googleapis.com`. Configure **regional PSC endpoints** + private Cloud DNS `A` records for regional hostnames while routing global hostnames through global PSC/PGA.
- **Identity Methods:**
  - **Google Cloud SSO** (saves `gcp.{project, location}` in CLI `settings.json`).
  - **BYOID / WIF:** Advanced SSO config string `locations/global/workforcePools/POOL_ID/providers/PROVIDER_ID` (e.g., Okta).
  - **ADC (Headless):** `gcloud auth application-default login --project {GCP_PROJECT}` + `AGY_ADC_AUTH=true` (CLI/App/IDE) or `~/.gemini/config/config.json` -> `userSettings.enableAdc: true`. Project resolution: ADC `quota_project_id` > `GOOGLE_CLOUD_QUOTA_PROJECT` (CLI) > metadata server. Location: `GOOGLE_CLOUD_LOCATION` (CLI) / ADC `location` (App/IDE) > `global`. Models older than Gemini 3 Flash unsupported; no image gen in `us`/`eu`.
  - OAuth/WIF tokens refresh proactively 5 min before expiry; stored in OS keyring (macOS Keychain `Antigravity CLI`, Linux D-Bus secret-service, Windows Credential Manager).
- **Cloud Audit & Diagnostics:** Request/response logging & telemetry log to the selected GCP project (1 license per project+location). Logs: CLI `~/.gemini/antigravity-cli/cli.log`; App macOS `~/Library/Logs/Antigravity/language_server.log`.
- **Managed Controls (`pluginControls` & Cloud Console):**
  - Non-Enterprise `admin_settings.json` supports `pluginControls`: `pluginsDisabled` (blocks all plugins incl. built-in, stops running plugin servers immediately on change) and `marketplaceDisabled` (disables marketplace, keeps installed plugins).
  - Enterprise Cloud Console controls: 3P model opt-in, browser disable / `Organization Allowed Domains (Read-Only)`, `/swarm` & UI plugin allow, remote control disable, permission ceilings.

### 7.3 Quotas, Error States & Critical Wrapper Compliance Constraints

- **Quota Model & Overages:** Models grouped by family (Gemini Flash, Gemini Pro, Claude & GPT) share 5-hour + weekly limits; consumption scales with agent work per prompt. AI Credit Overages (`Never` / `Always`, CLI `useG1Credits`) kick in on baseline exhaustion and revert automatically on refresh. CLI tracks via `/usage` (`/quota`), `/credits`, and statusline `AI Credits: N`.
- **Error / Quota / Lifecycle States a Client Must Handle:**
  - **Transient vs. Hard Rate Limits:** Transient per-minute/capacity 429 (`Retry-After`) waits & retries up to **4 minutes**; **spend-cap, prepaid-balance, individual/weekly quota exhaustion, and concurrent stream limits** stop immediately.
  - **Swarm & Browser Quota Edge Cases:** Exhausted Swarm members pause once and stay paused until model switch, board post, or DM (no auto-restart on quota refresh). Exhausted Browser Flash subagents stop & prompt the user.
  - **Auth & Environment Errors:** `Authentication expired` (re-login), missing OAuth scopes (e.g., voice dictation requires `/logout` + re-login), locked OS keyring (`secret keyring is locked`), `aiplatform.endpoints.predict` IAM denial, model unsupported in region, admin-blocked model/feature, corporate proxy HTML error pages (title extracted; non-JSON truncated at 500 bytes), stalled model connection auto-cancel/retry, self-updater advisory lock (`~/.gemini/antigravity-cli/updater/update.lock`, opt-out `AGY_CLI_DISABLE_AUTO_UPDATE=true`).
  - **Headless CLI (`-p`) Exit Codes & Envelopes:**
    - Exit `0`: `SUCCESS`. Exit `1`: general/validation error (e.g., unknown `--model`). Exit `2`: feature unavailable under `GEMINI_API_KEY` (`--remote-control`, `--remote-agent`). Exit `3`: agent/model API failure or `--print-timeout` exceeded — prints structured **`AGY_ERROR: {...}`** JSON on `stderr` (canonical status, HTTP/gRPC code, retryability, error ID) + `[agy] print timeout after ...` on timeout.
    - Terminal `status` enum in `json`/`stream-json`: `SUCCESS`, `ERROR`, `CANCELED`, `INTERRUPTED`, `INVALID`, `WAITING`, `RUNNING`.

> [!IMPORTANT]
> **Terms of Service / Wrapper Compliance Distinction (`antigravity.google/docs/faq`, lines 346–352, plus Google's T3 Code clarification):**
> - **Permitted (`Client App → official unmodified agy acp binary → Google API`):** Interactive single-user apps (such as T3 Code and OpenAgents) driving the official unmodified `agy acp` binary over ACP JSON-RPC and authenticating through `agy`'s own native login flow (`agy acp --login` / ACP `authenticate`, including `oauth-personal`) are permitted by Google.
> - **Prohibited / Triggers Automated Account Bans (`Fake Antigravity shim / extracted OAuth token → cloudcode-pa.googleapis.com`):** Third-party harnesses that extract or replay Antigravity OAuth Bearer tokens to call `cloudcode-pa.googleapis.com` / `daily-cloudcode-pa.googleapis.com` directly outside the official `agy` binary (e.g., `pi-antigravity`, OpenCode OAuth shims, OpenClaw) violate Antigravity Terms of Service and trigger automated `TOS_VIOLATION` account bans.
> - **Headless `-p` & Automation Restriction (`faq.md` L350–352):** Wrapping `agy -p` headless mode inside third-party coding agents or applications, or automating multi-account rotation, is explicitly NOT supported under consumer plans (use **Gemini Enterprise**, **Google Cloud Vertex AI / Agent Platform PAYG**, a **Google AI Studio API key**, or the official **`google-antigravity` Python SDK** for headless/API-key third-party agents).

### 7.4 Ecosystem: Build with Google Bundles

10 curated Marketplace bundles (skills + MCP):
- **Web & Chrome:** Modern Web Guidance, Chrome DevTools
- **Cloud & Data:** Firebase Bundle, Data Agent Kit Starter Pack, Google Cloud Developer
- **Mobile & Maps:** Android CLI, Dart and Flutter, Google Maps Platform
- **AI & Science:** Google Antigravity SDK, Science
- Plus the **Google Developer Knowledge MCP server** (`search_documents`, `get_documents`, `answer_query`, ADC auth, paired with `retrieving-developer-knowledge` skill).
