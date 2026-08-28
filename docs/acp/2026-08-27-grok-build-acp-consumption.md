# Grok Build as an ACP agent, and the OpenAgents ACP host

- Date: 2026-08-27
- Class: working document
- Status: source-grounded comparison
- Authority: current grok-build and OpenAgents source. This document does not
  admit a product, a peer, or a release.

This document describes how grok-build speaks the Agent Client Protocol so a
host can consume it, then maps that contract onto the OpenAgents ACP host.

ACP here is Zed's Agent Client Protocol: newline-delimited JSON-RPC between a
coding agent and a client (IDE, SDK, or another agent). It is not the Linux
Foundation Agent Communication Protocol and it is not A2A.

## Sources

| Tree | Pin | Role |
| --- | --- | --- |
| grok-build at `~/work/projects/repos/grok-build` | `SOURCE_REV` `70ec060ec3d28e77b9c4593be43c2ab0128bcd21` | ACP **agent** (`grok agent stdio`) |
| OpenAgents `openagents/main` | `736412cb30` | ACP **client/host** |
| Official ACP registry | `~/work/projects/agentclientprotocol/repos/registry/grok-build/agent.json` | Launch metadata for id `grok-build` |

Grok-build also contains an ACP **client**: the pager TUI
(`crates/codegen/xai-grok-pager/src/acp/`) talks to the same shell over ACP.
That is how grok-build's own UI consumes grok-build. An external host should
copy the agent's stdio contract, not the pager internals.

Related OpenAgents documents (planning and packages, older pins):

- [T3 Code ACP teardown](../teardowns/2026-07-16-t3-code-agent-client-protocol-implementation-teardown.md)
- [Runtime bridge ADR](../adr/2026-07-16-agent-client-runtime-bridge.md)
- [Session runtime ADR](../adr/2026-07-16-agent-client-session-runtime.md)
- [Grok CLI notes](../grok-cli/README.md) and the [minimal stdio client](../grok-cli/examples/acp-stdio-hello.mjs)
- Peer profile: `packages/agent-client-protocol/src/profiles/grok.ts`
- Live coder host: `crates/openagents-cli/src/acp.rs`

## Roles

```text
ACP client / host                         ACP agent
(IDE, OpenAgents coder, SDK)              grok-build
        |                                        |
        |  spawn: grok agent [--always-approve] stdio
        |  NDJSON JSON-RPC on stdin/stdout
        |                                        |
        |  initialize  ----------------------->  MvpAgent
        |  authenticate                        (acp::Agent)
        |  session/new
        |  session/prompt
        |  session/cancel
        |  session/load, set_mode, set_model
        |                                        |
        |  <------------------- session/update
        |  <------- session/request_permission
        |  <------- fs/*, terminal/*, x.ai/*
```

The host is the client. grok-build is the agent. The host starts one child
process per connection, owns stdin/stdout, and must answer every reverse
request or the agent hangs.

## How a host launches grok-build

### Command

```bash
grok agent --always-approve stdio
```

`--always-approve` (alias `--yolo`) is an agent flag and sits **between**
`agent` and `stdio`. Mode-specific flags sit after the mode name
(`serve --bind …`).

The binary is `grok` on PATH (install: `@xai-official/grok`, typically
`~/.grok/bin/grok`). The official registry entry `grok-build` launches:

```text
npx -y @xai-official/grok@<version> agent stdio
```

That argv already puts the process in ACP mode. There is no trailing `acp`
subcommand. Devin and OpenCode use `acp`; Grok uses `agent stdio`.

### Other transports (same protocol, different pipe)

| Command | Use |
| --- | --- |
| `grok agent stdio` | Local host. The usual IDE/SDK path. |
| `grok agent serve --bind 127.0.0.1:2419 --secret <token>` | WebSocket server. State survives reconnect. |
| `grok agent headless --grok-ws-url wss://…/ws` | Relay to a remote client. |
| `grok agent leader` | Shared leader process. Refused when a non-`off` sandbox is requested. |

A host that wants one process, one workspace, and no extra daemon uses stdio.

### Environment a host may set

| Variable | Effect |
| --- | --- |
| `XAI_API_KEY` | API-key auth (`xai.api_key`). Optional if `grok login` already wrote a cached token. |
| `GROK_HOME` | Config, auth, and session store (default `~/.grok`). |
| `GROK_CONFIG` | JSON overlay merged into config (`models`, `features`, a narrowed `toolset`, filter-only `shell_environment_policy`). Cannot set auth policy or spawn commands. |
| `GROK_CONFIG_PATH` | Extra JSON/TOML overlay. `GROK_CONFIG` wins if both are set. |
| `GROK_DEFAULT_SELECTED_PERMISSION` | Headless permission default. |
| `GROK_AGENT_SECRET` | Token for `serve`. |
| `GROK_AGENT_METADATA` | Object merged into `initialize` response `_meta.metadata`. |

