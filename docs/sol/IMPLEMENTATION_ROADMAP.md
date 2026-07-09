# Sol implementation roadmap — Sarah-first, grounded in current `main`

- Date: 2026-07-09
- Revision: 1
- Status: day-to-day implementation companion to
  [`MASTER_ROADMAP.md`](../fable/MASTER_ROADMAP.md) rev 6.19
- Source snapshot: `origin/main` at `93bfa6b7e3`; live GitHub audit: 30 open
  roadmap issues, no open PRs at inspection time
- Verification note: `bun run check:deploy` is not green at this snapshot. It
  stops in `check:effect-topology` because the four vendored Effect Native
  packages require Effect `4.0.0-beta.94` while the topology guard still
  permits only `4.0.0-beta.70` (plus its documented exceptions). This is a
  pre-existing EN integration residual under #8566 until a dedicated bug is
  filed; no Sol doc change caused it.

## Mandate

Fable owns high-level strategic planning. Sol owns the grounded implementation
translation: current-state reconciliation, subsystem design, dependency-aware
slice selection, and the day-to-day roadmap beneath the strategic plan.

This document does not weaken issue acceptance criteria, invariants, product
promise gates, or owner-only actions. It does make Sarah-first operational by
changing how existing work is interpreted and by naming integration work that
is missing from the issue list.

The current master-roadmap queue head remains binding:

1. #8615 OAV-5
2. #8620 SQ-3
3. #8619 SQ-2
4. #8616 OAV-6
5. #8610 OAV epic closeout

Far-forward work may continue only on non-colliding paths and should use the
priorities below.

## What changed under the Sarah-first implementation reading

The phase mechanics remain valuable, but their implementation target changes:

- Sarah's conversation and Blueprint canvas become the primary integration
  surface, not merely another consumer.
- The first Sarah→coding vertical slice becomes an explicit missing lane.
- Mobile Effect Native work is not just a framework rewrite; it must make
  Sarah the home relationship and prove cross-device continuation.
- Khala Code desktop and Pylon remain specialist power tools that project the
  same runs Sarah sees.
- CX work continues to build custody and cloud execution, but the default
  product entry for those capabilities becomes Sarah.
- Outbound identity and reply handling move from the tail of a generic queue
  into an active parallel revenue/relationship lane.
- Broad legacy-route conversion remains important but does not block closing
  the Sarah-to-outcome loop.

## Execution tracks

### Track A — make the front door dependable

Work strictly in the queue-head order unless the roadmap is explicitly
reordered:

- **#8615:** integrate QA-passed pre-rendered takes for the opener and semantic
  cache hits.
- **#8620:** complete the opener library defects, missing scripts, playback
  verdicts, and owned-renderer integration.
- **#8619:** select exactly one real-time and one offline recipe from the
  experiment matrix; avoid an endless model bake-off.
- **#8616:** run value-gated alternatives only after the matrix establishes a
  baseline and decision rule.
- **#8610:** close only when quality, integration, and receipts agree.

Implementation rule: the reliable text path and deploy simulator remain the
availability floor. Offline beauty work must not make live conversation less
reliable.

### Track B — prove Sarah can perform one real job

**Missing tracked lane: `SARAH-CODE-1`.** The high-level roadmap requires the
vertical slice but the 30-item queue has no issue that owns it end to end.
Before implementation begins, create or designate one concrete lane with this
contract:

> An authenticated owner asks Sarah to run a bounded public issue on their
> linked Pylon; Sarah invokes the existing typed
> Khala→Pylon→Codex/worker workflow, projects resumable progress into the
> Blueprint canvas, renders the verified closeout and exact usage receipt, and
> accepts a follow-up in the same conversation.

Scope it as integration, not a new executor:

- add one typed Sarah tool/intent for the existing coding workflow;
- resolve owner-linked Pylon/account refs server-side;
- use durable request/assignment refs as the public-safe state spine;
- project bounded progress, blocker, verification, and closeout events onto the
  existing Sarah SSE/Blueprint path;
