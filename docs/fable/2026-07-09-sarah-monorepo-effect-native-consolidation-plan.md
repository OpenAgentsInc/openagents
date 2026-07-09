# Sarah consolidation plan — OpenAgentsInc/sarah → the openagents monorepo, on Effect Native

Date: 2026-07-09
Status: owner-directed plan (2026-07-09 directive: "All Sarah shit must be
ported to Effect Native, moved into this codebase — no more separate sarah
repo"); execution tracked by the SM epic
Supersedes: the 2026-07-07 separate-repo decision, sarah#14 (Cloud Run
lift-and-shift of the Next app), sarah#15 (TanStack Start front-end port)
Pattern precedent: the Cloud repo consolidation (#8591,
`docs/cloud/2026-07-08-cloud-repo-open-source-consolidation-plan.md`) — freeze,
scrub, move, converge, cut over, retire

## 0. Executive decision

Sarah stops being a separate codebase. The private `OpenAgentsInc/sarah` repo
(Next.js 16 + Vercel AI SDK canary + eve, deploying to sarah.openagents.com)
moves into the openagents monorepo as `apps/sarah/`, and every Sarah surface
converges on the standing owner mandates:

- **UI**: Effect Native component set on the DOM renderer (§EN, rev 6 full
  conversion). Zero new React. The 2026-07-08 rescope already folded sarah#15
  into "render Sarah's UI via the Effect Native web renderer"; this plan makes
  it the only path and gives it a home.
- **App foundation**: Bun + Effect (the monorepo law for new TypeScript).
  Next.js does not enter the monorepo — the port happens as the move, not
  after it.
- **Serving**: our own cloud (Cloud Run) from monorepo-built images, mirroring
  the #8591 Phase 6 pattern. The interim Vercel deploy stays production only
  until the cutover lane lands, then is decommissioned.
- **Authority**: unchanged. The openagents.com API (the Cloud Run monolith)
  remains the system of
  record for CRM, credits, checkout, receipts, and the promise registry.
  In-monorepo Sarah still calls those APIs over their public contracts and
  never re-implements them — moving into the repo does not move authority.

Why now, beyond the directive: Sarah is P4's generalization seed
(`ai_employee.v1` hardens from her persona/CRM/authority patterns). Seeding
the formal employee record from an out-of-repo Next/eve app would mean
migrating her twice. Consolidating now means the P4 migration is a promotion,
not a rewrite. It also collapses a duplicated email stack (see §4.3) and puts
her Eval Suite inside the QAM discipline where every other suite lives.

## 1. What exists today (source inventory)

Source: `~/work/sarah` (private `OpenAgentsInc/sarah`), clean at `f027314`.
The surface is small — 36 `.ts` files, **2** `.tsx` files, one eve agent:

| Area | Contents | Port disposition |
|---|---|---|
| `agent/` (eve brain) | `agent.ts` (`defineAgent`, `openai/gpt-5-mini`), `instructions.md` persona, tools (`intake_capture`, `crm_contact_upsert`, `crm_activity_append`, `deal_rules_evaluate`, `human_handoff`, `checkout_link_create`, `demo_sales_context`), `schedules/follow-up`, `channels/` (realtime-transcript, email) | Moves intact in SM-1 (eve self-hosted beside the app); replaced by the owned Effect agent runtime in SM-4 with an identical tool inventory |
| `src/lib/` (19 services) | prospect-session, session-index, deal-rules, sarah-instructions, promise-registry, realtime-token-guard, gateway-realtime-browser, realtime-tools, openagents-crm-client, openagents-sales-client, follow-up-scheduler, email-approval-queue, email-suppression-list, resend-email-sender, eve-origin, … | Mostly framework-agnostic TS + zod: wrap as Effect services near-verbatim in SM-1. The three email modules are **not** ported — they converge on the monorepo CRM send rail (§4.3) |
| `src/app/` API routes | prospect/session, realtime/token, realtime/session-config, eve/tool-call, eve/turn, operator/{prospects,email-drafts,ops,follow-ups}, unsubscribe | Thin handlers → Bun/Effect HTTP routes in SM-1 (same paths, same contracts) |
| `src/app/` UI | `layout.tsx` + `page.tsx` (the S-10 branded voice surface: mic states, transcript, disclosure, text fallback) | **Not ported as React.** Reauthored in Effect Native components in SM-2 |
| `evals/` + scripts | `sarah-fixtures.json`, the S-12 eval suite, ~15 S-lane smoke scripts | Move into the monorepo test tree; the S-12 suite becomes the conformance oracle for every later phase (especially SM-4) |
| `docs/` | deployment notes, evidence | Redaction-audited in SM-0; classified active / historical / drop |

Issue state in the sarah repo: S-1..S-5, S-9, S-10, S-12, S-13 CLOSED;
S-6 (first real tools — residual gates), S-7 (CRM sync — residual gates),
S-8 (email channel), S-11 (Vercel production wiring) OPEN; S-14/S-15 OPEN and
**superseded by this plan**. All open scope is absorbed into the SM lanes
below and the sarah issues are closed with pointers.

## 2. Target layout

