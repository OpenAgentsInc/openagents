# The Local Claude Agent Bridge

Promise: `pylon.local_claude_agent_bridge.v1` (registry `2026-06-10.21`).
Design audit: `docs/autopilot-coder/2026-06-10-claude-agent-sdk-local-claude-pylon-audit.md`
(workspace root). Issues: #4717 (epic), #4718 (this doc's surface), #4719
(executor gate), #4720 (dispatch + smoke).

Pylon can talk to your local Claude: when this lane is fully built, the
Pylon worker loop hands a coding assignment to the Claude Agent SDK
running on your machine, with your credentials, in a bounded workspace,
and returns public-safe closeout refs. This document covers the bridge's
readiness probe, capability declaration, configuration, and the rules
that bind the lane.

## What ships in this surface (#4718)

- `@anthropic-ai/claude-agent-sdk` as an **optional dependency** with a
  **lazy import**: every Pylon command works when the SDK or its
  platform binary is absent. A Pylon without the SDK simply never
  declares the capability.
- `probeClaudeAgentReadiness()` (`src/claude-agent.ts`): reports one of
  `ready`, `sdk_missing`, `credentials_missing`, `platform_unsupported`,
  or `disabled_by_config`. The probe checks **presence only** — it never
  reads, logs, or persists credential values.
- Capability declaration: `pylon provider go-online` declares
  `capability.pylon.local_claude_agent` when and only when the probe
  reports `ready`, and strips a stale declaration when it does not. The
  capability ref is the only public signal that a local Claude exists;
  no machine details, paths, or account identifiers leave the device.
- Local dashboard composer: the source TUI can use the Claude Agent SDK as
  the interactive composer backend when `dev.defaultAdapter` is
  `claude_agent` or the process is launched with `--adapter claude`. This is
  the owner-supervised local dev surface, separate from the delegated
  assignment executor.

## BYOK setup (bring your own key — always)

The bridge uses your own credentials, never platform-supplied ones.

**If you are logged in to Claude Code on this machine, no setup is
required.** The probe detects your local Claude session (presence only:
the credentials file or the macOS keychain entry — values are never
read) and reports
`credential.source.claude_agent.local_claude_session`; the bundled SDK
binary reuses that same session for inference. This is still BYOK —
the session is your own subscription and you pay for your own
inference from it.

Otherwise, export an API key (an env key takes precedence over the
local session when both exist):

```sh
export ANTHROPIC_API_KEY=your-api-key   # from platform.claude.com
```

or one of the provider switches the SDK supports:

- `CLAUDE_CODE_USE_BEDROCK=1` plus AWS credentials
- `CLAUDE_CODE_USE_VERTEX=1` plus Google Cloud credentials
- `CLAUDE_CODE_USE_FOUNDRY=1` plus Azure credentials
- `CLAUDE_CODE_USE_ANTHROPIC_AWS=1` plus `ANTHROPIC_AWS_WORKSPACE_ID`
  and AWS credentials

Cost honesty: you pay for your own inference. Note that starting
2026-06-15, Agent SDK usage on Claude subscription plans draws from a
separate monthly Agent SDK credit. OpenAgents does not supply, proxy, or
broker Claude access, login, or rate limits — Anthropic's terms forbid
third parties offering claude.ai login, and the bridge is BYOK by policy.

## Configuration

Optional `claudeAgent` section in the Pylon config file
(`~/.pylon/config.json` by default):

```json
{
  "claudeAgent": {
    "enabled": true,
    "model": "claude-fable-5",
    "maxTurns": 12,
    "timeoutSeconds": 600
  }
}
```

