# 2026-02-10: Last ~6 Hours (DSE Overnight Loop) Report

In the last ~6 hours (from `e06739c6f` → `4fe547180`), we turned the “overnight self-improvement” concept into a **working, prod-verified, headless loop** and we **actually shipped one real policy improvement to production**.

## What Was Actually Shipped (Capabilities)

### 1) A real programmatic self-improve loop (no clicking)
Commits: `e06739c6f`, `9c6fd70e8`, plus the runner hardening series (`53f06de6b`…`190c2f73c`…`4fe547180`)

You can now run a single command that:
- imports a canonical dataset into Convex
- compiles a non-trivial search space (multiple candidates)
- starts a canary
- generates traffic deterministically (exerciser endpoints)
- polls canary counters
- promotes or stops/rolls back
- writes an **ops run timeline** to Convex so you can inspect it later

### 2) Convex-backed, read-only “what happened?” pages
Commit: `1e69b1808` (+ doc followups)
- `/dse` ops runs list
- `/dse/ops/:runId` run timeline
- `/dse/signature/:signatureId` per-signature history (active pointer, canaries, compile reports, datasets, receipts, trace links)

### 3) Judge-based eval plumbing for non-discrete outputs (recap/summarization)
Commit: `baa762bb9`
- pinned judge artifact
- `/api/dse/eval` + `dseEvalReports` stored in Convex
- read-only pages to inspect eval JSON

### 4) Headless trace mining pipeline (RLM traces → examples)
Commit: `dc8b1f2f2`
- list receipts in prod via ops-admin
- export trace-derived examples into `dseExamples`
- CLI miner script to run it headlessly

### 5) Compiler-visible “Phase G” knobs for RLM-lite jobs
Commit: `2941dfa0c`
- search spaces for controller instructions, chunking policy, roles, budget profiles
- compile reports store which knob set won

### 6) Autopilot UI + prod E2E aligned with DSE observability
Commits: `765ec6baa`, `8be98e82a` (+ doc updates)
- `/autopilot` renders each signature as a **one-line summary** that expands to the full debug card
- prod E2E updated to expand before asserting debug visibility

### 7) Ops hardening + prod wiring (secrets + deploys)
Commits: `c932ae568`, `190c2f73c`, `195805fe1`, `4fe547180` (+ docs)
- runner now surfaces `x-oa-request-id` and the failing endpoint/timeout in its JSON output
- prod got `OA_DSE_ADMIN_SECRET` wired and verified
- Worker/Convex deploys were done to ensure prod has the new Convex funcs and endpoints

All of this is now documented as the runbook in `docs/autopilot/OVERNIGHT_SELF_IMPROVEMENT_PLAN.md` (Implementation Log has timestamps + run ids).

## Did Agents Improve (User-Facing Behavior)?

### Yes: we promoted a better policy for SelectTool in production
This is the only concrete “agent got better” change that affects normal Autopilot chats right now.

- Signature improved: `@openagents/autopilot/blueprint/SelectTool.v1`
- New active compiled policy: `sha256:862f69e8a655c716e8eac0fe22fcfbdcf304702a8c729fa3a91e67cd2a9ee61a`
- The full prod loop succeeded (compile → canary → traffic → promote) in ops run:
  - `opsrun_4796ab2c-1544-4f17-9d88-500d171c454e`
- Canary health: `minSamples=20`, `errorRate=0`
- Promotion actually happened (control → new compiled_id) as recorded by the runner summary and Convex ops events.

Verification that prod is now using the new compiled policy:
- A fresh call to `/api/dse/exercise/predict` in prod returns `compiled_id = sha256:862f…` (meaning runtime predictions are coming from the promoted artifact, not the old control).

**What that means for Autopilot chats**
- Autopilot’s “should I run a blueprint update tool?” router should now be **more accurate** on the dataset distribution (34-example SelectTool fixture), i.e. fewer wrong tool triggers and better tool choice when the user says things like “call me X”, “rename yourself”, “new rule: …”, “export my blueprint”, etc.

### What did NOT “improve” yet (in user-visible behavior)
- RLM-lite is still not automatically used for normal users by default; most of the RLM work here is infrastructure (eval, trace mining, compile knobs, observability), not a global behavior flip.
- The successful prod overnight run I executed used `--no-e2e`, so it did **not** run the prod browser E2E suite as a gate on that specific promotion (the mechanism exists; it just wasn’t invoked on that run).

## Bottom Line
- **Infra outcome:** overnight self-improvement is now a real, agent-runnable program with Convex as the source of truth + read-only inspection surfaces.
- **Behavior outcome:** **one real policy improvement shipped**: `SelectTool.v1` was compiled, canaried, and promoted in production, and prod now serves the new `compiled_id` for that signature.