- never expose raw prompts, shell output, local paths, or provider payloads;
- keep the operator-exemption/dogfood gate until product authority is explicit;
- add behavior contracts and a fixture E2E before the live run.

This slice can be built against the existing owner-local Pylon rail. It does
not wait for CX-3's managed Agent Computer path. Production generalization does
wait for #8600's persona-neutral, receipted Khala inference.

### Track C — productionize the relationship and transaction loop

- **#8600 KHS-1:** move both Sarah brain paths through the Khala gateway;
  remove raw provider keys from `apps/sarah`; prove sustained speech, exact
  receipts, caps, and fallback behavior. This is a production gate for Track B,
  not a replacement for it.
- **#8607 KHS-8:** close a code-priced, authenticated purchase inside the
  conversation with a dereferenceable settled receipt.
- **#8543 P0.8:** run the unattended mobile straight line on both platforms
  when the owner supplies the seeded account; preserve the promise/copy gate.
- **#8467:** close the mobile MVP epic only after its proof set, not because
  Sarah-first changed the narrative.

### Track D — revenue and relationship continuity, active in parallel

These items should not wait behind the full EN/Pylon/CX tail:

- **#8558 OB-1:** finish opt-out round trip, warm-up cap refusal, and
  deliverability webhook→ledger proof for the already armed Sarah identity.
- **#8561 OB-4:** finish live reply ingestion into Sarah + CRM and prove the
  draft→batch approval→send loop at the target volume with measured operator
  time.

Both lanes preserve the approval-gated send law. They can run in parallel with
Track A when they do not touch the same Sarah hot paths.

### Track E — one application grammar

Prioritize Effect Native work by Sarah-first leverage:

1. **#8597 MB-EN:** add Sarah-as-mobile-home and authenticated conversation
   continuation to the port plan before navigation-root conversion. Preserve
   the existing P0 app throughout. The named cross-app Khala Sync proof remains
   necessary but should include a Sarah-originated conversation/run state, not
   only a generic chat message.
2. **#8573 EN-4:** continue disjoint route conversion (`/terms` next, then
   public/funnel batches) without blocking Sarah integration. Convert and
   delete; no local primitives.
3. **#8574 EN-5:** convert desktop as a specialist cockpit over the same typed
   run and intent model. Do not recreate a second product home.
4. **#8575 EN-6:** move Blueprint/graph/canvas semantics under the shared
   renderer contract while preserving evidence-backed edges.
5. **#8566:** close only after downstream surfaces actually run on the
   substrate.

Owner-only residuals:

- **#8571:** Search Console/domain verification for effectnative.org; do not
  redispatch implementation unless a new code blocker appears.
- **#8595:** copy/brand approval and root cutover; the catalog conversion is
  landed. Do not keep rewriting the landing while it waits.

### Track F — harden the execution engine

- **#8578 PY-1:** continue leaf extraction and top-of-graph convergence;
  preserve the Spark wallet and require RC-binary verification for its move.
  Exit is typed RPC consumption and deletion of stdout parsing, not package
  count.
- **#8547 CX-3:** the managed-cloud linchpin. Build a reproducible rootfs,
  broker redemption, scratch `CODEX_HOME`, `codex_app_server`, and a live
  Firecracker receipt. Do not confuse the control host's fake VM proof with the
  nested-virt execution host.
- **#8549 CX-5:** UI/CLI custody is landed; hold further surface work. Run the
  live `claude_pylon` Firecracker proof after CX-3.
- **#8588 MH-9:** hold behind CX-3; then prove Grok/Claude cloud parity through
  the same contract rather than adding per-harness cloud paths.
- **#8550/#8551:** fixture tiers are useful; live continuity and concurrency
  proofs follow CX-3.
- **#8552:** daily-driver steering and monorepo-scale evidence follow stable
  cloud execution.
- **#8553:** preserve the five-task mobile dogfood exit and add at least one
  Sarah-originated, Sarah-closed task once `SARAH-CODE-1` exists. Power-tool
  tasks may still prove engine mechanics during transition.

