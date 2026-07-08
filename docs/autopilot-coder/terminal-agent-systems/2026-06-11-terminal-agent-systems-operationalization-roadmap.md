# Terminal Agent Systems Operationalization Audit, Recommendations, And Roadmap

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

## Scope

This document decides how to operationalize the terminal-agent systems audits
relative to the live Autopilot MVP issue sprint tracked by #4786.

Inputs reviewed:

- #4786 via `gh issue view`, including the Codex executor and Agent Runtime
  Kernel addenda.
- Current #4755-#4809 issue state via `gh issue list`.
- `docs/autopilot-coder/README.md`.
- `docs/autopilot-coder/2026-06-11-autopilot-unified-audit-roadmap.md`.
- `docs/autopilot-coder/terminal-agent-systems/README.md`.
- `docs/autopilot-coder/terminal-agent-systems/2026-06-11-terminal-agent-systems-index.md`.
- The dated terminal-agent subsystem audits in this directory, with closer
  reads on the runtime kernel, worktree materialization, task/background
  execution, scheduling, notifications, artifacts/receipts, structured event
  log, permissions, Git/GitHub, external intake, testing/smokes, credentials,
  settings, security, telemetry, and retention audits.

This is a planning document only. It does not create product promises, widen
public claims, or change runtime invariants.

## Current State From The Issue Ladder

As of the `gh` check on 2026-06-12:

