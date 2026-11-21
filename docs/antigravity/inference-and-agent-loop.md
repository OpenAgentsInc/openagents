# Antigravity: inference path, agent loop and tool calls

This page covers the part of Antigravity that actually does work: how a prompt reaches a
Gemini model, who runs the agent loop, what a "step" is on the wire, how tool calls are
proposed and executed, and how the Electron host participates.

Everything here was derived read-only from the shipped bundle: the ASAR payload in
`Contents/Resources/app.asar` and `strings` over the 196 MB Go binary
`Contents/Resources/bin/language_server` (560,040 lines of strings). No app code was
executed and the app was never launched. Where a claim is an inference from symbol names
rather than from readable source, it is marked *(inferred)*.

---

## 1. The short answer

**There is no Antigravity CLI in the loop, and the Electron JavaScript does not call
Gemini.** The shipped executables are exactly:

```
Contents/MacOS/Antigravity                     # Electron shell
Contents/Resources/bin/language_server         # 196.9 MB Go binary — the real backend
Contents/Resources/bin/agy-desktop-helper      # 1.3 MB native desktop helper
Contents/Resources/bin/agy-desktop-script      # 129 KB script executor
Contents/Resources/bin/cable_ble_scanner       # 115 KB BLE pairing
```

The string `antigravity-cli` does appear in the Go binary, but only as a **settings
location** (`.gemini/antigravity-cli/settings.json`, a log path `antigravity-cli, Logs
Path: %s`, and a sandbox prefix `antigravity-cli-ro-*`) plus a reference to an
`antigravity-sdk-python`. It is not a program in the bundle. The thing you talk to is
`language_server`.

So the flow is:

```
Hub renderer (remote web UI in a BrowserWindow)
        │  HTTPS on 127.0.0.1:<port>, CSRF token, pinned cert
        ▼
language_server  (Go)
   ├── agent loop (“Cascade” / executor)          ← runs here
   ├── tool dispatch                              ← runs here
   ├── trajectory storage under <appDataDir>/brain/
   └── outbound HTTPS inference
            ├── https://generativelanguage.googleapis.com   (API-key mode)
            └── https://daily-cloudcode-pa.googleapis.com  (Google-account mode)
```

---

## 2. How the language server is launched

`dist/languageServer.js` (24.6 KB) owns the sidecar lifecycle and `dist/lsDefaults.js`
builds the base flag list. The flags the binary is given include:

```
--standalone
--https_server_port <port>
--api_server_url        https://generativelanguage.googleapis.com
--cloud_code_endpoint   https://daily-cloudcode-pa.googleapis.com
--app_data_dir          <userData>
```

Mechanics of the link, all in `dist/languageServer.js`:

- **Port discovery by stdout scraping.** The host parses
  `/listening on \w+ port at (\d+) for HTTP(S)?/i` to learn the port — the sidecar picks
  the port, the host does not assign it.
- **Bootstrap token / CSRF.** The host mints a token at spawn and passes it to the
  renderer, so the loopback HTTP surface is not open to any other local process.
- **TLS on loopback.** `setupLocalCertTrust` installs trust for the sidecar's local
  certificate; `dist/actuator/pinnedLsCert.js` pins it for the Connect clients.
- **Supervision.** Crash monitoring, plus a stamp/self-restart protocol: a dedicated exit
  code (commented in the source as "requesting an immediate self-restart") makes the host
  respawn it, and `mainActuatorClientPortChanged` re-points every client at the new port.
- **Host allow-listing.** Requests are only honoured for `hostname === '127.0.0.1' ||
  'localhost'`.

---

## 3. Talking to Gemini

The binary carries two generations of Google API plumbing.

### 3.1 The public-ish Gemini surface (`generativelanguage`)

Symbols present include the whole `generativelanguage.v1main` proto package
(`Content`, `Schema`, `ToolType`, `ModalityTokenCount`, `ExecutionMode`,
`generativelanguage/v1main/resolution.proto`) alongside the classic REST shapes:
`system_instruction`, `tools`, `functionDeclarations`, `toolConfig`, `safetySettings`,
`thinking_config`, `candidatesTokenCount`, `{model}:embedContent`, and
`BidiGenerateContent*` (the live/bidirectional streaming API).

