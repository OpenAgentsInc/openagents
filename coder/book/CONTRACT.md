# Coder guarantees and authorization

This is the shared interpretation of the implementation chapters. An
experiment can establish a property of its bounded profile without
establishing it for every live engine. "Implemented" means the chapter's
named scope runs; it is not a security audit or a performance comparison.

## Coverage by execution path

| Property | Bounded harness experiments | Microcoder and Psionic engine | Claude Code engine | Codex engine |
| --- | --- | --- | --- | --- |
| Request reconstruction | Every experimental token request is rebuilt from the experiment's log | Coder reconstructs turn input; pruning and every internal model step are not yet fully recorded | Coder reconstructs the input it submits; internal model history belongs to Claude Code | Coder reconstructs the input it submits; internal model history belongs to Codex |
| Prefix and condensation | Chapters 3 and 7 check logged transformations on finite token arrays | Bounded state is rebuilt and old actions/output are pruned; these transformations are not yet logged as chapter 7 events | Engine owns compaction and cache behavior | Engine owns compaction and cache behavior |
| Durable reporting | Chapter 2's hash-chain experiment is in memory | Coder JSONL lines sync before publication | Coder JSONL lines sync before publication | Coder JSONL lines sync before publication |
| Durable intent before tool effects | Policy experiments log decisions before simulated effects | Live result reporting can follow an effect; complete intent/result recovery remains open | Engine runs its tools; Coder may learn of the effect afterward | Engine runs its tools; Coder may learn of the effect afterward |
| Approval and confinement | Chapter 5 decides access to three simulated rooms; this is not an OS sandbox | Coder can gate commands under selected policy; this alone is not native process confinement | Policy uses permission control and PreToolUse hooks; process confinement still depends on engine/platform configuration | Policy answers engine approvals; Codex also enforces its selected sandbox |
| Default local execution | Explicit experiment grant and budgets | Commands run unless a selected policy or hook gates them | Permission bypass unless options select a policy or asking | Approval `never` and full access unless options select another mode |
| Provenance gate | Chapter 6 checks explicit source labels and planner/reader views | Experimental gate is not integrated through every native tool path | Live Router containment uses ceilings, not the experimental provenance gate | No equivalent live corpus qualification is established by chapter 6 |
| Consent ledger | Chapter 18 checks labelled sends, grants and revocation | Gates remembered facts; ordinary conversation/tool output are not universally ledger-gated | Gates remembered facts; ordinary conversation/tool output are not universally ledger-gated | Gates remembered facts; ordinary conversation/tool output are not universally ledger-gated |
| Crash recovery | Experimental replay rebuilds simulated state | Coder reads complete JSONL lines; arbitrary tool effects may be unknown | Router launch/delivery recovery is checked; engine tool effects remain engine-owned | Router launch/delivery recovery is checked; engine tool effects remain engine-owned |

The Psionic path currently shares Microcoder's loop on a Chat Completions
server. It does not inherit server prefix-cache or constrained-generation
qualification merely by selecting `--engine psionic`. See chapters 9–12
for named engine versions, recorded protocol fixtures and live limitations.
The same permission names across engines do not imply identical enforcement.

## What replay means

| Operation | Reuses what | Effects |
| --- | --- | --- |
| `coder replay ID` | Logged user text, memory/context snapshots and parent turns, compared with recorded engine-input digests | Reads local records; does not re-run the task, its tools or model inference |
| Gateway playback with `GW_REPLAY` | Recorded HTTP responses in order | Sends no gateway request, but commands in returned actions still run for real |
| Experimental seeded replay | Known model/weights, seed, finite tools and recorded state | Recomputes within the named deterministic experiment; not a promise of hosted-model reproducibility |
| Deliberate task re-execution | A task and a chosen environment | New execution with real effects; use a disposable checkout and explicit authorization |

Do not infer equivalent replay modes from the word alone. A missing parent,
engine-owned history or deliberately redacted input can limit reconstruction.
Treat an unavailable input as unavailable, not as a successful verification.
A verified hash chain establishes internal consistency relative to retained
evidence. A party that can replace all records and their trusted heads can
rewrite history consistently.

## Recovery and interrupted effects

A complete recorded result can be retained across recovery. A call that
started but has no durable completion may have acted already. It has an
unknown outcome until reconciled; a process dying does not make its effects
safe to repeat. Exactly-once external effects require receiver-supported
operation identity or reconciliation, not only a local intent record.

Chapter 20 checks specific Router boundaries: adopting an already started
Worker, finding a posted issue comment by its marker, and completing or
reporting interrupted work. Those checks do not qualify arbitrary shell,
file or network effects inside every engine. Automatic retry of such effects
needs a declared retry contract and its own evidence.

