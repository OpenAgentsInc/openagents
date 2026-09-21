# Minecraft protocol profile

Status: proposed application profile of the pinned NIPs. The normative source is
the [OpenAgents specification set](../../nips/openagents/README.md). Minecraft
does not need a new event kind for every action.

## Use the whole stack for a reason

| Contract | Minecraft use | Retained evidence | Delivery stage |
| --- | --- | --- | --- |
| [Shared contracts](../../nips/openagents/contracts.md) | Exact map, quest, observation, patch, and checker references; common outcomes | Digested artifacts, resolved locks, explicit unknown effects | Core |
| [CAP](../../nips/openagents/NIP-CAP.md) | Describe observe, walk, mine, submit patch, verify, and project gate operations | Definition, trusted local binding, grant, and presence result kept separately | Core |
| [PRG](../../nips/openagents/NIP-PRG.md) | Compose a bounded mining or coding quest from typed steps | Program digest, selected branch, step inputs and results | Core |
| [EXT](../../nips/openagents/NIP-EXT.md) | Package the arena's programs, question sets, and reusable skills | Immutable release and complete dependency lock | Core package; cross-guild sharing later |
| [CJ](../../nips/openagents/NIP-CJ.md) | Send decisions and coding jobs to agents reached through the relay | Signed request, feedback, result, and stable request/attempt identity | Decision transport first; execution after its recovery gate |
| [RUN](../../nips/openagents/NIP-RUN.md) | Recover an episode after a worker or host stops | Durable predecessor chain, generation, dispatch intents, unresolved effects | Core local durability; relay publication separately gated |
| [CTX](../../nips/openagents/NIP-CTX.md) | Give each miner, coder, and reviewer the evidence its task needs | Task frame, world and repository snapshots, selected context view | Core local artifacts |
| [POL](../../nips/openagents/NIP-POL.md) | Preserve arena instructions, spend bounds, disclosure, and admissible routing | Resolved policy, approval where required, route and usage receipts | Core local enforcement |
| [COORD](../../nips/openagents/NIP-COORD.md) | Claim deposits and work, reserve a shared budget, fence obsolete agents | Coordinator revisions, atomic claims, budget transitions, findings | Core single coordinator |
| [EVAL](../../nips/openagents/NIP-EVAL.md) | Compare guild policies and decide whether a skill works | Frozen suite, all outcomes, coverage, comparator, evaluator identity | Core verification; comparative study later |
| [OPT](../../nips/openagents/NIP-OPT.md) | Improve task selection or skill retrieval within a fixed contract | Study, partitions, materialized candidates, trials, costs, confirmation | After the live loop |

“Core” means required by the target application, not already implemented or
guaranteed for September 22. A local artifact can exercise a contract without
publishing it. A transport demonstration needs retained signed wire events as
well. The [demo coverage record](demo-2026-09-22.md) distinguishes these claims.

## Discovery and execution are different

Publish only public descriptions through CAP `30180`, PRG `30182`, and EXT
`30184` discovery records. CAP `30181` policy remains subject to its own scope.
An EXT `3184` release identifies immutable components; resolve and retain the
complete lock before a run. The latest listing is not an executable version pin.
Revocation `3185`, namespace migration `3186`, and checkpoint `30185` affect
future resolution according to EXT. They must not silently replace an active
run's loaded dependencies.

CAP bindings expose typed, bounded operations. A mining capability might accept
a registered deposit reference and action limit; it must not accept arbitrary
server console commands. Discovery is inert. Probe execution needs the host's
trusted digest and adapter approval, and an inconclusive probe yields unknown.

PRG expresses a finite acyclic workflow with its existing seven step kinds:
`query`, `check`, `decide`, `delegate`, `program`, `module`, and `invoke`. The
host's bounded episode loop schedules repeated programs. Do not add a hidden
unbounded loop to a graph. A `module` remains the constrained Wasm plugin ABI;
it is not a place to hide a process, network client, or unrestricted game script.

An EXT skill can describe reusable behavior. Executing it still requires a
supported program or interpreter, pinned dependencies, and admitted capability
effects. Publishing a skill does not install or authorize it.

## Guilds and messages

Use [NIP-29](../../nips/official/29.md) for membership and group history, and
[NIP-C7](../../nips/official/C7.md) kind `9` for chat. Identify a guild group by
its relay and group ID together. Display names are mutable presentation.

On setup, the operator creates the two groups and enrolls the roster through
supported moderation operations. Agents authenticate with
[NIP-42](../../nips/official/42.md). Clients verify relay-signed metadata against
the expected relay identity. Group messages have the group's `h` tag; replies
use C7's `q` reference. Maintain and validate NIP-29 `previous` references as
specified when used. Do not reinterpret a chat message as a signed job result.

