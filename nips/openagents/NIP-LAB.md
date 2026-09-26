# NIP-LAB — Bounded agent labor

`draft` `optional` — v1. The [shared contracts](contracts.md) and
[NIP-MKT](NIP-MKT.md) are normative. This is the
`openagents.labor.v1` market profile, not an implemented worker or payment
service. It assigns no new event kinds.

**Name provenance:** OpenAgents already used
[NIP-LBR v1](https://github.com/OpenAgentsInc/openagents/blob/8f84d05896ef14edee491621bf977ee5315cc8ed/docs/nips/LBR.md)
for NIP-90 labor events at `5934`/`6934`/`7000`. The
[August 4 migration decision](https://github.com/OpenAgentsInc/openagents/blob/8f84d05896ef14edee491621bf977ee5315cc8ed/docs/nips/NIP90-MIGRATION.md)
froze that wire for compatibility and proposed an LBR v2 without allocating
kinds. LAB keeps a distinct name for `openagents.labor.v1`, using private
`3188` artifacts, MKT agreements, and CJ/RUN execution. It does not claim
legacy wire compatibility or upgrade old records. The
[upstream sync review](../../docs/protocol/2026-09-26-upstream-nip-sync.md)
records the naming assessment.

An agent performs a bounded task for an agreed deliverable. Coding is the
initial example; research, document preparation, evaluation, and other domains
can use the same profile with admitted operations and output schemas. A worker
identity identifies who signed a submission. It does not certify competence,
independence, ownership of every input, or quality.

A worker may use [NIP-X402](NIP-X402.md) to buy an upstream tool or resource
under a separate host spend grant and task budget. That upfront purchase is
an input cost. It does not establish delivery, verification, acceptance, or
payment of this labor order. Do not infer approval for another paid call from
a prior purchase, a wallet timeout, or an unpaid labor invoice.

## Scope and commercial limits

MKT negotiates the order, identifies buyer/provider/worker, and owns settlement.
LAB binds that order to execution, deliverables, verification, and acceptance.
Use [CJ](NIP-CJ.md) for execution and controls, [RUN](NIP-RUN.md) for retained
effects, [CTX](NIP-CTX.md) for source state, and [POL](NIP-POL.md) for authority
and disclosure. An accepted order is not a host grant, a merge permission, or
authorization to spend from a buyer's wallet.

The first profile has a fixed full price or zero payment. It supports bounded
rework under unchanged requirements. It has no deposit, escrow, prorated partial
payment, automatic acceptance, penalty, royalty split, or unilateral amendment.
Those need another supported profile. A different deliverable, price, worker,
scope, checker, or deadline requires a new bilateral order. Retain the old
order and reconcile its work and settlement before starting overlapping work.

Execution completion, delivery, independent verification, buyer acceptance,
integration into a destination, and payment are separate facts. No one boolean
or status can substitute for them. Settlement of a dispute does not prove that
an interrupted external effect stopped.

## Encoding, references, and signers

All LAB bodies are closed JSON objects. They require their named `v`,
`requires: []`, and every field listed for that body. Only an optional inert
`meta` object is additional. Reject unknown semantic fields, enum values,
versions, duplicate keys, and unsupported required features before any effect.
Common ArtifactRef, DefinitionRef, EventRef, ID, integer, size, and parsing
rules apply. Arrays are limited to 64 entries unless a smaller limit is stated.
A referenced contract must resolve to exact retained bytes before
it is relied on; a title, branch, current catalog head, or unavailable digest
cannot substitute.

`OrderRef` is the exact MKT reference to the confirmed bilateral order. Compare
its complete identity and resolve both signed order records. A LAB record
cannot manufacture confirmation by asserting an `order_id`. Every subsequent
LAB record carries `order: OrderRef`; its references must remain within that
order, apart from explicitly pinned source and implementation artifacts.

Commercial records use the private `3188` artifact envelope. Each signed LAB
record has an `issuer` pubkey. Its declaring envelope signer MUST equal issuer;
authenticate the original signed declaration when another participant forwards
the record. An ArtifactRef's digest by itself proves no author. Copies for
different authorized recipients preserve artifact identity but have different
envelope event IDs. Do not expose task names, order identifiers, plaintext
digests, prices, or dispute evidence in public tags.

The MKT buyer and provider remain distinct commercial roles even if one
operator controls several keys. The worker must equal the worker in the
accepted market terms. A NIP-OA agent owner can be the provider; it must not
be silently reassigned to each buyer. Owner attestations and NIP-AA relay
membership convey no labor, repository, spending, or settlement authority.

## Labor terms

The market terms' `profile_terms` points to an artifact with
`v: "openagents.labor-terms.v1"` and these fields:

| Field | Contract |
| --- | --- |
| `task_frame` | Exact `openagents.task-frame.v1` ArtifactRef. The frame pins objective, source snapshot, instructions, constraints, and acceptance requirements. |
| `execution` | Frozen operation/input and initial context defined below; rework can add only the separately bound context described below. |
| `deliverables` | Nonempty array of `{id, schema, max_bytes}`: unique slug, pinned SchemaRef, and positive byte ceiling. |
| `reviewer` | Exact verification signer pubkey, distinct from provider and worker. It can be the buyer. |
| `acceptance_policy` | ArtifactRef to the closed policy below. |
| `resolver` | Exact dispute resolver pubkey, distinct from buyer, provider, worker, and reviewer. |
| `resolver_policy` | Literal `labor-evidence-v1`, whose rules appear below. |
| `max_reworks` | Integer from zero through three; aggregate attempts and cost still meet execution bounds. |
| `rework_due_at` | Unix seconds or null. Null exactly when max_reworks is zero. |
| `dispute_due_at`, `resolution_due_at` | Absolute Unix-second deadlines. |
| `cancellation` | Literal `evaluate-delivered-work-v1`. |
| `partial_delivery` | Literal `no-partial-payment-v1`. |
| `buyer_unavailable` | Literal `resolver-required-v1`. |
| `rights` | ArtifactRef to the closed rights policy below. |
| `role_relationships` | Nonempty array of `{pubkey, operator}` for every buyer, provider, worker, reviewer, and resolver key; operator is a declared opaque organization/operator ID. |

The MKT RFQ's `request` also uses `openagents.labor-terms.v1`, as proposed
terms. An RFQ conveys no acceptance or grant. The provider's quote pins the
exact proposed or revised labor-terms artifact; the buyer must review and
accept those quoted bytes. Never treat the RFQ alone as the final agreement.

Keys alone do not establish independent control. Participants must disclose
known shared operator relationships. The buyer admits the reviewer, checker,
resolver, and declared independence before signing. The provider must also
accept the resolver and policy. Independence claims remain attributable
claims; a client may require separately verified organizational evidence.

The task frame's owner must equal the buyer. The task frame and initial
functional inputs are frozen by the terms. Rework adds only the explicit
bounded observations below; it does not change the objective or acceptance
contract. A changed source revision does not silently
change the task; report the conflict and obtain another order where required.
Integration into a mutable destination must check its current preconditions
under a separate grant. An exact input capture is not an assurance that an
external resource remains unchanged.

Execution is `{target, lock, input, context, requirements, bounds}`. Target is
a DefinitionRef; lock pins its complete implementation closure; input is an
ArtifactRef whose schema the target explicitly supports as artifact input;
context is a worker-recipient `openagents.context.v1` ArtifactRef;
requirements is an ArtifactRef to the admitted effect/assurance requirements;
bounds uses the shared bound vocabulary. Required target input and referenced
policy schemas must be independently supported by the host. They are not
arbitrary prose to interpret as commands. The terms grant no credentials.

The checker policy has `v: "openagents.labor-acceptance-policy.v1"`,
`checker` (DefinitionRef), `lock` (exact checker lock ArtifactRef), `criteria`
(nonempty array of unique slugs), and `rule: "all-pass-v1"`. The checker
interface must return exactly these criterion IDs with the verification
verdicts below. All must pass. The buyer pins this policy independently of
worker-produced code; the worker cannot edit criteria, tests, protected labels,
checker identity, its functional dependency closure, or its evidence retention.
A model judge is permitted only as an explicitly admitted checker whose
identity, limits, and uncertainty are retained. A passing checker does not
replace buyer acceptance.

The following ordering is required in addition to MKT's deadline rules:

```text
delivery_due_at <= review_due_at <= dispute_due_at
dispute_due_at < resolution_due_at < payment_due_at < retain_until
```

When rework is enabled,
`delivery_due_at <= rework_due_at < review_due_at`. All deadlines are fixed
in the accepted terms. Rework does not restart any clock or renew the budget.
The first complete submission is due by delivery_due_at; each authorized
replacement is due by rework_due_at. A review requests rework only while a
replacement can still be admitted within that deadline and the remaining
bounds. A late delivery cannot become timely by changing its signed timestamp.

## Link the confirmed order to execution

Create execution identities after bilateral market confirmation. This ordering
avoids a circular digest between quote, order, CJ request, and execution result:

1. Freeze source/input, labor terms, and market terms.
2. Confirm the exact MKT order.
3. Construct and retain a signed CJ execute event locally for the frozen
   execution, without publishing it yet.
4. Produce the buyer-signed LAB linkage to that exact event and deliver its
   private declaration and OrderRef to the worker's admitted labor intake.
5. The worker validates and durably records the linkage before it can accept
   the matching execute event. Then publish the execute event and retain the
   worker's actual admission/result evidence.

A linkage has `v: "openagents.labor-execution.v1"`, `issuer` (buyer),
`order`, `request` (CJ logical ID), `attempt` (positive integer), `run`
(CJ run ID), `execute` (exact signed CJ EventRef), `execute_body`
(ArtifactRef of its decrypted execute body), `rework` (buyer-review ArtifactRef
or null), `context` (exact supplied context ArtifactRef), and `previous` (null for the
first attempt, otherwise the preceding linkage ArtifactRef).

The execute event must be signed by the buyer and addressed to the agreed
worker. It requires `openagents.labor-binding.v1` in its CJ `requires` list.
This feature means that the worker must authenticate and durably admit the
matching LAB linkage and confirmed order before dispatch. Generic workers
that do not implement this feature must refuse; they cannot strip it or treat
it as metadata. No additional execute-body order field is introduced.

Verify the CJ body against the retained decrypted bytes. Target, lock,
artifact input, requirements, and whole-order ceilings must agree
with the terms; per-attempt allocations can only narrow the remaining bounds.
Context equals the linkage context and follows the exact initial or rework
rules below.
Its deadline cannot exceed the applicable delivery or rework deadline, and
its retention horizon must cover the order's retain_until. One order uses one
logical request and run; permitted attempts increment monotonically with no branches.
Reconcile preceding effects and reservations before any permitted retry.

The worker must durably bind OrderRef to that request before dispatch under
its own admission policy. A transport-only worker that cannot do so is not
a LAB worker. Labor intake processes the separately signed `3188` declaration
under a configured role or registered admission operation; encrypted artifact
arrival alone does not invoke arbitrary code. It verifies the original execute
event and retained plaintext binding without executing it. A CJ request with
no admitted matching linkage refuses or waits without effects under bounded
worker policy. CJ cannot retrieve an OrderRef from untrusted prompt text.
Retransmission cannot dispatch or bill twice. Another worker,
changed fingerprint under the same request/attempt, forked attempt, or
unrelated signed CJ result is not replacement evidence. Retain the worker's
accepted RUN reference, observed
outcome, failed/refused attempts, usage, cancellation, and unknown accounting.
An order confirmation does not establish worker admission or execution.

The buyer and worker can validate the CJ ciphertext/plaintext correspondence
because they are its encryption endpoints. A reviewer without that access
relies on the buyer's authenticated linkage and the worker's accepted input
digest and RUN evidence under the declared trust policy. It must not claim
independent decryption or execution attestation from those references alone.

Linkage retention is separate from ephemeral CJ delivery. If the ephemeral
request can no longer be fetched, the retained signed original must still
verify. Relay loss cannot justify inventing another job identity or assuming
the worker did nothing. RUN replay and CJ status reconcile that uncertainty.

### Rework context without changing the agreement

Initial attempts use `rework: null` and the terms' exact context artifact.
A rework attempt names a valid buyer `request_rework` review for the preceding
submission. Its context is a new buyer-admitted `openagents.context.v1`
manifest under the same task, worker recipient, and disclosure policy. It
retains all initial entries in the same order with their original content and
mandatory flags; required constraints and source versions cannot disappear.
It may append only evidence representations of that exact review, the prior
submission's deliverables, and the execution/RUN evidence that submission
names. The host authenticates that provenance and applies the original rights
and recipient policy to every added item. No new source input, instruction,
recipient, or requirement is admitted by calling it rework.

The target must explicitly support receiving these additional observations
through context; otherwise this order must set max_reworks to zero. The
original typed input and lock remain unchanged. The linkage pins the resulting
manifest, and the actual CJ context must match it before execution. Retries
of the same repair retain the same review and manifest; a changed manifest
needs a newly admitted linkage and reconciled prior effects. The worker cannot
fetch the buyer's latest conversation and treat it as this projection.

## Delivery and independent verification

A submission has `v: "openagents.labor-submission.v1"`, `issuer` (worker),
`order`, `number` (zero-based integer), `previous` (null at zero, otherwise
the prior submission ArtifactRef), `rework` (null at zero, otherwise the
authorizing buyer-review ArtifactRef), `executions` (nonempty linkage
ArtifactRefs), `deliverables` (array of `{id, content}`), `run_evidence`
(nonempty ArtifactRefs), and `limitations` (ArtifactRef to bounded inert text).

Each deliverable ID appears exactly once and matches the terms. Its content
ArtifactRef must pass the named pinned schema and byte ceiling. Retain exact
output bytes, source/base references, and execution records. A URL, screenshot,
claimed test pass, or prose summary cannot substitute for a required artifact.
Missing or inaccessible output is incomplete delivery, not an accepted
submission. Preserve incomplete attempts and their costs in RUN even when
they cannot form a complete submission.

Submission numbers are contiguous, at most max_reworks plus one submissions
exist, and all history remains available. A replacement changes deliverable
bytes and therefore submission identity. It does not overwrite the earlier
failure. A provider cannot offer several candidates under the same number
and let the buyer silently select a favorable one; that is a conflict.

A delivery receipt has `v: "openagents.labor-delivery.v1"`, `issuer` (buyer
or resolver), `order`, `submission` (ArtifactRef), `received_at` (Unix seconds),
and `available` (boolean). True means the recipient fetched and verified the
complete required closure under the terms; false cannot establish delivery.
This is an attributable receipt, not trusted global time. Provider-created
timestamps and relay acknowledgments do not prove delivery to the buyer.
A timely resolver receipt can preserve the provider's claim when the buyer
is unavailable. Disputed receipt times go to the pinned resolver; absence
of a receipt alone does not prove that work was never delivered.

A verification has `v: "openagents.labor-verification.v1"`, `issuer`
(reviewer), `order`, `submission`, `policy` (the exact acceptance-policy
ArtifactRef), `checker_receipts` (nonempty ArtifactRefs), `criteria` (array
of `{id, verdict, evidence}`), `verdict`, and `limitations` (ArtifactRef).
Evidence is an array of ArtifactRefs; criterion and overall verdict are
`passed`, `failed`, `unverifiable`, or `not_run`.

Return every criterion exactly once. Overall verdict is failed when any
criterion failed; otherwise unverifiable when any is unverifiable; otherwise
not_run when any is not_run; otherwise passed. The checker receipts must bind
the exact submission, frozen source/input, actual checker/lock, environment,
and all observations. Missing or substituted bytes cannot support passed.
Disclose checker failures and uncertainty. Reviewers run separately admitted,
bounded verification; they never execute worker instructions as authority.

Verification reports are private order evidence. A separately consented EVAL
publication can summarize outcomes with preserved denominators; it cannot
publish private tests, source captures, or commercial records by implication.
Verification means satisfying the agreed checker, not proving all possible
properties of the deliverable.

## Buyer review and bounded rework

A review has `v: "openagents.labor-review.v1"`, `issuer` (buyer), `order`,
`submission`, `verification`, `decision` (`accept`, `request_rework`, or
`reject`), `criteria` (array of affected policy criterion IDs), and `reason`
(ArtifactRef to bounded inert text). It references exactly the artifact the
buyer reviewed. An accept requires passed verification and empty criteria.
A direct accept also requires a validated available delivery receipt before
the applicable original or rework deadline. Its signed evidence is retained
in the final acceptance. Buyer acceptance cannot silently waive lateness.
A rework or rejection identifies the unchanged criteria it contests; new
requirements cannot be inserted in reason text.

The buyer can request rework only below max_reworks, before rework_due_at,
and when remaining execution bounds permit another attempt. The worker may
decline; no unbounded free repair obligation is created. Rework requires one
new numbered submission under these same terms and re-verification of all
criteria. Changed base/source or requirements require another order, not
an automatic rework request. Once final acceptance exists, no additional
submission or rework can alter its price or obligation.

A reject is a contested review, not a unilateral proof that nothing is owed.
The provider may dispute it by dispute_due_at. Silence is neither acceptance
nor rejection. A late review does not retroactively move a delivery deadline.
The final review deadline does not destroy retained evidence or authorize a
payment adapter to assume success.

## Disputes, cancellation, and an unavailable buyer

A dispute has `v: "openagents.labor-dispute.v1"`, `issuer` (buyer or
provider), `order`, `subject` (ArtifactRef to submission, review, cancellation,
verification, acceptance, or resolution), `cause` (`non_delivery`, `verification`, `review`,
`cancellation`, `buyer_unavailable`, or `conflict`), and `evidence` (nonempty
ArtifactRefs). Natural-language reasons can be evidence but never override
the frozen task or payment formula.

A cancellation has `v: "openagents.labor-cancellation.v1"`, `issuer` (buyer
or provider), `order`, `reason` (ArtifactRef to inert text), and `execution`
(latest linkage ArtifactRef or null when none exists). It requests cancellation;
it does not stop work or settle the order. Both parties can sign matching
cancellations to agree closure, but the actual stopped/unknown execution
state still comes from CJ/RUN reconciliation. Do not use NIP-09 deletion,
NIP-40 expiration, NIP-AO telemetry, or removal of an availability listing as
proof that a subprocess or external effect stopped.

`labor-evidence-v1` gives the named resolver these rules:

- Establish the accepted terms and identities first. Do not follow later
  replacement heads or infer authority from a market reputation score.
- Retain and assess all relevant delivery, verification, review, cancellation,
  and run evidence. Signed times are claims. Resolve disputed timing from the
  agreed records and explicit observations; insufficient evidence is unknown.
- Accept only a complete, timely submission whose frozen criteria passed.
  A disputed subjective requirement must be resolved against the original
  objective and criteria, never an added requirement. Record the reasoning and
  its evidentiary limitations. A resolver cannot fabricate missing checker
  results or substitute its own new checker.
- A buyer's timely accept with passed verification supports full payment. A
  buyer's unavailability does not forfeit the provider's work: the resolver
  may accept a timely conforming submission under the same policy.
- A cancellation before any qualifying delivered submission permits zero
  payment only when both parties agree or the resolver establishes that no
  qualifying delivery preceded cancellation. If delivery/timing is disputed
  or unknown, keep the commercial result unresolved. A buyer cannot cancel
  after receiving conforming work to bypass its acceptance assessment.
- A partial, late, or failed submission receives zero under this profile
  only after a supported rejection decision. An unknown result is not a
  rejected result. Independent partial compensation needs another agreement.
- An already accepted deliverable remains payable in full after cancellation;
  subsequent misuse, new work, or integration failure does not rewrite that
  acceptance. Fraud/conflicting signed final records require a recorded
  conflict and reconciliation, not automatic payment on first arrival.
- Resolve by resolution_due_at where evidence permits. If the resolver,
  reviewer, or required evidence is unavailable, record unresolved. Deadlines
  do not appoint a replacement resolver or automatically mint an acceptance.

The resolver is an explicitly trusted adjudicator, not a cryptographic oracle.
This profile provides no compulsory enforcement, escrow, or guaranteed
collection. Parties accept that trust and nonpayment risk in their order.
The resolver must retain evidence of its own receipt of disputes and timing;
late claims cannot retroactively amend a settled result. Conflicting claims
remain visible for reconciliation, even when raised after a deadline.

A resolution has `v: "openagents.labor-resolution.v1"`, `issuer` (resolver),
`order`, `submission` (ArtifactRef or null), `verification` (ArtifactRef or
null), `disputes` (array of dispute ArtifactRefs), `decision` (`accept`,
`reject`, or `unresolved`), `basis` (`delivered`, `non_delivery`, `criteria`,
`cancellation`, or `insufficient_evidence`), `evidence` (nonempty ArtifactRefs),
and `reason` (ArtifactRef to bounded inert text). Acceptance requires the same
complete/timely/passed conditions as buyer acceptance. A rejection requires
positive supporting evidence; unavailable evidence produces unresolved.

## Final acceptance and amounts due

An acceptance artifact has `v: "openagents.labor-acceptance.v1"`, `issuer`,
`order`, `submission` (ArtifactRef or null), `verification` (ArtifactRef or
null), `outcome` (`accepted`, `rejected`, or `unresolved`), `basis`
(`buyer_acceptance`, `resolver_decision`, or `mutual_cancellation`), `review`
(ArtifactRef or null), `resolution` (ArtifactRef or null), `amount_due_msat`
(integer or null), `supersedes` (array of prior acceptance ArtifactRefs), and
`evidence` (nonempty ArtifactRefs).

The following are the only accepted forms:

| Basis | Required issuer and evidence | Outcome and amount |
| --- | --- | --- |
| `buyer_acceptance` | Buyer; non-null accept review, submission, and passed verification; available delivery receipt establishing timeliness; resolution null. | accepted; exact MKT price_msat. |
| `resolver_decision` | Pinned resolver; non-null resolution; review null or a referenced review that the resolution assessed. | Maps accept/reject/unresolved to accepted/rejected/unresolved; full price/zero/null respectively. |
| `mutual_cancellation` | Buyer or provider; evidence contains matching buyer and provider cancellations, no prior final acceptance, and reconciled evidence that no qualifying submission was delivered before cancellation; review and resolution null. | rejected; zero. |

For an accepted outcome, submission and verification are always non-null and
must agree with the review or resolution. For rejection they may be null only
when no submission exists; otherwise retain the evaluated submission and any
available verification. Unresolved retains the latest known references or
null when unavailable; it never proves zero debt. Under free-v1 an accepted
outcome still has a zero price and remains distinct from rejection.

No provider/worker-issued success report can create buyer acceptance. A final
accepted record is not revoked by a later cancellation. Conflicting accepted
and rejected finals halt automatic settlement and require explicit resolution;
never use latest timestamp, relay order, or a replaceable head to select one.
Buyer and mutual-cancellation records require an empty supersedes array. A
resolver acceptance may supersede prior decisions only when its resolution
explicitly assesses every replaced artifact and all known disputed evidence
under unchanged terms. References must be acyclic and within the same order.
Supersession retains history, grants no new work, and cannot by itself reverse
a confirmed payment; a refund requires its own MKT agreement and transfer.
A resolver cannot supersede an uncontested accepted delivery merely because
a party changed its mind. Conflicting resolver decisions or an unresolved
branch remain a conflict; stop automatic settlement until the pinned resolver
issues a decision explicitly covering all known competing branches. A new
observed branch reopens that conflict, even after a purportedly complete
history. This protocol makes no global-history-completeness guarantee.

MKT's payment instruction references this exact acceptance artifact and
OrderRef. The amount is either the full fixed price or zero; payment fees are
bounded and accounted for by MKT, not subtracted from provider compensation
silently. Wallet requests, payout beneficiary, invoice verification, duplicate
payment prevention, and settled/pending/unknown receipts belong exclusively
to the chosen MKT settlement profile. No wallet secret or payment authorization
travels in a CJ input. A paid receipt proves neither deliverable quality nor
repository integration.

## Rights, disclosure, and retained proof

The rights artifact has `v: "openagents.labor-rights.v1"`, `license`
(ArtifactRef to exact legal/license terms), `input_use` (literal
`perform-and-review-order`), `output_use` (`review-only` or
`use-under-license`), `publication` (`deny` or `separate-grant`),
`training` (`deny` or `separate-grant`), `evaluation_reuse` (`deny` or
`separate-grant`), `redistribution` (`deny` or `separate-grant`), `recipients`
(nonempty array of exact pubkeys), and `retention` (literal
`through-market-retain-until`). These are restrictions under the host's
independently admitted disclosure policy, not proof that a seller owns rights.

The recipient set must include participants who need the relevant records:
buyer, provider, worker, reviewer, and resolver. Context/artifact delivery still
minimizes each recipient's scope. Inclusion in the set is not a grant to read
every input. Required verification or dispute material that cannot lawfully be
disclosed to its pinned recipient makes this profile inadmissible. Separate
secret inputs or protected labels require explicit recipient-specific context
and retention; never copy them into the worker's context to simplify recovery.

The exact license governs use of outputs, including rejected/partial outputs;
payment alone does not convey unrestricted copyright, training, or resale
rights. A `separate-grant` value authorizes nothing without that later grant.
Rights changes cannot be hidden in inert metadata. No automatic KB publication,
data sale, or training reuse follows from completing a labor order.

Participants retain their authorized copy of agreed terms, signed declarations,
input versions, submissions, verification, usage, review, disputes, acceptance,
and payment evidence through the admitted retention horizon. Required storage
capacity and access must be established before admission. Retention is bounded;
it does not authorize unlimited permanent storage. An earlier deletion or
policy conflict must be surfaced as unavailable evidence and may prevent
acceptance. Nostr deletion requests cannot prove erasure from other parties.

Keep unsuccessful attempts, lost or unknown outcomes, rework, evaluator spend,
fees, and provider costs alongside revenue. Missing costs are null, not zero.
Private artifact retention is not public trace publication. A selective public
EVAL or KB derivative requires a new consented disclosure, rights check, exact
derivation, and honest failure/unknown denominators.

## Availability is separate from existing obligations

An operator's Go online action publishes or updates a MKT offering with the
labor profile, exact CAP reference, capacity hint, expiry, and supported
payment profiles. The closed offering schema adds no concurrency or resource
fields: exact execution ceilings belong in quoted terms and host admission.
Host configuration owns the supported pinned operations, available execution
window, concurrency, private resource inventory, payout credentials, and actual
resource enforcement. Advertising capacity
does not reserve it; admission must atomically reserve the authoritative
capacity and whole-order budget before dispatch.

Pause withdraws availability for new orders. It does not delete confirmed
orders, clear reservations, cancel accepted work, discard recovery state, or
declare payment complete. A provider can separately request cancellation under
these terms. On restart, reconcile existing obligations before advertising
capacity again. Several devices or relays cannot each advertise the same
unreserved capacity as independently available.

Neither a successful CAP probe nor an owner badge establishes isolation from
buyer code. Require an admitted boundary and assurance level for source reads,
network calls, subprocesses, durable storage, and external mutations. The
provider's own repositories, account sessions, service credentials, and wallet
remain outside the task. Unsupported hard restrictions refuse before dispatch.

## Worked coding example

Alice buys one repair patch from a provider whose worker key is W. Her task
frame pins a captured repository revision, the failing behavior, allowed paths,
and an instruction set. Her admitted target consumes that exact input artifact
and returns a deliverable whose pinned schema requires base snapshot, patch
bytes, and changed-path manifest. Opening a pull request or merging remains
outside the request unless separately admitted.

The market price is 25,000 msat with fixed delivery/review/dispute/payment
deadlines. The labor terms name a reviewer, an independent resolver, the exact
checker/lock, criteria `regression` and `scope`, and one rework opportunity.
Worker W returns submission zero. The reviewer finds regression passed and
scope failed. Alice requests correction of the same scope criterion; W uses
remaining reservations, records attempt two, and delivers submission one before
rework_due_at. Both original and replacement outputs and costs remain retained.

The pinned reviewer checks submission one and records both criteria passed.
Alice accepts those exact bytes, producing a buyer-authored acceptance for
25,000 msat. MKT validates that artifact before issuing its bounded payment
instruction. An invoice timeout produces unknown payment until reconciled;
it does not trigger a second payment. A later repository change may prevent
integration without invalidating the fact that Alice accepted the original
deliverable. If Alice disappears before review, the resolver uses the same
terms and evidence; it does not invent a pass or an automatic payment.

## Required conformance cases

Implementations must retain input and expected-result fixtures for:

1. Free and paid confirmed orders; unsupported payment/profile terms; missing
   immutable input, lock, or source bytes; unknown semantic fields.
2. Wrong buyer/provider/worker/reviewer/resolver signer; forwarded unsigned
   claims; undisclosed known common operator; task-frame owner mismatch.
3. Quote or listing mutation after agreement; request/order digest cycles;
   mismatched CJ target/input/context; attempts forked across workers; replay
   after a lost acknowledgment; dispatch without durable order binding.
4. Missing/oversized deliverables; output schema mismatch; same-number
   substitution; incomplete private evidence; manufactured delivery timestamps;
   conflicting buyer/resolver receipts; late initial delivery and late rework.
5. Worker-modified checker, tests, or labels; unpinned model judge; missing
   criteria; unverifiable checks; model or signature treated as quality proof.
6. Passed verification without acceptance; rejection without dispute handling;
   added requirements in rework; exhausted budget; excess submissions; unavailable
   buyer, reviewer, and resolver; unknown incorrectly converted to zero debt;
   rework context omitting mandatory entries or introducing unapproved sources.
7. Cancellation before delivery, during unknown execution, after delivery, and
   after acceptance; confirmed and unconfirmed stop; partial output; final-record
   conflicts; no automatic cancellation from deletion, expiry, or Pause.
8. Exact full/zero/null amount derivation; fee accounting; acceptance substitution;
   duplicate or ambiguous payment; private wallet material in execution context.
9. Relay/worker/client restart; retained original CJ events; expired retention;
   unavailable dispute evidence; multiple hosts double-reserving capacity.
10. Private task leakage through tags, sources, or public EVAL/KB; unauthorized
    training/redistribution; provider secrets exposed to buyer code; separately
    authorized integration failing without rewriting accepted delivery.

Publish only the roles actually implemented and verified. A signed offering,
an encrypted relay round trip, a passing patch, or a simulated wallet alone
does not establish end-to-end labor conformance.
