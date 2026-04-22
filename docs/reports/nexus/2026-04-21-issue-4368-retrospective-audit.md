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
using production Nexus as the debugger. Re-reading the closed #4385 thread and
its child issues makes the follow-up rule sharper: #4386, #4389, #4388, and
#4387 landed the prod-shaped authority runtime, isolated proof namespaces,
fleet orchestration, proof doctor, authority-state traces, transport split
view, closure summaries, and 4368-class retained-state fixtures. Issue #4400
then added a local simulation for the exact post-deploy payout-smoke failure
shape that blocked production. Future CS336 work should therefore extend and
run that proof system before using live Nexus as anything more than final
confirmation.

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
or document the deliberate modeling gap before returning to production. The
concrete gate from #4385 and #4400 is now: run the relevant local proof lane
from `main`, retain the proof namespace, `run-report.json`,
`authority-state-trace.json`, `proof-summary.json`, and the first red stage if
there is one. A production run can confirm a locally green result, but it
should not be the first place a CS336 scheduler, artifact, closeout, or payout
seam is discovered.

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

## Transcript 221-225 comparison

After re-reading `docs/transcripts/221.md` through `docs/transcripts/225.md`,
the public claim sequence is clear. Episode 221 introduced Pylon as a compute
miner that sits on a user's computer, talks to Nexus, participates in a
Nostr/NIP-90-style open network, and lets the user sell a subset of local
compute for Bitcoin. Episode 222 moved that from generic compute into the
distributed-training launch story: the existing online/liveness payouts were
described as a placeholder used to line up supply, and the next step was to
send real pieces of a decentralized training run to Pylons. Episode 223 made
the same economic argument more directly: pay the people, use Bitcoin, use
consumer Macs and GPUs instead of only datacenter-class hardware, and build
the revenue-sharing path that large AI labs had failed to deliver. Episode 224
is the specific homework episode. It says the network is no longer paying for
being online, describes the Stanford "language models from scratch" assignment
flow, says Assignment 1 was ported into Psionic, and promises that the new
Pylon binary will give nodes small pieces of that homework. Episode 225 then
widens the same payment thesis into developer and product bounties, Psionic
work, Probe, Forge, Autopilot, and future revenue share from products built on
the compute network.

The implementation, issue naming, and final proof are all `CS336 A1`, matching
Stanford's public language-models-from-scratch lane as it was encoded in the
repo. The proven lane is `run.cs336.a1...`, Assignment 1, Psionic-backed, with
one worker contribution accepted and one accepted-work payout settled. That
distinction matters because a precise public claim needs to bind to a run id,
assignment id, work-class, accepted outcome, and payout id rather than to loose
course shorthand.

The strongest thing that now works compared to the videos is the core
mechanism behind the homework promise. A Pylon can be admitted to Nexus for a
specific homework training network. Nexus can launch a bounded Assignment 1
run with pay-on-accept semantics. A clean worker Pylon can claim the lease,
run the Psionic homework runtime, produce checkpoint and closeout artifacts,
and upload terminal receipts. A separate validator Pylon can claim the
challenge, stage the worker artifacts, replay and score the work, finalize the
verdict, and cause Nexus to record an accepted outcome. Treasury can then pay
that accepted work exactly once over Spark, and the payment can reach
`confirmed` and `settled`. That is the minimum real version of "we pay for
useful homework work, not just online presence." It is materially different
from a dashboard counter, a placeholder heartbeat, or a manual statement that
the system should work.

The second thing that works is the public-state model needed to make those
claims honest. Nexus now has concrete fields and reports for training run
state, window state, accepted outcomes, accepted-work payouts, placeholder
payouts, weak-device/supporting-work counters, and strong-lane/progress-work
counters. The `transcript-222-launch-truth-contract.md` file is important
because it prevents the public narrative from collapsing presence, assignment,
acceptance, model progress, and payout totals into one vague "contributors"
number. That was one of the hidden risks in the video sequence. The videos
move quickly from "many Pylons are online" to "largest decentralized training
run" to "we are going to pay people for real work." The current system can
support honest narrower statements, but only when the statement names the
right count family: online, admitted, assigned, accepted, or model-progress
contributors. The #4368 proof satisfies accepted homework work for one clean
worker and one validator path. It does not justify using online Pylon counts
as accepted training contributors.

The third thing that works is the separation between placeholder payouts and
accepted-work payouts. Episode 224 explicitly says the network should stop
paying people merely for being online and should pay for the work sent to the
Pylon. That is now the right rule in code and operations for this lane. The
final proof used `pay_only_on_accept: true`, paid `1` sat for the accepted
work, left placeholder payouts disabled, and recorded a matching payment id
for the accepted contribution. This directly addresses the confusion around
periodic 600-sat/liveness payouts. Those payments may have served an early
bootstrap or supply-discovery purpose, but they are not the homework claim and
must not be used as evidence that the homework system works. Homework proof is
accepted-work proof.

The fourth thing that works is that the local proof runtime now exists for the
parts of the system that should not require slow production iteration. The
videos describe a fast public build-and-learn posture. The actual #4368
process showed that the old way was too slow because live Nexus, live Pylons,
artifact credentials, wallet health, VM deploys, and GitHub comments were all
mixed into one debugging loop. The #4385 proof runtime gives us a better
engineering implementation of the same "do it live, learn fast" posture: model
the scheduler, stale state, replacement attempts, accepted closeout, simulated
treasury, and payout eligibility locally first; then use production Nexus only
for the facts a simulator cannot honestly prove, especially real Spark
settlement and public deployment state.

What does not work yet is the broad public version implied by the videos. The
videos make it sound like a normal outside user can install or update Pylon,
join the network, receive homework pieces, contribute meaningful work, and get
paid without operator involvement. The #4368 proof did not reach that standard.
It used controlled local Pylon homes, explicit network configuration, explicit
worker and validator role separation, operator-managed launch, operator
polling, and a Google artifact bearer-token path. That is valid production
proof of the underlying machine path, but it is not yet a self-serve public
earner loop. A public user should not have to understand `training intake`,
`pylon serve`, `training sync`, Nexus run ids, retained closeout caches, GCS
credential fallback, or Treasury polling before the product can claim the
video-level experience is fully shipped.

What also does not work yet is the scale claim in the strong sense. The videos
talk about surpassing the 70-participant Bittensor/Templar reference and about
hundreds, thousands, or more Pylons contributing compute. The current proof is
not that. It is a single accepted worker contribution plus validator replay in
a fresh production run. It proves the path, not the scale. Public statements
can say the network has online Pylons if the presence counters support that.
They can say Nexus has paid accepted homework work when the accepted-work
ledger supports that. They cannot honestly say all online Pylons are doing the
homework, all online Pylons are training, or the homework run is largest by
accepted/model-progress contributors unless the admitted, assigned, accepted,
and model-progress count fields demonstrate that exact claim.

What does not work yet is broad assignment coverage across the Stanford course
sequence. Episode 224 describes a plan to work through the assignments over
days or weeks, starting with Assignment 1 and later expanding into Flash
Attention, distributed data parallelism, optimizer sharding, scaling laws, and
other later-course work. #4368 proves only the bounded Assignment 1 path that
was ported into Psionic and wrapped in the current Nexus/Pylon closeout
protocol. It does not prove Assignment 2, full DiLoCo-scale training,
multi-window training across heterogeneous public devices, arbitrary course
homework, human homework submissions, or a generalized assignment marketplace.
Those may be natural next steps, but they should be tracked as separate
implementation and proof issues rather than implied by #4368 closure.

What does not work yet is polished payout reliability as a user-facing product
surface. The accepted-work payout path worked in reality, but Treasury health
still reported degraded wallet sync with cached balance fallback. That was not
a blocker for #4368 because the exact accepted-work payout settled. It remains
a blocker for the broader video promise if the product wants ordinary users to
trust a wallet-balance UI without operator interpretation. A public earner
loop needs a clean wallet funding story, clear payout-class counters, explicit
failure messages, and no reliance on humans reading raw treasury status to
decide whether the system is healthy enough. The proof shows money can move.
It does not show the full user-facing money experience is smooth.

What does not work yet is the contributor/bounty process from Episode 225 as a
unified system connected to the homework lane. Episode 225 promises developer
and product bounties, Psionic help, and eventual product revenue share. That is
related to the same "pay people in Bitcoin" thesis, but it is a different
mechanism from Pylon accepted-work payouts. The homework lane pays a node for
accepted compute or validation work. Developer bounties pay humans for
accepted code or product contributions. Future revenue share pays compute,
data, model, or software contributors from product revenue. Those should share
wallet, ledger, and public-truth discipline where possible, but evidence for
one must not be substituted for evidence for another.

The honest comparison is therefore: the videos were directionally correct
about the architecture and economics, but ahead of the productized operational
state. We now have a real end-to-end proof that the Nexus/Pylon/Psionic/
Treasury/Spark path can pay accepted CS336 Assignment 1 homework work. We do
not yet have a broadly usable public system where arbitrary people can update
Pylon, receive CS336 homework pieces, know what they are earning, and withdraw
without operator help. #4368 closed the first statement, not the second. Future
work should make that boundary explicit in public copy, stats pages, issue
comments, and demos: "accepted CS336 A1 homework payout proven" is true;
"public self-serve homework marketplace fully works for everyone" is not yet
true.

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

## Addendum: scope to make the initial video claim true

Date: 2026-04-21

The next target is narrower than the full OpenAgents economy and broader than
#4368. The target is the initial video-level Pylon claim: a normal motivated
user can run Pylon, connect to the hosted Nexus, receive CS336 Assignment 1
training-network work, complete that work through the normal Pylon runtime
loop, and receive a real Bitcoin payout for accepted training work. This is
not the claim that every online Pylon is training, that the system is the
largest run by accepted contributors, that all Stanford assignments are
implemented, that arbitrary third-party buyers can self-serve runs, or that
Autopilot has a polished consumer UI. This addendum defines the minimum work
needed to move from the #4368 controlled production proof to that public
operator/user claim.

