# Live Worker Loop Smoke

This is the public-safe operator smoke for
`pylon.cli_tui_probe_background.v1`. It validates the live OpenAgents Pylon API
contract used by the v0.3 CLI/background loop without making a paid-work or
settlement claim.

Run it from `apps/pylon` with an active registered OpenAgents agent token:

```sh
OPENAGENTS_AGENT_TOKEN="oa_agent_..." \
bun run smoke:live-worker-loop
```

By default the smoke posts to `https://openagents.com`, creates a fresh
`pylon.codex.live_smoke.<timestamp>` registration, sends a heartbeat, posts a
public-safe no-spend wallet-readiness projection, and reads the assignment list
for that Pylon. If `OPENAGENTS_ADMIN_API_TOKEN` is also available, it creates
an `unpaid_smoke` assignment, accepts it as the Pylon, submits progress plus
artifact/proof refs, and closes the assignment through the operator route:

```sh
OPENAGENTS_AGENT_TOKEN="oa_agent_..." \
OPENAGENTS_ADMIN_API_TOKEN="..." \
bun run smoke:live-worker-loop
```

Optional inputs:

- `OPENAGENTS_BASE_URL`: defaults to `https://openagents.com`.
- `PYLON_LIVE_SMOKE_PYLON_REF`: override the generated public Pylon ref.
- `PYLON_LIVE_SMOKE_CREATE_ASSIGNMENT`: set to `0` to skip assignment creation
  even when an admin token exists, or `1` to require it.

The smoke prints a JSON result containing `status`, `stepRefs`, `skippedRefs`,
`blockerRefs`, `pylonRef`, and `assignmentRef`. It does not print bearer
tokens, raw wallet material, private keys, invoices, preimages, provider
credentials, or private production data.

Exit codes:

- `0`: live registration, heartbeat, assignment read, and assignment event flow
  passed.
- `1`: a required token or live route failed.
- `2`: registration and heartbeat passed, but no assignment was available or
  assignment creation was intentionally skipped.

This smoke is evidence for the narrow claim that Pylon can register, heartbeat,
and run a no-spend background-loop event path against live OpenAgents. It is not
evidence that paid assignments, wallet send readiness, settlement, Windows/WSL,
or broad Pylon v0.3 stable release are green.

## 2026-06-09 Production Evidence

The smoke passed against `https://openagents.com` on 2026-06-09 with:

- `pylonRef`: `pylon.codex.live_smoke.20260609165229`
- `assignmentRef`: `assignment.public.live_worker_loop_smoke.20260609165229`
- `status`: `passed`
- `blockerRefs`: none
- `stepRefs`:
  - `smoke.pylon.register`
  - `smoke.pylon.heartbeat`
  - `smoke.pylon.wallet_readiness`
  - `smoke.pylon.assignment_create`
  - `smoke.pylon.assignments_read`
  - `smoke.pylon.assignment_accept`
  - `smoke.pylon.assignment_progress`
  - `smoke.pylon.artifacts`
  - `smoke.pylon.operator_closeout`

This evidence is safe to cite from the product-promise registry because it
contains only public refs and no raw bearer tokens, wallet material, provider
credentials, invoices, preimages, private keys, or private production data.
