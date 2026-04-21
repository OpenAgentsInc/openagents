# Issue 4368 Retrospective Audit

Date: 2026-04-21

This audit records what happened during issue #4368, why it took far too long,
what the Nexus/Pylon/homework payout system actually is, what we changed, what
we proved, and what should change in future work. The short completion receipt
is `docs/reports/nexus/2026-04-20-issue-4368-reality-proof.md`. This document
is intentionally longer. It is meant to preserve the operational lessons from
the entire issue so the next agent does not repeat the same loop through
production Nexus, stale Pylon state, ambiguous completion criteria, and
comment-thread archaeology.

Issue #4368 was supposed to finish a distributed CS336/Episode 224 homework
run. That sounds narrow, but the acceptance criteria crossed several
subsystems: Nexus authority launch, worker matching, Pylon admission, Pylon
lease claim, local Psionic training execution, artifact staging, worker
closeout, validator challenge claim, validator replay, validator finalize,
window reconcile, accepted outcome projection, Treasury payout dispatch, Spark
payment confirmation, public/admin/raw status surfaces, and GitHub issue
closeout. Issue #4409 became related because it tracked the production Nexus
payout smoke path that had to prove the accepted-work payment was real after
service start. Issue #4385 was related because it created the local proof
runtime that should have been used as the primary development loop rather than
using production Nexus as the debugger.

The final state is complete. The final production proof ran on
`nexus.openagents.com` with clean local Pylon nodes and a unique training
network. The fresh run was
`run.cs336.a1.cs336_a1_reality_20260420t222320z.20260420222726.99d42cc0`.
The worker was `episode224-clean2` with pubkey
`a49673e52c37899f30ba424d6af714c1658c16137d78a2146401c6d2b326c27b`.
The validator was `episode224-clean1` with pubkey
`af0d800f07ca5df2a2f64a98239f6f819676c812d18828c841a340a577848e6c`.
The accepted contribution was
`9c92cfbbde3abaee8b3527a8cda1b7a9db8ee3ec47e9808de2aa5c2066c4728b`.
Treasury dispatched exactly one accepted-work payout for that run: `1` sat,
payment id `019dad0c-e701-7d23-96b6-1efebf1f9609`, status `confirmed`,
reconciliation status `settled`. The accepted-work pending payout count was
`0` at final verification. Placeholder payouts remained disabled and were not
used as evidence for #4368.

The final commit was `54acccabe`, pushed to `main`, titled `Record issue 4368
production reality proof`. That commit added the production reality-proof
report and fixed a Pylon status-projection bug discovered during final
verification. GitHub issues #4368 and #4409 were closed only after that commit
was on `main`, production proof existed, the payout was settled, and `gh issue
list --state open` returned no open issues. The fact that this statement has to
be this explicit is itself part of the audit. A branch, a worktree, a local
artifact, or a manually driven coordinator closeout is not issue completion.
For this class of issue, completion means integrated code and docs on `main`
plus the required local proof and production runtime proof.

The system involved has five main layers. Nexus is the authority and public
coordination surface. It owns the training-run launch API, admitted-node
records, lease scheduling, worker assignment state, validator challenge state,
window state, accepted outcome projection, stats, health, and admin/public API
views. Pylon is the local provider node. It owns node identity, payout target,
training network claims, worker and validator role claims, lease intake, local
runtime materialization, artifact publication, retained status, and operator
status projection. Psionic training runtime is the local execution engine used
by Pylon for the CS336 homework lane. It produces the actual training
artifacts, checkpoints, status packets, sealed-window bundle, closeout bundle,
validator score receipts, and verdict files. Treasury is the money-moving path
behind Nexus. It owns wallet balance, payout ledger, dispatch, confirmation,
and reconciliation. Spark is the payment rail used for these tiny sat payments.
The #4385 local proof runtime is the fast deterministic simulation harness that
models enough of Nexus/Pylon/Treasury to reproduce scheduler, artifact,
validator, closeout, and payout bugs without burning production time.

The correct development loop is local proof first, production confirmation
second. That was not just a preference. It became a hard operating rule because
the live production loop is slow and stateful. A production Nexus cycle can
involve image build, deploy, VM service restart, wallet hydration, public
status polling, clean Pylon admission, worker claim, artifact upload, validator
claim, replay, reconcile, and Spark confirmation. Every one of those stages can
fail for reasons unrelated to the code under test. The local proof runtime
exists to collapse that loop into deterministic authority and fleet runs. It
can create stale worker state, replacement attempts, simulated payout policy,
placeholder disabled mode, accepted-work payout records, and proof summaries
without waiting on live wallet sync or poll intervals. When that local runtime
does not model a blocker, the correct response is to improve the proof runtime
or document the deliberate modeling gap before returning to production.