### 3.2 The Cloud Code surface (`v1internal`)

A large internal proto package ships inside the same binary:

```
v1internal/cloudcode.proto            v1internal/model_configs.proto
v1internal/prediction_service        v1internal/jetski_service
v1internal/entitlement, credits, onboarding, remote_context
```

with these RPCs (counts = occurrences in the strings table):

| RPC | Role |
| --- | --- |
| `GenerateContent` (2,192) / `AsyncGenerateContent` | the completion call |
| `FetchAvailableModels` | model catalogue + per-user availability |
| `LoadCodeAssist` / `OnboardUser` (+ `OnboardUserBackgroundTasks`) | seat/entitlement and first-run onboarding |

and the HTTP bindings are visible verbatim:

```
v1internal:generateContent
v1internal:streamGenerateContent
v1internal:fetchAvailableModels
v1internal:loadCodeAssist
v1internal:onboardUser
```

`v1internal:streamGenerateContent` is the one the loop actually lives on — the UI is fed
incrementally, so inference is streamed, not awaited.

### 3.3 Two auth modes, chosen at runtime

- **Google account / OAuth.** Scopes found in the binary:
  `https://www.googleapis.com/auth/cloud-platform`, `…/auth/userinfo.profile`,
  `…/auth/experimentsandconfigs`, `…/auth/cclog`, `…/auth/aicode`. PKCE against a loopback
  redirect is handled by `dist/remote/signInUrls.js` (`classifyOpenUrl`,
  `isSignInOrigin`, `loopbackCallbackPort`) and `dist/remote/signInCallback.js`. This mode
  goes to the **Cloud Code** endpoint (`daily-cloudcode-pa.googleapis.com`) and carries the
  entitlement/seat machinery.
- **API-key mode.** `GEMINI_API_KEY` / `GOOGLE_API_KEY`, `x-goog-api-key`,
  `API_KEY_AUTH`, and the explicit error string `… not in Gemini Developer API mode.
  systemInstruction parameter is only supported in Gemini Enterprise …` — i.e. the binary
  branches on whether it is running as a Cloud Code (enterprise/seat) client or a plain
  Gemini Developer API client, and some parameters are only legal in one branch.

Three OAuth client IDs are embedded (`…apps.googleusercontent.com`), consistent with
stable/beta/internal channel builds.

### 3.4 The model catalogue is a placeholder indirection

The binary is full of `MODEL_PLACEHOLDER_M408 … M465` — opaque IDs resolved through
`v1internal/model_configs.proto`, with `AgentSetting.GetAgyAllowedModels` gating what a
given user may select. That is why the UI can offer models that do not appear in any
client-side list: the mapping is server-side.

Enumerated families visible in the strings table: `MODEL_GOOGLE_GEMINI_1_0_PRO`,
`…1_5_PRO`, `…2_0_FLASH`, `…2_5_FLASH` (plus `_LITE`, `_THINKING`, `_THINKING_TOOLS`,
`_IMAGE_PREVIEW`, dated `_PREVIEW_04_17` / `_05_20`), `…2_5_PRO`, and internal codenames
(`NEMOSREEF`, `PUREPRISM`, `ORIONFIRE`, `RIFTRUNNER_THINKING_LOW/HIGH`,
`INTERNAL_TAB_FLASH_LITE`). Alongside them sit literal serving names —
`gemini-3.7-flash-*`, `gemini-3.8-flash-low/-high`, `gemini-4-hydrogen-*`,
`gemini-4-carbon-*`, `gemini-4-neon-*`, `gemini-4-argon-*`, `claude-sonnet-*`,
`claude-opus-5-5-max`. So Antigravity is a **multi-vendor** client, not Gemini-only.

---

## 4. The agent loop

The loop is an internal subsystem called a **Cascade**. What is observable:

### 4.1 Shape

```
StartCascade (182×)  →  trajectory of steps  →  CancelCascade (149×)
                     →  GetCascadeTrajectory (220×)   ← replay/resume
```

