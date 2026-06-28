# forge.openagents.com — Stand-Up Spec (operationalize the built components)

> Status: stand-up / operationalization spec, 2026-06-28. Owner directive:
> stand up the owned coordination layer **immediately**. The FORGE-0 first wave
> (#6745) is built + merged but the components are libraries, not a running
> service. This spec is the concrete assembly + cutover plan to a live
> `forge.openagents.com`. Public-safe; no secrets.
> Update, 2026-06-28: the `apps/forge/` deploy bootstrap is live via #6759.
> `forge.openagents.com` now serves the separate Forge Worker/app landing page.
> The next filed stand-up slices are #6768 (SU-0 boundary lock), #6769 (SU-1B
> shell beyond landing), #6770 (SU-2 control-plane routes), and #6771 (SU-3 git
> intake wiring).
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
- **Separate Forge UI app bootstrap.** `apps/forge/` deploys the
  `openagents-forge` Worker to `forge.openagents.com`, reusing
  `@openagentsinc/ui` tokens while staying outside the main
  `openagents.com` logged-in route tree. #6759 shipped the first landing page
  with the exact copy `THE FORGE` / `where agents git it on`.
- **Legacy/source-material web surface.** `apps/openagents.com/apps/web/src/page/loggedIn/page/forge.ts` is older Forge dashboard material inside the main product app. It can be mined for copy/layout ideas, but it is not the target `forge.openagents.com` UI.
- Bindings already present in `apps/openagents.com/workers/api/wrangler.jsonc`: R2 (`ARTIFACTS`), D1 (`openagents-autopilot`), Durable Objects.

## The gap (why it isn't "stood up" yet)

The pieces are stores + parsers + protocols with no **control plane** binding them into a running service:

1. No HTTP **forge control-plane routes** exposing the coordination store / git intake / dispatch / verification.
2. The separate `apps/forge/` app/deploy exists, but it is still the bootstrap
   landing page. It has no live work queue, change inspector, verification
   state, merge queue, git/ref explorer, or `/api/forge/*` API integration yet.
3. The virtual merge queue + Blueprint gates are not wired to the coordination store to drive **`nextActualPromotion`** (owned merge authority).
4. No **GitHub mirror worker** (push promoted commits downstream so GitHub stays a read-only mirror).
5. The fleet/supervisor still coordinates through GitHub, not Forge.

## Product/UI boundary

Forge UI is a separate product surface, not another logged-in
`openagents.com` page.

- **Target app:** create `apps/forge/` for `forge.openagents.com`, following the
  same extraction pattern as `apps/forum`: separate app shell, routes, app model,
  release cadence, and deployment wiring.
- **Shared components:** reuse `@openagentsinc/ui`, shared tokens, and Foldkit
  primitives for the base visual language. Forge-specific work queues, change
  inspectors, merge/verification views, git-ref explorers, and operator controls
  may evolve in `apps/forge/` or a later `@openagentsinc/forge-ui` package once
  they stop being generic.
- **Main app relationship:** `openagents.com` may link to Forge and may keep
  shared auth/API infrastructure at first, but it must not own Forge page routing,
  navigation state, or the canonical coordination UX.
- **Old Forge page:** treat
  `apps/openagents.com/apps/web/src/page/loggedIn/page/forge.ts` as historical
  source material. Do not expand it as the new Forge UI.

## Stand-up architecture (assembly)

```
 operator/agent browser ──► [Forge UI app: apps/forge on forge.openagents.com]
                                │  shared @openagentsinc/ui components
                                ▼
                     [Forge API/control plane: /api/forge/*]
                                │
 agent/pylon ──git push──► [Forge git intake]  (git-receive-pack.ts)
                                │  tenant git auth (git protocol only)
                                ▼
                     [Packfile archive: R2]  (audit/evidence artifact)
                                │
                                ▼
                 [Canonical git object/ref store]  (source refs, ref locks)
                                │  refs+metadata
                                ▼
                 [Coordination store: D1]  (forge-coordination-store)
                  work/change/status/lease/merge-queue rows
                                │
                 [Verification dispatch]  (Worker enqueue)
                                │
                                ▼
                 [Pylon/runner execution]  (Docker-isolated bun test)
                                │  signed receipt + verdict → status
                                ▼
            [Virtual merge queue + Blueprint gates]  (blueprint-gates/*)
              merge-deploy-gate · issue-close-safe · command-verified
                                │  nextActualPromotion (fast-forward)
                                ▼
                    [GitHub mirror worker]  ──push──► GitHub (read-only mirror)
```

The API/control plane can initially live in the `apps/openagents.com` Worker to
reuse existing R2/D1/DO bindings. The Forge UI itself lives in `apps/forge/` and
is served on `forge.openagents.com`. It calls `/api/forge/*`; the API may later
move behind the same host or into its own Worker once the boundary is proven.

## Stand-up sequence (smallest-first, each shippable + green)

