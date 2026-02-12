# OpenAgents: Agent Contract (READ THIS FIRST)

## What OpenAgents is

OpenAgents is a runtime + compiler + (optional) market for autonomous agents:

- Runtime: executes tool/sandbox/job actions, enforces schemas + retries, records replayable receipts.
- Compiler layer (dsrs / DSPy): expresses agent behavior as typed Signatures + Modules and optimizes them via metrics into policy bundles.
- RLM/FRLM: execution modes for out-of-core reasoning over large repos / long sessions.
- Market layer (Pylon + NIP-90 + relays): makes compute and sandbox execution purchasable with receipts and budgets.
- Verification (tests/builds): anchors correctness; everything else is optimization.

If you are writing code here, you are usually adding:
1) a capability (tool/job/provider/lane),
2) a policy (signature/module/routing),
3) measurement (metrics/labels/counterfactuals/eval).

---

## Authority and conflict rules (non-negotiable)

1) If documentation conflicts with code behavior: CODE WINS.
2) If terminology conflicts across docs: GLOSSARY WINS.
3) If architecture intent conflicts (invariants, interfaces, contracts): ADRs WIN.
4) If implementation status conflicts across docs: prefer the active codebase (see docs/PROJECT_OVERVIEW.md) and current docs.
5) If priority/sequencing conflicts: ROADMAP wins.

<!-- effect-solutions:start -->
## Effect Best Practices

**IMPORTANT:** Always consult effect-solutions before writing Effect code.

1. Run `effect-solutions list` to see available guides
2. Run `effect-solutions show <topic>...` for relevant patterns (supports multiple topics)
3. Search `.reference/effect/` for real implementations (run `effect-solutions setup` first)
4. For Effect packages (`packages/dse`, `packages/effuse`, `packages/effuse-test`), run `npm run effect:patch` after install to enable build-time diagnostics.

Topics: quick-start, project-setup, tsconfig, basics, services-and-layers, data-modeling, error-handling, config, testing, cli.

Never guess at Effect patterns - check the guide first.

## Local Effect Source

The Effect repository is cloned to `.reference/effect/` for reference.
Use this to explore APIs, find usage examples, and understand implementation details when documentation isn't enough.
<!-- effect-solutions:end -->

Community presence:
- Moltbook skill + posting guidance: ./docs/MOLTBOOK.md
- When drafting Moltbook replies (including Codex sessions), consult `MOLTBOOK.md` first and any claim-hygiene guidance referenced there (archived in backroom if not in repo).
- Do not post or reply on content that mentions shitcoin tickers (e.g. $MOLTEN). Filter pattern: dollar sign + 3–7 alphanumeric chars; see MOLTBOOK.md “Shitcoin filter” and `apps/web/src/lib/shitcoinFilter.ts`.

---

## Engineering invariants (ship-quality rules)

Verification first
- Do not claim success without running the relevant verification harness (tests/build/lint as appropriate).

No stubs policy
- Do not add TODO-only “NotImplemented”, placeholder returns, mock implementations in production paths.
- If it’s not ready, gate behind a feature flag or remove the code path.

Typed contracts everywhere
- If it gates a decision or action, make it a Signature (or signature-backed pipeline).
- Tools must have schemas; runtime validates schemas before execution.

Everything is logged and replayable
- Tool calls must emit deterministic hashes + receipts.
- Decisions must be recorded with counterfactuals when migrating from legacy heuristics.

---

## Telemetry + Debugging (apps/web)

When debugging anything in `apps/web` (local or prod), **correlate by request id first**.

Full writeup (including production E2E auth bypass + how to run/debug prod smoke tests):
- `docs/autopilot/testing/PROD_E2E_TESTING.md`

### Cloudflare Worker request correlation

- Every Worker response includes `x-oa-request-id` (derived from `cf-ray` when present; otherwise a UUID).
- Every Worker telemetry log/event line includes a grep-friendly token: `oa_req=<id>`.

