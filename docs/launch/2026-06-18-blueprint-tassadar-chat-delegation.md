# Delegation: Build the Blueprint × Tassadar chat (EPIC #5449)

> Self-contained delegation for a fresh coding agent with **zero prior context**.
> Companion spec: [`2026-06-18-autopilot-tassadar-chat-blueprint-audit.md`](2026-06-18-autopilot-tassadar-chat-blueprint-audit.md).

## Mission
Build a **chat-centric Autopilot interface** where, once a user is past onboarding, the
default screen is a **Blueprint-driven chat**. The chat's exact computations are performed
by **Tassadar's verified computation modules**, composed as steps inside Blueprint
programs. You are implementing GitHub EPIC **#5449** and its children (**#5450–#5456**) in
`OpenAgentsInc/openagents`.

## The architecture — read this twice; it's the whole point
- **Blueprint is the chat framework.** Blueprint is our in-repo, DSPy-style
  **typed-program / signature** system. A chat turn = a **Blueprint program run**:
  `user turn → signature selected → typed program → scoped steps/tools → response`.
- **Tassadar's verified computation modules compose AS STEPS *inside* Blueprint
  signatures.** A signature step bound to a Tassadar module resolves and executes that
  module's **exact, replay-verifiable computation**, and carries the **exact-replay digest
  as the step's evidence** (born verified).
- The chat's *language* reasoning is the normal Blueprint program runner / agent runtime
  (the existing `session.spawn` / Probe substrate). Tassadar modules are the
  **exact-computation building blocks** that runner invokes — they are not the thing you
  "talk to."
- **DO NOT** build "inference on Tassadar," "chat with the Tassadar model," or any Tassadar
  serving/inference endpoint. That framing was explicitly rejected. Tassadar is
  *compiled + executed* (exact traces), not prompted. If you find yourself adding an
  LLM-inference seam to Tassadar, stop — you've gone wrong.

## Read these first (do not skip)
- `docs/launch/2026-06-18-autopilot-tassadar-chat-blueprint-audit.md` — **the corrected
  architecture audit; your spec.** Note its §0.1 correction + "NOT a gap" callout.
- The EPIC and children on GitHub: **#5449** (epic), **#5450, #5451, #5452, #5453, #5454,
  #5455, #5456**.
- `CLAUDE.md` and `apps/openagents.com/AGENTS.md` + `apps/openagents.com/INVARIANTS.md` +
  root `INVARIANTS.md`.
- Blueprint code: `apps/openagents.com/workers/api/src/blueprint/` (signatures, module
  versions, typed program runs, optimizer-run *schema*, repositories, services like
  program-run-authority / continuation-decision / release-gate, fixtures, contract export,
  HTTP routes). **Optimizer/GEPA/RLM is schema+design only — do not assume a live learning
  loop.**
- Blueprint consumer: `packages/probe/packages/runtime/src/blueprint/signature-lookup.ts`
  (signature selection + tool scoping).
- Tassadar modules: `dense-weight-module-runtime.ts`, `linked-dense-module-runtime.ts`
  (exact-execution units, module registry/marketplace refs, exact-replay digests); executor
  `packages/tassadar-executor/src/numeric-executor.ts`;
  `apps/openagents.com/workers/api/src/tassadar-*.ts`.
- Replay functionality (for #5456): proof-replay bundles, the generated-replay API, the
  desktop replay visualization + bundle loader (`LoadProofReplayBundle`), replay clips
  (#5411), the Tassadar "Run 1: First Real Bitcoin Settlement" replay.
- Desktop app (for #5453/#5454): `apps/autopilot-desktop/` — Electrobun (Bun + Foldkit).
  The onboarding wizard is already the first screen (set in `src/ui/initial-state.ts`
  `initialRuntimeState`, the app entry at `src/ui/main.ts`); active pane is `model.pane`
  (a `PaneId` literal union in `src/ui/model.ts`).

## Issues & required sequencing (build in this order)
1. **#5450 — Bind Tassadar modules as typed Blueprint signature steps** *(the core seam —
   everything depends on it).* A Blueprint signature step that resolves + executes a
   Tassadar dense/linked module and carries the exact-replay digest as the step's evidence.
2. **#5451 — Make the Tassadar module registry a live, resolvable service** (it's
   fixture/design today) so Blueprint can look modules up.
3. **#5452 — Blueprint chat-program runtime**: `turn → signature → typed program → steps
   (some = Tassadar modules) → response`, on the existing session/agent substrate.
4. Then, on top of 1–3:
   - **#5453 — Autopilot Desktop chat pane UI** (a new Foldkit pane that runs Blueprint
     programs and shows Tassadar-module steps inline).
   - **#5454 — Post-onboarding default → the chat pane** (small transition in
     `apps/autopilot-desktop/src/ui/*`).
   - **#5455 — Evidence surface**: each Tassadar-module step carries its exact-replay
     receipt inline in the chat.
   - **#5456 — Demo module set: replay as a Blueprint signature type** (the first concrete
     demo: "show me a replay of X" → `ShowReplay` signature → existing replay runtime →
     real replay bundle). **Real runtime module, NOT test-driven.**

Do not implement #5456 (or any module demo) before #5450–#5452 exist.

## Hard constraints (non-negotiable)
- **Semantic routing only.** Mapping a user turn to a signature MUST go through Blueprint's
  typed signature-selection (the `signature-lookup` / central semantic selector). **No
  ad-hoc keyword/string matching** for intent routing. Deterministic parsing is allowed
  *only after* a signature is selected, and only for bounded fields (IDs, slugs, enums).
- **Real modules, not test infra.** Modules execute the real runtime (e.g. the actual
  replay bundle source), not test fixtures or the test harness.
- **NEVER add GitHub Actions / GitHub-hosted CI** (any workflow file is a hard no).
- **Honest scope.** Mark EXISTS vs NEW; never overclaim; never flip a product-promise to
  green without dereferenceable receipts + owner sign-off.
- **Stack:** Bun, Effect, Effect Schema, Foldkit. Don't reintroduce Cargo/Tauri. Consult
  the `effect-solutions` guide before writing Effect code.
- **Neutral commit metadata** — no personal names in commits/trailers. End commits with:
  `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.

## Workflow
- Work on `main` (use your own git worktree off current `origin/main`). Commit + push per
  issue.
- **Tests via the proper runner** (`bun run test` / vitest), **not bare `bun test`** —
  worker tests need the `cloudflare:workers` shim or they false-fail.
- If you touch the worker (`apps/openagents.com/workers/api`), **`bun run check:deploy` must
  pass** before you claim success or deploy.
- Read `docs/DEPLOYMENT.md` before any deploy/publish.
- Close a GitHub issue only after the work is **merged to `main`** (and deployed/verified if
  it's live worker/web code). Comment progress on the EPIC #5449 as children land.
- Keep each PR small and behavior-scoped; include tests proving the new behavior (e.g. that
  signature selection is semantic, not keyword-based; that a module step returns a real
  result + its exact-replay evidence).

## Definition of done (EPIC #5449)
Past onboarding, the Autopilot Desktop default screen is a Blueprint-driven chat. A user
turn is routed by semantic signature-selection to a Blueprint program; when a step is a
Tassadar module it executes real, exact, replay-verifiable computation and shows its receipt
inline; and the **replay demo (#5456)** works end-to-end ("show me a replay of the first
settlement" → a real replay bundle rendered in the chat).
