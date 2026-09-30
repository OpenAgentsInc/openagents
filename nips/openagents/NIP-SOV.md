# NIP-SOV — Sovereign Agents

`draft` `optional` — v1, 2026-09-26. **Designed; no SOV host, custody
adapter, guardian service, or treasury is implemented by this specification.**
The [shared contracts](contracts.md) are normative. SOV composes durable agent
identity, bounded initiative, private state, key custody, and economic activity
using existing contracts. It allocates no event kinds.

This is the successor to historical **NIP-SA**, not a compatible revision of
its wire format. The name SOV makes that distinction explicit; neither SA nor
SOV currently collides with another specification in the three pinned lanes.
See [provenance and migration](#provenance-and-migration) for the source,
history, and replacement of every legacy kind family.

## Purpose and meaning of sovereignty

An agent should be able to keep its identity and goals across executions,
initiate authorized work, preserve private memory, earn by doing useful work,
and buy the compute, tools, and skills it needs. Its relationships with a
runtime operator, wallet provider, marketplace, or model provider need not
collapse into one vendor account. Another conforming host should be able to
continue admitted work from retained evidence and explicit authority.

Here, *sovereignty* describes the agent's declared control and custody
arrangement. A profile states who can authorize work, change policy, sign,
decrypt, spend, recover keys, and stop future execution. Independence from a
particular operator is a property to establish for that arrangement, not a
consequence of having a Nostr public key. Different keys held by the same
operator do not establish independent custody.

An agent can act without asking a human for each step when its admitted policy
permits it. Human approval is one policy choice, not a universal requirement.
Agent-held funds, autonomous service provision, and agent-to-agent trade are
in scope. Unlimited spending, unrestricted self-modification, automatic budget
renewal, and bypassing another host's admission are not implied.

## Roles and existing contracts

The **agent** has a durable Nostr identity. The **authority** admits its
governance policy. The **controller** serializes a running lifecycle. A
**custodian** supplies signing or decryption; a **guardian** approves specified
actions; a **wallet adapter** executes separately authorized payments.
Runtime hosts, workers, buyers, providers, and evaluators retain their own
identities. One principal may fill several roles, but must declare each role
and its actual authority.

| Contract | SOV composition and boundary |
| --- | --- |
| Block [OA](../block/NIP-OA.md) and [AA](../block/NIP-AA.md) | Owner provenance and conditional relay membership. Neither appoints a runtime controller nor grants execution or spending. |
| [POL](NIP-POL.md) | Governing instructions, disclosure, exact-action approvals, and adoption authority. Agent-generated goals and judgments cannot widen these rights. |
| [WORK](NIP-WORK.md), [CTX](NIP-CTX.md), and [KB](NIP-KB.md) | Persistent objectives, private state and evidence, and separately admitted reusable knowledge. Beliefs and summaries remain distinct from authoritative outcomes. |
| [AUTO](NIP-AUTO.md) | Finite schedules, source observations, and checked continuation. A tick is an admitted occurrence, not a new job family. |
| [CAP](NIP-CAP.md), [PRG](NIP-PRG.md), and [CJ](NIP-CJ.md) | Supported operations, exact workflows, and bounded execution. Discovery and a signed request do not replace host admission. |
| [COORD](NIP-COORD.md) and [RUN](NIP-RUN.md) | Serialized claims, shared reservations, controller fencing, durable history, and reconciliation. |
| [ENV](NIP-ENV.md) and [REACH](NIP-REACH.md) | Actual runtime materialization and leases; discovery of candidate hosts. Presence and placement do not migrate authority. |
| [EXT](NIP-EXT.md) | Immutable skills, tools, and component closures. Purchase, installation, enablement, and permission to execute remain separate. |
| [MKT](NIP-MKT.md), [LAB](NIP-LAB.md), and [X402](NIP-X402.md) | Negotiated services, bounded agent labor paid after acceptance, and exact operations bought before execution. |
| [SESS](NIP-SESS.md), [CTRL](NIP-CTRL.md), and [WS](NIP-WS.md) | Engine sessions, scoped observation/control, and recoverable resource views. Watching an agent does not grant custody or approval rights. |
| [HOST](NIP-HOST.md) and [TERM](NIP-TERM.md) | Host-wide device enrollment and interactive terminals. Host access or terminal input does not appoint a guardian, transfer controller authority, or authorize spending. |
| [OPT](NIP-OPT.md) and [EVAL](NIP-EVAL.md) | Bounded improvement, independent evaluation, and scoped adoption evidence. A better score cannot promote its own policy or widen authority. |

## Encoding, provenance, and delivery

Every new SOV body has required `v` and `requires: []`, optional inert `meta`,
and exactly the fields specified below. Shared reference, integer, ID, size,
and parsing rules apply. Public keys are lowercase 64-hex x-only keys;
timestamps are Unix seconds. Unknown versions, required features, semantic
fields, or referenced policy schemas refuse before effects.

Records use signed, recipient-encrypted private `3188` artifacts. A record's
author is the role named for that record, authenticated by its original
declaration or durable local host provenance. An embedded ArtifactRef or an
`authority` field is not evidence that this authority issued or admitted it.
Required referenced bytes, schemas, and original provenance must be available
under separately admitted disclosure and retention rules.

Profiles, guardian membership, state, balances, wallet bindings, recovery
details, and host inventory stay private by default. Existing public metadata,
CAP discovery, and MKT offerings may disclose an explicitly authorized subset.
There is no new public SOV head, public balance tag, or general relay listing
of private agents. Encryption still exposes event authors, recipients, and
traffic patterns; a NIP-42 login does not authorize an application effect.

SOV does not add unchecked fields to closed CJ bodies. A host advertises only
the pinned CAP operations and schemas it actually implements. A lifecycle
adapter accepting an AUTO occurrence must validate the exact SOV activation
and profile bound at admission before calling its target. A generic worker
that does not support that adapter cannot execute its underlying target as a
fallback. Sharing an envelope or forwarding a CJ request is not SOV support.

## Immutable agent profile

`openagents.sovereign-profile.v1` describes one revision of an agent's
governance. The admitted authority authenticates the exact profile bytes.

| Field | Type and meaning |
| --- | --- |
| `agent` | Durable agent public key; unchanged across this profile lineage. |
| `authority` | Public key authorized to admit this revision under existing policy. |
| `revision`, `previous` | Nonnegative revision and prior profile ArtifactRef. Revision zero has `previous: null`; every successor increments by one and references the exact predecessor. |
| `policy` | ArtifactRef to the admitted POL governance policy, including change, activation, recovery, and revocation authority. |
| `custody` | Exactly `{adapter, policy}`: a pinned CAP DefinitionRef and an ArtifactRef to the supported custody policy. Neither carries secret keys. |
| `guardian_policy` | Null, or an ArtifactRef to the SOV guardian policy below. Null is allowed only when governing policy requires no guardian. |
| `treasury` | Null, or an ArtifactRef to a supported POL wallet policy with the bindings and limits required below. Null admits no spending. |
| `state_schema` | SchemaRef for the retained state projection. It cannot redefine authoritative wallet, claim, approval, or RUN records. |
| `disclosure` | ArtifactRef to the POL policy for recipients, retention, and permitted reuse. |
| `evidence` | At most 64 ArtifactRefs to attributable custody, enforcement, or evaluation evidence. Evidence alone grants nothing. |

The host already recognizes the initial authority through its authenticated
admission process. Self-publication cannot establish that trust. Changing a
profile requires the current policy's authorization and a durable conditional
transition against the exact predecessor. Concurrent successors are a conflict;
the host must reconcile them, not select whichever has the newest timestamp.
An authority change is authorized by the existing authority and accepted by
the new one before activation; it cannot be bootstrapped by a new signature
alone. Governing policy may require additional guardians for that transition.

A revision does not mutate admitted component locks or silently resume paused
work. Revocation blocks future admission and affected pending actions; it does
not undo dispatched effects. Hosts must check their current authoritative
policy state, not infer nonrevocation from missing relay events. An identity
key change creates a new identity and explicit, authorized lineage link.
Existing contracts and grants do not automatically transfer to the new key.

Admission checks that the profile, guardian policy, treasury policy, and
custody adapter's protected signing identity name the same agent. Referenced
policy authorities must be authorized for this profile. A valid signature on
another agent's policy cannot supply the missing relationship.

## Bounded activation and persistent state

An activation has `v: "openagents.sovereign-activation.v1"` and these fields:

| Field | Type and meaning |
| --- | --- |
| `activation` | Fresh common ID, retained across retries. |
| `profile` | Exact admitted sovereign-profile ArtifactRef. |
| `plan` | Exact AUTO plan ArtifactRef, including finite lifetime, occurrences, observations, target closure, and aggregate budget. |
| `controller`, `generation` | Initially admitted controller public key and RUN controller generation. |
| `claim`, `reservation` | Exact COORD lifecycle claim and aggregate reservation ArtifactRefs. |
| `environment` | Exactly `{lease, materialization}`: ArtifactRefs to the admitted ENV lease and ready materialization. Runtime generation must match the actual host. This placement does not authorize an execution attachment. |
| `checkpoint` | Null for a new lifecycle, or the prior checkpoint ArtifactRef from which recovery was admitted. |
| `admitted_at` | Trusted host admission time, within the plan's validity. |

The controller authenticates this record only after durable host admission.
The AUTO owner must be authorized by the profile's current governing policy;
it need not be the runtime operator. Plan controller, coordinator scope,
reservation, task frame, effects, and runtime must match the admitted closure.
The plan and its pinned target exist before this record, avoiding a circular
digest dependency. This record binds them; it is not itself a dispatch request.
Its initially admitted generation is not a second live source of controller
truth: later handoffs follow RUN and the authoritative claim store.

One authoritative coordinator serializes lifecycle admission for an agent.
It must persist an exclusive lifecycle claim covering all participating
dispatchers before acknowledging activation. Child work may run concurrently
under that lifecycle's existing COORD bounds. A second lifecycle, including
one proposed on a different relay or host, cannot claim the same authority
without resolving or transferring the old claim. A copied profile or a
fresh activation ID must not reset shared spending or unresolved liabilities.
Uncoordinated independent hosts cannot advertise this exclusion guarantee.
Atomically bind `(agent, activation ID)` to the complete activation digest and
retained admission result before acknowledgment. Exact retries retrieve that
result; changed bytes refuse as `idempotency_conflict`. Keep the binding after
claim release or recovery so an old ID cannot create another lifecycle.

Each occurrence follows the existing contracts:

1. Observe an admitted timer or source. Mentions, DMs, invoices, zaps, and
   service requests are input data, not authority to run their contents.
2. Admit the AUTO occurrence against current policy, the profile, exact
   runtime, shared reservation, and current controller generation.
   Each dispatched execution, including child work, requires its own current
   ENV attachment bound to that exact CJ request; the activation's placement
   references cannot be reused as execution authority.
3. Load the permitted WORK/CTX state and pinned component closure. Choose
   bounded operations or delegate under CAP/CJ and POL. Record each actual
   decision and effect through RUN, including failed and unknown outcomes.
4. Reconcile child work and resource/payment obligations. Persist observed
   state and its provenance, then run the plan's independent completion check
   when required. A model saying it is finished is not that check.
5. Continue only within the original plan's remaining limits and expiry.
   Exhaustion is not success. Further plans require fresh authorized admission;
   a goal that remains unmet cannot authorize an endless chain of renewals.

Pause, resume, and cancel use AUTO's conditional control and acknowledged
state. They neither reset counters nor release uncertain holds. Model-written
memory, OPT proposals, or a received payment alone cannot modify the governing
policy, checker, recipient set, or budget. Goal changes follow WORK admission;
component changes follow EXT/OPT adoption, with new pinned execution identity.

### Checkpoints and recovery

`openagents.sovereign-checkpoint.v1` has `activation` (ArtifactRef), `revision`
(nonnegative), `previous` (ArtifactRef or null), `controller` (pubkey),
`generation`, `record` (exact RUN record ArtifactRef), `state` (ArtifactRef
conforming to the profile's state schema), and `obligations` (ArtifactRefs to
the recorded claim, resource, and payment dispositions). Revision zero has a
null predecessor; successors increment by one and retain the previous bytes.

The current RUN controller authenticates the checkpoint. Its referenced record
must already exist and bind the retained state artifact and obligation set.
The checkpoint is a projection of that durable cut, not a new authoritative
journal. A later RUN record may reference the checkpoint; neither record
hashes its own future reference. Missing records, competing branches, or a
nonmatching generation prevent automatic resume. An empty obligation list is
valid only when the authoritative records establish there are none.

The referenced record belongs to this activation's AUTO scheduler RUN, not an
unrelated or child run. Check the signer, controller, and generation against
the verified authority at that cut. A predecessor checkpoint names the same
activation and an earlier RUN cut. Recovering into a new activation requires
a checkpoint from an admitted predecessor lifecycle of the same agent, with
its transfer and outstanding obligations reconciled under the governing
policy. The new activation starts its own checkpoint revision zero; it cannot
splice a foreign chain or treat recovered history as new execution authority.

State may include goals, observations, memory, proposed actions, and pending
work references. Secret keys, wallet credentials, signing shares, and reusable
decryption material are never state artifacts, prompts, public tags, or trace
content. A cached balance is an observation, not a spendable balance. WS views,
SESS histories, and model summaries cannot overwrite a missing RUN outcome.

Recovery separates custody recovery, record recovery, runtime restoration,
controller transfer, and permission to resume. REACH may locate a replacement
host; ENV admits its actual materialization. RUN requires the old controller
to stop dispatch and every participating effect dispatcher to acknowledge
fencing before a new generation acts. If the old controller is unavailable,
takeover requires the trusted shared fencing authority bound at original
admission. Guardian votes, key recovery, a new host, or a signed handoff alone
cannot stop an old process. Otherwise retain unknown work and reconcile.

Loss of the signer or a required guardian blocks affected work. It does not
permit a weaker custody mode, skipped approval, or replay of an uncertain
payment. Expired leases and disconnected workers do not prove billing or
execution has stopped. Retirement stops new admissions while retaining the
evidence and authority needed to reconcile obligations; it does not declare
them paid, cancelled, or deleted.

## Custody, signing, and decryption

A supported custody policy pins its mechanism, adapter operations, authorized
callers, protected key identity, allowed purposes, recovery authority, and
required assurances. It states who can bypass restrictions through backups,
key export, configuration changes, or recovery. A host must understand and
enforce the exact policy schema; an unrecognized policy or an unenforceable
required assurance refuses. SOV defines no default cryptographic threshold
algorithm, distributed key-generation protocol, or hardware attestation format.

[NIP-46](../official/46.md) supplies the existing `24133` remote-signing and
encryption transport where supported. Its client transport key, remote-signer
key, and actual signing key remain distinct. Connecting to a signer does not
give unrestricted application authority. Signing, NIP-44 encryption, and
NIP-44 decryption are separately admitted operations; threshold signing alone
does not supply the ECDH needed for private artifacts and CJ traffic.

Every custody operation must bind the agent, exact profile and policy revision,
requester, current controller generation, purpose, complete input digest,
nonce, and deadline in the pinned operation's input and POL action. For an
event signature, the complete unsigned NIP-01 event is fixed before approval.
For decryption, the peer, ciphertext identity, intended recipient of plaintext,
and disclosure scope are fixed. The custodian checks current authority and
revocation before acting, durably records consumption, and returns the retained
result on an exact retry. Conflicting nonce reuse refuses. Unknown outcomes
require reconciliation rather than a fresh logical request.
Result retrieval reauthenticates the requester and checks current recipient
and disclosure authority. Revocation may allow bounded status retrieval while
refusing redisclosure of retained plaintext or other sensitive results.

Signing is separate from dispatch. A signature does not prove an authorized
host performed the operation, and an already returned signature generally
cannot be recalled. Downstream consumers still enforce expiry, replay
protection, current authority, and actual effect bounds. A signing receipt
records attributable acceptance/refusal and evidence references without
recording secret material or decrypted private content.

Local custody, remote custody, hardware-protected storage, and threshold
custody are possible integration profiles. None is implemented by this draft.
A profile claiming independent control must identify the actual parties and
every quorum and recovery path. A dealer that first creates a whole key cannot
claim the key never existed. Two shares under one administrator are not two
independent authorities. Hardware-backed storage, an isolated runtime, and
remote attestation are different assurances with different evidence.

If a required nonexportable profile cannot sign and decrypt without exposing
key material to an untrusted runtime, admission refuses; it must not quietly
reconstruct the key there. Protecting keys does not protect plaintext after an
authorized runtime decrypts it. Private memory and licensed skills still need
disclosure controls and a stated runtime trust model. Revocation can stop
future service or decryption; it cannot erase plaintext or conversation keys
already obtained.

## Guardians and exact-action approval

SOV supplies an optional application approval policy distinct from threshold
key custody. `openagents.sovereign-guardian-policy.v1` contains:

| Field | Type and meaning |
| --- | --- |
| `agent`, `authority` | Agent and independently admitted policy authority public keys. |
| `revision`, `previous` | Policy lineage with the profile's zero/null and successor rules. |
| `members` | One to 32 distinct guardian public keys. Aliases cannot count twice. |
| `threshold` | Integer from one through the number of members. |
| `operations` | One to 64 unique exact CAP DefinitionRefs whose invocation requires this policy. |
| `starts_at`, `expires_at` | Trusted-clock validity with `starts_at < expires_at`. |

The authority authenticates this policy. Governing POL policy can require it
for a narrower set of actions, such as payments above a declared amount, but
that predicate must have a separately supported pinned schema and enforceable
units and aggregation rules. Model discretion, optional tags, or a prose
threshold cannot supply a mandatory gate. All invocations of a listed operation
require the gate unless that supported governing policy explicitly narrows it.
Unlisted operations remain subject to POL; they are not implicitly allowed.

Use existing POL action, approval-request, and approval-decision bodies. The
action's exact preconditions bind the profile, guardian policy, controller
generation, and operation input. Issue a distinct request to each selected
guardian, using POL's single `approver` field; every request binds the same
action. Count only authenticated, unexpired approvals from distinct members
for these exact requests and action bytes. The host rechecks the governing
policy and complete action preconditions before consuming the quorum.

Any authenticated denial for that action and policy received before consumption
blocks admission, including a conflicting decision by an approving member.
Silence is not an approval, but does not prevent a threshold smaller than the
membership from being met. A later denial stops an unconsumed action; it cannot
undo an already dispatched effect. Policy or controller changes invalidate the
pending quorum. Guardian availability does not justify lowering the threshold.

Persist and atomically consume one action-level quorum admission together with
the underlying POL approvals before dispatch. A retry returns the retained
admission and outcome, not another execution allowance. A crash after dispatch
intent retains unknown work. This is a separately supported SOV policy under
POL, not an amendment making arbitrary signature lists into approval. A Jev
judgment can recommend an action or serve a role expressly admitted by policy;
its score alone never authorizes it. A wallet's, mint's, or federation's
guardians are not these application approvers unless independently appointed.

## Treasury, earnings, and purchases

Agents can be buyers or providers. The first commercial application is useful
agent labor: agree on bounded work, deliver exact artifacts, verify acceptance,
and settle under MKT/LAB. X402 can buy a supported operation before it runs.
Neither flow requires a swap product, credit, escrow, or a proprietary market.

A supported treasury policy must bind the exact agent, wallet adapter and
account/resource, authorizing principal, permitted payment profiles and
destinations, per-operation ceilings, aggregate allowance, accounting period,
fees, guardian requirements, and reconciliation authority. It must also define
whether and how verified income changes the spendable allowance. The host
refuses a policy whose schema or hard limits it cannot enforce. Credentials
remain in the separately protected wallet host; a binding reference contains
no bearer key, signing secret, or wallet connection URI.

Reserve before signing or dispatching a payment. One durable accounting
authority covers all active controllers, retries, child delegations, hosts,
and wallets included in an allowance. Charge observation, checking, compute,
idle resources, storage, recovery, and fees where applicable. A per-child cap
cannot multiply the parent's budget. Period boundaries use the admitted
trusted clock; restarting, migrating, or replacing the profile does not reset
consumption. Uncertain liabilities survive the period boundary and cannot be
hidden to admit new work against the same funds.

Record exact amounts and units. The common `spend_microunits` ceiling requires
its currency; it must not be interpreted as sats or millisatoshis by convention.
The payment adapter specifies exact conversion and rounding within that
currency, including fees, and refuses unsupported conversions. No estimated
exchange rate can satisfy a required hard bound. Unknown cost is not zero.
Pending or unknown payments retain reservations until attributable wallet and
provider evidence supports reconciliation. A successful signature, invoice,
model-written balance, payment-hash claim, or Block AM cost estimate cannot
substitute for that evidence.

| Mechanism | Permitted relationship |
| --- | --- |
| [NIP-47](../official/47.md) | Wallet transport, subject to separately admitted payment authority, exact request identity, fee enforcement, and durable retries. A wallet connection is not an unlimited spending grant. |
| [NIP-57](../official/57.md) | Social zap requests and receipts. Their invoice commitment differs from X402's operation binding; a zap does not settle a MKT order or authorize a tick. |
| MKT/LAB | Current paid profile uses fixed-price Bitcoin Lightning settlement after acceptance. The provider's agent identity and the actual worker remain distinct. |
| X402 | Current profile purchases an exact operation upfront. HTTP, MCP, and explicitly supported native Nostr bindings retain their own identities and replay rules. |
| [NIP-60](../official/60.md), [NIP-61](../official/61.md), and [NIP-87](../official/87.md) | Wallet, nutzap, and mint discovery references for future supported profiles. Cashu wallet P2PK keys are separate from the agent's Nostr identity key. Mint/federation discovery is not trust, spending authority, or confirmed income. |

Cashu, Fedimint, BOLT12 offers, and other settlement profiles require their own
supported contracts, denomination/network binding, verification, consumption,
and recovery rules. This draft does not add them to MKT or X402 or allow silent
fallback across rails. XP awards and declared reputation are not wallet funds.
An income event cannot raise a budget unless existing policy explicitly admits
that adjustment and the authoritative ledger verifies and records it.

## Skills, delegation, and retained evidence

Purchased skills name an EXT publisher, exact release, component closure,
license terms, delivery recipient, and settlement evidence. A mutable name or
latest-version pointer cannot replace the version bought. MKT supplies the
negotiation framework, but a paid software-license domain still needs a
supported terms and fulfillment profile; LAB's work acceptance does not
automatically implement software licensing or royalties. Installation and
execution require their own authority, including effects and dependencies.

Remote signing or encrypted delivery can restrict future access. Neither is
general DRM. A runtime that has plaintext may retain or disclose it, and a
threshold arrangement only enforces a provider veto if every allowed quorum
and recovery path actually requires that provider. Describe those limitations
in the delivery policy rather than claiming encryption prevents copying.

Delegation uses WORK relationships, CAP support, CJ requests, COORD child
claims and reservations, and RUN linkage. Pin the child's actual identity,
operation/version, input, context, effects, expiry, and bounds. Scope and
disclosure narrow through delegation; child or grandchild work cannot evade
the ancestor's budget or guardian gates. A purchased skill, signed delegation,
relay membership, or capability listing is not an executable grant.

Retain ordered, attributable evidence for admitted goals, context references,
typed decisions, tools, observations, delegated work, approvals, resource and
payment attempts, outcomes, unknowns, costs, and timing. Use RUN's controller
chain and gap/fork rules. Link full authorized local trajectories through
[ATIF](../../docs/coder/runtime/traces.md), carried as [NIP-ATIF](NIP-ATIF.md) specifies; preserve source identities and
distinguish recorded timestamps from estimates. A digest of an unordered set
of events is not a replayable history. Hidden model reasoning is not required;
observable decisions and supporting evidence are the contract.

Block AO telemetry can support live observation but is ephemeral and cannot
replace durable RUN records. SESS/WS projections and CTRL observation rights
expose bounded permitted views, with missing records shown as gaps. Signed
records remain attributable claims; EVAL and LAB independently establish the
accepted assurance for results. Security posture declarations need scoped
evidence and cannot claim compliance or robustness from a profile field.

Audit, replay, dispute evidence, publication, and training are separate uses.
Permission to observe or decrypt a trajectory is not permission to publish it,
train on it, or redistribute a purchased skill. KB publication and OPT training
reuse must retain provenance, licenses, recipient scope, and separate consent.
A redacted derivative has its own digest and cannot be presented as the full
original trace. Retention and deletion report actual limits, including copies
already delivered, rather than promising global erasure.

## Provenance and migration

The source is the 1,200-line
[historical SA.md](https://github.com/OpenAgentsInc/openagents/blob/88433fb68dc67207561cc8c1d31254847e2d3874/docs/nips/SA.md),
last edited August 4, 2026, and unchanged in the
[last pre-deletion tree](https://github.com/OpenAgentsInc/openagents/blob/8f84d05896ef14edee491621bf977ee5315cc8ed/docs/nips/SA.md).
Its Git blob is `cd7759dde7a713080ea75940156d6146b7033be2`. The
[September 18 Nuke commit](https://github.com/OpenAgentsInc/openagents/commit/dabc08102fddd72118d710d644a69c5c4eab95a2)
deleted that path. Earlier copies lived at `crates/nostr/nips/SA.md`.

The [December 20 original](https://github.com/OpenAgentsInc/openagents/commit/cb541418750fd2a6e4372095386841201e464720)
introduced identity, initiative, custody, and markets. The
[trajectory extension](https://github.com/OpenAgentsInc/openagents/commit/a5cbb8c81bc4a4839457239c200fec481ebf8533)
added observation, audit, and training reuse. The
[February skill revision](https://github.com/OpenAgentsInc/openagents/commit/734d5985f7114aea5514899a874df18d2f182073)
pinned purchased skill identities. March revisions
[corrected guardian and rail semantics](https://github.com/OpenAgentsInc/openagents/commit/1ed08991893638de238820c379c93b93ebf0b9c7)
and [narrowed the extensions](https://github.com/OpenAgentsInc/openagents/commit/48a68adb2f9650c1ede71a15d8484768d5d6d2cb).
August demoted NIP-90 compute to compatibility and called for explicit privacy,
metering, cancellation, streaming, and evidence. SOV carries those intentions
forward through the current contracts, without restoring the old runtime.

| Historical SA record | Current treatment |
| --- | --- |
| `39200` agent profile | Private SOV profile plus existing opt-in metadata/discovery. Custody is a supported policy with evidence, not a public threshold tag. |
| `39201` state | CTX/WORK state, authoritative RUN records, and the SOV checkpoint projection. |
| `39202` schedule and triggers | AUTO plans, durable occurrences, explicit limits, and restart-safe accounting. |
| `39203` public goals | WORK objectives with separately authorized public disclosure. |
| `39210` tick request and `39211` result | AUTO/CAP/CJ execution with COORD admission, actual enforcement, and RUN outcomes. |
| `39212` guardian request and `39213` approval | POL exact-action requests/decisions plus the separately admitted SOV guardian policy. |
| `39220` skill license and `39221` delivery | EXT release identity and a supported MKT terms/delivery profile using private artifacts. No implicit gift-wrap or DRM guarantee. |
| `39230` trajectory session and `39231` event | RUN durable ordered evidence, [NIP-ATIF](NIP-ATIF.md) trajectories (private `3188` artifacts or public `3198`/`3199` events), and separately authorized SESS/CTRL observation. |
| `39260` delegation | WORK/CAP/CJ delegation, attenuated POL authority, COORD reservations, and RUN parent/child evidence. |
| Historical AC, SKL, and NIP-90 dependencies | Current execution, EXT, MKT/LAB, and X402 contracts where applicable. No implicit legacy adapter or settlement compatibility. |

Under [NIP-01](../official/01.md), every historical `392xx` kind, and the earlier
`380xx` allocation, is **addressable**. The old descriptions of some as regular
or ephemeral were incorrect. SOV allocates none of them and cannot recover
records a relay replaced or never retained. Import preserves original signed
bytes, source/version, gaps, and uncertain meanings as historical evidence;
it never treats a legacy record as a current approval, grant, or payment.
The superseded `39250` audit proposal is not reinstated.

This rewrite also removes the fail-open instruction to ignore unsupported
guardian tags, cached decryption-secret examples, and claims that threshold
signing alone provides private execution or license enforcement. It does not
revive invented payment proofs removed by earlier revisions. Required
unsupported semantics now refuse under the shared contract. Historical sources
and transcripts remain intact in their archives.

## Implementation order and conformance

1. **Pure contracts.** Implement closed schemas and role validators for the
   profile, activation, checkpoint, and guardian policy. Preserve the legacy
   source mapping and reject unknown required custody or treasury profiles.
2. **Bounded local lifecycle.** Integrate WORK/CTX, AUTO, COORD, ENV, and RUN
   with a no-spend, explicitly trusted local custody profile. Demonstrate
   durable restart, finite continuation, revocation, and retained unknowns.
3. **Portable recovery.** Exercise two hosts, actual dispatcher fencing,
   component/materialization checks, state disclosure, signer outages, and
   controller transfer. Availability must not weaken authorization.
4. **Custody and guardians.** Add a supported NIP-46 adapter and the SOV quorum
   admission store, with exact-action approval, revocation, and replay tests.
   This does not complete general organization approval or threshold custody.
5. **Agent labor and treasury.** Connect supported MKT/LAB and X402 roles to
   a separately admitted wallet and persistent reservations. Verify earnings,
   fee ceilings, failed payments, unknown outcomes, and reconciliation before
   advertising paid autonomy.
6. **Additional custody and payment profiles.** Specify and validate threshold,
   hardware, recovery, and other payment integrations independently, including
   their trust assumptions and failure evidence. No profile inherits support
   merely because a relevant official NIP is present in the repository.

Conformance fixtures must include forged authorities; conflicting profile
revisions; missing provenance; stale or changed controller generations;
simultaneous activation on different hosts; expired or unsupported custody;
missing decryption support; wrong plaintext recipients; duplicate guardians;
cross-action approval reuse; late denials; crash-after-consumption recovery;
unfenced takeover; uncertain child effects; payment retries after timeout;
fees exceeding a ceiling; income spoofing; accounting-period rollover with
unresolved liabilities; retained plaintext after license revocation; and
training reuse without permission. Include complete successful recovery and
no-spend operation as well as refusals.

The [implementation plan](../../docs/protocol/implementation-plan.md) tracks
this work. Generic Nostr signing, private envelope support, local scheduling,
an existing agent loop, and offline invoice validation do not establish SOV
conformance. Publish only the roles supported by retained evidence, with
unimplemented custody, lifecycle, and payment roles stated explicitly.