Get the request id:
- Browser: DevTools → Network → failing request → Response Headers → `x-oa-request-id`
- CLI:
  ```bash
  curl -I https://autopilot-web.openagents.workers.dev/autopilot | rg -i "x-oa-request-id|cf-ray"
  ```

Tail prod Worker logs for just that request:
```bash
cd apps/web
npx wrangler tail autopilot-web --format pretty --search "oa_req=<PASTE_ID>"
```

Local:
- Local `wrangler dev` logs already include `oa_req=<id>`; search your terminal output for the token.

### Convex correlation

Convex failures typically include a Convex request id like `[Request ID: <id>]`.

```bash
cd apps/web
npx convex logs --prod --jsonl | rg "<CONVEX_REQUEST_ID>"
```

When to do this:
- Any user report of a blank page, SSR 500, or “no response” UI stall.
- Any `ConvexServiceError` in console output.
- Before changing logic “blind”: pull the `x-oa-request-id` and confirm the actual failing path.

### Production E2E Smoke Tests (apps/web)

We have an Effect-native E2E runner (`packages/effuse-test`) that can run against a **production URL** and log in
deterministically via an E2E-only auth bypass route (gated by a Cloudflare Worker secret).

Read: `docs/autopilot/testing/PROD_E2E_TESTING.md`

Quick smoke:

```bash
cd apps/web
EFFUSE_TEST_E2E_BYPASS_SECRET="..." \
  npm run test:e2e -- --base-url https://openagents.com --tag prod --grep "apps-web\\.prod\\.autopilot"
```

Artifacts land in `output/effuse-test/<runId>/` (look at `events.jsonl` first).

Adapters do serialization/parsing only
- Adapters do not own validation/retry logic. Runtime (or meta-operators like Refine) owns retries/guardrails.

Doc gate for contracts
- If you introduce/modify a **contract** (interface, invariant, protocol), you must:
  - Add/update an ADR in docs/adr/
  - Update docs/GLOSSARY.md if terminology changes
  - Update affected specs (ARTIFACTS.md, REPLAY.md, PROTOCOL_SURFACE.md)

---

## Agent autonomy: deploys and unblocking

**You ARE able to run deploys yourself.** Do not wait for a human to deploy unless the user explicitly tells you to slow down.

- **Main website:** `cd apps/web && npm run deploy` (Convex + Vite build + Cloudflare Workers deploy).
- **Other apps:** Check each app’s `package.json` for a `deploy` script, e.g.:
  - `apps/api` → `npm run deploy` (wrangler deploy)
- **Workflow:** Fix things, deploy, then test (e.g. hit live site or run E2E). Only pause and ask the user when they have asked you to slow down or when you lack credentials/access.

### Git Hygiene (No Surprise Worktrees)

- **Do not create `git worktree`s on your own initiative.**
- If you need a clean workspace for deploy/tests but `git status` is dirty, you MUST:
  - Tell the user what files are dirty.
  - Propose options: commit the changes, discard the changes, or (if the user explicitly agrees) use a temporary worktree.
- If a worktree is used (only with explicit user approval), remove it immediately after use (`git worktree remove …`) to avoid confusion and accidental deploys from the wrong checkout.

### Git Hygiene (No Surprise Stash)

- **Do not use `git stash` without explicit user confirmation.** This repo is often multi-agent; stashes hide work and can disrupt other agents.
- This includes: `git stash push`, `pop`, `apply`, `drop`, and `git stash branch`.
- If you need to park changes and the user has not explicitly approved stashing:
  - Prefer a WIP commit on a dedicated branch, or
  - Export a patch outside the repo (e.g. `/tmp/...`) and keep the worktree intact.

---

## "Where do I change things?" (map)

Use this to avoid scattering logic. **Rust/crates were removed and archived;** active code is TypeScript/Effect. See docs/PROJECT_OVERVIEW.md and docs/RUST_DOCS_ARCHIVE_2026-02-11.md.

### DSE (compiler layer)
- Signatures/modules/optimizers/metrics/tracing: `packages/dse/`
- If you change signature semantics, update docs + ensure parsing/tests still pass.

