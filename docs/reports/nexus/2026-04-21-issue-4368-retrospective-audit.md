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
from a hand-built local checkout with hidden operator context. The user runs
one Pylon command or one clearly documented two-command sequence. Pylon creates
or loads its wallet and node identity, joins the hosted Nexus, advertises the
right CS336 capability, receives a real Assignment 1 worker or validator lease,
drives runtime execution without the operator manually sequencing `training
intake`, `pylon serve`, and `training sync`, uploads required artifacts through
a credential path that does not require the user to hold OpenAgents operator
secrets, reports accepted or rejected status clearly, and shows the matching
accepted-work payout as confirmed or settled. The final proof must use code
merged to and pushed on `main`, must run against production Nexus only after
the local proof lane is green, and must produce a report with run id, node id,
assignment id, accepted outcome id, payout id, payment state, and the exact
user-facing command sequence.

The minimum path has four work packages. First, Pylon needs a self-serve CS336
earner mode so the user does not need to understand internal training
subcommands. Second, Nexus needs a hosted CS336 starter-work lane that can
assign and pay accepted work to self-serve Pylons without a bespoke operator
launch each time. Third, artifact and payout plumbing need to be safe for a
public Pylon: no ADC surprise, no operator bearer token requirement, no
placeholder payout ambiguity, and no raw treasury interpretation required by
the user. Fourth, the team needs one final public-style proof run from a clean
machine profile that demonstrates the exact video claim and records it in a
single closure report. The issues below are drafted for review before creating
them with `gh issue create`.

The current public install instructions are the `PYLON_AGENT_INSTRUCTIONS`
string in the `openagents.com` repo at `resources/js/pages/welcome.tsx`. Those
instructions are still accurate for the current standalone onboarding truth:
install or resolve Pylon through `npx @openagentsinc/pylon`, direct release
asset, or source fallback; verify `local_gemma`; diagnose Gemma 4; avoid
account linking by default; and treat local Gemma inference as a complete
local bring-up. They are not sufficient for the initial video claim. They do
not currently tell a user how to join a hosted CS336 Assignment 1 training
lane, how to require a paid-training-capable Pylon release, how to run the
future self-serve earner command, or how to distinguish Gemma-only readiness
from CS336 accepted-work earning readiness.

The latest Pylon binary release at the time of this addendum is
`pylon-v0.1.1`, published on 2026-04-17 from commit
`04b83f05e97d5f56afc8c6a1a151518e310ecd54`, with official `darwin-arm64` and
`linux-x86_64` assets. The npm bootstrap package is currently
`@openagentsinc/pylon` `0.1.4`, and its documented default behavior is to
resolve the newest tagged `pylon-v...` GitHub release unless a specific
`--version` is supplied. Therefore `pylon-v0.1.1` should be treated as the
minimum release for the current Gemma-local bring-up, not as the minimum
release for CS336 paid training. The planned minimum release for the public
CS336 earning claim should be the first tagged Pylon release that contains the
self-serve CS336 earner mode, hosted-lane compatibility, public-safe artifact
path, and accepted-work payout projection. If release numbering stays on the
current line, reserve `pylon-v0.1.2` as that minimum. If the release number
changes before these issues are created, update the issue bodies to name the
actual first paid-training-capable GitHub release tag.

When the paid-training release exists, the `openagents.com` instructions need
an explicit branch in the agent prompt. For ordinary local bring-up, they can
continue to say that `npx @openagentsinc/pylon` and `pylon gemma diagnose
gemma-4-e4b` are the right default path. For CS336 earning, they should require
`pylon --version` or equivalent release metadata to prove the installed binary
is at least the paid-training minimum release, instruct the agent to update via
the npm bootstrap or direct GitHub release asset if the installed version is
older, and then run the self-serve command such as `pylon earn cs336-a1`
rather than stopping at `pylon gemma`, `pylon online`, or `pylon serve`. The
instructions should say plainly that Gemma readiness is necessary for the
current inference lane but not sufficient evidence that the node is earning
from CS336 training work. CS336 earning requires admission, assignment,
accepted outcome, and accepted-work payout state.

### Draft issue 1

Title: `Ship self-serve Pylon CS336 earner mode`

Body:

```markdown
## Goal

Make Pylon capable of running the CS336 Assignment 1 earner loop through a
single public-facing command or a very small documented command sequence. A
normal user should not need to know the internal operator sequence of
`training intake`, `pylon serve`, and `training sync`.

## Scope

- Add a public Pylon mode such as `pylon earn cs336-a1` or equivalent.
- Set the planned paid-training minimum release to `pylon-v0.1.2` unless the
  first shipped self-serve CS336 release uses a different tag.
- Default that mode to the hosted Nexus endpoint and the current public CS336
  Assignment 1 training network policy.
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

- A fresh Pylon home can run the new mode against the local #4385 proof runtime
  and complete at least one accepted Assignment 1 worker or validator path.
- The same mode can run against production Nexus after local proof passes.
- The user-facing command emits enough status to understand whether the node is
  waiting, working, accepted, paid, or blocked.
- The command does not require an OpenAgents operator bearer token or manual
  GCS credential export by the public user.
- Documentation includes the exact install/update and run command.
- The `openagents.com` `PYLON_AGENT_INSTRUCTIONS` draft is updated or a
  companion patch is prepared so agents know that local Gemma bring-up remains
  valid but CS336 earning requires the paid-training-capable Pylon release and
  the new self-serve command.
- Completion is committed and pushed to `main`; branch-only work does not close
  this issue.

## Required proof

- Local proof command and artifact path.
- Clean Pylon-home transcript showing the user-facing command.
- Output proving the installed Pylon release is at or above the paid-training
  minimum release.
- Production run id or explicit statement that production proof is delegated to
  the final public-style proof issue.
```

### Draft issue 2

Title: `Create hosted CS336 starter-work lane for self-serve Pylons`

Body:

```markdown
## Goal

Make hosted Nexus provide enough CS336 Assignment 1 work for self-serve Pylons
to receive real training-network assignments and earn accepted-work payouts
without a bespoke manual launch for every proof.

## Scope

- Define the hosted CS336 Assignment 1 starter-work policy.
- Keep payout basis accepted-work only; placeholder/liveness payouts must stay
  disabled for this lane.
- Make the lane discoverable to self-serve Pylons that advertise the expected
  capability and payout target.
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

- Local proof runtime models the hosted starter-work lane and covers admitted,
  assigned, accepted, paid, rejected, and insufficient-funds cases.
- Production Nexus can make at least one starter Assignment 1 lease available
  to a clean self-serve Pylon.
- Accepted work produces exactly one accepted-work payout record.
- Public/admin stats distinguish online, admitted, assigned, accepted, and
  paid states for this lane.
- Documentation states what the hosted lane does and does not prove.
- Completion is committed and pushed to `main`.

## Required proof

- Local proof report path.
- Production Nexus run id and training network id.
- Accepted outcome id.
- Matching payout id and final payment state.
```

### Draft issue 3

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
  public-safe mechanism for self-serve CS336 Pylons.
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

- A public self-serve Pylon does not need `GOOGLE_APPLICATION_CREDENTIALS` or
  `OPENAGENTS_PYLON_TRAINING_GCS_BEARER_TOKEN` supplied by an operator.
- Artifact upload failures are reproduced in local proof and reported with a
  user-facing reason.
- Accepted-work payout state is visible from Pylon without querying raw Nexus
  or Treasury endpoints manually.
- Placeholder payout totals cannot be mistaken for CS336 accepted-work payouts
  in the Pylon status path.
- Completion is committed and pushed to `main`.

## Required proof

- Local proof covering artifact success and artifact failure.
- Local proof or production proof covering accepted-work payout projection.
- Clean Pylon status output with payout id and settlement state.
```

### Draft issue 4

Title: `Prove public-style CS336 Pylon earning end to end`

Body:

```markdown
## Goal

Close the initial video claim with a clean public-style proof: a fresh Pylon
user runs the documented command, receives CS336 Assignment 1 training-network
work, completes it, and gets paid Bitcoin for accepted work.

## Scope

- Start from `main` after the self-serve Pylon mode, hosted starter-work lane,
  and public-safe artifact/payout changes are merged and pushed.
- Use a fresh machine profile or fresh user-home equivalent with no retained
  Pylon state.
- Install or update Pylon using the documented public instructions and verify
  that the resolved binary is at least the paid-training minimum release
  (`pylon-v0.1.2` if that remains the first self-serve CS336 release).
- Run only the documented public command sequence.
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

- Fresh Pylon identity connects to hosted Nexus from the documented public
  command path.
- The node is admitted for the CS336 Assignment 1 lane.
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

- Local proof runtime report from the exact shipped commit.
- Production Nexus run id.
- Pylon command transcript from fresh state.
- Accepted outcome id.
- Payout id and settled or confirmed payment state.
- Report path committed to `main`.
```

These four issues are intentionally ordered. Issue 1 makes Pylon usable as an
earner and defines the release floor. Issue 2 ensures hosted Nexus has real
accepted-work demand for that earner. Issue 3 removes the operator-only
artifact and payout interpretation blockers. Issue 4 is the public-style proof
and should be the only issue that claims the initial video promise is complete.
If the first three issues reveal that the scope can be collapsed, the proof
issue can absorb the smaller remaining work. If any issue grows beyond the
CS336 Assignment 1 user-earning loop, split it rather than weakening the
definition of done. Before creating the issues with `gh issue create`, replace
`pylon-v0.1.2` if necessary with the actual first GitHub release tag that
contains the self-serve CS336 earning path.