- **SU-0 — Boundary/spec lock (P0, do first).** Freeze the execution boundary, auth model, canonical git object/ref store, receipt format, and UI app boundary before adding routes. Tenant git tokens are for smart Git HTTP only; control-plane calls use dedicated `forge:*` service/session/admin scopes. The R2 packfile archive is evidence, not the canonical ref store. Verification receipts include the change id, base/head refs, packfile digest, executor identity, command, exit code, timestamps, artifact refs, and log digest. Acceptance: docs/OpenAPI route notes name these boundaries and no route uses git tokens as control-plane auth.
- **SU-1 — Separate Forge UI shell.** Stand up `apps/forge/` for `forge.openagents.com`, reusing `@openagentsinc/ui` and shared tokens while owning its own app shell, navigation, queue/change/work inspectors, and route model. #6759 shipped the deploy bootstrap and landing page; #6769 tracks the next shell slice beyond the landing page. Acceptance: `forge.openagents.com` renders the Forge shell from the Forge API contract, and the old `openagents.com` logged-in Forge page is not the expansion target.
- **SU-2 — Control-plane routes.** Expose the coordination store as `/api/forge/*` Worker routes: create/read work records, change records, status transitions, leases, queue state, verification receipts, and promotion decisions. Register in the route registry/OpenAPI so `route_exists` grounding sees them. Acceptance: an authed control-plane caller can create a work record + a change record + transition status through the live API; rows land in D1.
- **SU-3 — Git intake → archive → canonical refs → coordination.** Wire a smart-Git receive-pack intake endpoint that parses the push (`git-receive-pack.ts`), archives the packfile to R2 (`forge-git-packfile-archive-store`), verifies/applies it to the canonical git object/ref store under a ref lock, and writes change/ref rows to the coordination store. Acceptance: a real `git push` to the forge endpoint lands a packfile in R2, updates the canonical git object/ref store, and creates a change record in D1.
- **SU-4 — Owned merge authority.** Wire the virtual merge queue + the Blueprint gates (merge-deploy-gate, issue-close-safe, command-verified, the anti-#6719 deletion guard) over the coordination store and canonical refs to compute **`nextActualPromotion`** — promotion is a gated ref fast-forward, never an O(N) PR merge or metadata-only D1 flip. Acceptance: two concurrent change records serialize through the queue with the gates enforced; a deletion-poisoned change is blocked structurally (the #6719 class becomes impossible).
- **SU-5 — Verification on intake.** The Worker enqueues verification for each change; Pylon/owned runner executes `forge-verification-runner` (Docker-isolated `bun test`) and posts a receipt + verdict back to Forge before the change is promotable. Acceptance: a failing change cannot reach `nextActualPromotion`, and the D1 status is backed by a receipt artifact.
- **SU-6 — GitHub mirror worker.** A one-way worker that pushes promoted commits to GitHub as a read-only mirror (keeps external visibility; removes GitHub from the critical path). Acceptance: a Forge-promoted commit appears on GitHub `main` via the mirror, not via a PR.
- **SU-7 — Dogfood one fleet lane.** Point ONE codex/Pylon lane at Forge (intake→verify→queue→promote→mirror) end-to-end; prove zero GitHub PR contention. Then widen the cutover lane-by-lane. Acceptance: a real fleet change is coordinated entirely through Forge, the separate Forge UI shows the live queue/change state, and GitHub is only mirrored.
- **SU-8 — Multi-tenant / AaaS.** Per-tenant token namespaces + Artifacts/relay isolation so external fleets connect (the real zero-spend throughput multiplier). Gated on the software-solid bar (48h zero-wedge, #6486/#6643 isolation prod-proven, #6640 dashboard live).

## Routing / deploy

- Maintain the `forge.openagents.com` custom-domain route/deploy for
  `apps/forge/`. The Forge app consumes the shared `@openagentsinc/ui` package, not the
  `apps/openagents.com` logged-in route tree.
- Serve `/api/forge/*` from the `apps/openagents.com` Worker first if that is
  the fastest route to reuse existing bindings (R2 `ARTIFACTS`, D1
  `openagents-autopilot`, DOs) in `wrangler.jsonc`; keep the UI deploy separate.
- Deploy via the standard `deploy:safe` gate; SU-1..SU-6 ship incrementally
  behind the route, dogfooded (SU-7) before any external exposure.

## Governance / safety (non-negotiable)

- Every promotion runs the **Blueprint gates** (merge-deploy-gate, issue-close-safe, command-execution-source-verified, operator-grounded-assertion) + the **anti-#6719 deletion guard** — the 119-duplicate-PR night and the stale-base mass-deletion are made *structurally* impossible, not policy-discouraged.
- Tenant isolation (SU-8) must pass the adversarial security harness (#6643) in prod before any external fleet is admitted.
- Own-capacity / $0; no external paid spend.

## Acceptance for "stood up"

A real change flows **agent → forge git intake → R2 archive → canonical git
object/ref store → D1 coordination → Pylon/runner Docker verification → gated
virtual-merge-queue promotion → GitHub mirror**, with the separate Forge UI
showing live work/change/queue state, the fleet dogfooding at least one lane,
and GitHub demoted to a mirror. SU-0..SU-7 green + deployed = the owned
coordination layer is live. SU-8 is the external multi-tenant expansion gate.

## Live issue map

- #6759 — SU-1 bootstrap: separate `apps/forge/` Worker, custom-domain deploy,
  and basic `THE FORGE` landing page. Closed after production verification.
- #6768 — SU-0 boundary/spec lock.
- #6769 — SU-1B Forge UI shell beyond the landing page.
- #6770 — SU-2 `/api/forge/*` control-plane routes.
- #6771 — SU-3 smart-Git intake to archive/canonical refs/coordination records.

---

*Stand-up spec authored 2026-06-28 at owner direction; informed to Artanis. The
FORGE-0 wave-1 components (#6746-#6752) are the substrate; this is their
operational assembly into `forge.openagents.com`.*
