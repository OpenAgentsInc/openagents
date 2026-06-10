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

## BYOK setup (bring your own key — always)

The bridge uses your own credentials, never platform-supplied ones:

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
  by the executor gate (#4719); they cap, never expand, what an
  assignment may do. Config is preference, not authority.
- Never put credential values in the config file; the bridge reads
  credentials from the environment only.

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

The probe, capability declaration, and config surface exist (#4718).
The executor gate (#4719) and the dispatch work class plus the
packaged-binary bounded real-task smoke (#4720) are required before any
copy can claim a Pylon executed coding work with a local Claude.