A conversation is a `CascadeState` holding an ordered list of `CortexStep`s, with
types `CortexStepRunCommand`, `CortexStepCheckpoint`, `CortexStepMcpTool`,
`CortexStepCheckpoint`, `CortexStepGeneratorMetadata`, `ToolCallStep`, `ToolCallParam`,
and a conversion layer `ToolCallToCortexStep`. A `ToolCall` is therefore *not* the atom —
the atom is the step, and a tool call is one step kind among many. This is what lets the
UI show a uniform transcript of "things the agent did".

### 4.2 Planner + executor

Two models are configured per run:

```
planner_model, planner_mode (PLANNER_MODE_DEFAULT | PLANNER_MODE_READ_ONLY | PLANNER_READONLY)
executor_model, executor_loop_status, executor_termination_reason
max_invocations, tool_call_choice / tool_call_choices, thinking_config, system_instruction
```

So: a **planner** model decides the approach (and can be pinned read-only, which is a
permission concept, not a model one), and an **executor** model runs bounded iterations
(`max_invocations`) issuing tool calls, feeding results back, and deciding whether to
continue. `PLANNER_RESPONSE` is itself a step type, so planning is visible in the
transcript.

### 4.3 Termination contract

From the documented comment string embedded in the binary: `terminationReason`, "e.g.
`NO_TOOL_CALL`, `MAX_INVOCATIONS`, `ERROR`, `USER_CANCELED`, `TERMINAL_STEP_TYPE`", where
`NO_TOOL_CALL` means *normal turn completion — the model finished without calling a
tool*. The full enum adds `MAX_FORCED_INVOCATIONS`, `MAX_TOKEN_BUDGET_EXCEEDED`,
`HALTED_STEP`, `EARLY_CONTINUE`, `CHAINED`, `INJECTED_RESPONSE`, `TERMINAL_CUSTOM_HOOK`.

That is the classic ReAct/function-calling loop, with explicit budgets: invocation count,
token budget, forced-invocation ceiling, custom terminal hooks, and user cancel.

### 4.4 Step status and step taxonomy

Steps carry `CORTEX_STEP_STATUS_{PENDING, QUEUED, RUNNING, WAITING, GENERATING, DONE,
HALTED, INTERRUPTED, CANCELED, CLEARED, ERROR, INVALID}`. Note `WAITING` and
`INTERRUPTED`: a step can block on a human (approval, question) and be resumed — which is
why `EXECUTOR_TERMINATION_REASON_INJECTED_RESPONSE` and `NEEDS_PERMISSION` exist.

The `CORTEX_STEP_TYPE_*` enum is the real map of what the agent can do. Condensed, it
covers:

- **File/workspace** — VIEW_FILE, VIEW_FILE_OUTLINE, LIST_DIRECTORY, GREP_SEARCH,
  FIND_ALL_REFERENCES, CODE_SEARCH, VIEW_CODE_ITEM, PROPOSE_CODE, WRITE_BLOB, MOVE,
  DELETE_DIRECTORY, FILE_CHANGE, FINDINGS.
- **Terminal** — RUN_COMMAND, SHELL_EXEC, READ_TERMINAL, SEND_COMMAND_INPUT,
  COMMAND_STATUS, WAIT, RESTART_DEV_SERVER.
- **Browser** — OPEN_BROWSER_URL, READ_BROWSER_PAGE, BROWSER_GET_DOM, CLICK_BROWSER_PIXEL,
  BROWSER_CLICK_ELEMENT, BROWSER_MOVE_MOUSE/DRAG_PIXEL_TO_PIXEL/MOUSE_DOWN/MOUSE_UP/
  MOUSE_WHEEL, BROWSER_PRESS_KEY, BROWSER_INPUT, BROWSER_SCROLL(+UP/DOWN),
  BROWSER_SELECT_OPTION, BROWSER_REFRESH_PAGE, BROWSER_RESIZE_WINDOW,
  EXECUTE_BROWSER_JAVASCRIPT, CAPTURE_BROWSER_SCREENSHOT, CAPTURE_BROWSER_CONSOLE_LOGS,
  BROWSER_LIST_NETWORK_REQUESTS / _GET_NETWORK_REQUEST, LIST_BROWSER_PAGES, BROWSER_SUBAGENT.