```text
apps/sarah/                      # Bun/Effect service, own Cloud Run service, sarah.openagents.com
  src/routes/                    # ported API routes (prospect, realtime token/config, tool-call, operator, unsubscribe)
  src/services/                  # the lib/ services as Effect services (Context.Service + Layer)
  src/ui/                        # Effect Native component tree (DOM renderer) — the voice surface
  agent/                         # SM-1..3: the eve agent dir, run self-hosted; SM-4: replaced by the owned runtime
  evals/                         # sarah fixtures + suite, wired into the normal sweep
  docker/ or Dockerfile          # monorepo-built image, #8591-style
docs/sarah/                      # migrated active docs + this migration's receipts
```

Boundaries that hold no matter what:

- `apps/sarah` never imports openagents.com API internals or touches its
  database directly — CRM,
  credits, checkout, receipts, and promise state go through the same public
  HTTP contracts the external repo used. In-repo proximity is not authority.
- Deal-rule **code** is public; owner-signed priced parameters beyond the
  already-public pack prices load from runtime config/Secret Manager, not git.
- The realtime voice loop stays browser↔provider direct (no server-side WS
  infra) — the token-minting route keeps its S-3 hardening (origin/rate
  limits, session caps/TTLs, spend alerts) through every phase.

## 3. Migration phases (SM lanes)

### SM-0 — freeze + redaction audit (gates everything)

1. Feature-freeze `OpenAgentsInc/sarah` (emergency fixes only); record the
   exact source commit in the migration receipt.
2. Redaction audit over every candidate file: env/tokens/gateway keys, priced
   deal parameters beyond public pack prices, prospect/customer PII in
   `docs/evidence` and `tmp/`, Resend/Twilio material, internal URLs. The
   private→public direction makes this the load-bearing phase — nothing moves
   until its file class is cleared.
3. Classify docs `active` / `historical-source` / `drop`.

Exit: explicit move-set with no forbidden material; receipt doc started
(`docs/sarah/MIGRATION.md`).

### SM-1 — port-as-move: the Bun/Effect service (backend parity)

1. `apps/sarah/` scaffolded on Bun/Effect; the 19 lib services wrapped as
   Effect services; API routes ported handler-by-handler with identical
   request/response contracts (the S-3 token guard behavior byte-for-byte).