Auth lives on disk after `grok login`, or in `XAI_API_KEY`. The host does not
send a bearer token on the wire. It picks an advertised method and calls
`authenticate`.

## Handshake a consumer must perform

Grok-build's agent implementation is
`impl acp::Agent for MvpAgent` in
`crates/codegen/xai-grok-shell/src/agent/mvp_agent/acp_agent.rs`.
The in-tree test client that drives the full lifecycle is
`crates/codegen/xai-grok-test-support/src/acp_client.rs` (`GrokStdioClient`).

### 1. `initialize`

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": 1,
    "clientCapabilities": {
      "fs": { "readTextFile": true, "writeTextFile": true },
      "terminal": true
    },
    "_meta": {
      "clientType": "openagents",
      "clientVersion": "0.1.0",
      "startupHints": {
        "nonInteractive": true,
        "skipGitStatus": true,
        "skipProjectLayout": true
      }
    }
  }
}
```

Wire version is ACP protocol version 1.

If the host advertises `fs` or `terminal`, grok-build will call those methods
back on the host. A host that cannot serve files or a PTY must advertise
`false` so Grok uses its own tools in `cwd`.

Response capabilities grok-build advertises:

| Field | Value |
| --- | --- |
| `protocolVersion` | `1` |
| `agentCapabilities.loadSession` | `true` |
| `agentCapabilities.promptCapabilities.embeddedContext` | `true` |
| `agentCapabilities.mcpCapabilities.http` / `sse` | `true` |
| `agentCapabilities.sessionCapabilities.close` | present |
| `agentCapabilities.sessionCapabilities.list` / `resume` | present unless process chat-mode is on |
| `authMethods` | subset of `cached_token`, `xai.api_key`, `grok.com`, `oidc` |
| `_meta.grokShell` | `true` |
| `_meta.defaultAuthMethodId` | the method the agent already selected; do not re-derive |
| `_meta.agentVersion` | Grok version string |
| `_meta.modelState` | available models and the default |
| `_meta.availableCommands` | slash commands for autocomplete |
| `_meta.currentWorkingDirectory` | launch cwd |

`_meta` also flags `x.ai/fs_notify`, hook capability, MCP-over-ACP
(`x.ai/mcp/sdk_call`), and `pluginDirs` on `session/new`.

`initialize` must run before `session/new`. A second `initialize` on reconnect
is accepted; auth-method state is written once.

### 2. `authenticate`

Required. Skipping this is the most common way a Grok stdio child dies before
the first prompt.

Pick `init._meta.defaultAuthMethodId` when present. Otherwise:

1. `xai.api_key` if `XAI_API_KEY` is set and advertised
2. `cached_token` if advertised (needs a prior `grok login`)
3. `grok.com` / `oidc` only with an interactive browser flow

```json
{
  "method": "authenticate",
  "params": {
    "methodId": "cached_token",
    "_meta": { "headless": true }
  }
}
```

`_meta.headless: true` keeps the agent from waiting on a TTY. Interactive
methods (`grok.com`, `oidc`) need a typed continuation; a headless host should
stop and report rather than hang.

### 3. `session/new`

```json
{
  "method": "session/new",
  "params": {
    "cwd": "/path/to/project",
    "mcpServers": [],
    "_meta": {
      "yoloMode": true
    }
  }
}
```

`session/new` `_meta` grok-build honors:

| Field | Meaning |
| --- | --- |
| `yoloMode` | Always-approve for this session |
| `autoMode` | Auto permission mode; ignored if always-approve is already on |
| `rules` | Extra rules appended to the system prompt |
| `systemPromptOverride` | Replacement system prompt |
| `agentProfile` | Profile name or JSON object |
| `sessionId` | Client-supplied UUID for the session |
| `modelId` | Starting model |
| `pluginDirs` | Extra plugin roots (gated by initialize `_meta`) |

`mcpServers` is the ACP list of stdio/HTTP/SSE MCP servers. Grok admits them,
merges them with on-disk MCP config for that `cwd`, and starts them for the
session. An empty array is valid.

Reply includes `sessionId`. Keep it for prompt, cancel, load, and mode/model
changes.

### 4. `session/prompt`

```json
{
  "method": "session/prompt",
  "params": {
    "sessionId": "<uuid>",
    "prompt": [{ "type": "text", "text": "List the files in this project" }]
  }
}
```

Assistant text does **not** arrive only in the result. It streams as
`session/update` notifications with `sessionUpdate: "agent_message_chunk"`.
The `session/prompt` result is completion metadata, including `stopReason`
(`end_turn`, `cancelled`, `refusal`, `max_tokens`, …).

Prompt content can include embedded context when
`promptCapabilities.embeddedContext` is true.

### 5. `session/update` (agent → host)

| `sessionUpdate` | Host action |
| --- | --- |
| `agent_message_chunk` | Append `content.text` to the answer |
| `agent_thought_chunk` | Reasoning stream; keep off the user transcript or dim it |
| `tool_call` | New tool (`title`, `kind`, `status`, input) |
| `tool_call_update` | Status or result for an in-flight tool |
| `plan` | Full plan snapshot; replace, do not merge |

Grok also sends namespaced notifications (`x.ai/fs_notify`,
`x.ai/session_notification`, fuzzy-search status, worktree status). A host
that does not understand a method must still reply to **requests** with
JSON-RPC `-32601`. Notifications can be ignored.

### 6. Reverse requests the host must answer

| Method | When |
| --- | --- |
| `session/request_permission` | Tool use while the session is not always-approve |
| `fs/read_text_file`, `fs/write_text_file` | If the host advertised those capabilities |
| `terminal/create`, `terminal/output`, `terminal/kill`, `terminal/wait_for_exit` | If the host advertised `terminal` |
| `x.ai/ask_user_question` (and `_x.ai/ask_user_question`) | Agent wants a structured question answered |

An unanswered reverse request hangs the turn. The OpenAgents TypeScript bridge
treats permission as a durable interaction; the Rust coder host auto-selects
an `allow*` option when ungated.

For unattended consumption, pass `--always-approve` or `_meta.yoloMode: true`
so permission reverse-calls do not fire. Deny rules and hooks still apply
inside Grok.

### 7. Session control

| Method | Notes |
| --- | --- |
| `session/cancel` | Stops the in-flight prompt. `_meta.cancelSubagents` defaults true. `_meta.cancelTrigger` is Grok-specific. |
| `session/load` | Restore by `sessionId` + `cwd` + `mcpServers`. Advertised via `loadSession`. |
| `session/resume` | Advertised on session capabilities (when chat-mode is off). |
| `session/close` / `session/list` | Advertised the same way. |
| `session/set_mode` | Grok mode ids: `default` (agent), `ask`, `plan`. Not Devin's `bypass` / `read-only`. |
| `session/set_model` | Uses ids from `initialize` `_meta.modelState`. |

Stdin EOF is supposed to shut the agent down. Grok has had Windows hang bugs
on persistent clients; hosts should still close stdin, wait, then SIGTERM.

## x.ai extension surface

Beyond stock ACP, grok-build serves `x.ai/*` methods from `ext_method` on
`MvpAgent`. The user guide groups them:

| Prefix | Examples |
| --- | --- |
| `x.ai/fs/*` | list, exists, read_file, write_file |
| `x.ai/git/*` | status, stage, commit, diffs, discard |
| `x.ai/git/worktree/*` | create, remove, apply, list, gc |
| `x.ai/search/*` | fuzzy/open, fuzzy/change, content |
| `x.ai/terminal/*` | create, kill, output, wait_for_exit |
| `x.ai/session/*` | fork, info, list, close, resolve_local_for_worktree_resume |
| `x.ai/auth/*` | get_url, submit_code, getApiKey, setApiKey |
| conversation | prompt_history, rewind, compact_conversation, recap |
| MCP | `x.ai/mcp/sdk_call` when initialize advertised `x.ai/mcp/sdk` |

The set grows across releases. Discover it from `initialize` and from
failed `-32601` replies. Do not treat a host that ignores extensions as
broken; treat a host that **does not reply** as broken.

OpenAgents' trusted Grok profile currently allowlists only
`x.ai/ask_user_question` and the underscore compatibility alias. Everything
else is quarantined, not granted.

## Minimal consumption recipe

This is the path grok-build's own tests and
`docs/grok-cli/examples/acp-stdio-hello.mjs` use:

1. `spawn("grok", ["agent", "--always-approve", "stdio"], { stdio: "pipe" })`
2. Drain stderr; do not parse it as protocol
3. `initialize` with protocolVersion 1
4. `authenticate` with `defaultAuthMethodId` or `cached_token` / `xai.api_key`, `_meta.headless: true`
5. `session/new` with `cwd`, `mcpServers: []`, optional `_meta.yoloMode: true`
6. `session/prompt` with a text block
7. Accumulate `agent_message_chunk` until the prompt result arrives
8. Close stdin, then kill the process group

A host that will resume later stores `sessionId` and calls `session/load`
on the next process (or keeps the process alive and prompts again).

Compatible public clients listed by grok-build: Zed, Neovim
(CodeCompanion / avante.nvim), Emacs agent-shell, marimo. JetBrains is
listed as coming.

## OpenAgents ACP host

OpenAgents has two stacks that both claim to be the ACP client. They do not
share code.

### A. TypeScript protocol packages (Desktop-era, still in tree)

Built to control Grok and Cursor from the former desktop app. Still the
strictest in-repo description of a Grok host.

| Package | Job |
| --- | --- |
| `@openagentsinc/agent-client-protocol` | Pinned schema, trusted peer profiles, Grok/Cursor extension allowlists |
| `@openagentsinc/agent-stdio-transport` | Bounded NDJSON stdio, generation-scoped lifecycle, no shell interpolation |
| `@openagentsinc/agent-client-runtime-bridge` | Native evidence + canonical projection + reverse-request authority |
| `@openagentsinc/agent-client-protocol-conformance` | Hermetic 23-method stable-wire oracles; optional live Grok/Cursor probes |
| `@openagentsinc/grok-harness` | Trusted `grok agent stdio` admission, authenticate policy, chat runtime |

The Grok trusted profile (`GROK_TRUSTED_PEER_PROFILE`) pins:

- executable `grok`, args `["agent", "stdio"]`
- env allowlist `HOME`, `XAI_API_KEY`
- auth methods `cached_token`, `xai.api_key`, `grok.com`, `oidc`
- restore via `session/load`
- cancel via `session/cancel`
- shutdown by disposing the process
- support state at most `experimental` until digest-bound live evidence exists

The production Grok harness refuses caller-supplied command arrays, prefers
cached-token, and stops before `authenticate` on the default cancellation
path. Interactive `grok.com` / `oidc` need an owner continuation.

These packages are not what `oa coder` runs today. The Electron desktop that
consumed them was deleted. They remain the written host contract and the
conformance authority.

### B. Rust CLI host (what the coder uses)

`crates/openagents-cli/src/acp.rs` is a hand-rolled JSON-RPC client. It was
written for Devin (`devin acp`): stream `tool_call`, `tool_call_update`,
`usage_update` (`cognition.ai/*` token fields), and `agent_message_chunk`.

Discovery (`crates/openagents-cli/src/coder/acp.rs`) reads the official ACP
registry (`ACP_REGISTRY` or
`~/work/projects/agentclientprotocol/repos/registry`). For each `agent.json`
it checks PATH / global npm / uv tools, then builds a launch command.

The coder `delegate` tool's `agent` parameter hands the whole task to one
discovered agent (`delegate_to_acp_agent` in `crates/openagents-cli/src/tools.rs`).
Events become `SubagentOutput` lines in the parent box. Session ids are kept
per agent id so a later `delegate` can `session/load`.

Handshake the Rust host actually sends:

1. `initialize` with `protocolVersion: 1` and `fs.readTextFile/writeTextFile: false`
2. **No `authenticate`**
3. `session/new` or `session/load` with `mcpServers: []`
4. Best-effort `session/set_mode`. Devin, OpenCode, and Gemini get
   `bypass` / `default` / `read-only`. Claude (`claude-agent-acp`) gets
   `bypassPermissions` / `default` / `plan`. Grok is in the default
   Devin map.
5. `session/prompt` with one text block
6. Auto-allow `session/request_permission` when ungated; JSON-RPC `-32601` for every other reverse request

Timeouts: 60s handshake, 900s per subsequent request. The child is its own
process group and is killed on every exit path.

## Gaps if OpenAgents consumes grok-build through the live coder path

These are mismatches against grok-build's agent as of `70ec060e`. The
TypeScript profile already names several of them; the Rust host does not
implement that profile.

| Gap | grok-build contract | OpenAgents Rust host |
| --- | --- | --- |
| Launch argv | `grok agent stdio` (registry: `npx … agent stdio`) | `launch_for` appends `acp` unless that token is already in args. For `grok-build` that yields `… agent stdio acp`, which is not ACP mode. |
| Binary name | `grok` | Discovery also looks for a command named `grok-build`. The shipped binary is `grok`. |
| Authenticate | Required after initialize | Never sent. A Grok child that needs a session token or API key will sit in `auth_required` or hang until the 60s handshake timeout. |
| Permission mode | `--always-approve`, `_meta.yoloMode`, or mode ids `default` / `ask` / `plan` | Claude already has its own `mode_id` map. Grok still gets Devin's `bypass` / `read-only`. A Grok agent that does not know `bypass` keeps ask mode and then permission-prompts. |
| Client capabilities | Optional fs + terminal reverse | Advertises fs false. Fine if Grok should use its own tools. Does not advertise `terminal`. |
| `session/update` | message, thought, tool_call, tool_call_update, plan | Handles `tool_call`, Devin `usage_update` (`cognition.ai/*`), `agent_message_chunk`. Drops `agent_thought_chunk`, `tool_call_update`, `plan`, and Grok usage `_meta`. |
| Token counts | Grok usage shape | Only `cognition.ai/inputTokens` / `outputTokens` |
| MCP inject | `session/new`.mcpServers merged into the session | Always `[]` |
| Extensions | Large `x.ai/*` surface including `ask_user_question` | Reverse handler unset → `-32601`. A Grok turn that asks a question gets "method not found". |
| Config overlay | `GROK_CONFIG` / `GROK_CONFIG_PATH` | Not set |
| Process env | Delegated children should not inherit host credentials | `delegate_to_acp_agent` leaves `env: None`, so the child inherits the coder process environment |
| Trusted launch | Profile forbids caller argv; pins basename, version, digest | Registry snapshot is used as a launch recipe, with `acp` appended |

The TypeScript Grok harness is closer to the grok-build contract (correct
argv, authenticate, experimental admission). It is not wired into
`oa coder` / `delegate`.

## What already works on the OpenAgents side

- Devin-shaped ACP over stdio: spawn, initialize, session, prompt, permission
  auto-allow, cancel-by-killing the process group, resume via `session/load`.
- Registry discovery of any agent.json that is installed locally.
- Streaming tool titles and answer chunks into the parent `delegate` box.
- One ACP child at a time per coder turn (`acp_spent`).
- Per-agent `session/set_mode` ids for Claude (`claude-agent-acp`), which
  is the pattern a Grok branch would follow.
- A documented, tested TypeScript host stack for `grok agent stdio` that
  still runs hermetic conformance and optional `GROK_ACP_LIVE` probes.
- A public-safe example client at `docs/grok-cli/examples/acp-stdio-hello.mjs`.

## What a Grok-capable OpenAgents host would change

Bounded, in launch order:

1. **Launch profile.** For id `grok-build` / executable `grok`, use
   `["agent", "stdio"]` and never append `acp`. Prefer `--always-approve`
   for unattended `delegate`.
2. **Authenticate.** After initialize, call `authenticate` with
   `defaultAuthMethodId` or `cached_token` / `xai.api_key` and
   `_meta.headless: true`. Surface `auth_required` instead of a generic hang.
3. **Mode.** Add a Grok branch next to the existing Claude `mode_id`
   map. Map OpenAgents `dangerous` → `--always-approve` / `yoloMode`
   (Grok has no `bypass` id), `prompt` → `ask`, and do not send
   `read-only` (Grok's `plan` is a different product mode).
4. **Updates.** Accept `agent_thought_chunk`, `tool_call_update`, and `plan`.
   Do not require Devin token `_meta`.
5. **Reverse.** Answer `x.ai/ask_user_question` (cancel or a single selected
   option). Keep other `x.ai/*` at `-32601` until a profile allowlists them.
6. **Env.** For delegated Grok, pass a scrubbed env (`HOME`, maybe
   `XAI_API_KEY` / `GROK_HOME`) rather than the coder's full environment.
7. **Reuse the TypeScript profile as the spec.** `GROK_TRUSTED_PEER_PROFILE`
   already pins argv, auth, restore, and cancel. Port those pins into the
   Rust host rather than growing a second recipe.

None of that is admitted by this document. It is the consumption contract
grok-build already implements, set next to the host OpenAgents actually runs.

## See also

- grok-build user guide: `crates/codegen/xai-grok-pager/docs/user-guide/15-agent-mode.md`
- grok-build config overlays: `…/docs/user-guide/05-configuration.md`
- grok-build agent trait: `crates/codegen/xai-grok-shell/src/agent/mvp_agent/acp_agent.rs`
- grok-build test client: `crates/codegen/xai-grok-test-support/src/acp_client.rs`
- ACP registry: `grok-build/agent.json` vs `devin/agent.json` vs `opencode/agent.json`