### Autopilot (product surfaces)
- Web product + Worker: `apps/web/`, `apps/autopilot-worker/`
- Tool contracts and handlers: `apps/autopilot-worker/src/tools.ts`, `server.ts`
- DSE catalog (signatures/modules): `apps/autopilot-worker/src/dseCatalog.ts`

### Protocol / market (when re-added or in archive)
- Typed job schemas, node software, relay: see docs/PROJECT_OVERVIEW.md; protocol docs were archived to backroom.

---

## Checklists (what to do when adding things)

### If you add a new decision point
- Create a Signature with confidence (if it routes/overrides).
- Confidence-gate behavior (fallback to legacy rules when low confidence).
- Record counterfactuals (DSPy output vs legacy output vs executed choice).
- Add outcome labeling (verification_delta, repetition, cost).
- Make it eligible for optimization targeting (rolling accuracy / impact).

### If you add a new tool
- Register it in the canonical tool registry.
- Provide a JSON schema for params; runtime validates before execution.
- Emit a receipt record:
  - tool, params_hash, output_hash, latency_ms, side_effects
- Bound outputs, add timeouts, deterministic failure modes.
- Add tests for schema, truncation, and error behavior.

### If you add a new provider / lane
- Add provider integration + health detection.
- Implement adapter formatting/parsing (no retries here).
- Add cost accounting (tokens/latency/msats).
- Make lane selection policy-driven (signature) and auditable.
- Add fallback/circuit breaker behavior.

### If you “improve performance”
- Don’t hand-tweak prompts inline.
- Convert the behavior into a signature/module, add a metric, compile into a policy bundle.
- Preserve rollback/canary path.

---

## Testing Framework (What We Use)

- **TypeScript unit + Worker tests:** `vitest`
  - `apps/web` uses `@cloudflare/vitest-pool-workers` for Cloudflare Worker tests.
- **E2E browser + visual regression:** `packages/effuse-test` (Effect-native runner, invoked via `bun`)
  - Web scripts: `cd apps/web && npm run test:e2e`, `npm run test:visual`
  - Docs: `docs/STORYBOOK.md`, `docs/autopilot/testing/PROD_E2E_TESTING.md`
- **Stream contract fixtures:** JSONL “wire transcripts” that drive UI determinism + regression tests
  - Docs: `docs/autopilot/testing/STREAM_TESTING.md`
- **How we self-improve DSE/RLM:** runbook
  - `docs/autopilot/runbooks/SELF_IMPROVE_RUNBOOK.md`

## Build + test quick commands (use these before claiming done)

**Rust/cargo binaries (autopilot, pylon, adjutant, etc.) were removed and archived.** Use the following for the active web/TypeScript stack.

Web (apps/web):

```bash
cd apps/web
npm run lint
npm test
npm run dev
npm run deploy
```

Autopilot worker (apps/autopilot-worker):

```bash
cd apps/autopilot-worker
npm run lint
npm test
```

Convex (apps/web):

- **Never run `npx convex deploy` (or `npx convex deploy --yes`) raw.** It uses dev env (e.g. CONVEX_DEPLOYMENT / .env) and can push the wrong config or hit WorkOS env mismatches against prod.
- Use the app deploy script, which loads production env and deploys Convex with the correct vars:
  ```bash
  cd apps/web
  npm run deploy
  ```
  For Convex-only deploy: `npm run deploy:convex` (uses `.env.production` then `npx convex deploy --yes`).

---

## Artifact expectations (when you finish an agent session)

The canonical output of an autonomous run is the Verified Patch Bundle:

* PR_SUMMARY.md
* RECEIPT.json
* REPLAY.jsonl (or ReplayBundle + exporter until native REPLAY.jsonl is wired)

See:

* docs/adr/ADR-0002-verified-patch-bundle.md
* docs/ROADMAP.md (MVP gate: Verified Patch Bundle)
* Artifact/replay schema details may be in archived dsrs docs (docs/RUST_DOCS_ARCHIVE_2026-02-11.md).

