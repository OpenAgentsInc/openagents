# forge.openagents.com — Stand-Up Spec (operationalize the built components)

> Status: stand-up / operationalization spec, 2026-06-28. Owner directive:
> stand up the owned coordination layer **immediately**. The FORGE-0 first wave
> (#6745) is built + merged but the components are libraries, not a running
> service. This spec is the concrete assembly + cutover plan to a live
> `forge.openagents.com`. Public-safe; no secrets.
> Companion: `docs/forge/2026-06-28-forge-openagents-com-owned-coordination-layer-audit.md`
> (the why + architecture) and `docs/forge/origin.md`.

## What already exists (FORGE-1..6, merged on main)

Real, tested components — grounded:

- **Coordination source-of-truth (D1).** `apps/openagents.com/workers/api/src/forge-coordination-store.ts` + migration `0251_forge_coordination_source_of_truth.sql` — work records (issues), change records (PRs), status, leases, virtual-merge-queue rows.
- **Git intake parser.** `apps/pylon/src/git-receive-pack.ts` — parses `git-receive-pack` so we own commit intake.
- **Packfile archive (R2).** `apps/openagents.com/workers/api/src/forge-git-packfile-archive-store.ts` + `0252_forge_git_packfile_archives.sql` — packfile blobs in R2, refs/metadata in D1.
- **Tenant git auth.** `apps/openagents.com/workers/api/src/forge-tenant-git-auth-store.ts` + `0253_forge_tenant_git_access_tokens.sql` — token-scoped per-tenant git access (multi-tenant / AaaS).
- **Dispatch protocol.** `apps/pylon/src/forge-dispatch-protocol.ts` + shared `packages/forge-protocol/` — typed Pylon→Forge task dispatch.
- **Verification runner.** `apps/pylon/src/forge-verification-runner.ts` — Docker-isolated `bun test` executor for untrusted/external code.
- **Gates already shared.** `apps/pylon/src/blueprint-gates/` (virtual-merge-queue, merge-deploy-gate, issue-close-safe, command-execution-source-verified) + `@openagentsinc/blueprint-contracts`.
- **Web surface.** `apps/openagents.com/apps/web/src/page/loggedIn/page/forge.ts`.
- Bindings already present in `apps/openagents.com/workers/api/wrangler.jsonc`: R2 (`ARTIFACTS`), D1 (`openagents-autopilot`), Durable Objects.

## The gap (why it isn't "stood up" yet)

The pieces are stores + parsers + protocols with no **control plane** binding them into a running service:

1. No HTTP **forge control-plane routes** exposing the coordination store / git intake / dispatch / verification.
2. No `forge.openagents.com` route/host wiring (or `/api/forge/*` surface) to the Worker.
3. The virtual merge queue + Blueprint gates are not wired to the coordination store to drive **`nextActualPromotion`** (owned merge authority).
4. No **GitHub mirror worker** (push promoted commits downstream so GitHub stays a read-only mirror).
5. The fleet/supervisor still coordinates through GitHub, not Forge.

## Stand-up architecture (assembly)

```
 agent/pylon ──git push──► [Forge git intake]  (git-receive-pack.ts)
                                │  tenant-auth (forge-tenant-git-auth-store)
                                ▼
                     [Packfile archive: R2]  (forge-git-packfile-archive-store)
                                │  refs+metadata
                                ▼
                 [Coordination store: D1]  (forge-coordination-store)
                  work/change/status/lease/merge-queue rows
                                │
                 [Verification runner]  (forge-verification-runner, Docker-isolated bun test)
                                │  verdict → status
                                ▼
            [Virtual merge queue + Blueprint gates]  (blueprint-gates/*)
              merge-deploy-gate · issue-close-safe · command-verified
                                │  nextActualPromotion (fast-forward)
                                ▼
                    [GitHub mirror worker]  ──push──► GitHub (read-only mirror)
```

All of it served from the `apps/openagents.com` Worker (existing R2/D1/DO bindings) under `forge.openagents.com` (host route) and/or `/api/forge/*`.

## Stand-up sequence (smallest-first, each shippable + green)

- **SU-1 — Control-plane routes (P0, do first).** Expose the coordination store + tenant-auth as `/api/forge/*` Worker routes: create/read work records, change records, status transitions, leases; auth via tenant git tokens. Register in the route registry/OpenAPI (so `route_exists` grounding sees them). Acceptance: an authed caller can create a work record + a change record + transition status through the live API; rows land in D1.
- **SU-2 — Git intake → archive → coordination.** Wire a receive-pack intake endpoint that parses the push (`git-receive-pack.ts`), archives the packfile to R2 (`forge-git-packfile-archive-store`), and writes change/ref rows to the coordination store. Acceptance: a real `git push` to the forge endpoint lands a packfile in R2 + a change record in D1.
- **SU-3 — Owned merge authority.** Wire the virtual merge queue + the Blueprint gates (merge-deploy-gate, issue-close-safe, command-verified, the anti-#6719 deletion guard) over the coordination store to compute **`nextActualPromotion`** — promotion is a gated fast-forward, never an O(N) PR merge. Acceptance: two concurrent change records serialize through the queue with the gates enforced; a deletion-poisoned change is blocked structurally (the #6719 class becomes impossible).
- **SU-4 — Verification on intake.** Run `forge-verification-runner` (Docker-isolated `bun test`) on each change before it's promotable; verdict → status. Acceptance: a failing change cannot reach `nextActualPromotion`.
- **SU-5 — GitHub mirror worker.** A one-way worker that pushes promoted commits to GitHub as a read-only mirror (keeps external visibility; removes GitHub from the critical path). Acceptance: a Forge-promoted commit appears on GitHub `main` via the mirror, not via a PR.
- **SU-6 — Dogfood one fleet lane.** Point ONE codex/Pylon lane at Forge (intake→verify→queue→promote→mirror) end-to-end; prove zero GitHub PR contention. Then widen the cutover lane-by-lane. Acceptance: a real fleet change is coordinated entirely through Forge, GitHub only mirrored.
- **SU-7 — Multi-tenant / AaaS.** Per-tenant token namespaces + Artifacts/relay isolation so external fleets connect (the real zero-spend throughput multiplier). Gated on the software-solid bar (48h zero-wedge, #6486/#6643 isolation prod-proven, #6640 dashboard live).

## Routing / deploy

- Add the `forge.openagents.com` custom-domain route to the `apps/openagents.com` Worker (or serve `/api/forge/*` first, add the host later). Bindings already exist (R2 `ARTIFACTS`, D1 `openagents-autopilot`, DOs) in `wrangler.jsonc`.
- Deploy via the standard `deploy:safe` gate; SU-1..SU-5 ship incrementally behind the route, dogfooded (SU-6) before any external exposure.

## Governance / safety (non-negotiable)

- Every promotion runs the **Blueprint gates** (merge-deploy-gate, issue-close-safe, command-execution-source-verified, operator-grounded-assertion) + the **anti-#6719 deletion guard** — the 119-duplicate-PR night and the stale-base mass-deletion are made *structurally* impossible, not policy-discouraged.
- Tenant isolation (SU-7) must pass the adversarial security harness (#6643) in prod before any external fleet is admitted.
- Own-capacity / $0; no external paid spend.

## Acceptance for "stood up"

A real change flows **agent → forge git intake → R2 archive → D1 coordination → Docker verification → gated virtual-merge-queue promotion → GitHub mirror**, with the fleet dogfooding at least one lane and GitHub demoted to a mirror. SU-1..SU-6 green + deployed = the owned coordination layer is live.

---

*Stand-up spec authored 2026-06-28 at owner direction; informed to Artanis. The
FORGE-0 wave-1 components (#6746-#6752) are the substrate; this is their
operational assembly into `forge.openagents.com`.*
