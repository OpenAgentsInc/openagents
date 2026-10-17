# Coder 1.0.0 release work

> **2026-10-16: the freeze is lifted.** Every row below is in scope for the
> big-bang 1.0.0 on Monday 2026-10-19
> ([brief](../audits/2026-10-16-coder-1.0-big-bang.md)). Remove a row when
> it lands. After launch, this page lists work for the release after 1.0.0.

These changes reach people through a new Coder binary. The release agent
tracks them for the Monday build and removes each row as its work lands.
Owner-only qualification steps stay in `NEEDS_OWNER.md`.

| Change | Issue | Why it needs a new binary |
| --- | --- | --- |
| Source security fix: hand a user's Vertex credential to Claude Code through a sealed anonymous memfd | #11771 | The source fix is on `main` in `cd193284a8`. The Monday build must include it because the user's own `coder` builds the launch script (`coder remote delegate --on boat\|gce`) |
| Call the Decisions API at `https://api.openagents.com/v1/decisions` (`jev::decisions::OPENAGENTS` is `https://openagents.com/api` in 1.0.0) | — | The address is compiled into `coder`; the old one keeps working meanwhile |
| Call the rest of the API at `https://api.openagents.com/v1` instead of `{origin}/v1/...` on the sign-in origin `https://openagents.com` (keep the sign-in origin for pages a person opens: `/device`, `/settings`, `/work/...`). In 1.0.0: `coder-delegate` `GENERATION_BASE_URL` (`POST /v1/responses`); `coder-new/src/free_tier.rs` (`/v1/agents/challenge`, `/v1/agents/register`, `/v1/chat/completions`); `coder-new/src/connection_tools.rs` (`/v1/connections...`); `coder-sdk/src/cloud_solver.rs` (`/v1/work...`); `openagents-login` (`/v1/device/code`, `/token`, `/sign-out`, `/v1/keys`) | — | The addresses are compiled into `coder`; `openagents.com/v1` keeps answering (marked `Deprecation`, docs/api/README.md "API address"). Move only after api.openagents.com's load balancer sends `/v1/...` to the site (`deploy/production/api-url-map.yaml`) |
| Consume the public Computers SDK for remote allocation, commands/files, and new computer/agent tools; preserve explicit placement, task identity, grants, and retry handles | [#11823](https://github.com/OpenAgentsInc/openagents/issues/11823) | Launch scripts and command/tool surfaces are compiled into the Coder programs; public API, client, and web changes must ship with their consumers |
| Project-scoped computer sharing for a second account through NIP-HOST and NIP-REACH | [#11911](https://github.com/OpenAgentsInc/openagents/issues/11911) | Existing host grants apply across the computer. Narrowing them to selected projects requires host checks on reads, task execution, file access, room admission, and revocation before a guest grant can be offered |