- **Web/knowledge** — SEARCH_WEB, READ_URL_CONTENT, RETRIEVE_CONTENT, INTERNAL_SEARCH,
  KNOWLEDGE_GENERATION, KNOWLEDGE_ARTIFACTS, KI_INSERTION, MEMORY, RETRIEVE_MEMORY,
  TRAJECTORY_SEARCH, TRAJECTORY_CHOICE, TOOL_SEARCH, CONVERSATION_HISTORY.
- **Orchestration** — PLAN_INPUT, PLANNER_RESPONSE, CHECKPOINT, TASK_BOUNDARY,
  INVOKE_SUBAGENT, TOOL_CALL_PROPOSAL, TOOL_CALL_CHOICE, SUGGESTED_RESPONSES,
  ASK_QUESTION, NOTIFY_USER, EPHEMERAL_MESSAGE, SYSTEM_MESSAGE, USER_INPUT,
  MANAGER_FEEDBACK, PROPOSAL_FEEDBACK, CRITIQUE, FINISH.
- **Ecosystem/cloud** — MCP_TOOL, RPC_ACTION, WORKSPACE_API, CLOUD_SQL_EXECUTE_SQL /
  UPDATE_SCHEMA / SET_UP_CLOUD_SQL, SET_UP_FIREBASE, DEPLOY_FIREBASE, GENERATE_IMAGE,
  EXECUTE_NOTEBOOK / EDIT_NOTEBOOK / READ_NOTEBOOK, START_CODE_REVIEW,
  POST_PR_REVIEW, PROPOSE_AI_COMMENTS, GIT_COMMIT, COMPILE, LINT_DIFF,
  RUN_EXTENSION_CODE, AGENCY_TOOL_CALL.

Two things follow. First, `CREATE_BROWSER_*` operands are first-class model outputs, not
post-processed intents — the model emits `browser_click_element`, etc. Second,
`TOOL_SEARCH` + `TOOL_CALL_CHOICE` mean the tool set is *retrieved*, not always fully
dumped into the prompt (tool search over a large catalogue, then a choice step), which is
how a 100-plus tool surface stays affordable.

---

## 5. Tool-call wire format

Names seen in the strings table: `tool_name` / `toolName` (49/38×), `arguments_json` (23×),
`args_json`, `tool_names` / `toolNames`, `tool_name_prefix`, `tool_name_mapping`,
`raw_args_json`, `denied_repeat`. So a call is `{tool_name, arguments_json}` plus a
correlation prefix, and a repeated denied call is tracked explicitly
(`denied_repeat`) — a loop-breaker for an agent hammering a refused tool.

Execution strategies are named in the binary: `executeToolCallsParallel` (10×) and
`executeToolCallsSequential` (2×). Independent read-only calls go parallel; anything
ordered goes sequentially.

**Where a given tool runs** is the important split:

| Class | Executed by | Examples |
| --- | --- | --- |
| Local to the LS | Go code inside `language_server` | `view_file`, `read_file`, `list_dir`, `grep_search`, `run_command`, `read_terminal`, `command_status`, `search_web` |
| Delegated to the host | Electron main process, over Connect | `computer_use`, `computer_screenshot`, all `browser_*` |
| Remote | MCP servers / remote SSH host | `mcp_tool_execute`, remote-side LS tools |

Tool literals confirmed in the binary: `"run_command"`, `"view_file"`,
`"read_file"`, `"list_dir"`, `"grep_search"`, `"search_web"`, `"read_terminal"`,
`"command_status"`, `"computer_use"`, `"computer_screenshot"`, plus the
`browser_*` family (`browser_get_dom`, `browser_list_network_requests`,
`browser_get_network_request`, `browser_origin_access`, `browser_authentication`,
`browser_subagent`).

### 5.1 The host round-trip: the Actuator contract

Host-side tools use an RPC contract, not an IPC channel. From `dist/actuator/mainClient.js`
(the header calls it "the Hub main process's ActuatorClient (actuator contract §4.1, D2)"):

```
getMainActuatorClient()            — lazy, process-wide, clientName: 'hub-main'
startMainActuatorClient(port, csrf) — called once the LS port + CSRF token are known
mainActuatorClientPortChanged()     — on every LS restart
stopMainActuatorClient()            — on shutdown (sends GOING_AWAY per type)
client.register(handler)            — e.g. computer-use kinds register here
```