One of the major reasons #4368 took so long was that this operating rule was
not followed strictly enough at the beginning. Production Nexus was repeatedly
used as the discovery mechanism for ordinary scheduler, wallet, and closeout
bugs. That inflated the issue thread and hid causal facts behind transient
production state. The same loop produced many GitHub comments that described
partial facts, stale facts, or evidence from branches and temporary worktrees.
Those comments were not useless, but they were not a coherent closure record.
They made it harder to answer the basic question: what is still broken, what is
actually fixed, what commit contains the fix, what environment proved it, and
what evidence can be re-read later. The thread volume was a symptom of missing
state discipline, not of inherent complexity in the final proof.

Another major reason was that the definition of done kept drifting. At several
points the system had a green local proof, or a partial production state, or a
manual server-side closeout, and those were treated as close to completion. The
user correctly rejected that. The final accepted standard was stricter and
correct: a fresh worker had to match, claim, execute, and submit; a validator
had to claim, replay, and finalize; Nexus had to record an accepted outcome;
Treasury had to pay accepted work exactly once; the payment had to settle; and
all of this had to be recorded from code integrated on `main`. That standard
invalidated the earlier manual-coordinator closeout. The earlier closeout did
prove that the coordinator and payout path could be driven to completion, but
it did not prove that Pylon could autonomously carry the worker and validator
path end to end. The final report explicitly replaced that caveat with a fresh
production run.

A third reason was stale local Pylon state. The default local Pylon config had
old retained closeout state from a previous run. That state included stale
pending closeout objects, retry records, idempotency conflicts, and old active
run context. Using that node would have made the final proof ambiguous. We
instead used clean Pylon homes: `pylon-episode224-clean2` as the worker and
`pylon-episode224-clean1` as the validator. We configured them onto a unique
network, `trainnet.cs336.a1.reality.20260420t222320z`, and separated their
role claims so the launch matched exactly one worker. That isolation mattered.
It prevented old all-network worker records, default Pylon records, or stale
proof nodes from satisfying the scheduler in a way that looked green but did
not prove the intended worker path.

A fourth reason was wallet and payout confusion. The system had an older
placeholder/liveness payout policy that paid small periodic stipends. That
policy was not the #4368 acceptance criterion. The user correctly insisted that
there should be no periodic 600-sat or window-based placeholder payment path
involved in this proof. Homework closeout should pay accepted work only. The
final run used `pay_only_on_accept: true` and `amount_sats: 1`. Placeholder
payout mode remained disabled. Treasury showed old skipped placeholder records,
but those were not accepted-work records and were not used as proof. The
accepted-work ledger produced one matching payout for the fresh run, and the
payment id tied the ledger entry to the exact accepted outcome and
contribution.

Funding was also a blocker, but it was not the only blocker. Nexus had only a
small cached spendable balance. The local Pylon wallet had fewer sats than
expected by the time final proof started. We generated a Nexus treasury funding
target and moved `2` sats from the local Pylon wallet to Nexus using the Pylon
wallet command. The final homework payout amount was set to `1` sat to prove
the rail without depending on a larger wallet balance. This was a pragmatic
choice. The acceptance criterion was not payment size; it was that accepted
homework work produces an exactly-once Bitcoin payment. The final Treasury
status reported wallet balance `4` after settlement, accepted-work pending
count `0`, and accepted-work confirmed count `2` for the 24-hour context, with
one matching payout for the fresh run.

Treasury health deserves a separate note. Treasury still reported degraded
health because full wallet sync timed out and the service used cached balance
plus bounded payment scan. That degraded status was not ignored. It was
recorded in the final proof report. It did not block #4368 because the
accepted-work payment was dispatched, confirmed, and settled. This distinction
matters. A broad Treasury continuity issue would require eliminating the sync
timeout or changing the sync strategy. Issue #4368 required a real accepted
homework payout after worker/validator closeout. The final proof satisfies the
homework payout requirement while honestly recording the residual Treasury
degraded-health condition.

A fifth reason was artifact credential mismatch. The local Pylon worker ran the
Psionic training runtime successfully and produced the local retained artifacts:
checkpoint manifests, status packets, `sealed_window_bundle.json`, and
`closeout_bundle.json`. The first serve loop did not progress closeout because
Pylon could not upload terminal receipts through the artifact courier.
`GOOGLE_APPLICATION_CREDENTIALS` and ADC were unavailable in the shell, so the
artifact courier reported `pylon_training_adc_credentials_unavailable`. The
system already had an explicit bearer-token path for artifact courier access:
`OPENAGENTS_PYLON_TRAINING_GCS_BEARER_TOKEN`. `gcloud auth
application-default print-access-token` failed, but `gcloud auth
print-access-token` succeeded. Passing that access token through the supported
Pylon environment variable allowed worker sync to upload artifacts and move
the closeout to `window_sealed`. The validator used the same token path to
stage, replay, finalize, reconcile, and observe payout.