- `enabled: false` disables the lane regardless of SDK/key presence.
- `model`, `maxTurns`, `timeoutSeconds` are runtime preferences consumed
  by the executor gate (#4719) and the local dashboard composer; they cap,
  never expand, what an assignment may do. Config is preference, not
  authority.
- Permission/execution-mode keys are deliberately not read from this
  section: the assignment lane is always bounded. Only the local-only
  `dev` section can change the composer's posture (#4845).
- Never put credential values in the config file; the bridge reads
  credentials from the environment only.

To make Claude/Fable the local dashboard composer default:

```json
{
  "dev": {
    "defaultAdapter": "claude_agent"
  },
  "claudeAgent": {
    "enabled": true,
    "model": "claude-fable-5"
  }
}
```

Per launch, `--adapter claude` selects the same backend without changing the
config file. The composer runs in the active repo (`PYLON_ACTIVE_REPO` or
`PYLON_CODEX_CWD` override the shell cwd), starts only after
`probeClaudeAgentReadiness()` reports ready, labels the TUI as `Claude` or
`Claude (<model>)`, and keeps the raw SDK session id local so follow-up
prompts can resume the same conversation. Feed/footer output uses hashed
session refs, not raw session ids.

## Local-only supervised permissive mode (#4845)

The Claude equivalent of the Codex `danger-full-access` composer mode is a
permission-system concept, not an OS sandbox:

- `local_bounded` (default): tool allowlist
  (`Read, Edit, Write, Bash, Glob, Grep`), `permissionMode: "acceptEdits"`,
  `settingSources: []`.
- `local_supervised_danger`: SDK `permissionMode: "bypassPermissions"`, no
  tool allowlist, and `settingSources: ["project"]` — the owner is watching
  their own checkout and wants their own `CLAUDE.md`/`.claude` instruction
  layers active. (The bounded lane keeps executor-style isolation; this
  asymmetry is deliberate and recorded on #4845.)

Explicit opt-in only, mirroring `--codex-danger`:

```json
{
  "dev": {
    "claudeExecutionMode": "local_supervised_danger"
  }
}
```

or `pylon --claude-danger` / `pylon dev --claude-danger`. The TUI labels the
backend `Claude DANGER` and the status line shows
`mode: local_supervised_danger | permissions: bypassPermissions`.
`pylon work`, `pylon assignment`, `pylon provider`, `pylon node`, and
`pylon attach` reject `--claude-danger` with typed blocker
`blocker.claude.local_supervised_danger_public_path`. The assignment executor
stays bounded regardless of dev config: `loadClaudeAgentConfig()` never reads
a permissive mode, and requesting `bypassPermissions` without the execution
mode is a typed error
(`blocker.claude.local_supervised_danger_requires_opt_in`).

## Boundaries

- **Your identity, your key, your machine.** The lane acts only with the
  local user's credentials; no platform keys on devices, ever.
- **Redaction law.** Raw SDK messages, prompts, file contents, session
  JSONL, provider payloads, and local paths never leave the device.
  Closeouts carry hashed refs only, through the existing
  `assertPublicProjectionSafe` boundary. SDK session files are
  operator-local evidence.
- **Authority unchanged.** Worker closeout is not accepted work; the
  lane grants no settlement, payout, deploy, spend, or Forum publication
  authority.
- **Copy law.** This lane is "Claude Agent" / "your local Claude" /
  "Powered by Claude" in any user-facing copy — never "Claude Code"
  (Anthropic branding terms). Nothing about this lane may be described
  as shipped or autonomous before its receipts exist; the promise's
  `unsafeCopy` binds.

## Current status

The probe, capability declaration, config surface, bounded executor gate,
dispatch work class, CI-safe packaged-binary smoke, and first production
live-device leg are built and receipt-backed (#4718, #4719, #4720, #4755).
The #4755 leg ran a deployed no-spend `claude_agent_task` assignment on an
operator-credentialed contributor machine with local-session Claude credentials
and posted public-safe closeout ref
`assignment.closeout.ae84ca67ada1584130b823d5`.

The #4756 proof boundary is also receipt-backed: production work order
`autopilot_work_order.46dc8c38-04c5-4f1c-9814-f35bfc00e7c3` carried
`claude_agent_task` plus `git_checkout` through API submission, own-Pylon
placement, local Claude Agent execution, independent `bun test` verification,
and public-safe delivered closeout
`assignment.closeout.2dc83bdc0d8481ebba14621e` against public repo
`AtlantisPleb/openagents-b2-git-checkout-fixture-20260611144040` at pinned
commit `1745cd4b54b8a12a50922f80b5d345314c91d70d`.
