# Proposed OpenAgents NIPs: All Work on Nostr

Status: proposal index. These NIPs are not yet drafted, implemented, or
advertised by any relay. Each entry becomes real only when its own spec file
lands in this directory with fixtures and an implementation decision. This
index is a protocol program, not a product availability claim.

## Philosophy

Block's Buzz NIPs encode a Nostr-native Slack: workspace chat where the relay
is the workspace, agents are first-class members with owner-attested keys
(NIP-OA/NIP-AA), private memory (NIP-AE), live telemetry (NIP-AO/NIP-AM),
public personas (NIP-AP), and relay-signed workspace state (NIP-DV, NIP-IA,
NIP-WP).

OpenAgents applies the same move to a different product shape: a Nostr-native
**Linear-class All Work system** — planning, issues, agent delegation, coding
sessions, review, evidence, and accepted outcomes — so that the complete
OpenAgents vision can run with Nostr NIPs as the backend. Where Buzz encodes
"Slack on Nostr," these NIPs encode "Linear plus coding plus All Work on
Nostr."

The source model is the All Work spine (`docs/allwork/model.md`) and the
canonical vocabulary (`docs/omega/GLOSSARY.md`):

> Work is the container. Sessions perform it. Blocks display and control it.
> Hosts run it. Intents request actions. Events record facts. Receipts bind
> facts to evidence.

The Linear adaptation studies
(`docs/teardowns/2026-08-02-linear-agents-openagents-nostr-adaptation.md`,
`docs/teardowns/2026-08-02-linear-api-sdk-all-work-adaptation.md`) supply the
resource grammar: Organization, Team, Initiative, Project, Cycle, Project
Milestone, Issue, Issue Relation, Workflow State, Label, Document, Decision,
Customer Need, Assignee/Delegate, Agent Session, Agent Activity, Notification.
The landed `openagents.signed-workroom.v2` profile
(`docs/omega/2026-08-03-signed-workroom-projection.md`) already pins the first
implemented kind range (32150-32163).

## Authority model

Every NIP in this program obeys the same laws, adapted from the current
INVARIANTS and the signed-workroom implementation:

1. **A signature proves signer and bytes.** It never proves OpenAgents
   admission, execution, verification, acceptance, release, or settlement.
2. **Client-signed events are proposals.** A typed Intent event from a
   participant key is a request. It has product meaning only after an
   authority-signed admission event exists.
3. **The authority is itself a Nostr identity.** In relay-backed deployment,
   the admitting service (the All Work authority) holds its own keypair and
   publishes admission receipts, canonical Work Events, and state heads as
   authority-signed events — the same pattern Immortal uses for relay-signed
   snapshots in NIP-DV and NIP-IA. Clients reconstruct canonical state by
   trusting the authority key, not the relay.
4. **Relay acceptance is transport evidence.** `persistedBeforePublish: true`,
   `relayAcceptanceIsAuthority: false` — the receipt semantics already fixed
   by the signed-workroom contract hold for every kind here.
5. **Ref-only privacy.** Public events carry opaque refs, digests, state, and
   causal structure. Raw prompts, private bodies, credentials, local paths,
   customer identity, wallet material, and provider payloads never enter a
   public event. Private payloads use NIP-44/NIP-59 with explicit audience, or
   stay off-relay behind a digest.
6. **Projections cannot self-amplify.** An Issue projection, planning link,
   membership row, or grant display never creates the authority it describes.

## Composition with existing lanes

These NIPs compose with, and do not duplicate:

- **Official NIPs:** NIP-01/09/10 (events, deletion, threading), NIP-17/44/59
  (private messages), NIP-22 (comments), NIP-29 (relay groups projecting
  Workrooms), NIP-32 (labels/attestations), NIP-34 (git), NIP-40 (expiry),
  NIP-42 (auth), NIP-46 (remote signing), NIP-51 (lists), NIP-65 (relay
  lists), NIP-78 (app data), NIP-89 (handlers), NIP-90 (DVM), NIP-94/92 and
  Blossom (files), NIP-98 (HTTP auth).
- **Block NIPs:** NIP-OA/AA (agent ownership and relay admission), NIP-AE
  (agent memory), NIP-AO/AM (live telemetry and turn metrics), NIP-AP
  (personas), NIP-RS (read state), NIP-ER (reminders), NIP-PL (push leases).