- Closed: B1-B4 (#4755-#4758), M1-M9 (#4759-#4767), M11-M13
  (#4769-#4771), A1-A4 (#4773-#4776), P2-P4 (#4778-#4780), P8-P9
  (#4784-#4785), CX1-CX5 (#4788-#4792), the Codex executor epic (#4793),
  worktree/materializer support (#4798, #4799), RK1-RK5 (#4805-#4809), the
  Agent Runtime Kernel epic (#4804), Pack A (#4813-#4823), Pack B
  (#4824-#4830), Pack C (#4831-#4835), and the public hygiene follow-ups
  #4836/#4837.
- Open: M10 overnight proof (#4768), M14 door-open decision (#4772), P1 first
  live negotiated labor job (#4777), P5-P7 market-provider proof
  (#4781-#4783), and the parent epic #4786. W3 (#4749) remains open as a
  separate research/evaluation track and is not an Autopilot MVP door-open
  dependency.

The practical result: the sprint has already landed the base dogfood loop,
own-Pylon/free-lane policy, cloud Pylon, card-on-file, provider/account
hardening, repo/delivery evidence, the Codex and runtime-kernel lanes, agent
parity contracts, and public freshness/order-book hygiene. The remaining work
is not another broad operationalization pack. It is live evidence: overnight
unattended proof, the MVP exit decision, and market-provider/settlement
receipts.

## Tracking Issues

Pack A was filed as GitHub issue #4813, with one child issue for each
subsystem audit that needed MVP proof and supervision operationalization
during the #4786 sprint:

| Pack issue | Subsystem audit                        | Primary sprint pressure                                                                |
| ---------- | -------------------------------------- | -------------------------------------------------------------------------------------- |
| #4814 PA1  | Task And Background Execution          | Scheduled/background run supervision for #4764, #4765, #4768, and #4773                |
| #4815 PA2  | Scheduling And Cron                    | Schedule, continuation, skip, and no-double-fire receipts for #4764, #4768, and #4773  |
| #4816 PA3  | Notifications And Attention            | Decision/completion/failure attention state for #4765, #4768, and #4773                |
| #4817 PA4  | Mobile And Web Companion               | Phone-sized decision/review/status projection for #4765, #4768, and #4773              |
| #4818 PA5  | Testing And Smoke                      | Proof-boundary and receipt discipline for #4767, #4768, #4772, and #4773               |
| #4819 PA6  | Artifact And Receipt                   | Evidence refs for schedules, tasks, decisions, notifications, reviews, and smokes      |
| #4820 PA7  | Structured Event Log                   | Replayable Pack A projections across web, Pylon, API, companion, and public-safe views |
| #4821 PA8  | Token And Cost Budgeting               | Budget stops, usage refs, rate-limit blockers, and own-Pylon zero-credit proof         |
| #4822 PA9  | Permission And Approval                | Shared approval contract for headless, background, mobile, Pylon, and API actions      |
| #4823 PA10 | Accessibility And Non-Interactive Mode | Structured output, CI/headless behavior, no-color status, and typed prompt blockers    |

Pack A is now implemented and closed. The parent #4813 body carries the Pack A
checklist, and the #4786 epic has the matching Pack A addendum comment.

## Pack B Readiness And Tracking Issues

As of the Pack B `gh issue list` check on 2026-06-11, the open Autopilot issue
set had narrowed to #4768, #4771, #4772, #4777, #4781, #4782, #4783, and the
parent #4786. The separate W3 evaluation issue #4749 also remained open, but it
was not part of the Autopilot MVP proof gate.

Pack A (#4813-#4823) is closed as a tracked issue set. M8 account-pool
visibility (#4766) and M9 rate-limit proof (#4767) were also closed. M13
provider peers (#4771) was still open then, and its issue comments identified
the remaining live non-Codex credentialed-run proof as the unresolved part of
that provider-peer lane. #4771 later closed on its scoped Gemini live-provider
proof; Pack B remains the evidence layer future provider/account claims should
cite, not a continuing blocker by itself.

That made Pack B ready to file at that point. The reason was not that Pack B
should block all remaining work; it was that the roadmap already treated
account, credential, and policy hardening as the prerequisite for broad
provider-peer claims. Pack B ran in parallel with the remaining #4786 Gate work
and now affects timing only where an issue depends on provider credentials,
account telemetry, managed policy state, retention guarantees, or
provider-peer security review.

Pack B was filed as GitHub issue #4824, with one child issue for each
subsystem audit that should harden the provider/account/policy lane:

| Pack issue   | Subsystem audit                           | Primary sprint pressure                                                                                                     |
| ------------ | ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| #4824 PACK B | Account, Credential, And Policy Hardening | Parent tracker for provider/account hardening after M8/M9 and before broad M13 claims                                       |
| #4825 PB1    | Authentication And Credential Storage     | Provider accounts, credential refs, leases, refresh, revocation, and stale-lease invalidation for #4771                     |
| #4826 PB2    | Settings And Configuration                | Effective config snapshots for provider, budget, approval, telemetry, retention, and routing decisions                      |
| #4827 PB3    | Security Review                           | ToS/credential-boundary/threat review before broad provider-peer work closes                                                |
| #4828 PB4    | Telemetry And Privacy                     | Redacted account-health, rate-limit, provider-routing, and reconnect telemetry fixtures                                     |
| #4829 PB5    | Data Retention And Deletion               | Retention classes, deletion behavior, tombstones, and projection invalidation for Pack B records                            |
| #4830 PB6    | Enterprise Managed Policy                 | Minimal policy snapshots and denial reasons for team, repo, provider, budget, retention, telemetry, and approved-user gates |

Final implementation pass:

- On 2026-06-12, #4825-#4830 were implemented, pushed to `main`, commented,
  and closed. #4824 is also closed as the Pack B parent.
- Pack B now supplies typed evidence surfaces for credential boundaries,
  effective configuration, security review, telemetry/privacy,
  retention/deletion, and managed-policy snapshots.
- Pack B does not prove live non-Codex provider readiness by itself. #4771
  later closed the scoped Gemini credentialed-run proof; future broad
  provider-peer claims must cite the #4771 closeout plus Pack B refs or a
  scoped exception.

Implementation status:

- #4825 PB1 is implemented by
  `apps/openagents.com/workers/api/src/provider-account-credential-boundary.ts`
  and
  `apps/openagents.com/workers/api/src/provider-account-credential-boundary.test.ts`.
  It defines the ref-only credential boundary over accounts, grants, active
  leases, artifacts, and receipts, including typed stale/revoked credential
  blockers and cache invalidation refs.
- #4826 PB2 is implemented by
  `apps/openagents.com/workers/api/src/provider-account-effective-config.ts`
  and
  `apps/openagents.com/workers/api/src/provider-account-effective-config.test.ts`.
  It resolves provider/account settings through explicit precedence into safe
  effective config refs, source-layer/value-tag projections, and typed
  blockers for missing or invalid required settings.
- #4827 PB3 is implemented by
  `apps/openagents.com/workers/api/src/provider-account-security-review.ts`,
  `apps/openagents.com/workers/api/src/provider-account-security-review.test.ts`,
  and `docs/autopilot-coder/2026-06-11-provider-peer-security-review.md`.
  It gates broad provider-peer claims on ToS, credential, threat, telemetry,
  retention, redaction, revocation, and high-risk control refs while preserving
  scoped-exception blockers.
- #4828 PB4 is implemented by
  `apps/openagents.com/workers/api/src/provider-account-telemetry-privacy.ts`,
  `apps/openagents.com/workers/api/src/provider-account-telemetry-privacy.test.ts`,
  and
  `docs/autopilot-coder/2026-06-11-provider-account-telemetry-privacy-fixtures.md`.
  It defines aggregate, local-only, and opt-out telemetry projections with
  freshness metadata, redaction fixture blockers, and ref-only debug/support
  bundle boundaries.
- #4829 PB5 is implemented by
  `apps/openagents.com/workers/api/src/provider-account-retention-policy.ts`,
  `apps/openagents.com/workers/api/src/provider-account-retention-policy.test.ts`,
  and
  `docs/autopilot-coder/2026-06-11-provider-account-retention-deletion-rules.md`.
  It declares Pack B data-class retention, deletion behavior, projection/cache
  invalidation, lease blockers, reconnect actions, tombstones, receipts, and
  retained audit refs.
- #4830 PB6 is implemented by
  `apps/openagents.com/workers/api/src/provider-account-managed-policy.ts`,
  `apps/openagents.com/workers/api/src/provider-account-managed-policy.test.ts`,
  and
  `docs/autopilot-coder/2026-06-11-provider-account-managed-policy-snapshots.md`.
  It resolves minimal organization, team, repository, user, device/local,
  provider, budget, retention, and telemetry refs into stable effective policy
  snapshots with typed allow, deny, stale, and unknown outcomes.

## Timing Impact On Other Open Issues

Pack A, Pack B, and Pack C are acceptance overlays, not replacement ladders.
They should change when broader claims can close, not stop unrelated work.

- **Do not pause unrelated open rungs.** M8 account-pool work, M11 repo/scope,
  M12 team budgets, M13 provider review, A2-A4, and the post-MVP P-rungs can
  continue when they do not depend on unattended execution, decision
  notifications, proof smokes, or headless/API parity.
- **Close product-surface rungs on their own scoped acceptance, but leave Pack
  A follow-up issues open when the proof/supervision contract is not complete.**
  For example, M6 and M7 can stay closed as product features while #4815,
  #4816, #4817, #4820, #4822, or #4823 carry the remaining operational
  hardening.
- **Do not close MVP-gating proof or door-open issues until the relevant Pack A
  children are satisfied.** M10 (#4768), M14 (#4772), and the proof side of A1
  (#4773) should wait on the Pack A receipts they cite, especially #4814,
  #4815, #4816, #4818, #4819, #4820, #4821, #4822, and #4823.
- **Treat M9 (#4767) as a split proof.** Its CI-safe deterministic leg can stay
  documented, but live smart-routing copy should wait on the live two-account
  proof plus the Pack A smoke/artifact/event/usage receipts (#4818-#4821).
- **Avoid retroactively reopening closed rungs only to hold operational debt.**
  Use the PA issues for that debt, cross-link the original rung, and update
  closeout notes if a claim boundary needs clarification.
- **Pack B is implemented and applies to provider/account/policy claims.**
  Do not reopen M8 or M9 only to hold Pack B debt. Do cite #4825-#4830 for
  future work that depends on provider credential refs, account telemetry,
  policy denial reasons, or retention guarantees. #4771 is now closed; do not
  use Pack B to broaden provider-peer claims beyond the refs or scoped
  exceptions named in that closeout.
- **Pack C is implemented and applies to repo/delivery evidence claims.** Do
  not reopen M11 or P3 only to hold Pack C debt. Do cite #4832-#4835 when
  future writeback, market, proof, or order-book work depends on repo identity,
  worktree identity, change capture, workspace authority, or delivery
  readiness receipts.
- **Do not close market or gate issues on Pack C alone.** Pack C is not live
  PR writeback, live labor settlement, accepted work, payout, or the #4772
  door-open proof. #4777, #4781, #4782, and #4783 still need live market or
  provider receipts; #4768 and #4772 still need MVP proof receipts.
- **Do not file Pack D yet.** As of the 2026-06-12 follow-up review, #4768
  and #4772 are still open. #4836/#4837 closed the public
  freshness/order-book hygiene pair, and the live order book is now correctly
  empty after the stale #4773-backed request was expired. That is cleaner, but
  it is not market proof: #4777/#4781 still need a fresh currently-open target
  plus independent provider quote, execution, validation, release, payout, and
  settlement receipts before broad intake/market unification is well-scoped.
- **Keep #4786, #4768, and #4772 focused on their proof gates.** They should
  cite Pack B only when their proof evidence relies on provider credentials,
  account telemetry, managed policy snapshots, or retention/deletion behavior,
  and cite Pack C only when their proof evidence relies on repo/delivery
  evidence.

## Pack C Readiness And Tracking Issues

As of the Pack C `gh issue list` check on 2026-06-12, M11 repo/data-scope
UX (#4769), M12 spend-to-evidence joins (#4770), and P3 writeback symmetry
(#4779) are closed as backend/contract slices. Their issue closeouts still
defer live self-serve repo connection UI proof, live real-repo PR draft proof,
and downstream receipt evidence. The open issue set had narrowed to the Gate
proof and market/live-evidence lanes when Pack C was filed. At final Pack C
closeout, #4771 was closed and the remaining open Autopilot set was #4768,
#4772, #4777, #4781, #4782, #4783, and #4786, plus W3 #4749. The later
#4836/#4837 public-hygiene follow-ups are also closed.

That made Pack C ready to file. Pack C should not reopen M11/P3 or claim live
PR writeback by itself. It hardens the repo/delivery evidence layer that
future writeback, market, and proof issues need: repository/worktree identity,
bounded change capture, file/shell/workspace authority, and delivery/PR
readiness receipts.

Pack C was filed as GitHub issue #4831, with one child issue for each
subsystem slice that should harden repo scope and delivery evidence:

| Pack issue   | Subsystem audit                                  | Primary sprint pressure                                               |
| ------------ | ------------------------------------------------ | --------------------------------------------------------------------- |
| #4831 PACK C | Repo Scope, Delivery, And Evidence               | Parent tracker for repo/delivery hardening after M11/M12/P3 contracts |
| #4832 PC1    | Git/GitHub, Worktree, Repository Memory          | Repository/worktree identity snapshots for scoped repo work           |
| #4833 PC2    | Diff/Patch Review, Artifact/Receipt, Diagnostics | Change capture and diff review artifacts                              |
| #4834 PC3    | File Tool, Shell Execution, Sandbox/Workspace    | File/shell/workspace authority boundary for delivery evidence         |
| #4835 PC4    | Git/GitHub Workflow, Artifact/Receipt, Writeback | Delivery authority and PR readiness receipt projections               |

Timing rule:

- Pack C should advance #4777, #4781, #4782, and #4783 market evidence and
  future #4779-style writeback claims.
- Pack C is not a live PR writeback claim, live labor settlement claim, or
  #4772 door-open proof by itself.
- #4768, #4772, and #4786 should cite Pack C only when their proof evidence
  depends on repo identity, change capture, file/shell/workspace evidence, or
  delivery/PR readiness receipts.

Final implementation pass:

- On 2026-06-12, #4832-#4835 were implemented, pushed to `main`, commented,
  and closed. #4831 is also closed as the Pack C parent.
- Pack C now supplies typed evidence surfaces for repository/worktree identity,
  bounded change capture, workspace authority, and delivery/PR readiness.
- No additional Pack C child issues are needed now. The next issue set should
  not be Pack D until #4768/#4772 MVP proof is closed or explicitly scoped.
  The immediate public freshness/order-book hygiene pair is now closed:
  #4836 covers product-promises freshness and announcement readiness, while
  #4837 covers Forum work-request closed-issue admission, live-at-read
  freshness, and expiration of the stale #4773 row.

Follow-up hygiene status:

- #4836 is implemented by adding `generatedAt`, `registryVersion`,
  `maxStalenessSeconds`, and a `live_at_read` staleness contract to
  `/api/public/product-promises`, plus an announcement-readiness helper that
  blocks announcement copy when the announced version does not match the
  served version.
- #4837 is implemented in `9730f6728`: `/api/forum/work-requests` now carries
  `generatedAt`, `maxStalenessSeconds`, and the shared `live_at_read`
  staleness contract; `buildBacklogWorkRequestFiling` rejects closed GitHub
  issues before producing an open-market work request; and the stale live
  #4773-backed request was expired by lifecycle receipt.
- No additional follow-up issues were created on 2026-06-12 from this review.
  #4836/#4837 were the missing public hygiene blockers. The remaining
  decision is sequencing: keep Pack D unfiled until #4768 and #4772 either
  close on Pack C evidence or are narrowed into explicit remaining proof
  slices.

Second 2026-06-12 review:

- Rechecked #4749, #4768, #4772, #4777, #4781, #4782, #4783, and #4786 with
  `gh issue view --comments`.
- No additional GitHub issues should be opened now. The open tail is blocked
  on live/operator evidence, independent provider participation, settlement
  receipts, or W3 training completion, not on missing issue decomposition.
- The next issue-creation point is after #4768/#4772 close or narrow: file
  Pack D only if the resulting market/intake work has a concrete acceptance
  slice beyond the already-open #4777/#4781/#4782/#4783 issues.

Third 2026-06-12 review:

- Rechecked the current open tail and recent comments with `gh issue view`.
- No additional issues were opened. The existing open issues already own the
  live proof and market evidence work; creating another issue now would
  duplicate #4768/#4772 or #4777/#4781/#4782/#4783.
- Updated this roadmap's historical Pack A/B/C wording so closed parents do
  not read like pending action items.

Implementation status:

- #4832 PC1 is implemented by
  `apps/openagents.com/workers/api/src/pack-c-repo-worktree-identity.ts`,
  `apps/openagents.com/workers/api/src/pack-c-repo-worktree-identity.test.ts`,
  and `docs/autopilot-coder/2026-06-12-pack-c-repo-worktree-identity.md`.
  It defines safe repository/worktree identity projections with freshness
  metadata, typed blockers, branch-ref parsing, and private material
  rejection.
- #4833 PC2 is implemented by
  `apps/openagents.com/workers/api/src/pack-c-change-capture.ts`,
  `apps/openagents.com/workers/api/src/pack-c-change-capture.test.ts`, and
  `docs/autopilot-coder/2026-06-12-pack-c-change-capture.md`. It defines
  digest-and-summary-only change capture projections with verification,
  diagnostic, caveat, authority, freshness, and typed blocker refs.
- #4834 PC3 is implemented by
  `apps/openagents.com/workers/api/src/pack-c-workspace-authority.ts`,
  `apps/openagents.com/workers/api/src/pack-c-workspace-authority.test.ts`,
  and `docs/autopilot-coder/2026-06-12-pack-c-workspace-authority.md`. It
  defines file/shell/workspace authority projections with command, path,
  approval, sandbox, timeout, cancellation, redaction, and typed denial refs.
- #4835 PC4 is implemented by
  `apps/openagents.com/workers/api/src/pack-c-delivery-readiness.ts`,
  `apps/openagents.com/workers/api/src/pack-c-delivery-readiness.test.ts`,
  and `docs/autopilot-coder/2026-06-12-pack-c-delivery-readiness.md`. It
  defines delivery readiness and PR draft receipt projections over repository
  identity, worktree identity, change capture, verification, writeback
  authority, review, and human-merge caveat refs while keeping market/agent
  delivery separate from merge, acceptance, settlement, payout, and
  public-claim authority.

## Audit Finding

The terminal-agent folder is a dependency graph, not a flat backlog. The 62
audits cluster into five operational sets:

1. **Runtime truth set**: Agent Runtime Kernel, Structured Event Log,
   Conversation/Query, Tool Registry, Context Assembly, Compaction, Token/Cost
   Budgeting, Model Provider Abstraction, Prompt/Instruction Layering, Error
   Taxonomy, and Plan/Todo state. These define the event-sourced truth the rest
   of the product should read.
2. **Authority and workspace set**: Permission/Approval, Sandbox/Workspace
   Boundary, File Tool, Shell Execution, Worktree/Workspace Materialization,
   Settings/Configuration, Authentication/Credential Storage, and Security
   Review. These keep local, hosted, and delegated work from inventing
   separate safety policies.
3. **Operational proof set**: Task/Background Execution, Scheduling/Cron,
   Notifications/Attention, Mobile/Web Companion, Testing/Smoke, Telemetry/
   Privacy, Performance, Migration, Update/Release, and Data Retention. These
   decide whether the product can run while the operator is away and still
   prove what happened.
4. **Delivery and evidence set**: Git/GitHub Workflow, Diff/Patch Review,
   Artifact/Receipt, External Work Intake, Team/Shared Memory, Multi-Agent
   Coordination, and Repository Memory/Onboarding. These connect work orders to
   real repos, reviews, PR drafts, acceptance, and later market inventory.
5. **Extension and polish set**: MCP client/server, Plugin, Skill, Hook/Event,
   IDE/Editor, Browser/Desktop, Voice/Multimodal, Command/Input/TUI, Prompt
   Suggestions, Tips, Theme, Accessibility/Non-Interactive, Localization, and
   Enterprise Policy. These are important, but most should consume the runtime
   and authority sets rather than define new authority paths.

The live issue ladder has already operationalized much of set 1 through RK1-
RK5 and much of the workspace slice through #4798/#4799. The open MVP issues
now stress set 3 first: M6 scheduling/continuation, M7 decisions/
notifications, M9 live rate-limit proof, M10 overnight unattended proof, and
A1 API parity. If those ship without the task, schedule, notification, smoke,
artifact, and event-log contracts, the product will have useful features but
weak receipts.

## Recommendation

Operationalize the audits in packs, not one subsystem at a time.

### Pack A: MVP Proof And Supervision, During The Current Sprint

Operationalize this pack immediately, alongside M6, M7, M9, M10, and A1:

- Task And Background Execution
- Scheduling And Cron
- Notifications And Attention
- Mobile And Web Companion
- Testing And Smoke
- Artifact And Receipt
- Structured Event Log
- Token And Cost Budgeting
- Permission And Approval
- Accessibility And Non-Interactive Mode

Why this pack first:

- M6 cannot be honest without schedule records, continuation policy, budget
  gates, skip/fired receipts, and no double-fire behavior.
- M7 cannot be honest without typed attention events, decision projections,
  remote/mobile-safe approval state, and public-safe notification payloads.
- M10 cannot be accepted as an overnight proof without task supervision,
  scheduling receipts, notification receipts, and replayable event evidence.
- A1 cannot be enforced if browser status, terminal status, and agent API
  status are not projections over the same event and receipt records.
- Non-interactive/headless behavior is not polish here; it is the difference
  between unattended execution and a hidden prompt waiting forever.

Acceptance shape for this pack:

- Every scheduled or continued run has a schedule or continuation receipt.
- Every background run has a task ref, output ref, artifact refs, terminal
  state, and exactly-once completion notification behavior.
- Every proof smoke states its boundary and emits public-safe receipts.
- Every new public or agent-readable projection carries `generatedAt` and the
  applicable staleness metadata under the #4751/#4800 law.
- Every new API route or route shape is represented in the served OpenAPI
  contract under #4752.
- Every no-prompt/headless blocker is a typed denial or waiting state, not an
  indefinite hang.

### Pack B: Account, Credential, And Policy Hardening, Start During M8/M13

Operationalize this pack as soon as M8 account-pool work starts, and treat it
as a prerequisite for broad provider-peer work in M13:

- Authentication And Credential Storage
- Settings And Configuration
- Security Review
- Telemetry And Privacy
- Data Retention And Deletion
- Enterprise Managed Policy, only as the typed policy shape needed for teams
  and approved-user gates

Why this pack second:

- M8 shows provider account state, cooldowns, low-credit flags, and reconnect
  nudges. That surface is only safe if credentials, leases, refresh state, and
  redaction boundaries are represented as refs and effective-policy snapshots.
- M13 adds provider peers. The audit decision says credential storage is the
  trust root before more provider peers, and the roadmap already requires ToS
  review first.
- Team and design-partner use will need policy explanations before enterprise
  polish. The minimum is resolved policy snapshots and denial reasons, not a
  full managed-admin product.

Acceptance shape for this pack:

- Raw tokens and device credentials never enter mission, artifact, receipt, or
  public projection records.
- Account leases and credential refs attach to runs without exposing secrets.
- Provider-peer additions include ToS/credential-boundary review, redaction
  fixtures, and revocation behavior.
- Telemetry uses refs and aggregates rather than raw transcript, prompt,
  shell-output, or private-repo collection.
- Retention/deletion behavior is declared for every persisted data class added
  by M8, M13, and team-budget work.

### Pack C: Repo Scope, Delivery, And Evidence, Start During M11/P3

Operationalize this pack when M11 repo/data-scope UX and P3 writeback symmetry
begin, with early design input during M7/M10 because proofs need artifacts:

- Git And GitHub Workflow
- Diff And Patch Review UI
- File Tool System
- Shell Execution System
- Sandbox And Workspace Boundary
- Worktree And Workspace Materialization
- Artifact And Receipt
- Repository Memory And Onboarding
- LSP And Diagnostics, as optional typed context only

Why this pack third:

- The MVP already uses `git_checkout` and own-Pylon execution, but public
  issue-to-PR claims wait on P3. The Git audit correctly says low-level Git
  safety, repository identity, diff bounding, and review-thread ingestion are
  prerequisites for writeback symmetry.
- M11 needs per-mission scope declarations and placement explanations. That is
  the workspace boundary audit in product form.
- P3 needs change capture and delivery authority records, not shell-command
  transcripts.

Acceptance shape for this pack:

- Repository identity, worktree identity, branch refs, change captures, and
  delivery authority are typed records.
- PR draft writeback consumes artifact/change refs and emits delivery
  receipts.
- Branch names, refs, and Git metadata are safely parsed and never interpolated
  raw into shell commands.
- Public artifacts include summaries, digests, refs, and caveats, not private
  remotes, raw paths, raw shell logs, or private repo contents.
- Scope denial, data-classification denial, and placement explanations are
  visible to both web and agent/API surfaces.

### Pack D: Intake And Market Unification, After MVP Proof Is Credible

Operationalize this pack around A3, A4, P1, P2, P5, P6, and P7:

- External Work Intake
- Multi-Agent Coordination
- Team And Shared Memory
- MCP Server, only for deliberately exported bounded capabilities
- Artifact And Receipt
- Testing And Smoke
- Settlement Visibility Law from P9

Why this waits:

- External work intake should be one admission pipeline with many doors, but
  the current MVP still needs scheduling, decisions, account visibility, data
  scope, and proof smokes.
- P2 mission/work-order unification becomes easier and safer after the event
  log, artifact refs, task supervision, and Git delivery records have already
  been exercised by the MVP surfaces.
- Lane C paid provider work must wait for settlement bridge and settlement
  visibility receipts. Payment evidence must not substitute for acceptance or
  payout settlement.

Acceptance shape for this pack:

- UI, API, Forum, issue, schedule, autonomous, and agent-originated requests
  normalize into one admitted work-order shape with idempotency.
- Admission, rejection, routing, execution, review, delivery, acceptance, and
  settlement are separate receipts.
- Multi-agent decomposition is a supervision graph over normal assignments,
  not a privileged runtime mode.
- Market inventory and backlog faucet projections carry freshness metadata and
  do not expose private repo data.

### Pack E: Extension, Local Developer Experience, And Polish, Defer Until The Spine Holds

Defer broad operationalization of this pack until Pack A and the relevant
parts of Packs B/C are in place:

- MCP Client
- Plugin
- Skill
- Hook/Event
- IDE/Editor Integration
- Browser/Desktop Integration
- Voice/Multimodal Input
- Terminal UI Shell
- Input/Keybinding
- Command System
- Prompt Suggestions
- Tips/Education
- Theme/Visual Design
- Internationalization/Localization

The exception is when a specific open rung requires a thin slice. For example,
M2 already needed Pylon CLI status/review, and A1 needs structured JSON/non-
interactive output. In those cases, implement the minimal slice against the
runtime, permission, artifact, and settings services. Do not let extension or
polish systems become alternate authority channels.

## Roadmap

### R0: Lock The Operating Rule For The Current Sprint

Before taking more M-rungs, add Pack A audit references to the implementation
checklists for M6, M7, M9, M10, and A1.

Required rule:

- A rung that schedules, continues, notifies, blocks, asks for approval,
  completes in the background, or claims a proof must emit typed events and
  receipts. Model prose is not acceptance evidence.

### R1: M6/M7/M10 As One Unattended-Execution Slice

Implement scheduled launches, auto-continuation, decision queue,
notifications, and the overnight proof smoke as one operational slice even if
the GitHub issues stay separate.

Deliverables:

- Schedule records and fired/skipped/cancelled receipts.
- Continuation policy with budget, credential, workspace, and approval gates.
- Task supervisor projection for each queued or background run.
- Decision and notification projection derived from runtime events.
- Mobile-responsive review surface plus API parity.
- Overnight smoke covering SHC and own-Pylon/cloud-Pylon, with both web and
  terminal status visible.

Primary issue anchors:

- #4764 M6
- #4765 M7
- #4768 M10
- #4773 A1

### R2: Finish The Rate-Limit And Account Visibility Story

Treat M8 and the remaining M9 live leg as a paired credibility slice.

Deliverables:

- Account-pool dashboard reading credential refs, lease load, cooldowns,
  reset hints, low-credit state, and reconnect state.
- Live two-account rate-limit rotation proof, or an explicitly scoped blocker
  that keeps smart-routing copy narrow.
- Telemetry/privacy fixtures proving account-health and rate-limit metrics do
  not collect raw prompts, private repo data, or credentials.
- Credential revocation and stale-lease invalidation behavior.

Primary issue anchors:

- #4766 M8
- #4767 M9
- #4771 M13, only for the ToS/credential-boundary review that should start
  early

### R3: Repo Scope And Writeback

Make M11 and P3 a typed delivery slice, with M12 spend-to-evidence consuming
the same artifact/receipt model.

Deliverables:

- Repo connection and data-scope records.
- Placement explanations backed by trust-tier and lane-policy refs.
- Repository identity and safe Git state snapshots.
- Change capture records for patches, verification output, and PR draft
  candidates.
- Delivery authority and delivery receipt records.
- Ledger-to-mission-to-artifact joins for team and spend views.

Primary issue anchors:

- #4769 M11
- #4770 M12
- #4779 P3

### R4: Agent Parity And Intake Unification

Once scheduled/background work and repo delivery have receipts, converge agent
and human intake.

Deliverables:

- A1 parity matrix covering submit, status/events, decisions/review,
  scheduling, lane/pricing visibility, repo scope, and receipts.
- Forum-to-coding intake as a normal admitted work order.
- Autonomic work proposals using the same admission and budget rules.
- Mission/work-order unification plan that treats the event log and
  artifact/receipt layer as the shared record substrate.

Primary issue anchors:

- #4773 A1
- #4775 A3
- #4776 A4
- #4778 P2

### R5: Market Lane And Settlement

Only after MVP proof and intake unification are credible, operationalize the
market-facing audits.

Deliverables:

- First negotiated labor job pointed at a real backlog issue.
- USD-credit-to-sats settlement bridge with conversion refs and linked ledger
  entries.
- Backlog faucet into budgeted work requests.
- Spare-capacity Pylon provider mode behind capability envelopes.
- Lane C fanout with opt-in and public-tier-only placement at first.
- Settlement visibility law: recipient- and auditor-readable payout receipts
  before any broad labor claim.

Primary issue anchors:

- #4777 P1
- #4780 P4
- #4781 P5
- #4782 P6
- #4783 P7
- #4785 P9

### R6: Extension And Product Polish

After the spine is receipt-backed, expand local developer experience and
extension systems without changing authority boundaries.

Deliverables:

- MCP client and plugin installation as scoped capability ingestion.
- MCP server export only for narrow, schema-bound trusted capabilities.
- IDE/editor and browser/desktop integrations behind explicit capabilities and
  private evidence defaults.
- Voice/multimodal as an attachment-to-context pipeline, not an alternate
  instruction authority.
- Theme, tips, localization, and prompt suggestions that reflect capability
  state and never outrun receipts.

## Decision

Pack A is implemented as #4813-#4823 and remains the proof/supervision overlay
for the still-open M10/M14 gates. Pack B is implemented as #4824-#4830 and
should be cited where provider/account/policy claims depend on its evidence.
Pack C is implemented as #4831-#4835 and should be cited where repo/delivery
claims depend on its evidence. #4836/#4837 closed the immediate public
freshness/order-book hygiene pair.

Do not file another broad operationalization pack now. Hold Pack D and Pack E
until #4768/#4772 close or explicitly narrow, and until the market lane has a
fresh currently-open target plus independent provider, validation, release,
payout, and settlement receipts under #4777/#4781/#4782/#4783. The current open
tail is already well-owned; the missing ingredient is live evidence, not
another issue set.

The audit rule for future work is simple: if a capability changes work state,
spends money, touches credentials, mutates files, pushes code, asks for
approval, or supports public copy, it must land as a typed event, policy
decision, artifact, receipt, or projection. If it only lands as terminal text,
it is not operationalized.
