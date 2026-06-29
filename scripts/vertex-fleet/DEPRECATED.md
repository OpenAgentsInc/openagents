# vertex-fleet — RETIRED 2026-06-20

**Do not launch new Vertex batches.** This fleet (Anthropic-Claude-on-Vertex) is
retired.

**Why:** Anthropic Claude on Google Vertex AI is a third-party (partner-model)
SKU that is **not covered by the GFS credit**, so every Vertex batch was direct
card spend (metered per input/output token via Google Cloud billing).

**Superseded by:** [`scripts/codex-fleet/`](../codex-fleet/) — the same
PR-per-agent fleet, but the engine is `codex exec` running on the OpenAgents
**ChatGPT/Codex subscription** (no per-token card spend), and auth is pulled from
the **central device-flow provider-account store** in openagents.com (no
per-machine interactive login). It produces the same shape of PRs (branch prefix
`codex-fleet/<promise>` instead of `vertex-fleet/<promise>`), so the existing
merge gate `/tmp/fleet-merge.sh` still gates it unchanged.

**This directory is kept for history only** (the scripts still work if Vertex is
ever explicitly re-authorized by the owner). Route new fleet work to
`scripts/codex-fleet/`.

See:

- `scripts/codex-fleet/README.md` — usage + how the central device-flow auth works.
- `apps/openagents.com/docs/2026-06-05-chatgpt-device-login-operator-runbook.md`
  — the central ChatGPT/Codex device-login system this fleet authenticates against.