- **Existing OpenAgents NIPs:** NIP-SKL (skills), NIP-SA (sovereign agents),
  NIP-AC (credit), NIP-LBR (labor market), NIP-DS (datasets), NIP-TRN
  (training).

The existing six OpenAgents NIPs are the open-market layer (selling skills,
labor, data, credit, training coordination to strangers). The NIPs proposed
here are the workspace layer: how one Organization plans, delegates,
executes, reviews, and accepts its own Work. The two layers meet at NIP-LBR
(external labor fulfills internal Work), NIP-AC (credit funds Work), and
NIP-SKL (skills gate what a Delegate may load).

## Kind allocation strategy

- Addressable All Work records use the OpenAgents-owned `32100-32449`
  neighborhood. `32150-32163` is already pinned by
  `openagents.signed-workroom.v2` and must not be reused.
- Agent-economics records that extend the NIP-SA neighborhood use `39xxx`
  beside the existing 39200-39260 (SA), 39240-39246 (AC), and 39500-39530
  (TRN) reservations.
- Append-only streams either use unique-`d` addressable events (the
  signed-workroom precedent) or a regular-kind block chosen at drafting time.
- Tentative kinds below are placeholders. Each NIP fixes its exact kinds when
  drafted, after a collision review against the official registry and the
  Block lane.

## Proposed NIPs

### Layer 0 — Foundation

Layer 0 is drafted. The four spec files live beside this index.

| NIP | Name | Tentative kinds | Draft |
| --- | --- | --- | --- |
| NIP-WK | Work | 32170-32179 | [`WK.md`](WK.md) |
| NIP-WI | Work Intents and Admission | 32180-32189 | [`WI.md`](WI.md) |
| NIP-EV | Evidence, Verification, and Dispositions | 32190-32199 | [`EV.md`](EV.md) |
| NIP-OT | Organizations and Teams | 32100-32109 | [`OT.md`](OT.md) |

**NIP-WK — Work.** The root object. An addressable Work record binds
objective ref, Work Domain, Work Class, owner ref, state, revision,
participant refs, planning refs, session refs, and outcome refs. Append-only
Work Events (created, classified, related, assigned, delegated, state-moved,
blocked, closed, reopened, superseded, archived) carry sequence, actor,
causal parents, and admission provenance. One Work object can represent a
repository task, CI job, deployment, incident, research task, service
request, or any other bounded outcome — the NIP must not assume software
development is the only domain.

**NIP-WI — Work Intents and Admission.** The command wire. A client-signed
Intent event names actor, target Work, operation, arguments digest, expected
revision, generation, idempotency key, and required authority refs. The
authority answers with an authority-signed admission or refusal event that
binds the intent, the accepted revision, the resulting Work Event refs, and a
typed reason on refusal. Identical idempotent replay returns the original
admission; changed bytes under the same key conflict. This NIP is what makes
"relay as backend" honest: proposals and admissions are both on the wire,
and no client-signed event mutates canonical state by itself. Includes an
intake profile for untrusted candidate Work (the strict-bug-candidate
pattern): externally sourced reports enter as `untrusted`/`pending`
candidates requiring an explicit triage admission before Work exists.

**NIP-EV — Evidence, Verification, and Dispositions.** The trust wire.
Evidence receipts bind produced material (digest, kind, producer, target
criteria) to Work. Verification receipts bind an admitted evaluation
(verifier, criteria, evidence refs, verdict) with producer-verifier
separation — the producing key cannot sign the verification of its own
obligation. Owner Disposition events record the accountable human's separate
accept/reject/waive decision. Receipt-graph edges connect evidence,
verification, disposition, and settlement refs without letting any single
record imply the next rung. This is the wire form of the No-Evidence-No-Claim
rule and the Proof Rung ladder.

**NIP-OT — Organizations and Teams.** Addressable Organization and Team
records with membership projections, role labels, and workflow policy refs.
Composes with NIP-29 (a relay group can project a Team Workroom) and Block
NIP-OA/NIP-AA (agents join through owner attestation). Membership rows are
projections of the membership authority — displaying a member never grants a
capability, and a generation change fences stale membership
(the organization-membership-authority pattern).

### Layer 1 — Planning (the Linear half)