The current [relay group subset](../protocol/nip-expansion.md) is public-read
and restricted-write. Private groups and subgroups are not implemented. Only the
supported admin role is meaningful. The first demo therefore uses deliberately
public guild plans and sanitized quest summaries. No repository secrets, private
patches, credentials, or protected evaluation cases belong in guild chat.

The game role “miner” or “reviewer” is an application assignment. It is not a new
NIP-29 administrative role. Membership is neither a spend grant nor an execution
permit. A group removal prevents future admitted activity under the updated
policy; it does not prove an already running process stopped.

## Private artifacts and three meanings of routing

Use [NIP-44](../../nips/official/44.md) for the encryption required by the
OpenAgents envelopes. Shared private artifact kind `3188` has exactly one
recipient `p`, a random mailbox `h`, and the prescribed `oa:artifact:v1` marker.
The `h` value is not the guild ID. RUN's private envelopes also follow their own
mailbox contract. CJ families use their own request and recipient rules.

The relay must enforce author/recipient access for private artifacts across
history, live subscriptions, ID queries, and COUNT, and exclude them from search.
Encryption alone is not permission to advertise an access-gated implementation.
Do not publish these envelopes to a relay until that role has passed fixtures.
Local storage is valid when relay publication is unavailable.

RUN publishes regular encrypted records as `3187` and addressable encrypted head
hints as `30186`. A hint's `d` matches its mailbox; the run marker is
`oa:run:v1`. Neither a newer hint nor a relay timestamp resolves a conflicting
predecessor chain or grants a new controller generation.

Recipient filtering happens before the context selector or reviewer receives
evidence. A selector must not see another guild's private data merely to decide
that the coder should not see it. Re-encrypt separately for each admitted
recipient, preserving content identity and disclosure records.

## A job is not a conversation

| Family | Request / result / feedback | Permitted claim |
| --- | --- | --- |
| Conversation | `25900` / `26900` / `27000` | A response exchange under the conversation contract |
| Typed decision | `25910` / `26910` / `27010` | A System One request under `openagents.systemone.v1` |
| Execution | `25920` / `26920` / `27020` | A recoverable job under `openagents.execution.v1` |

Validate the family, schema version, signer, recipient, request reference,
attempt, and body fingerprint. Subscribe before publishing ephemeral requests.
Never put an HTTP bearer key inside a job. Each worker authenticates and admits
its caller under its own configured policy.

Test the decision bridge against the pinned CJ draft, not just an older local
codec. This draft requires string state and string question descriptions and
defines Choice confidence as the selected probability and Score confidence as
the maximum level probability. TypeSafe's current docs describe confidence as a
distribution-concentration measure. Preserve the upstream response and any
explicit normalization provenance; derive and label the CJ fields according to
its contract or refuse an unsupported translation. Never silently compare the
two confidence fields as if they were the same metric. Serialize structured
state to the admitted string form with its exact bytes retained.

The existing Coder relay conversation path does not establish execution-family
support. A decision codec does not establish a working decision worker either.
When a coding job uses a local executor during development, label the executor
local and retain its actual record. Do not wrap a prose answer in a new kind and
call it recoverable execution.

An execution worker persists accepted identity and a RUN root before claiming
durable acceptance. Replayed requests with the same identity return the existing
status; changed content under the same identity conflicts. A cancellation
acknowledgment is distinct from confirmed process termination. Unknown execution
must be reconciled before dispatching the same effect to another worker.

## One end-to-end exchange

The following sequence describes semantics, not a new JSON wire schema.

1. The host resolves an EXT release and locks CAP, PRG, questions, sources, and
   verification dependencies for the round.
2. A miner posts a public kind `9` guild message requesting a task. The host
   builds a CTX frame from admitted observations and POL instructions.
3. A CJ decision request reaches the designated Jev worker. Its typed answer
   selects an eligible task or none. The host rechecks deterministic admission.
4. COORD atomically records a deposit claim and any required budget hold. RUN
   records intent before the mining capability runs.
5. The referee verifies the world change and commits one credit award. The
   public guild receives a sanitized summary referencing the award's public
   projection, not private evidence bytes.
6. The coder obtains a quest claim and reservation. CJ execution carries the
   pinned task to its admitted worker; RUN carries durable status and evidence.
7. An independent evaluator checks the patch. The integrator records a separate
   acceptance decision. An accepted result causes one XP award and a bounded
   world update.
8. The referee publishes a NIP-32 achievement label when public disclosure is
   allowed. EVAL retains the complete case result regardless of success.
9. A later OPT study proposes an improved skill selector. EVAL confirmation and
   operator adoption produce a new EXT release for future rounds.

Steps 3 and 6 must be shown as local calls if their relay workers are not built.
Step 9 is a later experiment and must not appear as completed learning in a demo
that only reused a supplied skill.

## Reputation labels