The transport is **Connect (gRPC-compatible) over the loopback HTTPS port**, built on
`@connectrpc/connect-node` + `@bufbuild/protobuf`, with certificate pinning
(`actuator/pinnedLsCert.js`) and the CSRF token. The generated stubs in `dist/proto/` map
one-to-one onto the host capabilities:

| Proto | Service |
| --- | --- |
| `actuator.proto` | the generic actuator registration/call channel |
| `computer_viewer.proto` / `computer_payload.proto` | screen frames + input events for `computer_use` |
| `browser_viewer.proto` | the streamed-browser control plane (`browser_*`) |
| `host_bridge.proto` | `dist/hostBridgeServer.js` — a local HTTP+Connect bridge |

So the loop is: the Go executor decides `computer_use`, calls back over Connect into
Electron main, `computerActuator/` (114 KB handler + `scriptJail.js` sandbox) performs the
OS action, the result becomes the tool output, and that is appended as the next step and
fed back to the model. Browser tools take the same path into `browserEngine/`
(CDP pipe to a local Chrome) and `browserStream/`.

### 5.2 Subagents

`invoke_subagent` (95×) is a tool call like any other, and `CORTEX_STEP_TYPE_INVOKE_SUBAGENT`
is its step. `SubagentsConfig`, `SubagentSpec`, `SubagentTool`, `SubagentHub`,
`SubagentState`, `SubagentResult`, `SubagentModelOption`, `SubagentSkillsConfig`,
`subagent_snapshots`, `subagent_reminder` show that a subagent gets its own model
selection, its own tool subset and skills, its own state, and returns a `SubagentResult`
to the parent — with snapshots for resumption. `browser_subagent` is a specialised
variant. There is also `SubagentsCall` (25×), i.e. fan-out of several subagents in one
step.

---

## 6. Persistence: the `brain/` directory

The trajectory is written to disk, not just held in memory:

```
<appDataDir>/brain/<cascadeId>/
  .system_generated/logs/transcript.jsonl
  .system_generated/logs/transcript_full.json
  browser_sandbox/
  browser_sandbox/<cascadeId>/…
```

and the read-only rules are explicit in the binary's own strings: a parent conversation or
any other `brain/<other_id>` artifact directory is **read-only reference only**. There is a
revert path (`revert_to_step_called`, `[ForkConversation]`), and a checkpointer with
`CASCADE_USE_EXPERIMENT_CHECKPOINTER` configuration:

```json
{ "strategy": "CHECKPOINT_STRATEGY_SAME_MODEL", "max_token_limit": "256000", ... }
```

so long runs are compacted at a token threshold and re-generated after a mid-hop
checkpoint. `CORTEX_STEP_TYPE_CHECKPOINT` is a visible step, meaning compaction is
something the user sees in the transcript, not a silent truncation.

---

## 7. Streaming the loop to the UI

The renderer never polls. The LS exposes `LanguageServerService` streaming RPCs, all
visible as strings:

```
LanguageServerService/StreamCascadeReactiveUpdates
LanguageServerService/StreamCascadePanelReactiveUpdates
LanguageServerService/StreamCascadeSummariesReactiveUpdates
LanguageServerService/StreamUserTrajectoryReactiveUpdates
LanguageServerService/StreamAgentStateUpdates
LanguageServerService/StreamTerminalShellCommand / StreamTerminalOutput
LanguageServerService/StreamSearchCode
LanguageServerService/StreamAudioTranscription
LanguageServerService/CallMcpAppTool
LanguageServerService/RegisterInteraction / Heartbeat
```

"Reactive updates" = the trajectory is an observable stream; each new step or status
change is pushed. That is why the agent's progress appears token-by-token and
step-by-step, and why a page reload can reattach to a running cascade via
`GetCascadeTrajectory`.

---

## 8. Putting it together (one turn)

1. User submits input in the Hub renderer.
2. Renderer → loopback HTTPS + CSRF → `language_server` (`StartCascade`).
3. LS assembles the prompt: system instruction, planner/executor config, tool catalogue
   (searched/selected via `TOOL_SEARCH`), compaction summary if the trajectory is long.
4. LS → `v1internal:streamGenerateContent` on `daily-cloudcode-pa.googleapis.com`
   (account mode) or `generativelanguage.googleapis.com` (API-key mode), with an OAuth
   bearer token or `x-goog-api-key`.