| NIP | Name | Tentative kinds |
| --- | --- | --- |
| NIP-PI | Project Issue | 32200-32209 |
| NIP-WR | Work Relations | 32210-32214 |
| NIP-WS | Workflow States and Labels | 32215-32219 |
| NIP-PG | Planning Graph | 32220-32239 |
| NIP-RP | Release Planning | 32240-32249 |
| NIP-DD | Documents and Decisions | 32250-32259 |
| NIP-CN | Customers and Needs | 32260-32269 |

**NIP-PI — Project Issue.** The Issue projection of Work: team-scoped
identifier, title, description ref, Workflow State ref, priority, label refs,
Assignee ref, Delegate ref, parent/child refs, Project/Cycle/Milestone/
Release Planning refs, estimate, due date, and history cursor. An Issue
shares its Work identity and revision — it is never a second writable
record, and Issue mutation events are NIP-WI intents against the underlying
Work. This is the event shape that lets any Nostr client render a
Linear-class list, board, cycle, or triage view from relay data alone.

**NIP-WR — Work Relations.** Typed edges between Work objects: parent,
child, blocks, blocked-by, duplicate, related, supersedes, split-from,
merged-into. Each relation event names source, target, relation kind, actor,
and admission provenance. Relations never silently mutate either endpoint.
Includes the forensic prior-work relation set (occurrence and root-cause
identity, explicit confidence) so defect lineage is queryable.

**NIP-WS — Workflow States and Labels.** Team- and Organization-scoped
configuration records: ordered Workflow States with type and position, Label
definitions with optional hierarchy, Priority vocabulary, and SLA policy
refs. State labels are policy data — the owning Work policy admits
transitions; a display label is never executable authority.

**NIP-PG — Planning Graph.** The portfolio layer: Initiative, Roadmap,
Project, Project Status, Project Milestone, and Cycle as addressable records
with owner, lead, member, Work refs, dates, and progress projections. Also
carries Project Updates and Initiative Updates as dated authored health
reports, distinct from lifecycle state and from canonical Work Events. Every
layer is optional per Work Domain — incidents and research need not pretend
to be product development.

**NIP-RP — Release Planning.** Release Planning Records, Release Pipelines,
Release Stages, and Release Scope Links as planning data. Hard boundary
inherited from the glossary: a stage named "published" or a pipeline flagged
production cannot create a Release Candidate, pass a Deployment Gate, or
authorize a Release. Actual release authority stays with the release
contract and its gates; this NIP only makes intended-delivery planning
portable and signed.

**NIP-DD — Documents and Decisions.** Versioned Documents (identity,
revision, authorship, body ref or encrypted body, Work/Project links) and
Decision records (question, decision maker, alternatives, rationale, date,
affected Work). Comments reuse NIP-22 anchored to these records. A discussion
message is not a Decision until recorded as one; a Decision is not an
admission.

**NIP-CN — Customers and Needs.** Customer, Customer Need, and Customer
Signal records connecting external stakeholder context to Work. Privacy-first
by construction: public events carry opaque customer refs and typed need
links only; identity, contact, consent, and source material stay encrypted to
the Organization audience or off-relay. Customer identity never creates a
commitment, grant, or Work State.

### Layer 2 — Agents and execution (the coding half)

| NIP | Name | Tentative kinds |
| --- | --- | --- |
| NIP-AD | Assignment and Delegation | 32270-32279 |
| NIP-AS | Agent Sessions | 32280-32289 |
| NIP-AV | Agent Activity | 32290-32299 |
| NIP-RC | Repository Work Claims | 32300-32309 |
| NIP-CC | Code Context and Coding Sessions | 32310-32319 |
| NIP-RV | Reviews | 32320-32329 |
| NIP-WA | Workroom Activity | 32150-32163 (pinned) |

**NIP-AD — Assignment and Delegation.** The structural Assignee/Delegate
split, which is the single most important Linear behavior to port. An
Assignee event binds the accountable human (or admitted organizational role)
to an Issue/Work. A Delegation Grant event binds an Agent Member to bounded
Work under an issuer, allowed actions, tool policy ref, budget ref, privacy
class, validity interval, generation, and revocation semantics. Revocation
fences later commands by generation. An agent is never the accountable human
by implication, and a grant display is not the grant authority. Composes
with NIP-SA delegation (kind 39260) for cross-agent sub-delegation and with
NIP-SKL for what a Delegate may load.