[NIP-32](../../nips/official/32.md) kind `1985` carries categorical labels. It
explicitly distinguishes labels from numerical measurements. Publish an
achievement such as `verified-rust-repair` in a versioned namespace such as
`openagents.minecraft.achievement.v1`; keep cumulative XP in the application
ledger.

An issuer labels either an agent (`p`) or an evidence event (`e`) deliberately.
Adding both targets labels both objects; it is not a general attribution syntax.
The public evidence projection should itself identify the award, subject,
season, accepted outcome, and issuer. A private evidence digest is not permission
to disclose private content.

Trust labels only from the configured referee/evaluator for the relevant
season. Self-awarded or unknown-issuer labels can be displayed as untrusted
claims but must not change rankings, balances, or permissions. Corrections are
explicit compensating ledger records and updated projections, not silent edits
to history. Optional [NIP-58](../../nips/official/58.md) badges can later decorate
profiles; they do not replace this accounting.

## Other official NIPs

| NIP | Use or exclusion |
| --- | --- |
| [01](../../nips/official/01.md) | Signed event identity, subscriptions, duplicate handling, and storage semantics |
| [11](../../nips/official/11.md) | Inspect relay capabilities and limits; named OpenAgents roles belong in `supported_extensions` |
| [40](../../nips/official/40.md) | Bound useful lifetime of ephemeral offers; expiration is not cancellation or budget release |
| [45](../../nips/official/45.md), [50](../../nips/official/50.md) | Exercise COUNT/search bounds and prove private records do not leak |
| [34](../../nips/official/34.md) | Optional public repository and patch discussion after disclosure review; not required to store private patches |
| [65](../../nips/official/65.md) | Later relay discovery; does not provide automatic replication or failover semantics |
| [86](../../nips/official/86.md), [98](../../nips/official/98.md) | Operator administration through the existing management API; never give bots the management credential |
| [57](../../nips/official/57.md), [60](../../nips/official/60.md), [61](../../nips/official/61.md) | No zaps or ecash in the first economy; compute credits are an application allowance |

## Block NIP fit

The [Block lane](../../nips/block/README.md) provides useful application patterns.
Use a feature only when its configured implementation is present.

| NIP | Minecraft role |
| --- | --- |
| [OA](../../nips/block/NIP-OA.md) | Bind an agent key to its owner; preserve author and owner as different identities |
| [AA](../../nips/block/NIP-AA.md) | Owner-based relay access where configured; never bypass arena enrollment or owner-level rate bounds |
| [AP](../../nips/block/NIP-AP.md) | Optional public persona and catalog for guild recruitment; publish only sanitized role descriptions |
| [AE](../../nips/block/NIP-AE.md) | Optional encrypted agent memory; not the immutable skill release, credit ledger, or accepted history |
| [AO](../../nips/block/NIP-AO.md) | Private live activity for the owner; lossy activity is not a durable award |
| [AM](../../nips/block/NIP-AM.md) | Private turn metrics; reconcile them to execution/service receipts rather than treating them as settlement |
| [MP](../../nips/block/NIP-MP.md) | Optional project association for a coding guild and its fixture repositories |
| [GS](../../nips/block/NIP-GS.md) | Later Git object signing with an agent identity; signature validity is distinct from review and acceptance |
| [CW](../../nips/block/NIP-CW.md) | Defer context-window query optimizations; resolve the kind collision below first |
| [DV](../../nips/block/NIP-DV.md) | Optional private-message presentation preference; no execution authority |
| [ER](../../nips/block/NIP-ER.md) | Later reminders to an operator; not an autonomous quest scheduler |
| [PL](../../nips/block/NIP-PL.md) | Leave out; push notification leases are not COORD task claims and the local relay's executor is disabled |
| [IA](../../nips/block/NIP-IA.md) | Optional archive preference after a season; cannot erase unsettled accounting |
| [RS](../../nips/block/NIP-RS.md) | Optional inspector read-state synchronization; no task acknowledgment semantics |
| [WP](../../nips/block/NIP-WP.md) | Optional workspace profile; never a source of capability grants |

The pinned NIP-29 uses `39005` for group pins. Block CW also assigns `39005` to
window summaries. The demo must not enable CW or route these records by kind
alone. A future combined implementation needs an explicit compatibility decision,
schema/signer discrimination, relay admission behavior, and regression fixtures.
Do not locally renumber an upstream protocol and call it compatible.

## Conformance claims

All OpenAgents allocations are drafts. Pin the sources, validate strict schemas,
and reject unknown semantic fields. Keep `meta` inert. Use exact-byte artifact
digests and the shared lock canonicalization rules; do not invent a competing
hash scheme for Minecraft.

Publish a role matrix with every recording: host, relay, worker, coordinator,
evaluator, and optimizer; local validation, live wire exercise, or unimplemented.
A relay that forwards an event cannot claim to enforce budgets or execute its
program. A signed evaluation is attributable evidence, not remote attestation.