5. Response streams back as content + `functionCall`s. Each becomes a `CortexStep` /
   `ToolCallStep` with `tool_name` + `arguments_json`.
6. Local tools run in Go. Host tools are dispatched over Connect to Electron main
   (`computer_use`, `browser_*`), executed under the jail / CDP / driver, and returned.
7. Tool outputs append as steps, stream to the UI, and the executor loops back to (4)
   until `NO_TOOL_CALL` (normal), `MAX_INVOCATIONS`, `MAX_TOKEN_BUDGET_EXCEEDED`,
   `USER_CANCELED`, or an error.
8. Every step is checkpointed into `brain/<cascadeId>/`, and the terminal/agent updates
   stream to the renderer as they happen.

---

## 9. Observations

**Good design choices**

- The model loop is isolated in a supervised sidecar with a token + pinned cert + loopback
  allow-list. Compromising the renderer does not by itself grant inference or tool access.
- Steps, not messages, are the unit — which gives free resumability, replay
  (`GetCascadeTrajectory`), forking and revert.
- Explicit budgets and a named termination enum (`NO_TOOL_CALL`, `MAX_INVOCATIONS`,
  `MAX_TOKEN_BUDGET_EXCEEDED`) make runaway loops a first-class, observable condition
  rather than a hang.
- `TOOL_SEARCH` + `TOOL_CALL_CHOICE` decouple catalogue size from prompt size.
- Read-only planner mode and `NEEDS_PERMISSION` / `WAITING` step states put human approval
  inside the state machine.

**Things to be aware of**

- **Everything interesting is a 196 MB unstripped Go binary.** The JS you can read is
  supervision and plumbing; the agent loop, tool semantics, model routing and safety
  filters are opaque in `language_server`. There is no source available for auditing the
  approximation of the loop from the outside — this document is an inference from symbols.
- The binary contains **internal codenames and unreleased model identifiers**
  (`MODEL_GOOGLE_GEMINI_NEMOSREEF`, `ORIONFIRE`, `RIFTRUNNER_THINKING_HIGH`, …), plus
  non-Google vendors. Upstream sibling of the Gemini CLI/Code Assist lineage, clearly.
- `api_key` appears 102× with secret-version machinery (`api_key_secret`,
  `api_key_secret_version`, `api_key_project_number`) — the enterprise path stores keys in
  Secret Manager; local API-key use is a separate, less-guarded path.
- Trajectory transcripts (`.system_generated/logs/transcript_full.json`) sit unencrypted in
  plain application support by default, and subagent snapshots multiply that.
- Two endpoints and two auth modes double the trust surface: OAuth bearer to a Cloud Code
  host, or an API key to the public Gemini host, with a documented capability split
  (`systemInstruction` is only valid in one of them) that is easy to get wrong.

---

## 10. How this was produced

```sh
# extract the ASAR without executing it
node -e '…parse app.asar header, walk entries, write to /tmp/ag…'

# the loop lives in the sidecar
strings -n 8 /Applications/Antigravity.app/Contents/Resources/bin/language_server \
  > /tmp/ls_strings.txt          # 560,040 lines / 43 MB

grep -oE 'CORTEX_STEP_TYPE_[A-Z0-9_]+' /tmp/ls_strings.txt | sort -u
grep -oE 'EXECUTOR_(TERMINATION_REASON|LOOP)_[A-Z0-9_]+' /tmp/ls_strings.txt | sort -u
grep -oE 'v1internal[a-z0-9_./-]*:(generateContent|streamGenerateContent|fetchAvailableModels|loadCodeAssist|onboardUser)' /tmp/ls_strings.txt | sort -u
grep -oE 'LanguageServerService/[A-Za-z]+' /tmp/ls_strings.txt | sort -u
grep -oE 'gemini-[0-9][a-zA-Z0-9._-]*|claude-[0-9][a-zA-Z0-9._-]*' /tmp/ls_strings.txt | sort -u

# the host side of the tool round-trip
sed -n '1,50p' /tmp/ag/dist/actuator/mainClient.js
sed -n '1,140p' /tmp/ag/dist/lsDefaults.js
```

No application code was executed and the app was not launched.