**NIP-AS — Agent Sessions.** The agent-specialized Session: Work ref, thread
ref, run ref, agent ref, initiating actor, Delegation Grant ref, context
manifest ref, plan ref/revision, placement ref, requested and effective
runtime refs, generation, state (pending, active, awaiting-input, complete,
error, stale), activity cursor, artifact refs, provider-result ref,
verification ref, and disposition ref. A session survives client disconnect;
provider "done" is a fact, not an accepted outcome. Live telemetry stays on
Block NIP-AO; durable per-turn metrics stay on Block NIP-AM; this NIP owns
the durable session lifecycle record.

**NIP-AV — Agent Activity.** The typed, safe, append-only activity stream:
progress, plan, elicitation (question/approval request with answer fencing),
action (tool attempt with grant and idempotency refs), artifact (digest-bound
ref), result, verification ref, error (typed, with retry posture), and
disposition. Never raw chain of thought, never raw provider payloads —
bounded summaries, labels, and counts only, matching the Observed Agent
Activity redaction rules. Activities reference their Session and Work and
can name an Effect ref, but a receipt is never inferred from completion
text.

**NIP-RC — Repository Work Claims.** The multi-agent collision ledger as
signed events: Work Packet records (canonical Work, repository, bounded
scope, owned paths, hot files, hot contracts, verification command) and
Repository Work Claim records (holder, claim generation, evidence heartbeat,
state, explicit release). Encodes the admission laws already implemented
natively: refusal on overlapping scope, evidence-bearing heartbeats,
90-minutes-plus-audit takeover, stale-generation fencing, and audit entries
for refused takeovers. A claim is coordination, not an Assignee, Lease,
merge authority, or proof of progress.

**NIP-CC — Code Context and Coding Sessions.** Binds Work to exact code:
repository refs (NIP-34 announcements), pinned commits, worktree identity,
context manifest digests, verification commands, and the effective code
access intersection (owner grant ∩ policy ∩ host permission ∩ work refs).
A Coding Session record specializes NIP-AS with repository, branch, pinned
commit, containment, and PR refs. Git objects and refs remain
Git-authoritative; these events make the binding portable and signed.

**NIP-RV — Reviews.** Review requests, review verdicts (approved,
changes-requested, rejected, inconclusive) with evidence refs, inline
change-request threads (NIP-22 anchored to NIP-34 patches), Review Guide
records (regenerable evidence-linked explanations that never approve or
merge), and Review Inbox projections. A verdict contributes evidence toward
NIP-EV verification; it is not merge or release authority.

**NIP-WA — Workroom Activity.** Formalizes the already-implemented
`openagents.signed-workroom.v2` projection profile and its pinned kind range
32150-32163: membership, thread, mention, assignment, delegation,
agent_session, agent_activity, code_change, review, decision, evidence,
verification_ref, receipt_ref, revocation. Includes the two-phase
prepare/commit external-signing lane, actor grants for organizational/
device/agent signers, audience-scoped relay policy, the durable
persist-before-publish outbox, and delivery receipts with
`relayAcceptanceIsAuthority: false`. This NIP is the bridge between the
native authority and the relay: drafting it first costs nothing because the
implementation already exists.

### Layer 3 — Automation and attention

| NIP | Name | Tentative kinds |
| --- | --- | --- |
| NIP-GB | Guidance Bundles | 32330-32339 |
| NIP-AL | Automation Loops | 32340-32349 |
| NIP-TP | Triage Proposals | 32350-32359 |
| NIP-AT | Attention and Notifications | 32360-32369 |

**NIP-GB — Guidance Bundles.** Versioned organization, team, project,
repository, and workflow guidance with an explicit precedence chain and
deny-fails-closed conflict handling. A run manifest names every Guidance
revision it consumed. Natural language cannot grant tools, budgets,
repository access, release, or settlement — guidance is instruction data,
not a capability grant. Complements NIP-SKL (procedures) as the policy/
knowledge side.

**NIP-AL — Automation Loops.** Loop definitions with immutable draft/
published revisions: trigger spec (Work events, code checks, schedules,
signed workroom events from admitted actors), condition spec, agent and
skill refs, tool/approval/budget/concurrency/retry policies, circuit
breakers, and structural self-trigger exclusion (a Loop's output event
classes are excluded from its own trigger graph by construction). Loops
start in suggest/ask mode; automatic mutation requires an admitted policy
revision. Duplicate events, echoes, and retry storms must not create
duplicate or unbounded Work.