2. The eve `agent/` dir moves intact and runs self-hosted beside the app
   (its own process/container), exactly as sarah#14 specced — `eve build &&
   eve start`, workflow state on our Cloud SQL. eve is a dependency here, not
   an architecture commitment (see SM-4).
3. Smoke scripts and the S-12 eval suite move into the monorepo and run in
   the normal sweep against a local instance.
4. The existing Vercel deployment remains production, untouched.

Exit: local `apps/sarah` passes every ported smoke + the S-12 suite;
`bun test` green; no Next.js anywhere in the monorepo.

### SM-2 — the voice surface in Effect Native

1. Reauthor the S-10 branded UI as Effect Native components on the DOM
   renderer: AI-disclosure banner, mic capture states, VAD/level indication,
   streaming transcript, text-input fallback, handoff/checkout cards —
   Protoss blue, one theme.
2. Component gaps route upstream through the EN-2 (#8572) demand register
   into `GAPS.md` under the growth rule — the audio/streaming interaction
   class (mic button, level meter, live transcript list) is exactly the
   demand §EN wants surfaced, never app-local one-off primitives.
3. `gateway-realtime-browser` wraps as an Effect service emitting the typed
   intent vocabulary (the MH/Sync law: steering and UI state as serializable
   data end to end).
4. Deterministic snapshot + intent tests per the EN testing discipline;
   visual baseline captured against the current production page.

Exit: the Sarah surface renders entirely from the component set; zero React
in `apps/sarah`; sarah.openagents.com and openagents.com are one component
system by construction (the WEB-1 synergy, now literal).

### SM-3 — email + CRM convergence (delete the parallel stack)

1. Replace `resend-email-sender` + `email-suppression-list` +
   `email-approval-queue` with the monorepo's existing approval-gated CRM
   send rail (the SELL/LG substrate: sequence authoring, `dispatchCrmSend`,
   suppression, receipts) — one suppression list, one approval queue, one
   email ledger for both Sarah and the OB-1..6 outbound engine.
2. Sarah's mailbox inbound routes through the event-ledger email source →
   `inbox_match` (the SR-3 shape); operator email-draft routes point at the
   shared queue (and inherit OB-4's batch-approval UX when it lands).
3. S-7 CRM-sync and S-8 email-channel residual gates close here, on the
   monorepo rail.

Exit: no duplicate email/suppression/approval code in `apps/sarah`; every
Sarah send is a `crm_activity` + email-ledger row under the standing
no-send-without-approval-receipt contracts.

### SM-4 — the owned Effect agent runtime (eve retired)

1. Replace eve with an owned Effect runtime for Sarah's brain: durable
   sessions, `instructions` persona loading, typed tools, schedules, and
   channels as Effect services — deliberately shaped to converge with
   `agent_definition.v1` and the P4 `ai_employee.v1` record, because Sarah is
   the flagship instance that migrates onto it (P4/AE-2). The runtime built
   here is the seed of that system, not a Sarah-only fork.
2. Conformance bar: identical tool inventory and behavior — the S-12 eval
   suite (qualification, honesty, discount-pressure, injection) plus the
   ported smokes are the oracle; they must pass unchanged before eve is
   removed.
3. Model/provider access moves off the Vercel AI Gateway key onto our own
   provider path (the key's removal path was already flagged in sarah#14;
   model pin + cost caps are owner-gated).

Exit: `eve` gone from dependencies; S-12 green on the owned runtime; the
gateway-key removal recorded (or its explicit short-term retention
re-receipted by the owner).

### SM-5 — production cutover

1. sarah.openagents.com served by the monorepo-built Cloud Run service
   (image from `openagents` source only); DNS/env/model-pin/cost-caps owner
   actions via NEEDS_OWNER (absorbs S-11).
2. S-12 suite + smokes green against the live deployment; the S-13
   follow-up schedules verified on the new runtime.
3. Vercel project decommissioned; interim deploy torn down.

Exit: production Sarah runs from public monorepo source on our cloud; no
Vercel in the serving path.

### SM-6 — retire the repo

`OpenAgentsInc/sarah` README/AGENTS pointed at the monorepo, marked read-only
historical (the #8591 pattern); workspace-root routing docs updated; the
roadmap doc-map row flipped.

Ordering: SM-0 → SM-1 → SM-2/SM-3 (parallel) → SM-4 → SM-5 → SM-6. P1's Track
C outbound work (OB-1..6) is independent and never blocks on this; the SR
exit receipts in the roadmap are unchanged — this plan changes where Sarah
lives and what renders her, not what she must prove.

## 4. Decisions recorded (and what they supersede)

1. **Separate-repo posture (2026-07-07) — reversed** by the 2026-07-09 owner
   directive. The original rationale ("newest voice-agent surface works
   unmodified before any monorepo integration") is spent: the realtime loop
   works, S-1..S-13 are majority-closed, and the remaining work is
   integration-shaped — exactly what the monorepo is for.
2. **sarah#14 (Cloud Run lift-and-shift of the Next app) — superseded.** We
   still land on Cloud Run, but there is no Next container to lift: the
   Bun/Effect port (SM-1) is the move. eve's self-host posture from #14
   survives as SM-1's interim brain arrangement.
3. **sarah#15 (TanStack Start port) — superseded** by SM-2 per the standing
   rev 5/6 rescope: the UI is authored in Effect Native, not ported to
   another React host.
4. **eve (DECIDED 2026-07-07) — retained through SM-3, retired in SM-4.**
   The 07-07 decision correctly bought speed; the rev 6 whole-app posture
   (Effect as the foundation) plus the P4 employee-record trajectory make an
   owned Effect runtime the destination. The S-12 suite is the safety rail
   that makes the swap honest.
5. **Email stack — converged, not ported** (§SM-3): the monorepo rail already
   enforces the approval-gated contracts; running a second Resend path would
   split the suppression list and the ledger. This is the single biggest
   dedup the consolidation buys.

## 5. Invariant and law notes

- `apps/sarah` lands with its own `INVARIANTS.md` section (or file):
  openagents.com API authority boundary; token-route hardening floor; AI disclosure always-on;
  no-improvised-pricing (deal-rules enforcement, S-9) carried forward; email
  only via the approval-gated rail.
- Behavior contracts stated for Sarah UX (disclosure, one-question-at-a-time,
  honesty grounding) register in the owning surface's behavior-contract
  registry as SM-2 lands — statement verbatim + oracle, per the standing
  mandate.
- Public-safe discipline: the persona is the public-scoped Artanis pattern
  and may be public; priced parameters, prospect PII, and provider keys never
  enter git (SM-0 is the gate).
- This is an engineering migration with a definition of done, so it follows
  the dated-plan-doc pattern (this doc), not a Product Spec — per
  `specs/CONVENTIONS.md`. Sarah's *product* intent (SR-1..3 receipts) already
  has its authority docs; if her market contract changes later, that change
  gets a spec.

## 6. Verification matrix

- Every ported route: contract-identical request/response tests.
- S-12 eval suite (qualification, honesty probes, discount-pressure,
  injection) green at: SM-1 local, SM-2 (UI swap), SM-4 (runtime swap), SM-5
  (live deployment) — it is the through-line oracle for the whole plan.
- S-3 token-guard smoke at every phase (the public-deploy blocker stays
  enforced).
- Deal-rules property tests (no unruled price reachable) unchanged and green.
- EN snapshot/intent tests + visual baseline for the SM-2 surface.
- Email: suppression + approval-receipt tests on the converged rail.
- Cutover: production smoke + follow-up schedule receipt on the new runtime.

## 7. Definition of done

No production Sarah behavior builds from `OpenAgentsInc/sarah`; the app is
Bun/Effect with an Effect Native UI and no React/Next/eve dependencies;
email flows through the single monorepo rail; the S-12 suite guards it all in
the normal sweep; sarah.openagents.com serves from monorepo-built images on
our cloud; the old repo is read-only historical.