Owner-proof holds:

- **#8546 CX-2:** code-complete; owner device-auth re-test.
- **#8548 CX-4:** code-complete; live target/fallback proof depends on owner
  auth plus CX-3.

### Track G — cockpit consolidation and retirement

- **#8579 PY-2:** begins after enough PY-1 typed RPC and EN-5 substrate exists.
  The cockpit is a deep projection of Sarah-visible fleet state: accounts,
  capacity, runs, approvals, receipts.
- **#8580 PY-3:** retire OpenTUI only after cockpit parity and owner receipt.
  Retirement is an exit step, not an implementation shortcut.

## Forward phases: implementation reinterpretation

The 30-item queue does not yet enumerate most P3–P7 work. Their implementation
shape should be corrected now so future issues do not recreate pre-Sarah-first
surface assumptions.

### P3 — standing employees become durable Sarah roles first

- Represent the responsibility as `agent_definition.v1`, not a new bespoke
  agent app.
- Expose schedule, budget, last run, next action, and approval state through
  Sarah's canvas and the shared event-ledger inbox.
- Compile tool authority before dispatch; a persona or role label grants
  nothing.
- Prove one seven-day responsibility on the same execution and receipt rails
  used by interactive work.
- Add a separately named colleague only when role confusion or UX evidence
  shows Sarah should delegate, not merely because another template exists.

### P4 — the company brain becomes a Blueprint Map migration

- Promote Sarah's existing prospect/owner memory into versioned,
  provenance-bearing `company_brain.v1` collections.
- Promote Sarah onto `ai_employee.v1`; do not create a parallel formal employee
  that later has to absorb her.
- Keep reads role-scoped and external writes behind Action Submissions.
- Make correction, deletion, access explanation, and source inspection part of
  the canvas before broad ingestion.
- Prove two roles with disjoint brain slices before claiming a shared company
  brain.

### P5 — templates are extracted proven configurations

- A template packages a role program, tool policy, brain slice, schedule,
  budget, verification rubric, and authority floor.
- Extract the first template from a role Sarah has already performed with
  external outcome receipts.
- Keep customer and vertical variation as config, never code forks.
- Route GitHub, Slack, CRM, email, and other connectors through brokered grants
  and the same approval model.
- Treat time to first verified receipt—not “agent created”—as activation.

### P6 — trust controls land on the Sarah path first

- Run skill-registry provenance, injection audit, and capability manifests on
  Sarah's own active toolset before marketing the registry.
- Enforce the untrusted-input authority ceiling on inbound email and retrieved
  content through shared policy, not prompt warnings.
- Put data-posture and canary results into bounded receipt projections Sarah
  can explain without exposing private audit material.
- Make permission and data-scope explanation available at the point of action.

### P7 — scale extends the relationship, not the surface count

- Complete business-dashboard depth as canvas/cockpit projections over shared
  state rather than a separate authority stack.
- Use assessment results to prefill Blueprint with explicit source refs and
  consent boundaries.
- Graduate outcomes into public stories, forum identity, referrals, and routed
  work only through consented receipt projections.
- Keep operator-minutes per outcome as the scaling falsifier.
- Introduce separate suite names or employee identities only where customers
  demonstrably need a distinct workflow or trust boundary.

## All open roadmap items: Sarah-first disposition