That credential lesson is important. There are two different Google credential
paths in play. ADC is the conventional application-default path, but local
operator shells may not have ADC configured even when the user is logged into
gcloud. The Pylon artifact courier should be allowed to use an explicit bearer
token when the operator intentionally provides it. The repo already had a test
for that support. The production proof validated the path in a live run. Future
operator docs should state plainly that Pylon production artifact sync can use
`OPENAGENTS_PYLON_TRAINING_GCS_BEARER_TOKEN="$(gcloud auth print-access-token)"`
when ADC is unavailable, provided the token is not printed or committed.

A sixth reason was that Pylon command semantics were not obvious enough. For a
clean node with no retained run manifest, `training refresh` is not the
admission path. It failed with `training refresh requires at least one retained
run manifest`. The live admission and lease path is `training intake`. Before a
run exists, `training intake` can admit the node and then fail the lease request
with `training_scheduler_run_not_found`. After the run is created and
materialized to `leaseable`, the same command can claim and acknowledge a real
lease. One-shot `training intake` claims and materializes the runtime manifest,
but it does not by itself keep driving the local Psionic runtime. Runtime
execution is driven by `pylon serve` while the node is online. Terminal receipt
upload and authority sync can be driven by `training sync` after the runtime
has exited. Those command semantics became clear only by reading code and
observing the proof. They should be made explicit in operator docs.

The final production sequence was therefore precise. We confirmed `main` was
clean and issues #4368/#4409 were open for honest re-proof. We verified Nexus
Treasury state and placeholder payout mode. We funded Nexus with a tiny Spark
payment because the wallet had enough to prove a 1-sat accepted-work payout.
We configured two clean Pylon configs onto a unique network, with the worker
and validator separated by role claims. We admitted both nodes via the Pylon
intake path. We launched homework through
`https://nexus.openagents.com/v1/admin/homework/launch` using `pay_only_on_accept`
and `amount_sats: 1`. The launch matched exactly one worker. We waited until
the run reached `leaseable`. The worker claimed a lease and executed. We used
the supported GCS bearer-token path to sync terminal worker receipts. The
validator claimed, staged artifacts, ran replay, finalized, and reconciled. We
polled Treasury until the matching accepted-work payout moved from
`dispatching` to `dispatched` to `confirmed` and `settled`.

The final state of Nexus was real, not inferred. Public health returned
`ok: true`. The production VM service `nexus-relay` was active. The running
Docker image was
`us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:fc01e80c03e3`.
The VM release link was
`/opt/nexus-relay/releases/226f1ff857c75dcf8e62628f3eece7b8b8d01bb7`.
The final run detail showed the accepted outcome
`accepted.training_window.window.cs336.a1.cs336_a1_reality_20260420t222320z.20260420222726.99d42cc0.0001`
and the accepted contribution with `validator_disposition: accepted`,
`aggregation_eligibility: eligible`, and `accepted_for_aggregation: true`.
The final validator Pylon status showed `stage: paid`, `acceptance_state:
rewarded`, `payout_state: confirmed`, and the payment receipt id. The final
Treasury status showed the matching accepted-work payout as `confirmed` and
`settled`.

The final proof also uncovered a narrow Pylon status bug. The worker Pylon had
a local progress record at `window_sealed` waiting for `await_validator_claim`.
After a separate validator Pylon recorded the rewarded closeout and payment,
that old worker progress record should not have produced a stale operator
issue. The underlying production outcome was correct, but the local worker
status projection was misleading. We fixed
`training_closeout_progress_superseded_by_terminal_assignment` so a rewarded
or accepted closeout in the closeout cache suppresses non-terminal stale
progress for the same run/window. We added
`training_closeout_progress_issue_ignores_worker_progress_after_rewarded_closeout`
as a regression test. After rebuilding Pylon, both clean Pylon status reports
showed `recent_issues: null` and `pending_closeout_objects: null`.

The proof commands after that fix were focused and appropriate:
`cargo fmt --check -p pylon`, `cargo test -p pylon
training_closeout_progress_issue_ignores_worker_progress_after_rewarded_closeout
-- --nocapture`, and `cargo build -p pylon --bin pylon`. This was not a full
repo test sweep, and it did not need to be. The code change was a contained
status-projection fix, and the production proof itself had already exercised
the real worker/validator/treasury path. The final report records both the
test commands and the production evidence. That combination is stronger than
an unfocused long test run that does not touch the real settlement surface.