**NIP-TP — Triage Proposals.** Typed proposals from a Triage Engine: team/
project routing, duplicate candidates, labels, priority, owner/delegate
suggestions, service level, readiness, and required questions — each with
confidence, evidence refs, and expiry. A human disposition or an admitted
auto-apply policy converts a proposal into a NIP-WI intent; a proposal never
mutates Work by itself, and proposal acceptance/correction rates are
measurable from the event stream.

**NIP-AT — Attention and Notifications.** Principal-scoped Attention Items
(assigned, delegated-active, waiting-for-answer, mentioned, blocked, failed,
verification-disagreed, review-requested, decision-required, budget-
exceeded, degraded) with stable subject and reason refs, plus Notification
Subscription preferences. Private to the recipient (encrypted or
recipient-gated). Composes with Block NIP-RS (read state), NIP-ER
(reminders), and NIP-PL (push leases) — this NIP owns what needs attention;
those own seen-state, scheduling, and wake delivery. Triggering a
notification never triggers Work.

### Layer 4 — Hosts, outcomes, and public trust

| NIP | Name | Tentative kinds |
| --- | --- | --- |
| NIP-HP | Hosts and Placement | 39560-39579 |
| NIP-OC | Outcome Closeout | 39580-39589 |
| NIP-PP | Product Promises | 32440-32449 |

**NIP-HP — Hosts and Placement.** Host records (identity, kind, owner,
generation, health, capability refs, grant refs, last-observed time) for
local machines, remote hosts, sandboxes, Pylons, Agent Computers, CI
workers, and production targets, plus capacity statements and admitted
Placement/Dispatch Decision records with exact policy revision and input
refs. Host reachability is never an execution grant; a capacity
advertisement is never a Dispatch Decision. Extends the TRN node-record
pattern (39501) from training networks to general Work placement.

**NIP-OC — Outcome Closeout.** The terminal economics record: Accepted
Outcome events binding acceptance, verification receipts, the receipt
graph, contributor refs, and attribution/split refs; and closeout events
covering accepted, rejected, refunded, and no-reward outcomes. Links to
NIP-AC settlement receipts (39244) and NIP-LBR closeouts without granting
payment authority — settlement stays with its designated ledger. This is
the wire form of accepted-outcome accounting: the unit the whole engine
prices.

**NIP-PP — Product Promises.** The public trust registry as addressable
events: one Product Promise record per named capability with public claim
text, scope, Promise State (green/yellow/red/degraded/planned), evidence and
blocker refs, verification guidance, and transition receipts. Registry
transitions are authority-signed; marketing copy, screenshots, or individual
receipts cannot silently flip a state. Makes the existing
`openagents.com/promises` contract portable and independently verifiable.

## Suggested drafting order

1. **NIP-WA** — the implementation already exists; write the spec around the
   pinned 32150-32163 range and the prepare/commit lane.
2. **NIP-WK + NIP-WI + NIP-PI** — Work, its command wire, and the Issue
   projection. This is the minimum for a Nostr-backed issue tracker.
3. **NIP-AD + NIP-AS + NIP-AV** — delegation and agent sessions, the agent
   half of the Linear port.
4. **NIP-EV + NIP-RC** — evidence/verification and repository claims, the
   trust and coordination substrate for real coding work.
5. **NIP-CC + NIP-RV** — code context and reviews over NIP-34.
6. **NIP-OT + NIP-WS + NIP-WR + NIP-PG + NIP-DD** — the full planning graph.
7. **NIP-GB + NIP-AL + NIP-TP + NIP-AT** — automation and attention.
8. **NIP-RP + NIP-CN + NIP-HP + NIP-OC + NIP-PP** — release planning,
   customers, placement, outcomes, and public promises.

## Non-goals

- No NIP here makes a relay an OpenAgents command, membership, execution,
  verification, receipt, release, payment, or settlement authority.
- No NIP reproduces Linear's service, schema bytes, or branding; the
  vocabulary is adopted where meanings match, per the MIT-licensed API
  study.
- No NIP replaces the Block lane: agent identity, memory, telemetry, read
  state, reminders, and push remain composed from Buzz's specs.
- No kind number in this index is final until its NIP is drafted and passes
  a collision review.