The definition of done for this target should be concrete. A fresh machine or
fresh user-home equivalent starts from public install/update instructions, not
from a hand-built local checkout with hidden operator context. After Pylon is
installed or the npm bootstrap has resolved the binary, the user runs exactly
`pylon`. That command should create or load the wallet and node identity, join
the hosted Nexus, go online for all currently relevant hosted jobs, advertise
the machine's capabilities, and accept available work without a CS336-specific
opt-in. If CS336 Assignment 1 is the hosted work currently available, an
eligible online Pylon should receive that work as part of normal operation.
Pylon should then receive a real Assignment 1 worker or validator lease, drive
runtime execution without the operator manually sequencing `training intake`,
`pylon serve`, and `training sync`, upload required artifacts through a
credential path that does not require the user to hold OpenAgents operator
secrets, report accepted or rejected status clearly, and show the matching
accepted-work payout as confirmed or settled. The final proof must use code
merged to and pushed on `main`, must run against production Nexus only after
the local proof lane is green, and must produce a report with run id, node id,
assignment id, accepted outcome id, payout id, payment state, and the exact
`pylon` command transcript.

The minimum path has four work packages. First, the bare `pylon` command needs
to become the default online earning loop so the user does not need to
understand internal training subcommands or explicitly choose CS336. Second,
Nexus needs a hosted CS336 starter-work lane that can assign and pay accepted
work to eligible online Pylons without a bespoke operator launch each time.
Third, artifact and payout plumbing need to be safe for a public Pylon: no ADC
surprise, no operator bearer token requirement, no placeholder payout
ambiguity, and no raw treasury interpretation required by the user. Fourth,
the team needs one final public-style proof run from a clean machine profile
that demonstrates the exact video claim and records it in a single closure
report. The issues below were created with expanded bodies through `gh issue
create` on 2026-04-21:

- [#4410 Make pylon run the default online earning loop](https://github.com/OpenAgentsInc/openagents/issues/4410)
- [#4411 Create hosted CS336 starter-work lane for default online Pylons](https://github.com/OpenAgentsInc/openagents/issues/4411)
- [#4412 Remove public-user blockers from CS336 artifact and payout plumbing](https://github.com/OpenAgentsInc/openagents/issues/4412)
- [#4413 Prove public-style CS336 Pylon earning end to end](https://github.com/OpenAgentsInc/openagents/issues/4413)

The issue bodies on GitHub are the canonical, expanded working versions. The
snapshots below preserve the audit-level scope and ordering.

All four created issues must preserve the #4385 simulation-first operating
rule. Before any live Nexus, live Treasury, or live Spark proof is attempted,
the issue must either pass an appropriate local proof lane from `main` or land
the missing proof-runtime extension that models the relevant gap. The required
local evidence is the exact command, namespace, `run-report.json`,
`authority-state-trace.json`, `proof-summary.json`, and first-red-stage
summary if the lane blocks. If a behavior is genuinely live-only, the issue
must say why the local runtime cannot model it, must narrow the production
confirmation to that live-only seam, and must not use production as the
general debugger again.

The current public install instructions are the `PYLON_AGENT_INSTRUCTIONS`
string in the `openagents.com` repo at `resources/js/pages/welcome.tsx`. Those
instructions are still accurate for the current standalone onboarding truth:
install or resolve Pylon through `npx @openagentsinc/pylon`, direct release
asset, or source fallback; verify `local_gemma`; diagnose Gemma 4; avoid
account linking by default; and treat local Gemma inference as a complete
local bring-up. They are not sufficient for the initial video claim. They do
not currently tell a user how to join a hosted CS336 Assignment 1 training
lane, how to require a paid-training-capable Pylon release, how to run the
future default online earning loop from the bare `pylon` command, or how to
distinguish Gemma-only readiness from CS336 accepted-work earning readiness.

The latest paid-training-capable Pylon release floor at the time of this
update is now `pylon-v0.1.5`, with npm package `@openagentsinc/pylon` `0.1.5`.
The earlier `pylon-v0.1.4` release, published on 2026-04-21 from commit
`401bb2accdb1449e99ff5c703842605c22603ac1`, remains important historical
evidence because it proved public install, public release-asset resolution,
identity creation, online admission, worker execution, artifact upload, and
window sealing. It is not sufficient for the final CS336 paid-training earning
claim because the public proof left validator challenges queued and no
accepted-work payout could be produced. The closeable floor moved to
`pylon-v0.1.5` because the package bootstrap must launch the earning loop by
default, Pylon must enable validator intake by default, default role claims
must clear validator work before asking for more worker assignments, and stale
starter-run scheduler errors must be nonfatal.

The #4412 implementation tightened that release requirement without changing
the user-facing command. A public paid-training Pylon must be on a release that
includes Nexus-brokered signed artifact access (`nexus_signed_url`),
accepted-outcome-bound payout matching, status fields for accepted outcome id,
accepted-work payout id, payment id, and payout reconciliation state, default
validator intake, validator-first training role claims, and the package
bootstrap default that starts the earning loop. All older provisional
`pylon-v0.1.2` references in the issue snapshots below should be read as
historical planning text, superseded first by the partial `pylon-v0.1.4`
public-worker proof and finally by the `pylon-v0.1.5` closeout floor.

The `openagents.com` instructions need an explicit paid-training branch in the
agent prompt. For ordinary local bring-up, they can continue to say that
`npx @openagentsinc/pylon` and `pylon gemma diagnose gemma-4-e4b` are valid
Gemma-local checks. For CS336 earning, they should require `pylon --version`
or equivalent release metadata to prove the installed binary is at least
`0.1.5`, instruct the agent to update via npm bootstrap or the matching direct
GitHub release asset if the installed version is older than `0.1.5`, and then
run only `pylon` after install or bootstrap. The instructions should not
introduce `pylon earn cs336-a1`, because users should not have to opt into CS336
specifically. They should say plainly that Gemma readiness is useful for the
local inference lane but not sufficient evidence that the node is earning from
CS336 training work. CS336 earning requires the node to be online for available
hosted jobs and then reach admission, assignment, accepted outcome, and
accepted-work payout state.

The current "OpenAgents Pylon for Agents" prompt should therefore remain
accurate for Gemma/local bring-up only if it continues to treat
`docs/pylon/README.md` as source of truth and does not imply that Gemma
diagnosis proves paid-training earning. For the paid-training branch it must
add four constraints. First, verify the installed Pylon is at least
`pylon-v0.1.5` / npm `0.1.5`, or use a newer release that explicitly includes
the same paid-training guarantees. Second, tell the operator to run only
`pylon` after install or update; the node should stay online for all currently
relevant hosted work and should not require a CS336-specific opt-in command.
Third, do not ask public users to configure `GOOGLE_APPLICATION_CREDENTIALS`
or `OPENAGENTS_PYLON_TRAINING_GCS_BEARER_TOKEN`. Those remain operator-only
fallback/test credentials. The public artifact path is Nexus-signed temporary
read/write authorization, surfaced by the `nexus_signed_url` credential source,
with failures reported as `artifact_authorization` or `artifact_transfer` in
Pylon status. Fourth, verify production Nexus is running the matching 0.1.5
hosted-starter fix set before claiming that public `pylon-v0.1.5` nodes can
receive the hosted starter lane automatically; otherwise a node can come
online correctly and still churn on stale starter scheduler failures.

### Created issue 1: [#4410](https://github.com/OpenAgentsInc/openagents/issues/4410)

Title: `Make pylon run the default online earning loop`

Body:

```markdown
## Goal

Make the bare `pylon` command run the public online earning loop. A normal user
should not need to choose a CS336-specific command or know the internal
operator sequence of `training intake`, `pylon serve`, and `training sync`.
When the hosted Nexus has CS336 Assignment 1 work available, an eligible online
Pylon should receive it through the default `pylon` flow.

## Scope

- Make `pylon` itself enter the default online earning flow after install or
  bootstrap. Do not require a user-facing `pylon earn cs336-a1` opt-in.
- Set the planned paid-training minimum release to `pylon-v0.1.2` unless the
  first shipped default online earning release uses a different tag.
- Default that flow to the hosted Nexus endpoint and all currently relevant
  hosted work classes, including the public CS336 Assignment 1 lane when it is
  available.
- Create or load the node identity and payout wallet through the normal Pylon
  path.
- Advertise the correct worker and/or validator capability claims for the
  hosted CS336 lane.
- Keep the node online and drive intake, lease claim, runtime execution,
  terminal receipt sync, and status projection without manual operator
  sequencing.
- Show clear terminal status for: connected, admitted, waiting for work,
  assigned, running, uploading artifacts, validating or awaiting validation,
  accepted, paid, rejected, and failed.
- Preserve the lower-level commands for operators, but make them unnecessary
  for the initial user claim.

## Out of scope

- Autopilot GUI polish.
- Later CS336 assignments.
- Arbitrary third-party training jobs.
- Claims about largest decentralized training run.
- Non-CS336 marketplace lanes.

## Acceptance criteria

- A fresh Pylon home can run bare `pylon` against the local #4385 proof runtime
  from `main` and complete at least one accepted Assignment 1 worker or
  validator path when that work is available.
- The local proof lane models the bare `pylon` default online loop before any
  production Nexus confirmation is attempted. If the existing runtime cannot
  model the blocker, this issue first extends the #4385 proof runtime rather
  than skipping straight to production.
- The same bare `pylon` command can run against production Nexus after local
  proof passes.
- The `pylon` command emits enough status to understand whether the node is
  waiting, working, accepted, paid, or blocked.
- The command does not require an OpenAgents operator bearer token or manual
  GCS credential export by the public user.
- Documentation includes the exact install/update path and states that `pylon`
  is the only command the user should need to run for online earning.
- The `openagents.com` `PYLON_AGENT_INSTRUCTIONS` draft is updated or a
  companion patch is prepared so agents know that local Gemma bring-up remains
  valid but online earning requires the paid-training-capable Pylon release and
  the bare `pylon` flow.
- Completion is committed and pushed to `main`; branch-only work does not close
  this issue.

## Required proof

- Local #4385 proof command, namespace, and artifact paths for
  `run-report.json`, `authority-state-trace.json`, and `proof-summary.json`.
- First red stage and proof-runtime gap, if the local lane blocks.
- Clean Pylon-home transcript showing `pylon` as the only user command for the
  earning loop.
- Output proving the installed Pylon release is at or above the paid-training
  minimum release.
- Production run id or explicit statement that production proof is delegated to
  the final public-style proof issue.
```

### Created issue 2: [#4411](https://github.com/OpenAgentsInc/openagents/issues/4411)

Title: `Create hosted CS336 starter-work lane for default online Pylons`

Body:

```markdown
## Goal

Make hosted Nexus provide enough CS336 Assignment 1 work for eligible Pylons
running the default `pylon` flow to receive real training-network assignments
and earn accepted-work payouts without a bespoke manual launch for every proof.

## Scope

- Define the hosted CS336 Assignment 1 starter-work policy.
- Keep payout basis accepted-work only; placeholder/liveness payouts must stay
  disabled for this lane.
- Make the lane available to eligible online Pylons that advertise the expected
  capability and payout target. Do not require users to opt into CS336
  specifically.
- Ensure Nexus can admit, assign, close out, validate, reconcile, and pay at
  least one fresh external-like Pylon without custom per-node operator state.
- Add guardrails for limited treasury balance: tiny payout amounts are fine,
  but insufficient-funds failures must be explicit and must not look like
  training failures.
- Expose the lane state in existing admin/public stats fields without
  overstating participation.

## Out of scope

- Fully open third-party buyer launch.
- Dynamic market pricing.
- Later assignments or arbitrary training tasks.
- Any return of periodic online/liveness payouts.

## Acceptance criteria

- Local #4385 proof runtime models the hosted starter-work lane from `main`
  and covers admitted, assigned, accepted, paid, rejected, and
  insufficient-funds cases before any production Nexus launch.
- The insufficient-funds and post-deploy-smoke shapes are covered by simulated
  treasury / #4400-style proof behavior instead of discovered first through a
  live wallet failure.
- Production Nexus can make at least one starter Assignment 1 lease available
  to a clean Pylon running the default `pylon` flow.
- Accepted work produces exactly one accepted-work payout record.
- Public/admin stats distinguish online, admitted, assigned, accepted, and
  paid states for this lane.
- Documentation states what the hosted lane does and does not prove.
- Completion is committed and pushed to `main`.

## Required proof

- Local #4385 proof command, namespace, and artifact paths for
  `run-report.json`, `authority-state-trace.json`, and `proof-summary.json`.
- First red stage and proof-runtime gap, if the local lane blocks.
- Production Nexus run id and training network id.
- Accepted outcome id.
- Matching payout id and final payment state.
```

### Created issue 3: [#4412](https://github.com/OpenAgentsInc/openagents/issues/4412)

Title: `Remove public-user blockers from CS336 artifact and payout plumbing`

Body:

```markdown
## Goal

Remove the operational blockers that made #4368 require expert intervention:
artifact credentials, payout ambiguity, and raw treasury interpretation. A
public Pylon user should not need OpenAgents operator secrets or direct
treasury debugging to complete a paid CS336 Assignment 1 job.

## Scope

- Replace or wrap the current production artifact upload credential path with a
  public-safe mechanism for default online Pylons.
- If Nexus must broker upload credentials, make those credentials scoped,
  temporary, run-bound, and non-secret in user docs.
- Ensure Pylon status surfaces artifact credential failures as actionable user
  errors instead of internal ADC/GCS language.
- Keep placeholder/liveness payout records separate from accepted-work payout
  records in every user-facing status path.
- Make Pylon show the accepted-work payout id and payment state when available.
- Make insufficient treasury balance, payout dispatch failure, and payout
  settlement delay visible as payout states rather than generic training
  failures.
- Document any remaining live-only Spark/Treasury limits.

## Out of scope

- Rebuilding Treasury.
- Supporting arbitrary object stores.
- Supporting external user-provided payout rails beyond the current Pylon
  wallet path.
- Polished Autopilot wallet UI.

## Acceptance criteria

- A public Pylon does not need `GOOGLE_APPLICATION_CREDENTIALS` or
  `OPENAGENTS_PYLON_TRAINING_GCS_BEARER_TOKEN` supplied by an operator.
- Artifact upload success and failure are reproduced in local #4385 proof and
  reported with a user-facing reason before production is used for
  confirmation.
- Accepted-work payout projection, insufficient balance, dispatch delay, and
  settlement-delay states are modeled through simulated treasury proof behavior
  before a live Spark/Treasury confirmation.
- Accepted-work payout state is visible from Pylon without querying raw Nexus
  or Treasury endpoints manually.
- Placeholder payout totals cannot be mistaken for CS336 accepted-work payouts
  in the Pylon status path.
- Completion is committed and pushed to `main`.

## Required proof

- Local #4385 proof command, namespace, and artifact paths for artifact success
  and artifact failure, including `run-report.json`,
  `authority-state-trace.json`, and `proof-summary.json`.
- Local proof covering accepted-work payout projection and payout-state
  reporting before production proof.
- Explicit proof-runtime extension or documented live-only gap if any artifact
  credential behavior cannot be modeled locally.
- Clean Pylon status output with payout id and settlement state.
```

Implementation note for #4412: the public path now defaults artifact transfer
to Nexus-brokered signed read/write URLs through `nexus_signed_url`. A public
node without `GOOGLE_APPLICATION_CREDENTIALS` or
`OPENAGENTS_PYLON_TRAINING_GCS_BEARER_TOKEN` can upload and download retained
training artifacts through Nexus, and Pylon re-verifies object digest and byte
length after transfer. Direct GCS credentials still exist, but only as an
explicit operator/test fallback when those env vars are intentionally set. The
status path now separates artifact authorization blockers from artifact
transfer blockers, treasury-balance blockers, payout-dispatch blockers, and
settlement blockers. It also projects accepted outcome id, accepted-work payout
id, payment id, and payout reconciliation state in both human and JSON training
status. Accepted-work payout matching now requires the accepted outcome id, so
placeholder or liveness payment records cannot satisfy the homework payout
state.

The local #4385 proof for this implementation was:

```bash
cargo run -p pylon --bin oa -- proof run cs336-a1-hosted-starter \
  --namespace proof.4412.public-artifact-payout.20260421T041736Z \
  --workers 1 \
  --validators 1 \
  --timeout-seconds 180 \
  --json
```

It completed with
`window.cs336.a1.starter.20260421041832.eae260e9.0001` reconciled, one
accepted contribution, `closeout=rewarded`, one quiesced worker, and one
quiesced validator. Evidence files:

- `/Users/christopherdavid/.openagents/pylon/proof/namespaces/proof.4412.public-artifact-payout.20260421T041736Z/fleet/run-report.json`
- `/Users/christopherdavid/.openagents/pylon/proof/namespaces/proof.4412.public-artifact-payout.20260421T041736Z/fleet/authority-state-trace.json`
- `/Users/christopherdavid/.openagents/pylon/proof/namespaces/proof.4412.public-artifact-payout.20260421T041736Z/fleet/proof-summary.json`
- `/Users/christopherdavid/.openagents/pylon/proof/namespaces/proof.4412.public-artifact-payout.20260421T041736Z/artifacts/object-trace.jsonl`
- `/Users/christopherdavid/.openagents/pylon/proof/namespaces/proof.4412.public-artifact-payout.20260421T041736Z/state/treasury-state.json`

The proof treasury state contains exactly one accepted-work payout record for
the accepted outcome:
`accepted.training_window.window.cs336.a1.starter.20260421041832.eae260e9.0001`,
status `confirmed`, payment id
`simulated:6ba3ffb5867f97ab8597f73838c346122a5db24c0608aa8a04e50bd786dcb7f3`.
The validator's `pylon training status` output shows the same accepted outcome,
accepted-work payout id, payment id, and `payout reconciliation: settled`.
The local proof runtime still models artifact failure primarily through
unit-level signed-access and blocking-class tests rather than a separate
fleet-level forced-denial lane; if the final production proof in #4413 exposes
a live-only signed URL failure, that proof gap should become a narrow follow-up
instead of reopening production as the general debugger.

### Created issue 4: [#4413](https://github.com/OpenAgentsInc/openagents/issues/4413)

Title: `Prove public-style CS336 Pylon earning end to end`

Body:

```markdown
## Goal

Close the initial video claim with a clean public-style proof: a fresh Pylon
user runs `pylon`, receives CS336 Assignment 1 training-network
work, completes it, and gets paid Bitcoin for accepted work.

## Scope

- Start from `main` after the default `pylon` online earning loop, hosted
  starter-work lane, and public-safe artifact/payout changes are merged and
  pushed.
- Run the #4385 local proof lanes and any #4400-style post-deploy smoke
  simulation from the exact shipped commit before touching production Nexus.
- Use a fresh machine profile or fresh user-home equivalent with no retained
  Pylon state.
- Install or update Pylon using the documented public instructions and verify
  that the resolved binary is at least the paid-training minimum release
  (`pylon-v0.1.2` if that remains the first default online earning release).
- Run only `pylon` for online earning after install/bootstrap.
- Use production Nexus as the final confirmation surface after local proof is
  green.
- Record all evidence in a report under `docs/reports/nexus/`.
- Only close this issue after the report proves the full user-facing loop.

## Out of scope

- Proving scale beyond one or a small bounded beta set of Pylons.
- Proving later CS336 assignments.
- Proving arbitrary third-party buyers.
- Proving the largest decentralized training run claim.
- Proving Autopilot GUI onboarding.

## Acceptance criteria

- The local proof gates are green from the exact shipped commit before the
  production proof starts.
- Fresh Pylon identity connects to hosted Nexus from the documented public
  `pylon` command path.
- The node goes online for all currently relevant hosted jobs.
- The node is admitted for the CS336 Assignment 1 lane without a
  CS336-specific user opt-in.
- The node receives a worker or validator assignment.
- Pylon completes the assignment without manual internal subcommand sequencing.
- Nexus records accepted work.
- Treasury records exactly one accepted-work payout for that accepted outcome.
- Spark payment reaches `confirmed` or `settled`.
- Pylon displays the accepted/payout state in user-facing output.
- The final report records command transcript, run id, node id, accepted
  outcome id, payout id, payment state, commit, and any remaining caveats.
- The issue is closed only after the report commit is pushed to `main`.

## Required proof

- Local #4385 proof command, namespace, and artifact paths from the exact
  shipped commit, including `run-report.json`, `authority-state-trace.json`,
  and `proof-summary.json`.
- #4400-style post-deploy smoke simulation output if deployment or live payout
  smoke is part of the proof path.
- Statement that production Nexus was used only after the local proof gates
  were green, or a documented live-only gap explaining the narrow exception.
- Production Nexus run id.
- Pylon command transcript from fresh state showing `pylon` as the only online
  earning command.
- Accepted outcome id.
- Payout id and settled or confirmed payment state.
- Report path committed to `main`.
```

These four issues were intentionally ordered. Issue 1 made `pylon` itself the
default online earner and defined the release floor. Issue 2 ensured hosted
Nexus has real accepted-work demand for that default online earner. Issue 3
removed the operator-only artifact and payout interpretation blockers. Issue 4
is the public-style proof and should be the only issue that claims the initial
video promise is complete. If any issue grows beyond the CS336 Assignment 1
user-earning loop, split it rather than weakening the definition of done. Do
not create issue bodies that omit the #4385 simulation-first gate; that gate
is the main lesson of #4385, #4400, #4409, and #4368. The issue-body snapshots
above reserve `pylon-v0.1.2` as the likely release floor because they preserve
the planning state when the issues were opened. The actual paid-training
closeout floor is now `pylon-v0.1.5`; `pylon-v0.1.4` is retained in this audit
as the partial public-worker proof that exposed the validator and starter-reuse
gaps.

## Addendum: public Pylon v0.1.4 and #4413 live gate

Date: 2026-04-21

Issues #4410, #4411, and #4412 are now closed. The implemented state is better
than the initial plan: bare `pylon` starts the default online earning loop,
the hosted CS336 A1 starter lane exists in the lease-claim path, public
artifact transfer defaults to Nexus-signed URLs, accepted-work payout status
is projected in Pylon status, and placeholder/liveness payouts cannot satisfy
the homework payout state. The minimum public paid-training release is not the
historical provisional `pylon-v0.1.2`; it is `pylon-v0.1.4`, with npm package
`@openagentsinc/pylon` `0.1.4`. That release was cut from
`401bb2accdb1449e99ff5c703842605c22603ac1` and published as
`https://github.com/OpenAgentsInc/openagents/releases/tag/pylon-v0.1.4`. The
release asset currently proven here is `pylon-v0.1.4-darwin-arm64.tar.gz` with
archive digest
`sha256:72d49f2fba8bdcfea45a177509974c42f079e99d112161f3ed2ee8a5a566a2c1`.

There are now two distinct version floors. The user-side Pylon floor is
`pylon-v0.1.4`. The server-side Nexus floor for the automatic hosted starter
path is `da4ef29613ce39bee381ae03ca0d1fefcf999b12` or later. That Nexus commit
changes hosted starter targeting to require online Pylons at
`min_pylon_version=0.1.4` and stops requiring the public Pylon's build digest
to match the Nexus service build. Without that server-side change deployed, a
public `pylon-v0.1.4` node can install correctly, create a payout destination,
come online, advertise worker and validator roles, and still receive
`training_scheduler_starter_work_unavailable` from the default lease path.
That is exactly what the first public-style #4413 attempt observed. It is not
a CS336 opt-in failure and not a reason to ask the user to run a different
command. It is a Nexus deploy/readiness failure.

The local proof gate for the current code is green. From pushed `main` at
`da4ef2961`, the command
`ISSUE_4368_PROOF_STAMP=20260421T114220Z ISSUE_4368_PROOF_OUT_DIR=var/proof/issue-4413-local-gate-20260421T114220Z scripts/pylon/issue-4368-local-closure.sh`
completed successfully. It ran the #4385/#4400-style local closure path,
including `cargo fmt -p nexus-control --check`, `git diff --check`, the
recovery comparison and cutover tests, the live-starter targeting test
`launch_homework_on_all_updated_online_pylons_and_pay_on_accept`, Pylon and
Nexus binary builds, post-deploy smoke simulations for funding timeout,
connected insufficient balance, and homework-only placeholder-disabled
behavior, plus the replacement-attempt and stale-recovery proof lanes. The
closure summary is
`var/proof/issue-4413-local-gate-20260421T114220Z/closure-summary.json`, and
the status is `completed`.

The public install/update half of #4413 is also partially proven. A clean proof
home at
`var/proof/issue-4413-public-v014-live-20260421T112546Z` ran
`npx --yes @openagentsinc/pylon@0.1.4 --version 0.1.4` against the public npm
bootstrap and resolved the GitHub release asset rather than a local cache. The
bootstrap result recorded `version=0.1.4`, `tagName=pylon-v0.1.4`,
`installMethod=release_asset`, and `cached=false`. The only online earning
command in the transcript was bare `pylon`, and the transcript shows:
`pylon: created local Spark payout destination for paid training work` followed
by `pylon: node pylon is online; running default online earning loop`. Nexus
then saw the node as online and eligible, with pubkey
`dd6bb49dc529bfc1ad25f973476f7011ae8a08b231ef0f79086dd487aed36840`,
release id `openagents.pylon@0.1.4`, build version `0.1.4`, worker and
validator role claims, and settlement destination
`spark1pgssxklk9nhj3uwhsx7c4csyfsxy0hl5q3759uwalv26eac37fnaxr0fr92tth`.

That public proof is not enough to close #4413. A manual full homework launch
against production did match the clean `pylon-v0.1.4` node, but that was not
the default hosted starter path and it did not produce an accepted assignment
or payout. More importantly, the default intake trace from the same node showed
the worker lease request failing with
`training_scheduler_starter_work_unavailable`; the validator request then
correctly surfaced `training_scheduler_self_validation_forbidden` as a
nonfatal condition for a single node that already attempted the worker path.
The code fix for the worker-side starter availability is on `main`, but it is
not live until Nexus is deployed at `da4ef2961` or later. Therefore #4413 must
remain open until production Nexus is updated and a fresh public-style run
proves assignment, accepted outcome, and accepted-work payout from the bare
`pylon` command.

The current deployment blocker is operational authority, not code. The active
local gcloud service account
`nexus-mainnet@openagentsgemini.iam.gserviceaccount.com` can describe some
artifact state but cannot read Compute instances, list or submit Cloud Build,
enable services, or SSH to the Nexus VM. The user account
`chris@openagents.com` is present in local gcloud config but its credentials
cannot refresh non-interactively. A browser-based `gcloud auth login
--no-launch-browser --update-adc` flow is waiting for the verification code.
Until that code is entered, the safe deploy scripts cannot build and deploy
the `da4ef2961` Nexus image or binary release. This audit should not hide that
fact: the code is pushed and locally proven, but the production service still
needs an authenticated deploy before the final public proof can run.

The definitive remaining #4413 sequence is short. Authenticate gcloud as an
account with Cloud Build and Compute/IAP authority. Build and deploy Nexus from
`da4ef2961` or later through the repo deploy scripts, not through ad hoc VM
mutation. Run the deploy verification gates, including Treasury status and
homework-only placeholder-disabled smoke. Start a fresh public proof home with
`npx @openagentsinc/pylon@0.1.4 --version 0.1.4`, then run only `pylon`.
Confirm the node comes online for all currently relevant hosted jobs without a
CS336-specific opt-in. Let the default hosted starter lane assign work. Require
Nexus to record accepted work and Treasury to record exactly one accepted-work
payout for that accepted outcome. Poll until the Spark payment is at least
`confirmed` and preferably `settled`. Only then add the final #4413 report,
push it to `main`, comment the issue with run id, node id, accepted outcome id,
payout id, payment state, release/version, Nexus commit/image, and proof paths,
and close #4413.

## Addendum: follow-up implementation ledger and next actions

Date: 2026-04-21

After the original #4368 retrospective and the first #4413 addendum, the
remaining work was split into a concrete implementation sequence: make the
bare `pylon` command the earning loop, make Nexus offer hosted starter work to
default online Pylons, remove public-user artifact and payout blockers, cut a
public Pylon release, then prove the public-style flow end to end. The first
three implementation issues are now complete. #4410 landed in `21a7a968f`,
making no-argument `pylon` and config-only `pylon --config-path <path>` enter
the default online earning loop instead of the TUI. That change also moved the
TUI behind explicit commands (`pylon-tui`, `pylon tui`, or `cargo pylon-tui`)
and updated the proof fleet so local proof nodes start through the same
default Pylon entrypoint that a real provider would use. The important product
effect is that a user no longer has to learn the internal sequence of
`training intake`, `pylon serve`, and `training sync` before the node can
begin earning-loop work.

#4411 landed in `6b60639c1`, adding the hosted CS336 A1 starter-work lane to
the normal Nexus training lease-claim path. When an eligible default worker
Pylon asks for work and no explicit run has been selected, Nexus can now
auto-launch or reuse the bounded `starter` run on the hosted CS336 A1 starter
network. The lane is accepted-work-only and does not require a CS336-specific
user opt-in command. It also added the `cs336-a1-hosted-starter` local proof
lane so the behavior can be tested in the #4385 proof runtime before touching
production. That proof lane starts default Pylons, lets their normal lease
request create the hosted starter demand, and waits for accepted contribution
and closeout instead of using the old admin-launch-only path.

#4412 landed in `06aa89c50`, closing the biggest public-user blockers. Pylon
now defaults retained training artifact transfer to Nexus-brokered signed
read/write URLs through `nexus_signed_url` when the operator has not supplied
direct GCS credentials. That means a public Pylon should not need
`GOOGLE_APPLICATION_CREDENTIALS` or
`OPENAGENTS_PYLON_TRAINING_GCS_BEARER_TOKEN`; those remain operator/test
fallbacks only. The same work separated artifact authorization failures from
artifact transfer failures, made terminal sync persist publication errors into
closeout progress, and changed payout matching so accepted-work payout state
must bind to the accepted outcome id. This prevents placeholder or liveness
payout records from satisfying the homework payout proof. Pylon status now
projects accepted outcome id, accepted-work payout id, payment id, and payout
reconciliation state in the user-visible training status path. The local proof
for this issue used namespace
`proof.4412.public-artifact-payout.20260421T041736Z` and completed with one
rewarded closeout and one simulated accepted-work payout settled.

The next implementation step was to cut a real public Pylon release. That
became `pylon-v0.1.4`, published from
`401bb2accdb1449e99ff5c703842605c22603ac1` with npm package
`@openagentsinc/pylon` `0.1.4`. That release includes the #4410 default
earning loop, the #4412 public-safe artifact and payout projection path, and a
client-side #4413 fix that treats
`training_scheduler_self_validation_forbidden` as nonfatal during lease claim.
That nonfatal handling matters for a normal single Pylon that advertises both
worker and validator roles: after it asks for worker work, it may also ask for
validator work, and Nexus should be allowed to reject self-validation without
making the entire training intake look failed. The release artifact proven in
the first public-style attempt was
`pylon-v0.1.4-darwin-arm64.tar.gz`, archive digest
`sha256:72d49f2fba8bdcfea45a177509974c42f079e99d112161f3ed2ee8a5a566a2c1`,
resolved through the public npm bootstrap with `cached=false`.

The first public-style install proof showed that the user-side release works
as far as it can without the updated server. A fresh proof home under
`var/proof/issue-4413-public-v014-live-20260421T112546Z` installed
`@openagentsinc/pylon@0.1.4`, resolved `pylon-v0.1.4` from the GitHub release
asset, created a local Spark payout destination, and ran only the bare
`pylon` command. The transcript showed the intended user-facing behavior:
`pylon: created local Spark payout destination for paid training work` and
`pylon: node pylon is online; running default online earning loop`. Production
Nexus saw the node as online, eligible, build version `0.1.4`, release id
`openagents.pylon@0.1.4`, with worker and validator role claims and a Spark
settlement destination. That proves the install/update and "run only `pylon`"
half of the video-level claim. It does not prove assignment or payment yet.

The reason it did not prove assignment or payment is now understood and fixed
in source. The live production server still required the public Pylon build to
look like the current Nexus service build for hosted starter targeting. That
is the wrong rule for a public release: a released `pylon-v0.1.4` binary should
not have to share a build digest with the server it talks to. Commit
`da4ef29613ce39bee381ae03ca0d1fefcf999b12` changed the hosted starter request
to target online Pylons by `min_pylon_version=0.1.4`, set
`require_updated_build=false`, and keep the starter lane bound to the actual
node that requested work. The corresponding Nexus test,
`default_pylon_lease_claim_auto_launches_hosted_cs336_starter_work`, now seeds
a forward public Pylon version and asserts that the auto-launched starter
request uses minimum-version targeting instead of digest coupling. The live
failure `training_scheduler_starter_work_unavailable` from the first public
attempt is therefore a server-deploy blocker, not a missing user command and
not a reason to introduce a CS336 opt-in.

The audit and operator docs were then updated in `d5e3aa0bc`. `docs/pylon/README.md`
now states that the minimum public paid-training Pylon release is
`pylon-v0.1.4` / npm `0.1.4`, that Nexus must be on `da4ef2961` or later for
automatic hosted starter assignment, that users should run only `pylon`, that
CS336-specific opt-in commands should not be introduced, and that public users
should not be asked for OpenAgents operator bearer tokens or direct GCS
credentials. This retrospective was also updated to replace the earlier
provisional `pylon-v0.1.2` release-floor language with the real `pylon-v0.1.4`
floor. The open #4413 issue body was edited to match those facts, and #4413
received status comments recording both the public install proof and the
remaining Nexus deployment blocker.

A fresh local closure gate was rerun from current pushed `main@d5e3aa0bc` so
the latest docs commit also has a current proof baseline. The command was
`ISSUE_4368_PROOF_STAMP=20260421T115658Z ISSUE_4368_PROOF_OUT_DIR=var/proof/issue-4413-local-gate-20260421T115658Z scripts/pylon/issue-4368-local-closure.sh`.
It completed successfully. The summary at
`var/proof/issue-4413-local-gate-20260421T115658Z/closure-summary.json` reports
`status=completed`; the replacement-attempt lane completed with closeout
`refused` and `caveat_count=0`; the stale-recovery lane completed with closeout
`rewarded`, `accepted_contributions=1`, and `caveat_count=0`; and the three
post-deploy smoke simulations covered funding-target timeout rollback,
connected-wallet insufficient balance against the old 600-sat policy
rollback, and homework-only placeholder-disabled pass. That keeps the
simulation-first requirement green for current `main`, but it still does not
replace production proof for #4413.

The honest current issue status is therefore split. #4412 is fully complete
and closed because the public-safe artifact path, accepted-work-bound payout
projection, and local proof evidence landed and were pushed. #4413 is not
complete and should not be closed yet. The user-side Pylon release exists and
the local proof gate is green, but production Nexus still needs the
`da4ef2961` or later server change deployed before the hosted starter lane can
assign work to a public `pylon-v0.1.4` node through the default lease path.
The active local gcloud service account
`nexus-mainnet@openagentsgemini.iam.gserviceaccount.com` still lacks Compute
and Cloud Build authority, and the cached `chris@openagents.com` credential
still cannot refresh non-interactively. A browser-based `gcloud auth login
--no-launch-browser --update-adc` flow is the current route to unblock the
safe deploy scripts.

The next operator should do five things, in order. First, authenticate gcloud
as an account with Cloud Build and Compute/IAP authority, then confirm with
`gcloud config get-value account`, `gcloud compute instances describe
nexus-mainnet-1 --project openagentsgemini --zone us-central1-a`, and
`gcloud builds list --project openagentsgemini --limit 1` that the credential
can actually deploy. Second, build and deploy Nexus from current `main`
through the repo scripts rather than ad hoc VM mutation. The expected safe path
is `scripts/deploy/nexus/01-build-and-push-image.sh`, then
`DEPLOY_IMAGE=<built image> scripts/deploy/nexus/03-configure-and-start.sh`,
then `DEPLOY_IMAGE=<built image> scripts/deploy/nexus/04-verify-gates.sh`.
The deployment must leave production Nexus running `da4ef2961` or later and
must preserve the homework-only payout policy with placeholder/liveness
payouts disabled. If the deploy gate rolls back, do not force it; record the
failure and add the missing failure shape to the local proof runtime if it is
not already modeled.

Third, run a fresh public-style #4413 proof from a new Pylon home after the
deploy is verified. The proof should use the documented public path,
`npx --yes @openagentsinc/pylon@0.1.5 --version 0.1.5`, and then the only
online earning command should be bare `pylon`. Do not manually launch the
homework run as the primary proof, do not ask the user to run a CS336-specific
command, and do not provide operator GCS credentials to the public Pylon. The
proof must show that the node comes online, that Nexus admits it for the
hosted starter lane, that the default lease path assigns worker or validator
work, that Pylon completes the work without manual internal subcommand
sequencing, and that Nexus records accepted work.

Fourth, prove payment with the accepted-work ledger, not with placeholder
payments or broad treasury counters. The final #4413 evidence must include the
production run id, network id, node pubkey, role, assignment id, contribution
id when applicable, accepted outcome id, accepted-work payout id, Spark
payment id, payment status, and reconciliation status. Treasury must record
exactly one accepted-work payout for the accepted outcome. The payment must be
at least `confirmed`, preferably `settled`. If the wallet has insufficient
funds, hydrate the Nexus wallet through the documented treasury path and rerun
the payment proof; do not revive the old 600-sat placeholder policy and do not
claim issue completion from a dispatching-but-unconfirmed payment.

Fifth, write the final #4413 closure report under `docs/reports/nexus/`, push
it to `main`, comment #4413 with the report path and all required identifiers,
and close #4413 only after that commit is on `main`. The closure comment
should be terse and factual: commit, Nexus image or release, Pylon release,
run id, node id, accepted outcome id, payout id, payment state, and proof
paths. If any caveat remains, state it directly and leave #4413 open unless
the caveat is explicitly outside the issue acceptance criteria. The main rule
from #4368 still applies: local proof is necessary, production proof is
required for the public earning claim, and a GitHub issue is not complete until
the integrated system has produced the exact evidence the issue asks for.

## Addendum: paced homework dispatch recommendation implemented

Date: 2026-04-21

The most relevant remaining operational recommendation was to stop treating
homework launch as an ad hoc one-off operator action and give Nexus a narrow,
repeatable admin surface that can intentionally meter work and accepted-work
payout exposure. That surface now exists as
`POST /v1/admin/homework/cs336-a1/dispatch`, with the legacy internal
`/api/admin/homework/cs336-a1/dispatch` alias for the same handler. The endpoint
does not change the public Pylon contract: users still run only `pylon`, stay
online, and receive whichever bounded starter homework work Nexus offers. The
new control is for the admin side, where a cron job can decide how many fresh
homework runs to launch, how many contributors each run may assign, how many
sats each accepted contribution is worth, and what per-call maximum payout
exposure is allowed.

The important design decision is that the endpoint defaults to fresh,
non-reused runs. `reuse_existing_run=false` means every cron call generates a
new batch id, new run slugs, and new training run ids, so the system can
deliberately duplicate CS336 A1 starter work across intervals while still
using the same scheduler, artifact, validation, accepted-outcome, and treasury
rails as the hosted starter path. The default request is conservative:
`run_count=1`, `max_contributors_per_run=1`, `amount_sats=1`,
`only_online=true`, `min_pylon_version=0.1.5`, `require_updated_build=false`,
and a 30-minute homework window. Operators can raise `run_count`,
`max_contributors_per_run`, or `amount_sats` to increase throughput, and can
set `total_budget_sats` so Nexus rejects any request whose maximum possible
accepted-work payout exceeds the intended per-call cap.

This is still accepted-work-only. The dispatch endpoint creates paid homework
runs with a Lightning payout policy and `pay_only_on_accept=true`; it does not
send sats at launch time, does not revive placeholder/liveness payments, and
does not use the old every-four-hours 600-sat mechanism as evidence for the
training claim. Payouts are only queued after the training window reconciles
accepted homework contributions, and the existing treasury dispatch loop is
still responsible for sending and reconciling those payments. The new endpoint
therefore gives the admin the pacing knob the system was missing without
weakening the core audit rule from #4368: the earning claim is only satisfied
by accepted homework and accepted-work-bound payout records.

## Addendum: production deploy cleanup after authenticated Google access

Date: 2026-04-21

After Google access was refreshed, the current `main` image was built and
deployed to production Nexus as
`us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:62d21b6338b1`.
The first deploy attempt did not replace the running service because the
production VM root disk was full under `/var/lib/docker`; the service remained
on the prior image during that failure. We recovered the host by pruning stale
Docker images and temporary Docker pull state, vacuuming oversized journals,
and rerunning the scripted deploy path. The second deploy completed and left
`nexus-relay` active on the registry image tagged from `62d21b6338b1`, with
placeholder payouts still disabled and the paced homework dispatch endpoint
available in production.

That deploy exposed two separate operational facts that matter for #4413 and
for future agents. First, the old placeholder-liveness payout backlog from
earlier experiments was still present in treasury state. Those rows were not
accepted homework and should not have blocked the homework-only system after
the policy was changed to `placeholder_payout_mode=disabled`. The treasury
continuity logic now filters stalled-payout alerts by the active payout policy:
legacy `placeholder_liveness` rows no longer raise `dispatch_stalled` or
`confirmations_stalled` when placeholder payout mode is disabled, while
`accepted_work` and `beta_bonus` rows remain alert-relevant. Targeted tests
prove both sides: disabled placeholder backlog does not raise a stale critical
alert, and disabled placeholder mode still raises a critical confirmation
alert for a stuck accepted-work payout.

Second, the deploy verifier itself had two mismatches with the live production
runtime. Its treasury-enabled path attempted to read
`/etc/nexus-relay/nexus-relay.env` without sudo, so the gate failed before it
could reach the service assertions. It also treated any stale wallet-sync age
as a hard deploy failure, even though the service intentionally avoids
expensive wallet refreshes when there is no accepted-work payout requiring
reconciliation. The verifier now reads the service env with sudo and treats
stale wallet sync as acceptable only when the wallet runtime is connected and
`accepted_work_pending_payout_count` is zero. During a real payout, or any time
accepted-work reconciliation is pending, wallet freshness remains a hard gate.

The remaining production proof step for #4413 is not to revive liveness
payments or to count historical placeholder rows. The next run must use a
fresh public-style Pylon home, the public `pylon-v0.1.5` bootstrap, and then
only the bare `pylon` command for earning. An admin may pace work with
`POST /v1/admin/homework/cs336-a1/dispatch` and a one-sat proof amount if the
wallet remains nearly empty, but the issue can only close when Nexus records
accepted work and Treasury records exactly one accepted-work payout for that
accepted outcome with a confirmed or settled Spark payment. If the Nexus wallet
cannot fund even that bounded accepted-work payout, the correct next action is
wallet hydration through the documented treasury funding path, not another
server-side workaround.

## Addendum: public Pylon 0.1.5 closeout gate after the 0.1.4 proof attempt

Date: 2026-04-21

The next proof attempt used a fresh public-style Pylon home and the public npm
bootstrap path for `@openagentsinc/pylon@0.1.4`. That run was useful because it
proved several real pieces rather than just repeating local source tests: the
public package could resolve the GitHub release asset, verify the published
checksum, initialize an isolated Pylon home, create a real Pylon identity and
Spark payout destination, come online against production Nexus, receive a
targeted CS336 A1 assignment, run the local worker path, publish artifacts, and
seal the training window. That was the first clean evidence that a non-source
public Pylon release could execute the worker side of the claim against
production infrastructure. It was not enough to close #4413. The resulting
window was sealed with one contribution admitted but not accepted for
aggregation, and the validator challenges remained queued with no final result.
Because the accepted outcome never materialized, Treasury correctly had no
accepted-work payout record for that proof node and run.

That failure changed the minimum honest release floor. `pylon-v0.1.4` is now
historical evidence for install, identity, worker execution, artifact upload,
and window sealing, but it is not the minimum closeable public earning release.
The closeable path now requires `pylon-v0.1.5` and a matching Nexus deployment.
The client-side reasons are concrete. Bare Pylon must be online for validator
work by default, because a worker-only proof can leave the exact validation
challenge needed for acceptance queued. The default role order must try
validator intake before requesting more worker assignments, because clearing
validation backlog is more important than creating additional unaccepted
worker windows. Lease-claim errors such as
`training_scheduler_run_not_schedulable` and `training_scheduler_run_not_found`
must be nonfatal during the automatic intake pass, because stale or exhausted
starter runs should not abort the process. The npm bootstrap must launch the
installed `pylon` earning loop by default rather than opening `pylon-tui`;
otherwise the user-facing instruction "run only pylon" is technically false
for package-managed users.

The server-side reason is equally concrete. Production Nexus was willing to
auto-launch hosted CS336 A1 starter work for broad worker lease claims, but
the reuse path could select an existing kernel-schedulable starter run whose
scheduler assignment slot was no longer claimable. Once that happened, the
lease claim returned `training_scheduler_run_not_schedulable`, and a default
Pylon loop could churn on the same bad starter reuse instead of receiving new
work. The fix is to treat reuse as valid only when the scheduled run's current
window still permits assignment claims and the scheduler has a planned,
replaceable, or otherwise claimable worker assignment. If the prior starter run
is exhausted, sealed, leased, or otherwise not claimable, the hosted-starter
path must create a fresh starter run. A new regression test proves this by
having one public-style Pylon claim the first hosted starter slot and a second
public-style Pylon request broad default work; the second claim must receive a
different fresh starter run instead of reusing the exhausted first run.

The documentation and operator contract therefore move from
`min_pylon_version=0.1.4` to `min_pylon_version=0.1.5`. The user-facing
instruction remains intentionally simple: install or update through the npm
bootstrap or matching release asset, then run only `pylon`. Users should not
have to opt into CS336, supply admin tokens, supply Google credentials, or run
special homework commands. CS336 A1 starter work is just the current hosted
starter lane available to online paid-training-capable Pylons, and the admin
side controls pacing through the homework dispatch endpoint or automatic
starter launch policy. The closeout proof still has the same financial bar:
accepted homework contribution, accepted outcome, exactly one accepted-work
payout queued for that outcome, and a confirmed or settled Spark payment. The
0.1.5 changes are about making that proof reachable from the public default
path, not about weakening the accepted-work-only payout rule.

## Addendum: why the public earning floor moved from 0.1.5 to 0.1.6

Date: 2026-04-21

The public `pylon-v0.1.5` proof attempt exposed one more client-side problem
that the local simulation suite had not made painful enough. The fresh public
Pylon home installed the real `pylon-v0.1.5` GitHub release asset through the
npm bootstrap path, verified the checksum, created a real local Spark payout
destination, came online against production Nexus, and advertised both worker
and validator roles. That was progress over the earlier attempts, but the
default intake loop then found older validator challenge work before the fresh
targeted one-sat homework run. The local Psionic subprocess for that retained
validator challenge failed, and the retained lease stayed in the local cache as
`acked` rather than becoming terminal. Because `acked` leases are intentionally
reused across restarts, later intake passes kept reusing the failed retained
validator lease instead of requesting the newly launched paid worker
assignment. From the user's perspective, that violates the only acceptable
onboarding claim: install Pylon, run `pylon`, and get paid for accepted hosted
training work without knowing about CS336-specific commands or operator-only
control surfaces.

The fix is deliberately narrow. `pylon-v0.1.6` keeps validator support enabled
by default, because the system still needs public nodes to help close the
validation loop, but it changes the default role order back to worker first and
validator second. A public node should claim fresh paid homework worker slots
before it tries to drain arbitrary validator backlog. The validator path still
runs when no paid worker assignment is available, so the node remains online
for the relevant hosted jobs. The second `0.1.6` fix marks a retained lease
terminal when the active Psionic runtime has failed locally. That prevents a
failed `acked` validator or worker lease from indefinitely blocking subsequent
intake passes. The corresponding regression test constructs a failed active
runtime with an `acked` retained validator lease and verifies that the lease is
converted to `failed`, making `newest_pending_training_work_offer` empty so a
fresh scheduler claim can proceed.

That means the minimum public paid-training release is now `pylon-v0.1.6` /
`@openagentsinc/pylon` `0.1.6`, not `0.1.5`. The production Nexus floor must
also require `min_pylon_version=0.1.6` for the default hosted starter and admin
paced homework dispatch paths. The older `0.1.4` and `0.1.5` artifacts remain
useful historical evidence: `0.1.4` proved public install, worker execution,
artifact upload, and sealed windows; `0.1.5` proved the package-managed bare
earning-loop launch and validator-capable default. They should not be used as
the closeout floor for #4413 because they can still fail to progress from
public onboarding to an accepted-work payout without manual operator
intervention. Future agents should not try to rescue #4413 by manually claiming
a specific run through an admin API and calling that equivalent to a public
Pylon user. The proof has to use a fresh Pylon home, the public `0.1.6` package
or matching release asset, and then the bare `pylon` command.

## Addendum: why the public earning floor moved from 0.1.6 to 0.1.7

Date: 2026-04-21

The public `pylon-v0.1.6` proof attempt got farther than `0.1.5`, but it still
exposed one more closeout-ordering defect that prevents an honest issue close.
A fresh public-style Pylon installed through `npx @openagentsinc/pylon@0.1.6`,
created a local Spark payout destination, came online against production Nexus,
received the targeted one-sat CS336 A1 homework run, executed the worker side,
uploaded the worker artifacts, and sealed the window. The run then needed
validator challenges to move from `replay_required` to accepted. A separate
fresh validator Pylon also installed through the public `0.1.6` package and
the bare `pylon` command successfully claimed validator work and ran the local
Psionic validator replay. That proved the validation compute path was real.
The failure was after replay: terminal authority reporting was ordered behind
artifact/TRN publication. During live proof the validator completed replay but
then blocked in `publish_training_trn_state -> upload_bundle` while trying to
send retained evidence through the signed artifact path. Because that happened
before `finalize_validator_challenge`, `reconcile_window`, accepted-outcome
projection, and payout observation, a slow evidence upload could wedge the
public earning loop before Nexus ever accepted the work or Treasury ever saw
an accepted-work payout. That is not a cosmetic bug. It means `0.1.6` can
leave a real user with completed local work and no payout-producing closeout
until an operator intervenes.

The trial also exposed an operational mistake future agents must avoid. Do not
run standalone `pylon training intake` or `pylon training sync` against the
same Pylon home while a bare `pylon` process is already running. The standalone
CLI and the long-running process share the same JSON state and artifact
directories, but they do not share the same in-memory supervisor slot. In the
`0.1.6` proof that produced confusing validator behavior: one process could
claim or materialize a challenge while the other process still owned a
previous supervisor lifecycle, which made stale leases and overwritten
invocation manifests look like backend bugs. If a live bare Pylon is running,
inspect it through its admin endpoint or stop it before running standalone
commands. This is now recorded in the Nexus deploy runbook because otherwise
future agents will rediscover it under pressure and contaminate their own
proof.

The `0.1.7` fix is narrow and preserves the product claim. Terminal worker and
validator status is now reported to Nexus before artifact/TRN publication is
attempted. That means validator finalization, window reconciliation,
accepted-outcome projection, and accepted-work payout queuing are no longer
blocked behind slower evidence publication. Artifact/TRN publication still
runs afterward and remains important evidence, but it has a bounded terminal
publication timeout and can retry later instead of wedging the earning loop.
This keeps the public user path aligned with the video claim: install or
update Pylon, run only `pylon`, be online for available hosted training work,
and receive payout only when the homework work is accepted. It does not bring
back placeholder payments, periodic 600-sat liveness sends, or admin-only
manual closeout as proof.

The minimum honest public paid-training release is therefore
`pylon-v0.1.7` / `@openagentsinc/pylon` `0.1.7`, not `0.1.6`. Production Nexus
must require `min_pylon_version=0.1.7` for newly launched hosted starter and
admin-paced homework runs once the corresponding release exists. Older
artifacts remain useful historical evidence for the sequence of bugs: `0.1.4`
proved public install and worker sealing, `0.1.5` proved package-managed bare
earning-loop launch, and `0.1.6` proved the worker-first retained lease fixes
and validator-capable public default. None of those older releases should be
used as the closeout floor for #4413 because the live proof showed they can
still fail to progress all the way from public `pylon` to accepted-work payout.

The operational lessons from this sequence have also been extracted into
`docs/deploy/PYLON_NEXUS_EARNING_RELEASE_RUNBOOK.md` so future agents do not
need to reconstruct them from this audit. That runbook is the practical
checklist for the next release attempt: use the local proof runtime first,
refresh both Cargo locks before Nexus image builds, publish the public Pylon
release before claiming public onboarding proof, do not mix standalone
`training sync` commands with a running bare `pylon` over the same home, and
close issues only after pushed `main`, deployed Nexus, public-style Pylon
execution, accepted work, and payout evidence all agree.

## Addendum: paid funding did not end the closeout because treasury persistence was hot-looping

Date: 2026-04-21

After the `0.1.7` release and the first production deploy, the next apparent
blocker was an underfunded Nexus treasury wallet. A 50,000 sat Lightning
invoice was generated through the hosted funding-target endpoint and paid.
That did solve the funding side: treasury status showed a spendable balance
near the invoice amount, and fresh accepted-work homework payouts began
dispatching and confirming. The lesson is that invoice payment has to be
verified from treasury status or payout movement, not from invoice creation,
not from a `504`, and not from a small cached balance movement. Once the paid
invoice was visible, insufficient funds was no longer the active blocker.

The deploy still could not be honestly accepted because the production gate
then failed on latency. Health, stats, provider scan, and training rollout
probes were slow even though CPU and memory were not saturated. The decisive
production clue was the Linux process I/O counter for `nexus-relay`: write
bytes increased by roughly gigabytes over seconds. A bounded `strace` showed
the service repeatedly writing `/var/lib/nexus-relay/treasury/treasury-state.tmp`
at roughly eighty megabytes and renaming it to `treasury-state.json`. The hot
file was not large because of current homework evidence. It was large because
the treasury state still contained tens of thousands of historical
`placeholder_liveness` payout records from the earlier system. Those rows were
no longer payable after placeholder/liveness payments were disabled, but they
still lived in the persistent payout map. No-op queue refreshes also persisted
the whole state file, so harmless repeated scheduling passes became repeated
eighty-megabyte disk writes.

The fix keeps the accepted-work payout contract intact. No-op payout queue
refreshes now refresh only the in-memory public snapshot instead of rewriting
the treasury file. Placeholder-liveness records that are already confirmed,
failed, skipped, or queued only because placeholder payouts are disabled are
compactable; retention now prunes old compactable placeholder records and caps
the remaining compactable placeholder set while retaining accepted homework
and beta-bonus records. When placeholder payout mode is disabled, attempted
non-accepted-work records are marked `skipped` with
`placeholder_payouts_disabled` so they can be compacted later instead of
staying in a misleading queued state. The tests added for this bug prove that
duplicate queue requests do not rewrite the treasury file, compactable
placeholder history is pruned without dropping accepted homework records, and
the earlier wallet-refresh reconciliation path for balance-blocked accepted
payouts still works.

The first deployed version of that fix reduced the file from roughly eighty
megabytes to under one megabyte, but a short `strace` still showed repeated
`treasury-state.tmp` writes at provider heartbeat pace. That remaining churn
came from public Pylons reissuing payout-target challenges and registering the
same Spark/Bitcoin payout target after the relay restarted. The system was
treating every unchanged verification as a new durable registration receipt.
That was unnecessary and expensive. Challenge issuance is now in-memory, and
registering an unchanged target consumes the live challenge, refreshes the
last-verified timestamp in memory, and returns without persistent state
rewrite or receipt emission. A real target change still persists and emits the
registration receipt. The endpoint also refreshes the public stats cache after
the mutation so the UI and `/api/stats` reflect newly registered targets
without depending on persistent treasury snapshot churn.

Future agents should treat this as a distinct class of production failure. A
paid treasury invoice can clear the funding blocker while a state-persistence
bug still prevents deploy acceptance. If deploy gates fail on latency after
funding is confirmed, check `/proc/<pid>/io`, state-file size, and a short
`strace` before changing gate thresholds or redeploying the same image. The
correct closeout proof remains stricter than "wallet funded" or "some payouts
confirmed": a pushed `main` commit, a registry image built from that commit, a
scripted production deploy, passing gates without hot state rewrites, a fresh
public-style Pylon run from the minimum public release, accepted homework work,
and a confirmed accepted-work payout.

## Addendum: issue 4413 public Pylon earning proof completed

Date: 2026-04-21

The final production closeout used the pushed `main` commit
`2a7986b42d77e9210bbb873d6df550ebbaee02e8` and the production image
`us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:2a7986b42d77`.
That image was built by the canonical Cloud Build script, deployed through the
canonical Nexus deploy script, and verified by the canonical deploy gates. The
tracked receipts are
`docs/reports/nexus/20260421-220923-cloudbuild-image-2a7986b42d77.json` and
`docs/reports/nexus/20260421-221747-deploy-receipt.json`. After the deploy,
the treasury state file remained compact, the hot write loop stopped after
startup compaction, and the gate script passed instead of hiding the issue by
relaxing latency thresholds.

The public Pylon proof used the shipped public package
`@openagentsinc/pylon@0.1.7`, resolving tag `pylon-v0.1.7` with
`installMethod=release_asset` and `cached=false`. The first proof attempt
intentionally isolated `HOME` as well as Pylon state, which exposed a useful
operator lesson: a synthetic `HOME` also hides the user's Rust toolchain from
`rustup`, so `psionic-train` exits before doing any work even though Pylon can
discover the sibling `psionic` checkout from the workspace. The successful
proof kept Pylon state isolated with `OPENAGENTS_PYLON_HOME` and
`OPENAGENTS_PYLON_CONFIG_PATH` but used the normal user `HOME`, matching the
way a real local user has Rust and the local training runtime installed. The
user-facing command after bootstrap was still bare `pylon`; there was no
CS336-specific command, direct GCS credential, or operator-only Nexus credential
in the worker Pylon environment.

The worker Pylon came online from fresh state, created a local Spark payout
destination, and the admin-paced homework dispatch endpoint was also exercised
against production. Nexus matched the online Pylon at pubkey
`8cf8a9a1878a6db868d8f9be4bc623c4617ecd9ccab915b3b4db3f4d1763651d`. Because
the production starter path also auto-launched work for online
paid-training-capable Pylons, the worker produced several sealed homework
windows. A second fresh public Pylon was run as validator-only on distinct
local ports (`127.0.0.1:9469` for admin and `127.0.0.1:9571` for checkpoint
serving) so pending validation windows could close without relying on a hidden
manual operator reconcile. The validator reached `validator_finalized`, Nexus
reconciled accepted outcomes, and Treasury created accepted-work payouts only
after accepted closeout.

The final payment proof is stronger than "Nexus says dispatched." Treasury
status showed zero accepted-work payouts pending, zero accepted-work payouts
needing attention, no active continuity alerts, and confirmed settled payout
records for the worker's accepted outcomes. The worker's own Pylon wallet then
showed `100` sats total and four completed receive records of `25` sats each:
`019db229-bb3f-79b1-b38b-6f00c21a7b24`,
`019db229-4c39-7ea1-aa56-c9abf26c7672`,
`019db228-cd41-7262-bf84-8e5b93afa2f6`, and
`019db228-52ba-7a42-be79-a394ef5838e9`. The corresponding Treasury records
were `confirmed` with `reconciliation_status=settled` and payout class
`accepted_work`, not placeholder or liveness payment. The tracked proof receipt
with the exact run ids, accepted outcome ids, contribution ids, and payment ids
is `docs/reports/nexus/20260421-223232-issue-4413-public-pylon-proof.json`.

This closes the narrow original launch claim in the form that is currently
true: a local operator can install/update to Pylon `0.1.7`, run `pylon`, stay
online for available hosted starter training work, complete accepted homework
work, and receive Bitcoin in the Pylon wallet. It does not prove a broad
permissionless training marketplace, arbitrary assignments, GPU rental, or a
dashboard-login flow. It also does not remove the operational prerequisite that
this local training lane needs an installed Rust toolchain and a discoverable
compatible `psionic` runtime checkout until that runtime is bundled into the
standalone release. Future product/docs work should make that prerequisite
explicit or eliminate it, but it no longer blocks the issue 4413 proof because
the system paid a fresh public Pylon for real accepted hosted training work in
production.

## Addendum: the post-4413 path moved the defensible npm/admin-dispatch floor to 0.1.8

Date: 2026-04-22

After the `0.1.7` public earning proof, the next goal was not merely to show
that hosted starter work could eventually pay a public Pylon. The stricter goal
was to make the operator-paced homework path work from npm in the shape we
actually want to run: one process starts Pylon from the public npm bootstrap,
a separate admin process triggers bounded homework work, a validator closes the
work, Treasury pays only accepted homework, and an operator can put that trigger
behind cron to pace payouts. That exposed several bugs that were hidden by the
earlier proof. The earlier `0.1.7` floor remains historically important because
it proved "run Pylon and get paid" for hosted starter training. The current
floor for the npm proof plus admin-paced homework dispatch is now
`pylon-v0.1.8` and Nexus `fb60b9167` or newer.

The first set of post-audit changes made dispatch selection deterministic
enough for operator use. Nexus worker lease claims now prefer existing
admin-dispatched homework runs before auto-launching fresh hosted starter work.
Without that fix, a Pylon could be online and eligible while repeatedly taking
starter jobs, leaving the operator-triggered run unclaimed. Validator claims
now prioritize `homework_dispatch` windows before draining starter backlog.
Without that fix, a worker could seal the operator-triggered window while a
validator spent its time elsewhere. These were not convenience changes; they
made the admin pacing endpoint useful as a real operations control rather than
a best-effort debug button. The relevant commits were `8387d22f4` for worker
lease priority and `58aa9e0b7` for validator claim priority.

The second set of changes narrowed the validation surface to what the current
homework lane can honestly defend. We disabled per-contribution sample
challenges for `homework_dispatch` and kept the aggregate validator challenge
as the live proof gate. That was necessary because the retained
contribution-sample replay path could still produce artifact-manifest digest
drift under npm Pylon. Pylon `0.1.8` then fixed the retained validator replay
case where a same-host local target path or stale retained target artifact id
could point at bytes whose digest no longer matched the claim. The released
binary now falls back to the bridge-inline payload or rewrites the target
artifact id to match the materialized digest. The relevant commits were
`512b51f4b` for aggregate-only homework validation and `064d64feb` for the
Pylon `0.1.8` replay stabilization.

The third change fixed the final authority gap in Nexus closeout. Once
homework validation became aggregate-only, a live window could reach
validator-finalized and verified but still reconcile as refused, because the
aggregate verdict was not mapped back onto the contribution outcome in the
absence of a per-sample contribution disposition. That was the last serious
"everything succeeded locally but the worker still does not get paid" class of
bug in this sequence. Nexus now treats aggregate-only homework validation as a
defensible accepted contribution outcome when the aggregate terminal
disposition is accepted. The commit was `fb60b9167`, and the focused regression
was the aggregate-only retained homework closeout test that proves a
per-sample disposition is not required for reward eligibility in this lane.

The production deploy sequence for those changes created a clear record of
what did and did not work. We first deployed the `08841d0dc03` image, which
contained the `0.1.8` release and dispatch/validation-priority fixes, and it
proved worker contribution plus aggregate validator finalization but still
exposed the aggregate-only closeout reward bug. After `fb60b9167`, we built
and deployed
`us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:fb60b91678ca`.
The retained receipts are
`docs/reports/nexus/20260422-031716-cloudbuild-image-08841d0dcc03.json`,
`docs/reports/nexus/20260422-033200-deploy-receipt.json`,
`docs/reports/nexus/20260422-034137-cloudbuild-image-fb60b91678ca.json`, and
`docs/reports/nexus/20260422-034836-deploy-receipt.json`. The important
discipline was that the failed `08841d0dc03` live proof did not get renamed
as success. It became evidence for the next local regression and production
fix.

The final npm end-to-end proof used a fresh proof root at
`/private/tmp/pylon-npm-e2e-20260422T034929Z`, public package
`@openagentsinc/pylon`, release `pylon-v0.1.8`, an isolated worker Pylon home,
and an isolated validator Pylon home on separate local admin/checkpoint ports.
The operator-triggered run was
`run.cs336.a1.codex-npm-e2e-20260422035018_20260422035019_b9ece834_0001.20260422035019.6fea0c9f`
with window
`window.cs336.a1.codex-npm-e2e-20260422035018_20260422035019_b9ece834_0001.20260422035019.6fea0c9f.0001`.
The worker contribution
`b7ec87b71ee077d40eeac38d5801096e8f7993368173310b54799891260b16a6`
reconciled with `accepted_contributions=1`, `replay_required_contributions=0`,
`closeout_status=rewarded`, and `payout_eligible=true`. Treasury then recorded
a confirmed, settled accepted-work payout of `25` sats with payment id
`019db352-4986-7ff3-8b9a-b3f8f1331cbe` to the worker Spark payout target.
The worker wallet balance showed `25` sats. The proof receipt is
`docs/reports/nexus/20260422-035746-pylon-npm-e2e-fb60b91678ca.json`.

The proof also added two operator lessons that were not obvious from the video
claim. First, the npm launcher opens `pylon-tui` by default. That is fine for
interactive onboarding, but it is not safe for noninteractive proof automation:
in a noninteractive shell it can fail with `Device not configured`. The
runbook now tells future operators to use `npx @openagentsinc/pylon --no-launch`
for bootstrap and then run the installed `pylon` binary directly in the worker
or validator process. Second, Spark wallet history may return an empty
`payments` list for this internal Spark receive even when the worker balance
has increased and Treasury has a confirmed settled accepted-work payout. The
completion criterion is therefore treasury confirmed+settled accepted-work
record plus worker wallet balance, not wallet history alone.

The admin-facing operator surface is now documented as a pacing mechanism,
not a one-off rescue path. The endpoint
`POST /v1/admin/homework/cs336-a1/dispatch` accepts `run_count`,
`max_contributors_per_run`, `amount_sats`, `total_budget_sats`,
`window_duration_seconds`, `only_online`, `min_pylon_version`, and optionally
a `network_id`. That is enough for an operator to run a cron-like loop that
creates a bounded amount of duplicate-allowed homework work at each interval
and caps maximum payout exposure per batch. The corresponding runbook is
`docs/2026-04-22-pylon-homework-dispatch-operator-runbook.md`. The product
contract remains simple for the provider: install/update Pylon, run `pylon`,
stay online for relevant paid training jobs, and receive sats only for accepted
homework work. The provider does not have to opt into the homework assignment
manually.

Finally, the repo/workspace hygiene problems that made this sequence feel
untrustworthy were cleaned up. The main `openagents` checkout had broad
unrelated dirty edits across Autopilot, Data Market, Pylon TUI, Nostr, kernel,
and generated receipt files. Those were not part of the Nexus/Pylon proof and
were preserved in a named stash instead of being committed as noise or deleted
as possible user work. The workspace root and `openagents.com` had unrelated
dirty files as well, and those were also preserved in named stashes. Stale
worktrees were removed, merged local branches in sibling repos were deleted,
and merged remote feature branches in `probe` and `forge` were pruned. After
that cleanup, `openagents`, `openagents.com`, `psionic`, `psionic-pylon`,
`probe`, `forge`, `treasury`, `alpha`, `dataroom`, `control`, `backroom`, and
the workspace root were all back on clean `main` checkouts tracking
`origin/main`. That matters because the recurring failure mode in this issue
family was not just code defects; it was the inability to tell which checkout,
branch, worktree, deploy image, release tag, or proof artifact was authoritative.