---

## Documentation pointers (don’t duplicate; link)

Core:

* docs/GLOSSARY.md (canonical vocabulary)
* docs/ROADMAP.md (what to build next; MVP gates)
* docs/PROJECT_OVERVIEW.md (product + stack overview; active codebase map)
* docs/RUST_DOCS_ARCHIVE_2026-02-11.md (archived Rust-era and protocol docs)

Architecture Decisions (ADRs):

* docs/adr/INDEX.md (decision index)
* docs/adr/README.md (ADR process)
* Key ADRs: ADR-0001 (authority), ADR-0002 (Verified Patch Bundle), ADR-0003 (replay), ADR-0004 (lanes), ADR-0005 (step_utility)
* Note: many ADRs reference archived crates paths; see docs/adr/README.md.

Repo Directory (high-signal paths):

Apps (product surfaces):

* `apps/web/README.md` (primary web product surface)
* `apps/web/src/` (web UI + Worker host integration)
* `apps/web/src/effuse-host/worker.ts` (Worker entry path for web runtime)
* `apps/web/convex/README.md` (Convex model + server functions)
* `apps/web/tests/worker/` (Worker behavior tests)
* `apps/web/wrangler.jsonc` (web Worker config)
* `apps/web/wrangler.effuse.jsonc` (alternate Worker config)
* `apps/web/scripts/` (DSE/admin operational scripts)
* `apps/autopilot-worker/wrangler.jsonc` (autopilot-worker runtime config)
* `apps/autopilot-worker/src/` (worker runtime logic, tools, DSE catalog)
* `apps/autopilot-worker/scripts/autopilot-smoke.ts` (worker smoke test)
* `apps/expo/README.md` (mobile client surface)

Packages (shared TypeScript libraries):

* `packages/dse/README.md` (DSE compiler/runtime package)
* `packages/dse/src/` (compile/eval/runtime logic)
* `packages/effuse/README.md` (Effect-first runtime primitives)
* `packages/effuse/src/` (core Effect runtime + router)
* `packages/effuse-test/README.md` (E2E/visual harness)
* `packages/effuse-test/src/` (runner, probes, browser service)
* `packages/effuse-panes/README.md` (pane model package)
* `packages/effuse-flow/` (flow abstractions)
* `packages/effuse-ui/` (UI primitives)
* `packages/hud/README.md` (HUD/UI support package)

Autopilot docs (start here for product behavior):

* `docs/autopilot/spec.md` (autopilot behavior spec)
* `docs/autopilot/testing/PROD_E2E_TESTING.md` (prod E2E + request correlation)
* `docs/autopilot/testing/STREAM_TESTING.md` (stream fixture and contract testing)
* `docs/autopilot/runbooks/SELF_IMPROVE_RUNBOOK.md` (self-improvement workflow)
* `docs/autopilot/runbooks/DSE_PLAYBOOK.md` (DSE operation and tuning)
* `docs/autopilot/testing/TRACE_RETRIEVAL.md` (trace retrieval/debug workflow)
* `docs/autopilot/admin/AUTOPILOT_ADMIN_TEST_USER_TRIGGER.md` (admin trigger flow)
* `docs/autopilot/reference/THREAD_STUCK_STREAMING_FIX.md` (known issue + mitigation)

Infra / integration (if present; some paths archived):

* `docs/cloudflare/` (Workers deployment notes)
* `docs/liteclaw/` (tunnel and ingress notes)
* `docs/lightning/reference/LIGHTNING_AGENT_TOOLS.md` (L402/lnget integration plan)

---

## Final note

If you are uncertain where something belongs:

* Prefer keeping policy in DSE (Signatures/Modules/Pipelines) and apps/autopilot-worker (dseCatalog, tools).
* Keep execution enforcement (schema validation, retries, receipts) in the runtime/tooling layer.
* Keep UI/UX wiring in apps (web, expo).
