# Autopilot Coder Full Flow Audit — "Ask My Agent, Coding Gets Done"

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-10 (evening)

## The question under audit

The owner's target experience, verbatim intent: **through my Pylon, ask my
agent to do coding work, and the coding work gets done ASAP — in the cloud
or wherever.**

This audit measures the entire Autopilot Coder system against that sentence:
every doc in `docs/autopilot-coder/`, all open and closed issues in the lane,
the live-smoke evidence, the relevant product promises, and the open issues
that are (and are not) carrying the remaining distance.

## Source set

- `docs/autopilot-coder/README.md`, `implementation-log.md`,
  `2026-06-09-autopilot-coder-current-status-gap-audit.md`,
  `2026-06-09-probe-autopilot-sites-agent-api-audit.md`,
  `no-spend-e2e-smoke.md`, `paid-e2e-smoke.md`, `paid-l402-boundary.md`.
- Closed issues: `OA-AUTO-001` through `OA-AUTO-027` (#4575–#4592,
  #4610–#4618), epics #4619 and #4620, live-smoke runbook #4633, Mission
  Briefing #4628, promiseRef linkage #4631.
- Open issues swept for relevance: the full open list as of tonight
  (#4641–#4716), with #4661, #4713, #4696, #4684, #4654, #4656, #4662
  read in detail.
- Promise registry `docs/promises/registry.md` (live registry at
  `2026-06-10.20`), the green roadmap, and the v0.3 agent-economy sprint
  doc `docs/pylon/2026-06-10-v03-sprint-agent-economy.md`.
- `docs/2026-06-10-tassadar-executor-pylon-v03-readiness-audit.md` and
  `docs/artanis/2026-06-10-artanis-pylon-tassadar-full-status-audit.md`.

## Executive summary

The Autopilot Coder control plane is **real and live in production**. On
2026-06-09 a live registered agent submitted a no-spend work order to the
deployed worker, production placement selected a real registered Pylon,
the deployed scheduler created a durable assignment lease, the worker loop
ran offered → accepted → running → proof_submitted → closeout_submitted
through the deployed Pylon API, the order reached `delivered`, and an
owner review accepted it (#4633, work order
`autopilot_work_order.a1aef38e-66e7-488f-a06c-05dd02b34b35`). That cleared
the `current_codex_path_needs_evidence` blocker on
`autopilot.codex_probe_pylon_successor.v1` in registry `2026-06-09.21`.

What is still missing is exactly the thing the owner's sentence is about:
**no coding has ever been done by this system.** The live smoke's "work"
was a validation-class task whose closeout the agent itself drove through
the API — not the packaged Pylon binary checking out a repo, making a
change, and proving it with tests. The payment leg is verified end to end
in CI-safe smokes but no external sats have moved through the Autopilot
work-order route. The cloud fallback lane ("or wherever") has lease-intent
plumbing but **no production executor at all**. GitHub writeback, Sites
adapters, Forum reporting, and settlement remain unbuilt by design.

One sentence: **the order-taking machine is live and honest; the kitchen
has cooked exactly one dish, and it was a proof of the oven, not a meal.**

## 1. The chain, leg by leg

The target chain and the current truth of each leg:

| Leg | Status | Evidence / gap |
| --- | --- | --- |
| 1. "Through my Pylon, ask…" — entry surface | **Not built** | Today the only entry is a registered-agent HTTP call to `POST /api/autopilot/work` with an owner-granted `customer_orders.write` token. Sprint issue #4713 builds `pylon forum post/read/reply` and `pylon ask-artanis`, but **no issue yet gives Pylon a `pylon work submit`-class command that files an Autopilot coding work order.** This is the single missing entry leg. |
| 2. Agent discovery | **Live** | `/AGENTS.md`, `/.well-known/openagents.json`, OpenAPI, onboarding docs all advertise `submit_autopilot_work` / status / events (OA-AUTO-004). `autopilot.open_source_agent_entrypoint.v1` yellow. |
| 3. Typed work order intake | **Live in production** | `openagents.autopilot_work_request.v1`, idempotency, registered-agent auth, D1 persistence, event stream (poll + SSE), typed access requirements, repo authority projection, deterministic quote. Proven against deployed prod by #4633. |
| 4. Payment (L402/MDK) | **Verified contract, no live movement** | Signed quote-bound L402 challenge/retry, fail-closed verifier wired to the buyer-payment ledger, CI-safe paid smoke green (`smoke:autopilot-coder:paid`). Missing: a deployed MDK/L402 reconciler writing ledger rows from real payment movement, MDK checkout creation, and one staging/live agent-wallet payment. No real invoice has ever been issued or paid on this route. |
| 5. Placement | **Live in production** | Production Pylon-store-backed selector (owner linkage, heartbeat, version, wallet readiness, assignment + local-coding capability), needs_input/retry_later guidance, durable lease creation by the deployed scheduler. Proven live by #4633. Caveat: the live fleet is currently dark (19 registered Pylons, 0 online tonight per the full-status audit), so placement would return needs_input for most owners right now. |
| 6. **Execution** | **THE GAP** | No real coding execution exists in any lane. The live smoke worker submitted public-safe refs for a validation-class task, driven by the agent through the API. The packaged Pylon binary has never done a repo checkout, patch, or test run for an Autopilot order. The one real execution proof in the whole system is the **Tassadar executor lane** (numeric digest-pinned workloads, `compute.tassadar_executor_poc.v1` GREEN: real Pylon executed, worker replay-verified byte-identically, 1,000 real sats settled) — which proves the runtime spine works and makes coding the missing *work class*, not a missing *architecture*. |
| 7. Delivery + review | **Live** | Worker closeout ingestion → `delivered` with public-safe artifact/build/test/preview/result refs; owner review accept/reject/request_changes API; Mission Briefing projection (#4628); promiseRef linkage so accepted orders auto-appear as promise transition evidence (#4631). All exercised live in #4633. |
| 8. Cloud fallback ("or wherever") | **Intent only** | `hosted_gemini`, `shc`, `cloud_sandbox` fallback lease intents exist; the route has an `executeReadyWork` hook proven in harness. **No production executor binding exists, and no open issue builds one.** With the Pylon fleet dark, this means there is currently no runner at all for an owner without their own online Pylon. |
| 9. GitHub writeback (branch/commit/PR) | **Not built** | Authority refs and grants are modeled; no adapter creates branches/PRs. No open issue. |
| 10. Sites adapter | **Not built** | Sites control plane exists; no `site_generation`/`site_adjustment` task adapter. No open issue. |
| 11. Forum reporting | **Not built** for Autopilot orders | Redacted lifecycle renderer + idempotent posting bridge unbuilt. (Artanis's forum-scan responder, #4714/#4715, is adjacent machinery but a different lane.) |
| 12. Settlement / worker payout | **Blocked by design; machinery exists elsewhere** | Autopilot orders explicitly never grant payout authority; the accepted-work→payout-eligibility bridge is unbuilt. But the Pylon paid lane has settled real sats (Tassadar PoC closeout, 1,000 sats over real Lightning; the reliable-tips ladder is green), so the money rails exist — the bridge from accepted Autopilot work to them does not. |

## 2. What the issue flow built (closed work, compressed)

- **#4575–#4592 (OA-AUTO-001…018, all closed 2026-06-09):** the typed
  spine — request contract, routes, events, agent docs, access
  requirements, repo grants, deterministic quotes, L402/MDK intake,
  funding-vs-payout separation, typed tasks, assignment planner, queue
  dry-run inventory, placement policy, Pylon presence input, local-codex
  capability model, Pylon assignment synthesis, SHC/cloud fallback lease
  adapter, refusal/retry states.
- **#4610–#4618 (OA-AUTO-019…027) + epics #4619/#4620:** production
  Pylon placement wiring, durable assignment leases, the normalized
  `openagents.autopilot_coding_assignment.v1` payload shared across all
  lanes, the bounded no-spend Pylon worker loop, closeout/artifact
  ingestion to `delivered`, the customer review API, signed
  verifier-gated L402, and both CI-safe smokes
  (`smoke:autopilot-coder:no-spend`, `smoke:autopilot-coder:paid`).
- **#4628/#4631/#4633 (the get-to-green wave):** Mission Briefing
  projection, promiseRef linkage, and the **live production no-spend
  smoke** — including a new repeatable operator grant route
  (`POST /api/operator/agents/scoped-grants`) so future agents don't need
  DevTools to get owner-scoped tokens.

Redaction discipline held throughout: every projection scans for private
paths, wallet/payment material, provider payloads, raw prompts/logs/source
archives, and forbidden hosted-infrastructure wording; unsafe closeout refs
are rejected before persistence.

## 3. Relevant product promises (registry 2026-06-10.20)

| Promise | Status | Relation to the target sentence |
| --- | --- | --- |
| `autopilot.codex_probe_pylon_successor.v1` | **Yellow** | The load-bearing promise. `current_codex_path_needs_evidence` cleared by #4633; remaining blocker `live_probe_pylon_runtime_gates_incomplete` is exactly the real-execution gap, owned by open issue **#4661**. |
| `compute.tassadar_executor_poc.v1` | **Green** | Proof the dispatch→execute→replay-verify→settle spine works on real devices for a real (numeric) work class. |
| `autopilot.free_coding_task_beta.v1` | Yellow | "Request a useful coding task through openagents.com" — intake exists; fulfillment doesn't, so copy stays scoped. |
| `autopilot.issue_to_pr_loop.v1` | Red | The "coding shit gets done" end state. Stays red until execution + writeback + review gates are live. |
| `autopilot.autonomous_coding_loop_origin.v1` | Red | Historical overnight-coding claim; same dependency. |
| `autopilot.open_source_agent_entrypoint.v1` | Yellow | Discovery is live; capability stays scoped. |
| `payments.agent_lightning_l402_api.v1` | Yellow | L402 route contract verified; live paywall payment movement missing. |
| `sites.autopilot_sites_handoff.v1` | Yellow | Sites adapter for Autopilot tasks unbuilt. |
| `autopilot.agent_trace_revshare.v1` | Red | Downstream of real execution producing real traces. |
| `pylon.v03_agent_economy.v1` | Yellow (new, sprint) | Builds the "through my Pylon, ask my agent" entry surface — identity, memories, model adapters, forum commands (#4711–#4713). |
| `payments.reliable_tips_sweepable_balances.v1` | **Green** | The money-receiving rails any future worker payout will ride. |

## 4. Open-issue map against the target

**Striking finding: the Autopilot Coder lane itself has zero open issues.**
Every OA-AUTO issue and both epics are closed; momentum moved to the Pylon
v0.3 release cluster and tonight's agent-economy sprint. The open issues
that actually carry the owner's sentence:

| Issue | Carries which leg | Note |
| --- | --- | --- |
| **#4661** pylon: packaged-binary real-task runtime smoke (Codex-backed, bounded sandbox) | Leg 6, execution | **The most load-bearing open issue for this audit.** Explicitly written to close the #4633 caveat: a real Codex-backed repo checkout + change + verifiable output, executed by the installed `pylon` binary's worker loop through the live assignment lifecycle. Clears the last yellow blocker on `codex_probe_pylon_successor`. |
| **#4713** pylon sprint 3/6: agent identity, memories, forum surface | Leg 1, entry | Makes the Pylon the agent runtime ("the end of AGENTS.md-pasting"), but its surface is Forum commands + `ask-artanis`, **not** coding-work submission. |
| #4711, #4712 | rc2 + native tips | Release vehicle and money UX the coder flow will ship inside. |
| #4696 | Tassadar lane in v0.3 (capability default, smoke leg) | Keeps the proven execution spine in the shipped package (in progress in the working tree tonight). |
| #4684 | executor-trace homework work class | Generalizes paid exact-replay work; the settlement pattern coding work will eventually reuse. |
| #4654, #4656, #4662 | CI release gate, packaged live network smoke, stable 0.3.0 | Release hygiene gating everything above. |
| #4658 | live install-to-bitcoin smoke | The earn-side twin of the spend-side coder flow. |

**Gaps with no open issue (recommended to file):**

1. **Pylon coding-work entry command** — `pylon work submit/status/review`
   wrapping `POST /api/autopilot/work` with the registered identity. This
   is the literal "through my Pylon ask my agent" leg; sprint 3/6 builds
   the adjacent forum surface but not this. Small issue: the API is live,
   the token/grant mechanism exists (#4633's operator grant route), the
   client patterns exist in `apps/pylon`.
2. **Cloud fallback executor binding** — the "or wherever / ASAP"
   guarantee when the owner's Pylon (or the whole fleet) is dark. Hosted
   Gemini executor binding was P1 in the gap audit; nothing tracks it.
   Tonight, with 0/19 Pylons online, this is the difference between
   "works when my laptop is on" and "gets done ASAP."
3. **Live paid movement** — deployed MDK/L402 reconciler + one
   staging/live paid smoke with an agent wallet (gap audit P0 items 1–3).
4. **GitHub writeback lane** (gap audit P1.1) — without a PR coming back,
   "coding shit gets done" delivers refs, not merged code.
5. **Accepted-work → payout-eligibility bridge** (gap audit P0.6) — the
   marketplace half; the Lightning rails are green and waiting.

## 5. Honest distance to the demo

The shortest honest path to the owner sitting at a terminal and typing
something like `pylon work submit "fix the failing test in <public repo>"`
and watching it get done:

1. **#4661** lands — the packaged Pylon binary really executes a
   Codex-backed bounded coding task through the live lifecycle. (All
   plumbing for this exists; it is one work-class implementation plus a
   runbook.)
2. A **`pylon work` command** (new issue, above) files the order with the
   user's registered identity — entry leg done, no HTTP hand-rolling.
3. The **no-spend demo is then real end to end**: ask → place → execute
   (real edit, real test) → deliver → accept, all on production, all
   public-safe. This alone flips `codex_probe_pylon_successor` fully and
   makes `free_coding_task_beta` honestly claimable in scoped copy.
4. For "ASAP in the cloud or wherever": one **hosted executor binding**
   behind the existing fallback-lease seam, so orders run even with a
   dark fleet.
5. For the paid version of the sentence: deployed reconciler + one live
   paid smoke, then the accepted-work→payout bridge so the executing
   worker actually earns.

Steps 1–3 are days of work on existing seams, not architecture. Steps 4–5
are the same pattern the Tassadar and tips lanes already proved on their
own surfaces.

## 6. One-sentence truth

The system can already take, place, deliver, and review a coding work
order on production with full payment and redaction discipline — what it
cannot yet do, anywhere, in any lane, is the coding; #4661 is the door,
a `pylon work` command is the doorbell, and a cloud executor is the
guarantee that someone is always home.
