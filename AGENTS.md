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
4) If implementation status conflicts across docs: prefer the crate sources + SYNTHESIS_EXECUTION.
5) If priority/sequencing conflicts: ROADMAP wins.

<!-- effect-solutions:start -->
## Effect Best Practices

**IMPORTANT:** Always consult effect-solutions before writing Effect code.

1. Run `effect-solutions list` to see available guides
2. Run `effect-solutions show <topic>...` for relevant patterns (supports multiple topics)
3. Search `.reference/effect/` for real implementations (run `effect-solutions setup` first)

Topics: quick-start, project-setup, tsconfig, basics, services-and-layers, data-modeling, error-handling, config, testing, cli.

Never guess at Effect patterns - check the guide first.

## Local Effect Source

The Effect repository is cloned to `.reference/effect/` for reference.
Use this to explore APIs, find usage examples, and understand implementation details when documentation isn't enough.
<!-- effect-solutions:end -->

Community presence:
- Moltbook skill + posting guidance: ./docs/MOLTBOOK.md
- When drafting Moltbook replies (including Codex sessions), consult `MOLTBOOK.md` first and follow `crates/moltbook/docs/CLAIM_HYGIENE.md`.
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
- `docs/autopilot/PROD_E2E_TESTING.md`

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

Read: `docs/autopilot/PROD_E2E_TESTING.md`

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

Use this to avoid scattering logic:

### dsrs (compiler layer)
- Signatures/modules/optimizers/metrics/tracing: crates/dsrs/
- Docs: crates/dsrs/docs/
- If you change signature semantics, update docs + ensure parsing/tests still pass.

### Adjutant (execution engine + DSPy decision pipelines)
- DSPy pipelines + session tracking + auto-optimization: crates/adjutant/
- Tool registry (local tools): crates/adjutant/src/tools.rs

### Autopilot (product surfaces)
- UI/CLI orchestration + user-facing flow: crates/autopilot/
- Core execution flow + replay impl: crates/autopilot-core/

### RLM / FRLM
- Local recursion tooling + signatures: crates/rlm/
- Federated recursion conductor + map-reduce: crates/frlm/

### Protocol / Marketplace plumbing
- Typed job schemas + hashing: crates/protocol/
- Node software (provider + host): crates/pylon/
- Relay (agent coordination): crates/nexus/

---

## CLI surfaces (which binary owns what)

| Binary | Purpose | Status |
|--------|---------|--------|
| `autopilot` | Product CLI: sessions, run, export, replay, policy, UI | 🟢 Implemented |
| `pylon` | Network node CLI: jobs, wallet, provider mode, doctor | 🟢 Implemented |

`adjutant` is an internal library/codename for our DSPy decision pipelines—not a user-facing CLI. All CLI commands use `autopilot`.

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

## Build + test quick commands (use these before claiming done)

Workspace:
```bash
cargo build --release
cargo test
```

Web (apps/web):

```bash
cd apps/web
npm run lint
npm test
npm run dev
npm run deploy
```

Autopilot:

```bash
cargo build -p autopilot
cargo test  -p autopilot
cargo run   -p autopilot
```

Adjutant + dsrs:

```bash
cargo test -p adjutant
cargo test -p dsrs
```

Pylon:

```bash
cargo build --release -p pylon
cargo test -p pylon
./target/release/pylon doctor
```

Nexus (worker):

```bash
cd crates/nexus/worker
bun install
bun run deploy
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

* crates/dsrs/docs/ARTIFACTS.md
* crates/dsrs/docs/REPLAY.md
* ./docs/ROADMAP.md (MVP gate: Verified Patch Bundle)

---

## Documentation pointers (don’t duplicate; link)

Core:

* ./GLOSSARY.md (canonical vocabulary)
* ./SYNTHESIS_EXECUTION.md (how the system works today)
* ./docs/ROADMAP.md (what to build next; MVP gates)
* ./docs/PROJECT_OVERVIEW.md (product + stack overview)
* ./docs/AGENT_FOUNDATIONS.md (conceptual foundations and checklists)

DSPy/dsrs:

* crates/dsrs/docs/README.md
* crates/dsrs/docs/ARCHITECTURE.md
* crates/dsrs/docs/SIGNATURES.md
* crates/dsrs/docs/TOOLS.md
* crates/dsrs/docs/METRICS.md
* crates/dsrs/docs/OPTIMIZERS.md
* crates/dsrs/docs/EVALUATION.md

Protocol / network:

* docs/protocol/PROTOCOL_SURFACE.md
* crates/protocol/
* crates/pylon/
* crates/nexus/

Architecture Decisions (ADRs):

* docs/adr/INDEX.md (decision index)
* docs/adr/README.md (ADR process)
* Key ADRs: ADR-0001 (authority), ADR-0002 (Verified Patch Bundle), ADR-0003 (replay), ADR-0004 (lanes), ADR-0005 (step_utility)

---

## Final note

If you are uncertain whether something belongs in the runtime, dsrs, or a product crate:

* Prefer keeping policy in dsrs/adjutant (Signatures/Modules/Pipelines),
* Keep execution enforcement (schema validation, retries, receipts) in the runtime/tooling layer,
* Keep UI/UX wiring in product crates.
