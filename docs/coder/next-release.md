# Waiting for the next Coder release

Changes that only reach people through a new Coder binary. While the binary
is frozen at 1.0.0 ([1.0-freeze.md](1.0-freeze.md)) they wait here. Each
entry names its issue. Feature work stays open while it waits. Security
fixes can land under the freeze exception; their remaining release
decisions are recorded in `NEEDS_OWNER.md`.

| Change | Issue | Why it needs a new binary |
| --- | --- | --- |
| Source security fix: hand a user's Vertex credential to Claude Code through a sealed anonymous memfd | #11771 | The launch script is built by the user's own `coder` (`coder remote delegate --on boat\|gce`); 1.0.0 retains the old script until the owner approves a release |
| Call the Decisions API at `https://api.openagents.com/v1/decisions` (`jev::decisions::OPENAGENTS` is `https://openagents.com/api` in 1.0.0) | — | The address is compiled into `coder`; the old one keeps working meanwhile |
| Call the rest of the API at `https://api.openagents.com/v1` instead of `{origin}/v1/...` on the sign-in origin `https://openagents.com` (keep the sign-in origin for pages a person opens: `/device`, `/settings`, `/work/...`). In 1.0.0: `coder-delegate` `GENERATION_BASE_URL` (`POST /v1/responses`); `coder-new/src/free_tier.rs` (`/v1/agents/challenge`, `/v1/agents/register`, `/v1/chat/completions`); `coder-new/src/connection_tools.rs` (`/v1/connections...`); `coder-sdk/src/cloud_solver.rs` (`/v1/work...`); `openagents-login` (`/v1/device/code`, `/token`, `/sign-out`, `/v1/keys`) | — | The addresses are compiled into `coder`; `openagents.com/v1` keeps answering (marked `Deprecation`, docs/api/README.md "API address"). Move only after api.openagents.com's load balancer sends `/v1/...` to the site (`deploy/production/api-url-map.yaml`) |
| Consume the public Computers SDK for remote allocation, commands/files, and new computer/agent tools; preserve explicit placement, task identity, grants, and retry handles | [#11823](https://github.com/OpenAgentsInc/openagents/issues/11823) | Launch scripts and command/tool surfaces are compiled into the frozen Coder programs; public API/client/web changes ship separately without a new binary |