## Outbound authorization and current enforcement

An explicit instruction to use a named engine authorizes sending the task
and the context needed for that work to that engine's provider. That is
ordinary inference. It is distinct from uploading logs for telemetry or
training. Engine selection does not authorize unrelated disclosures, and
data found inside a repository or historical transcript cannot grant a new
destination or widen policy. Existing user authorization persists; a ledger
is a mechanism for selected flows, not a reason to ask the same question
again.

The following inventory distinguishes that intended authorization contract
from ledger enforcement. It does not claim the current runtime enforces
every row uniformly.

| Flow | Authorization boundary | Current enforcement and remaining work |
| --- | --- | --- |
| Task, files and tool output to selected engine | Explicit task and selected/default engine in the user's configured workflow | Ordinary conversation goes to that engine; not every byte passes chapter 18's ledger |
| Remembered user/project facts to engine | Additional persistent context must have an active destination/class grant | Chapter 18 gates/minimizes remembered facts; missing grants withhold them |
| Task/event text or watchdog excerpt to System One | Automatic secondary inference needs a session-class System One grant | Routing, Router batch and watchdog use the ledger; refusal yields documented fallback/drop/stuck states |
| `coder decide` question and state | Explicit command authorizes that particular System One call | Not ledger-gated; chapter 18 documents this explicit-call exception |
| Query and facts to Vertex embeddings | Explicit ranked recall uses a remote embedding provider; fact text and query leave the machine | Chapter 19 implements remote embeddings; equivalent destination/class ledger coverage is not established |
| Native gateway fallback | A different provider is a new destination, even if the task is unchanged | Microcoder uses its configured credentialled gateway plan; per-destination consent enforcement across fallback remains open |
| Local foreign-history reader | Explicit history selection or admitted tool call authorizes local reading within existing permissions | Bounded inert data, not a new filesystem grant; secret filtering is not guaranteed |
| Handoff content into another engine | Selection/read permission alone is not authorization for every remote recipient | Govern by the continuing task and authorized engine; a universal labelled handoff send gate remains open |
| MCP/remote tool arguments | Granted tool, destination and task scope | Finite harness adapter checks its policy; chapter 18's labels are not yet joined to all outbound calls |
| Worker/SSH/Pylon delegation | Authorized machine, provider, tool grant and task | Machine grants and credential handles cover named paths; a universal consent-labelled mesh send path remains open |
| Local logs and caches | Task/session persistence policy | Local writing is storage, not outbound sharing; it can retain sensitive content |
| Optional telemetry/training export | Separate explicit purpose and destination authorization | No broad permission follows from ordinary inference or local logging; gate and qualify any exporter before use |

A refused System One decision may still use the first eligible automatic
engine: that engine's ordinary inference still occurs. With no eligible
engine, Coder refuses dispatch. `CODER_ROUTE_ENGINES` bounds automatic
candidates; recent typed provider failures impose local cooldowns. Explicit
`--engine` intentionally overrides those hints. A failure never silently
resends an existing task to another engine. Local
Psionic inference may have a local destination, but the surrounding gateway
plan, embedding or decision calls must be considered separately.

Revocation stops future covered sends; it cannot recall bytes already sent.
Summaries retain their source classes. A full, invalid or concurrently
updated consent store must not invent a grant. Chapter 18's finite ledger
capacity and cross-process behavior limit the current integration.

## Plugin and tool trust

Local shell plugins run commands and hooks with host authority. Bounded
Wasm plugins receive admitted imports and resource bounds. A signature
authenticates an artifact; it does not convert shell code into confined code
or establish that its behavior is benign. Chapter 15 documents both forms.

Coder's policy ceiling is an admission decision. OS confinement is a
separate mechanism. Path checks and tool labels alone do not confine an
arbitrary program that opens files or accesses the network indirectly.
Selected hooks must cover self-approved engine tools as well as explicit
permission requests; chapter 20's Claude lab demonstrates why.

## Evidence and maintenance

Correctness, privacy and authority requirements remain obligations even if
they reduce task completion. Compare optional mechanisms against a simple
baseline; do not use that comparison to remove a mandatory boundary.

Survey sections describe adopted designs at three states: implemented with
named evidence, proposed for an experiment, or rejected with a reason.
"What Coder takes" is a design adoption list, not an implementation status
marker: an item without named live evidence in its owning chapter remains
proposed. The owning chapter and the coverage matrix govern product claims.
Present-tense language about another project's behavior is scoped to its
pinned revision, not today's installed product. Update this contract when
an integration qualifies a stronger claim, linking the actual check and
supported host/engine profile.

For the next practical steps see the
[repository journey and coding pilot](../../docs/coder/repository-pilot.md).
