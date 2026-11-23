# Antigravity ACP Protocol, Endpoints, Models & Auth Reference

Ground-truth reference for the Antigravity Agent Client Protocol (`agy acp`), all Antigravity cloud/local network endpoints, model catalogs, dynamic reasoning controls, execution modes, authentication methods, and billing/quota tiers.

Derived from:
1. Live read-only JSON-RPC probes (`initialize` and `session/list`) against `/Users/christopherdavid/.local/bin/agy` (`v1.5.14`),
2. Read-only Go symbol extraction of `google3/third_party/jetski/cli/acp/{schema,server,router,sessionconfig}` inside `agy v1.5.14`, and
3. Complete official docs (`ide/extensions/acp.md`, `cli/acp.md`, `models.md`, `install.md`, `enterprise.md`, `workspace.md`, `pay-as-you-go.md`, `plans.md`, `changelog.md`).

---

## 1. Live-Verified `agy acp` Handshake (`v1.5.14`)

Sending `initialize` over stdio JSON-RPC 2.0 (`agy acp --stdio`) produces the following exact response on `agy 1.5.14`:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": 1,
    "agentCapabilities": {
      "loadSession": true,
      "load_session": true,
      "promptCapabilities": {
        "image": true,
        "audio": false,
        "embeddedContext": true
      },
      "sessionCapabilities": {
        "resume": {},
        "close": {},
        "load": {},
        "list": {},
        "additionalDirectories": {}
      },
      "session_capabilities": {
        "resume": {},
        "close": {},
        "load": {},
        "list": {},
        "additionalDirectories": {}
      },
      "auth": {
        "logout": {}
      },
      "_meta": {
        "antigravity": {
          "tier": true,
          "systemPrompt": {
            "append": true
          }
        }
      }
    },
    "agentInfo": {
      "name": "Antigravity ACP Server",
      "version": "1.5.14"
    },
    "authMethods": [
      {
        "id": "oauth-personal",
        "name": "Log in with Google",
        "description": "Log in with your Google account"
      },
      {
        "id": "oauth-business",
        "name": "Log in with Gemini Enterprise",
        "description": "Log in with your Gemini Enterprise account"
      },
      {
        "id": "gemini-api-key",
        "name": "Gemini API key",
        "description": "Use an API key with Gemini Developer API"
      },
      {
        "id": "agent-platform",
        "name": "Gemini Enterprise Agent Platform",
        "description": "Use Gemini Enterprise Agent Platform (formerly Vertex AI) with Application Default Credentials"
      }
    ]
  }
}
```

### Live-Verified `session/list` Response (`v1.5.14`)
`{"jsonrpc":"2.0","id":2,"method":"session/list","params":{}}` returns all saved native Antigravity conversations (observed returning 83 live sessions directly from the host's Antigravity store without scraping `~/.gemini/` files):
```json
{
  "sessions": [
    {
      "sessionId": "3150b8a4-8100-498d-ac32-26bb5d0e6abe",
      "cwd": "/Users/christopherdavid/work/openagents",
      "title": "Access Antigravity Documentation Programmatically",
      "updatedAt": "2026-11-23T06:11:09Z"
    }
  ]
}
```

---

## 2. Complete ACP JSON-RPC 2.0 Surface (`agy acp` `v1.5.14`)

Verified against `google3/third_party/jetski/cli/acp/server.(*Server)` and `acp/router.(*Router)` handlers.

### 2.1 Client -> Server RPCs & Notifications
| Method | Server Handler | Behavior, Parameters & Version Notes |
| --- | --- | --- |
| `initialize` | `handleInitialize` | Negotiates ACP `protocolVersion: 1`, `agentInfo`, `authMethods`, `agentCapabilities`. Client may advertise `fs.{readTextFile,writeTextFile}`, `terminal`, `auth.terminal`, `auth._meta.gateway`, and `elicitation.form`. |
| `authenticate` | `handleAuthenticate` | Authenticates via `methodId` (`oauth-personal`, `oauth-business`, `gemini-api-key`, `agent-platform`, `terminal-login`, or `gateway`). Supports live credential rotation and automatic backend rebind (`v1.3.0+`). Response carries `_meta.antigravity.tier`. |
| `logout` | `handleLogout` | Signs out active session/profile (`agentCapabilities.auth.logout`). Subsequent model/mode changes fail with `-32000` (`Authentication required`). |
| `$/cancel_request` | `handleCancelRequest` | Standard JSON-RPC request cancellation. |
| `session/new` | `handleSessionNew` | Params: `cwd` (expands `~`), `mcpServers[]`, optional `additionalDirectories[]`, `_meta.antigravity.{systemPrompt.append, eagerMcpServers, mcpMaxOutputBytes}`, `_meta.agy.{enabledTools, disabledTools}`, `disableBuiltInTools`. Returns `sessionId`, `models`, `modes`, `configOptions` (`model`, `mode`, `thought_level`), and `_meta.antigravity.tier`. |
| `session/load` | `handleSessionLoad` | Reopens existing `sessionId` **with** full history replay (`replayHistory` / `sendHistoricalStepUpdates`). Preserves session mode (`v1.5.4+`), accepts `_meta.antigravity.systemPrompt.append` and tool filters, returns `_meta.antigravity.tier` & emits `usage_update`. |
| `session/resume` | `handleSessionLoad` / resume path | Reopens existing `sessionId` **without** history replay (`v1.5.0+`). Preserves session mode, strips stale thought signatures, accepts `_meta.antigravity.systemPrompt.append`, returns `_meta.antigravity.tier`. |
| `session/list` | `handleSessionList` | Lists saved Antigravity conversations (`sessionId`, `cwd`, `title`, `updatedAt`). |
| `session/close` | `handleSessionClose` | Closes session, cancels in-flight prompts (ending as `cancelled`), and kills running subagents & background tasks (`v1.5.10+`). |
| `session/prompt` | `handleSessionPrompt` | Submits turn with multipart `prompt[]` blocks: `text`, `image` (`promptCapabilities.image: true`), `embeddedContext` (`@file`, `embeddedContext: true`). **Steering:** sending a second `session/prompt` while a turn is active steers the active turn (`v1.5.0+`). Returns `stopReason` (`end_turn`, `max_tokens`, `max_turn_requests`, `refusal`, `cancelled`) and `usage` (`inputTokens`, `cachedReadTokens`, `cachedWriteTokens`, `thoughtTokens`, `outputTokens`, `totalTokens`). |
| `session/cancel` | `handleSessionCancel` | Notification canceling active turn, closing open tool calls, terminating session subagents/background tasks (`v1.5.10+`), and abandoning pending permissions/question forms (`v1.5.1+`). |
| `session/set_mode` | `handleSessionSetMode` | Sets `modeId` (`default`, `accept-edits`, `auto`, `plan`). |
| `session/set_model` | `handleSessionSetModel` | Sets exact `modelId` from `models.availableModels`. |
| `session/set_config_option` | `handleSessionSetConfigOption` | Updates `configId` (`model`, `mode`, `thought_level`) to `value`. Defers agent rebuild until next turn (`v1.4.0+`) and emits updated `configOptions` + context window `usage_update` (`v1.5.8+`). |

### 2.2 Server -> Client Requests & Notifications
| Direction / Method | Server Emitter | Payload & Semantics |
| --- | --- | --- |
| `session/request_permission` (request) | `requestPermissionFromClient` | Pauses turn for approval (`toolCall`, `options[]` with `optionId` & `kind`: `allow_once`, `allow_always`, `reject_once`, `reject_always`; optional `_meta.agy.security.warning`). Also carries native interactive questions when `toolCall.toolCallId` starts with `"interaction_"` (if client lacks elicitation forms). No 60s timeout; auto-closed on cancel/prompt end. |
| `elicitation/create` (request) | `askQuestionWithForm` / `elicitQuestionAnswers` | Sent for `ask_question` when client advertises `clientCapabilities.elicitation.form` (`SupportsElicitationForm`). Otherwise falls back to chat / `interaction_*`. |
| `fs/read_text_file`, `fs/write_text_file` (request) | Client FS capability | Sent when client advertises `clientCapabilities.fs.{readTextFile,writeTextFile}`. |
| `terminal/*` (request) | Client terminal capability | Sent when client advertises `clientCapabilities.terminal: true`. |
| `session/update` -> `agent_message_chunk` | `sendMessageChunk` | Streaming assistant text deltas. |
| `session/update` -> `agent_thought_chunk` | `sendThoughtChunk` | Streaming model reasoning deltas. |
| `session/update` -> `user_message_chunk` | `sendUserMessageChunk` | Replayed user prompt chunks during `session/load`. |
| `session/update` -> `tool_call` / `tool_call_update` | `sendStepUpdatesAs`, `sendOutOfTurnStepUpdates` | Tool call start/progress/completion. Includes `toolCallId`, `kind` (`read`, `search`, `fetch`, `edit`, `delete`, `move`, `execute`), `title`, `status` (`pending`, `in_progress`, `completed`, `failed`, `cancelled`), `locations[]`, `rawInput` (MCP tools include both flattened args and nested `arguments`), `rawOutput` (includes `exit_code` for commands), and `content[]` blocks: text, **structured diffs (`type: "diff"`, `path`, `oldText`, `newText` — trailing newline bug fixed in `v1.5.9`)**, and **inline base64 images (`generate_image`)**. `sendOutOfTurnStepUpdates` streams post-turn background task & subagent wakeups immediately (`v1.5.10+`). |
| `session/update` -> `plan` | `sendStepUpdatesAs` | Agent plan / todo step list updates. |
| `session/update` -> `available_commands_update` | `sendAvailableCommands` | Live list of slash commands & skills scoped to the session's active agent (`v1.5.2+`). |
| `session/update` -> `current_mode_update` | `sendModeUpdate` | Active execution mode changes (`currentModeId`). |
| `session/update` -> `config_option_update` | `sendConfigOptionUpdate` | Server-driven config changes (e.g., per-model `thought_level` refresh or enterprise model fallback). |
| `session/update` -> `session_info_update` | `sendSessionInfoUpdate` | Live conversation title / cwd / `updatedAt` updates. |
| `session/update` -> `usage_update` | `sendUsageUpdate` / `sendStateUsageUpdate` | Real-time context window occupancy (`used` tokens and `size` context window capacity) emitted on session open/load, mid-turn steps, turn end, and model switches (`v1.5.8+`). |

### 2.3 Process-Per-Session Router Architecture (`--process-per-session`)
`google3/third_party/jetski/cli/acp/router.(*Router)` runs a single front ACP process over stdio that spawns an isolated `agy acp --worker` child process per session (`startWorker`, `servesSession`, `routeClientResponse`, `retire`). Required when the ACP client passes HTTP MCP servers in `session/new`.

### 2.4 Custom ACP `_meta` Extensions
| Scope | `_meta` Path | Purpose |
| --- | --- | --- |
| `initialize` (client) | `clientCapabilities.auth._meta.gateway` | Advertises custom LLM gateway auth support. |
| `authenticate` (client) | `_meta.gateway` | `{ "baseUrl": "...", "headers": { ... } }` for `gateway` auth. |
| `authenticate` (client) | `_meta` (API key) | Client-supplied Gemini API key (Xcode compat) with `GEMINI_API_KEY` fallback. |
| `authenticate`, `session/{new,load,resume}` (server) | `_meta.antigravity.tier` | `{ "name": "...", "id": "...", "upgradeUri": "..." }`. |
| `session/{new,load,resume}` (client) | `_meta.antigravity.systemPrompt.append` | Appends client system instructions (preserved across model switches). |
| `session/new` MCP config | `_meta.antigravity.eagerMcpServers`, `forceAllToolsEager`, `mcpMaxOutputBytes` | Controls lazy vs eager MCP tool loading and output byte limits. |
| `session/{new,load}` (client) | `_meta.agy.enabledTools`, `_meta.agy.disabledTools` | Per-session built-in tool allowlist/denylist. |
| `session/request_permission` (server) | `_meta.agy.security.warning` | Security warning text for high-risk tool approvals. |

### 2.5 Dual-ACP Split & Minimum Version Matrix
Two ACP implementations exist in the ecosystem:
1. **Standalone ACP Registry Server (`antigravity-acp` / `agy_acp_server.exe`, `v1.0.0`–`v1.5.0`)**: Shipped via JetBrains/Zed/Xcode/Android Studio registries. Uses mode IDs `default`, `auto_edit`, `yolo`.
2. **CLI-Embedded ACP Server (`agy acp`, CLI `v1.5.0+`)**: Uses mode IDs `default`, `accept-edits`, `auto` (`v1.5.4+`), `plan`.

**Minimum `agy acp` version behavior matrix (why wrappers must target `>= 1.5.10`):**
- `1.5.0`: Initial `agy acp`; restricts 3rd-party models for non-allowlisted `clientInfo.name`; races `session/prompt` cancellation.
- `1.5.1`: Lifts 3rd-party model restriction for all ACP clients; fixes cancellation/reload races & `session/update` ordering after reply.
- `1.5.4`: Adds `auto` mode to `agy acp`; fixes mode reverting to `default` on load/resume; fixes hang on second permission approval.
- `1.5.8`: Adds `usage_update` context window updates on open/reload/model switch.
- `1.5.9`: Fixes `agy acp --login` credential fallback, special chars in MCP names, specific turn errors (instead of generic termination), UTF-8 BOM loading, and removes stray trailing newline in `oldText`/`newText` diffs.
- `1.5.10`: Streams post-turn background subagent/task wakeups immediately; kills orphan subagents & daemons on `session/cancel` / `session/close`.
- `1.5.14`: Live-verified version with `session/{new,load,resume,list,close}`, `additionalDirectories`, `auth.logout`, `_meta.antigravity.{tier,systemPrompt.append}`.

---

## 3. Complete Network & Local Endpoints Directory

### 3.1 Cloud Inference, Auth, Quota & Enterprise Endpoints (TCP 443)
| Category | Endpoint / Host | Role & Notes |
| --- | --- | --- |
| **Cloud Code Assist (Consumer/OAuth)** | `https://daily-cloudcode-pa.googleapis.com` (`--cloud_code_endpoint` default)<br>`https://cloudcode-pa.googleapis.com` | Internal RPCs: `v1internal:generateContent`, `v1internal:streamGenerateContent`, `v1internal:fetchAvailableModels`, `v1internal:loadCodeAssist`, `v1internal:onboardUser`, `v1internal:retrieveUserQuotaSummary`. |
| **Gemini Developer API (API Key)** | `https://generativelanguage.googleapis.com` (overridable via `GOOGLE_GEMINI_BASE_URL`) | Public Gemini REST/SSE endpoints (`x-goog-api-key`). |
| **Gemini Enterprise Orchestration** | `https://businessaicode.googleapis.com`<br>`https://businessaicode.us.rep.googleapis.com`<br>`https://businessaicode.eu.rep.googleapis.com`<br>`https://businessaicode.mtls.googleapis.com` | Enterprise AI dev tools control & inference routing. Global endpoint **must** remain reachable even in `us`/`eu` residency for license discovery & catalog. |
| **Vertex AI / Agent Platform** | `https://aiplatform.googleapis.com`<br>`https://aiplatform.us.rep.googleapis.com`<br>`https://aiplatform.eu.rep.googleapis.com`<br>`https://aiplatform.mtls.googleapis.com` | Enterprise & Pay As You Go model inference (`aiplatform.endpoints.predict`). |
| **Discovery Engine (Enterprise Seats)** | `https://discoveryengine.googleapis.com`<br>`https://us-discoveryengine.googleapis.com`<br>`https://eu-discoveryengine.googleapis.com` (+ `.mtls.`) | Seat license discovery & Gemini Enterprise entitlement checks (`roles/discoveryengine.agentspaceUser`). |
| **VPC-SC & Regional PSC Pitfall** | Global PGA/PSC VIPs serve `*.googleapis.com` (single-level wildcard) | Fails TLS `x509` on `*.<loc>.rep.googleapis.com`. Enterprises must provision **regional PSC endpoints** + private Cloud DNS `A` records for regional hostnames while routing global hostnames through global PSC/PGA. |
| **Google OAuth & CIMD** | `https://accounts.google.com/o/oauth2/{auth,v2/auth}`<br>`https://antigravity.google/oauth-callback`<br>`https://antigravity.google/oauth/client-metadata.json` | OAuth scopes: `cloud-platform`, `userinfo.profile`, `experimentsandconfigs`, `cclog`, `aicode`. CIMD client ID for MCP OAuth 2.0 PKCE (`S256`). |
| **Remote Control & Remote Agents** | `https://antigravity.google.com`<br>`https://antigravity.google.com/r/<instance-id>?p=c%2F<conv-id>`<br>`https://antigravity.google.com/remote` | Web/PWA Remote Control relay dashboard (WebRTC DataChannel + WebChannel signaling + PIN pairing) and Cloud Remote Agents. |
| **CLI Auto-Updater & Binaries** | `https://antigravity-cli-auto-updater-974169037036.us-central1.run.app/manifests/<target>.json`<br>`https://storage.googleapis.com/antigravity-public/antigravity-cli/<ver>-<build>/...`<br>`https://antigravity.google/cli/install.{sh,ps1,cmd}` | Signed SHA-512 CLI manifests & install scripts. |
| **App Auto-Updater** | `https://antigravity-hub-auto-updater-974169037036.us-central1.run.app/manifest/`<br>`https://antigravity.google/download` | `electron-updater` generic feed for Antigravity App. |
| **Plugin Marketplaces & Schemas** | `https://www.gstatic.com/antigravity/plugins/marketplace.json`<br>`https://www.gstatic.com/antigravity/plugins-partners/marketplace.json`<br>`https://antigravity.google/schemas/v1/{settings,plugin}.json`<br>`https://antigravity.google/llms.txt` | Official & partner plugin catalogs, JSON schemas, and LLM documentation index (every `/docs/<path>` exposes `/docs/<path>.md`). |

### 3.2 Local / Loopback Endpoints & IPC
- **Language Server (App / IDE / VS Code `agy --hub`)**: Loopback HTTPS/HTTP (`127.0.0.1:<port>`, default fallback `5387` in `ANTIGRAVITY_LS_ADDRESS`) with pinned TLS cert & `x-codeium-csrf-token`. Exposes Connect-RPC / gRPC `exa.language_server_pb.LanguageServerService/*` (`StartCascade`, `CancelCascade`, `GetCascadeTrajectory`, `StreamCascadeReactiveUpdates`, `StreamAgentStateUpdates`, `StreamTerminalOutput`, `CallMcpAppTool`, `RefreshMcpServers`, `RegisterInteraction`, `/healthz`).
- **Actuator Reverse RPC (LS -> Electron Main)**: Connect-RPC over loopback HTTPS (`actuator.proto`, `computer_viewer.proto`, `browser_viewer.proto`, `host_bridge.proto`).
- **Voice SSH Loopback Bridge**: `agy mic-serve` on `127.0.0.1:4713` (forwarded via `ssh -R` into `ANTIGRAVITY_MIC`).
- **Local Model Servers (SDK)**: LiteRT managed loopback HTTP (`127.0.0.1:<ephemeral>`) or OpenAI-compatible endpoints (Ollama `:11434/v1`, LM Studio `:1234/v1`, vLLM).

---

## 4. Complete Model Catalog, Dynamic Reasoning & Orchestrator

### 4.1 Model Availability & Thinking Effort Matrix
| Model | CLI / ACP Slugs | Effort Levels (`CLI` / `App`) | Free & Plus | Pro | Ultra | Enterprise | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **Gemini 4 Neon** | `gemini-4-neon{,-low,-medium,-high,-xhigh}` | `low`, `medium` *(def)*, `high`, `xhigh` (`Extra`) | No | Yes† | Yes | Yes | Default model on eligible plans; 512K context; API key (`v1.5.12+`). |
| **Gemini 4 Carbon** | `gemini-4-carbon{,-low,-medium,-high}` | `low`, `medium` *(def)*, `high` | No | No | Yes | Yes | Flagship long-horizon SWE/science; API key (`v1.5.8+`) & ADC (`v1.5.5+`). |
| **Gemini 4 Argon** | `gemini-4-argon{,-low,-medium,-high}` | `low`, `medium` *(def)*, `high` | No | Yes† | Yes | Yes | API key (`v1.4.3+`). |
| **Gemini 4 Hydrogen** | `gemini-4-hydrogen{,-low,-medium,-high,-xhigh}` | `low`, `medium` *(def)*, `high`, `xhigh` (`Extra`) | No | Yes** | Yes | Yes | Fast coding model; default Orchestrator executor & SDK `v0.1.24` default. |
| **Orchestrator** | `gemini-pro-orchestrator-neon`, `gemini-pro-orchestrator` (Carbon), `gemini-pro-orchestrator-argon` | Fixed in CLI (`--effort` disabled); per-role configurable in App & CLI `v1.5.14` | No | Yes† | Yes | Yes‡ | In-turn Planner + Executor collaboration (`‡` Enterprise/ADC fixed to Carbon High + Hydrogen High). |
| **Gemini 3.8 Flash** | `gemini-3.8-flash{,-low,-medium,-high}` | `low`, `medium`, `high` | Yes | Yes | Yes | Yes | Default Flash model; API key & ADC. |
| **Gemini 3.7 Flash** | `gemini-3.7-flash{,-low,-medium,-high}` | `low`, `medium`, `high` | No | No | No | Yes | Enterprise + API key. |
| **Gemini 3.6 Flash** | `gemini-3.6-flash{,-low,-medium,-high}` | `low`, `medium`, `high` | No | No | No | Yes | Enterprise + API key. |
| **Gemini 3.1 Pro** | `gemini-3.1-pro-preview`, `gemini-3.1-pro-low`, `gemini-3.1-pro-high` | `low`, `high` | No | No | No | Yes | Enterprise + API key. |
| **Claude Sonnet 5.5 (thinking)** | `claude-sonnet-5-5{,-low,-medium,-high,-max}` | `low`, `medium` *(def)*, `high`, `max` (`Max`) | No | Yes** | Yes | Opt-in | Admin opt-in & per-token billing on Enterprise; uses 1P Gemini for background summaries. |
| **Claude Opus 5.5 (thinking)** | `claude-opus-5-5{,-low,-medium,-high,-max}` | `low`, `medium` *(def)*, `high`, `max` (`Max`) | No | Yes** | Yes | Opt-in | Same opt-in & hybrid execution semantics. |
| **Claude Sonnet/Opus 4.6 & GPT-OSS-120b** | *(legacy)* | — | Retiring | Retiring | GPT only | No | **Scheduled for removal Nov 30, 2026.** |
| **Nano Banana 2** | *(internal)* | — | Internal | Internal | Internal | `global` only | Image generation (`image-generator` subagent); unavailable in `us`/`eu`. |

*Notes:* `†` Paid AI Pro or eligible device trial. `**` Non-trial paid AI Pro. Custom subagent tiers: `inherit`, `flash_lite`, `flash` (3.8 Flash), `pro` (4 Argon). Live `agy models` (v1.5.14) returns 26 concrete slug rows across these families.

### 4.2 Execution Modes (`Shift+Tab` Cycle)
1. **`default`** (`agentMode: ""`): Prompts inline diff approval before every file edit; shell commands follow `toolPermission`.
2. **`accept-edits`**: Auto-approves workspace file edits/creations (incl. subagents); commands follow `toolPermission`. Also available in `--remote-agent` (`v1.5.10+`).
3. **`auto`** *(default since `v1.5.7` where available)*: Auto-approves workspace edits, runs commands in OS terminal sandbox, and uses a secondary reviewer agent (Gemini 4 Hydrogen Medium) to auto-approve safe reversible out-of-sandbox commands while prompting on irreversible/risky ones. Available on personal accounts & PAYG ADC where OS sandbox exists; **unavailable on Gemini Enterprise**.
4. **`plan`**: Read-only exploration & reviewable Implementation Plan before any edits. Also available in `--remote-agent` (`v1.5.14+`).

*Precedence:* Explicit `Deny > Ask > Allow` rules and `toolPermission: "strict"` or `"always-proceed"` override execution mode; Enterprise admin policies override all local settings.

---

## 5. Complete Authentication & Tier Matrix

| Auth Method | ACP `methodId` | Credential Source | Billing / Terms | Key Constraints |
| --- | --- | --- | --- | --- |
| **Personal Google OAuth** | `oauth-personal` | Browser OAuth / SSH code paste -> OS keyring (`0600` file fallback) | Antigravity ToS (Free, AI Plus, Pro, Ultra + optional AI Credit Overages) | **Consumer ToS prohibits wrapping consumer logins in third-party agents (`faq.md`).** |
| **Google Workspace (non-Enterprise)** | `oauth-personal` | Standard Google OAuth | Limited Starter Quota (unless admin disabled) | No AI Pro/Ultra plans or AI credits; upgrade via GCP PAYG or Gemini Enterprise. |
| **Gemini Enterprise SSO** | `oauth-business` | Cloud SSO + seat picker -> saves `gcp.{project,location}` | Google Cloud ToS; pooled seat credits (\$10 Standard / \$15 Plus) or GE Pay-as-you-go; zero training; telemetry -> GCP project | Needs `roles/discoveryengine.agentspaceUser`, `aiplatform`, `discoveryengine`, `businessaicode` APIs. `auto` mode unavailable; no standalone Antigravity IDE. |
| **GCP Pay As You Go** | `oauth-business` (PAYG billing) | Google Cloud OAuth -> GCP billing project | Google Cloud ToS; per-token billing; telemetry off | Needs `aiplatform.googleapis.com` + `roles/aiplatform.user`; Claude needs Model Garden. Gets `auto` mode (`v1.5.6+`). |
| **BYOID / Workforce Identity (WIF)** | `oauth-business` (Advanced SSO) | External IdP (Okta, etc.) via `locations/global/workforcePools/.../providers/...` | Google Cloud ToS | Tokens refresh 5 min before expiry. |
| **Application Default Credentials (ADC)** | `agent-platform` | `gcloud auth application-default login` + `AGY_ADC_AUTH=true` or `userSettings.enableAdc: true` | Google Cloud ToS | Project order: ADC `quota_project_id` -> `GOOGLE_CLOUD_QUOTA_PROJECT` -> metadata server. No models older than Gemini 3 Flash; no image gen in `us`/`eu`. |
| **Gemini API Key** | `gemini-api-key` | `GEMINI_API_KEY` (+ `modelProvider: "gemini"` in CLI `settings.json`) | AI Studio API billing | Supports `customModelsConfig` & `GOOGLE_GEMINI_BASE_URL`. Disables Remote Control, Remote Agents, `/usage`, `/credits`, `/voice` (exit `2`). |
| **Enterprise Custom Gateway** | `gateway` | `admin_settings.json` + `AGY_LLM_GATEWAY_*` or ACP `_meta.gateway` | Org gateway | Requires `https://` unless loopback (`v1.5.5+`). |
| **Local / Offline (SDK only)** | N/A | None (`LiteRTAgentConfig` or `LocalOpenAIAgentConfig`) | Local compute | Python SDK only (`.litertlm`, Ollama, LM Studio, vLLM). |