The issue took too long because the system did not initially force a clean
separation between development proof, production proof, and issue closeout.
Agents moved between branches, worktrees, local artifacts, production deploys,
GitHub comments, and VM probes without always reducing those facts into a
single authoritative state. The user repeatedly asked for clean `main`, clean
worktrees, branch deletion after consolidation, and no issue closure from
branch work. That feedback was correct. The repo-level `AGENTS.md` now states
that an issue can be closed only after the required code/docs are merged to and
pushed on `main`, with required deployment or runtime proof completed from that
integrated state. This rule should have been present and followed earlier.

The issue also took too long because production Nexus was overloaded with
debugging responsibility. Nexus should confirm that the integrated system works
with real public state and real money movement. It should not be the first
place ordinary scheduler and closeout bugs are discovered. The local proof
runtime from #4385 exists specifically to increase iteration speed. It should
be treated as the default path for distributed training, homework, Nexus
authority, Pylon fleet, artifact, validator, reconcile, closeout, and payout
proof work. If a bug appears only in production, the next step should be to add
that failure shape to the local proof runtime unless it depends on a live-only
primitive such as real Spark settlement. The final closure plan documented
this, and the final proof followed it more closely.

The issue also exposed a communication failure. The GitHub thread accumulated
too many comments because each partial action was reported as if it were close
to completion. The better pattern is fewer comments with stronger state
summaries. A good issue comment for this class of work should state the exact
commit, branch, proof command, proof artifact, production run id, worker id,
validator id, accepted outcome id, payout id, payout state, and remaining
blocker. If one of those fields is unknown, the comment should say so. If the
state is only local, the comment should say local. If the state is only on a
branch, the comment should say branch. If production proof is caveated, the
comment should not close the issue.

The actual architecture now has a clearer operational contract. Nexus is the
authority and evidence collector. Pylon is the executor and status reporter.
Psionic is the local training runtime. Treasury is the payout dispatcher and
ledger. Spark is the payment rail. The #4385 proof runtime is the fast
simulation system for scheduler, artifact, validator, closeout, and payout
logic. GitHub issues are not operational state; they are a public-facing
ledger of what has been proven. `main` is the only branch that counts for
completion. Production Nexus is the final settlement confirmation surface, not
the general-purpose test harness.

Several practical rules follow from this audit. Use unique networks for
production proof runs so matching is deterministic. Use clean Pylon homes for
fresh proof unless the test is explicitly about stale retained state. Separate
worker and validator role claims when proving end-to-end homework closeout.
Keep payout amounts small when proving payment rails from a limited wallet.
Disable placeholder/liveness payout modes for homework closeout proof. Use
`training intake` for admission and lease claim. Use `pylon serve` to drive
the runtime. Use `training sync` to push terminal receipts after runtime exit.
Use the explicit Pylon GCS bearer-token path when ADC is unavailable. Poll
Treasury until accepted-work payout is confirmed and settled. Record every
artifact path in a report before closing issues.

Several code and documentation follow-ups remain useful even though #4368 and
#4409 are complete. Pylon should make the `training refresh` versus `training
intake` distinction clearer in CLI help or operator docs. The artifact
credential fallback should be documented in a runbook. The local proof runtime
should keep expanding its modeled failure shapes so production-only debugging
keeps shrinking. Treasury degraded health from wallet sync timeout should be
tracked separately if the goal is a fully green Treasury health surface rather
than merely a functioning accepted-work payout path. The issue-close template
for training/payment work should require run id, accepted outcome id, payout id,
commit, and proof artifact path. Operator scripts should minimize long-lived
process leaks because the environment repeatedly warned about too many open
unified exec processes during this work.

The final lesson is not that the system should be thrown away. The final proof
shows that the system can work end to end: Nexus launched, Pylon claimed,
Psionic executed, Pylon published, validator replayed, Nexus accepted, Treasury
paid, and Spark settled. The failure was the operating model around the system.
The system needed a stricter proof hierarchy, cleaner state isolation, clearer
command semantics, and a harder definition of completion. After the final
changes, those rules are encoded in repo guidance, the production proof report,
and the regression test. Future work should preserve that discipline. A local
proof that is green but not on `main` is not done. A production closeout that
is manually forced is not enough for an autonomous Pylon issue. A payout that
is dispatching but not confirmed is not settled proof. A GitHub issue is not
closed until the integrated system has produced the evidence the issue asked
for.

The honest status of #4368 as of this audit is complete. The honest status of
#4409 as of this audit is complete for accepted-work payout smoke. The honest
status of broader Treasury health is that accepted-work dispatch and settlement
worked, while full wallet sync still reported a bounded timeout and cached
balance fallback. That residual condition should not be conflated with #4368
completion, and it should not be hidden. It should be handled as a separate
Treasury continuity/hydration concern if the operator wants a fully green
Treasury health indicator. The closure of #4368 was justified because the
homework system, as scoped, worked in reality.