| Issue | Sarah-first role | Current grounded status | Day-to-day disposition |
| --- | --- | --- | --- |
| #8615 | Fast, polished front-door responses | Open; queue head | **NEXT, serial** |
| #8620 | Perfect opener/standard takes | Open; follows #8615 | **Serial Track A** |
| #8619 | Choose production quality recipes | Open | **Serial Track A; decision, not endless research** |
| #8616 | Value-gated renderer/model ladder | Open | **After matrix baseline** |
| #8610 | Owned-avatar program closure | Children remain | **Closeout only** |
| #8600 | Production Sarah brain on Khala | Raw-key removal/live proof remain | **Core production gate** |
| #8607 | Conversation→settled purchase | Open | **Core transaction slice** |
| #8543 | Unattended mobile launch proof | Owner-seeded account gated | **Owner gate + E2E** |
| #8467 | Mobile MVP program closure | P0.8 open | **Closeout only** |
| #8571 | Effect Native public hosting | Code deployed; owner domain verification | **Owner-only residual** |
| #8595 | EN marketing landing | Catalog landed; copy/root flip remain | **Owner-only residual** |
| #8573 | Legacy web absorption | Inventory + `/download` + `/privacy` landed | **Parallel route slices; non-blocking** |
| #8597 | Sarah in the pocket on one UI model | Full rewrite unstarted/early; substrate epic upstream | **Promote within EN; add Sarah-home acceptance** |
| #8574 | Specialist desktop cockpit substrate | First cockpit proof exists; full conversion open | **After/with PY typed RPC** |
| #8575 | Shared Blueprint/canvas renderer | Open | **Needed for long-term canvas unity, not first slice** |
| #8566 | Effect Native adoption closure | Multiple children open; deploy topology guard is stale against EN `beta.94` | **Closeout + integration guard repair** |
| #8578 | Stable Pylon engine beneath Sarah/power tools | Large leaf set landed; top graph/RPC/wallet residual | **Active parallel engine lane** |
| #8579 | Deep fleet power tool over shared state | Depends PY-1 + EN-5 | **Dependency-held** |
| #8580 | Remove duplicate TUI surface | Depends cockpit parity | **Owner-gated closeout** |
| #8547 | Managed Agent Computer for user Codex | Live Firecracker/rootfs proof missing | **Execution linchpin** |
| #8588 | Cloud multi-harness parity | Strictly depends #8547 | **Hold** |
| #8546 | Connect user's Codex from phone | Code-complete | **Owner re-test; no agent redispatch** |
| #8548 | Target/harness selection | Code-complete; live proof pending | **Owner/CX-3 proof hold** |
| #8549 | Claude parity | Mobile + CLI landed; live cloud turn missing | **Hold behind #8547** |
| #8550 | Resume across ephemeral VMs | Fixture tier done | **Live proof after #8547** |
| #8551 | Account concurrency/rotation | Fixture tier done | **Live proof after #8547** |
| #8552 | Daily-driver steering | Open | **After stable execution; feed Sarah intents** |
| #8553 | Mobile dogfood cutover | Ledger seeded; owner day missing | **Exit proof; add Sarah-originated subset** |
| #8558 | Sarah outbound identity health | Live send armed; deliverability residual | **Active parallel revenue lane** |
| #8561 | Sarah reply + approval-at-volume | Batch approval landed; live reply/volume residual | **Active parallel revenue lane** |

## Integration acceptance gates

Every Sarah-first implementation slice must answer these before landing:

1. **Entry:** Which conversation or canvas intent starts it?
2. **Identity:** Which authenticated relationship and owner scope does it use?
3. **Authority:** Which service—not the model—authorizes the action?
4. **State:** Which durable record and Khala Sync scope carry progress?
5. **Execution:** Which existing typed workflow performs the work?
6. **Failure:** Which typed blockers and retry rules appear to the user?
7. **Evidence:** Which exact/private receipts prove completion?
8. **Projection:** What bounded, public-safe state appears in Sarah?
9. **Memory:** Which Blueprint facts may update, with what provenance?
10. **Parity:** How do mobile and desktop observe or steer the same work?
11. **Verification:** Which unit, contract, fixture, live, and deploy gates run?
12. **Deletion:** Which duplicate or legacy path can be removed now?

## Update rule

Sol should update this file after any landing that changes:

- the first ready implementation slice;
- a dependency or owner gate;
- an issue's code-complete versus proof-complete status;
- the Sarah-first acceptance criteria for a subsystem;
- an alternate path that can now be deleted.

Do not append a revision diary indefinitely. Reconcile the tables and keep a
short revision note at the top so this remains a usable daily roadmap.
