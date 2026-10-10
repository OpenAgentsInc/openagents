# NIP-CJ — Agent Jobs

`draft` `optional` — v1. The [shared contracts](contracts.md) are normative.

This NIP defines encrypted conversation, typed-decision, and recoverable
execution jobs; [NIP-DEC](NIP-DEC.md) specifies typed decisions. Tasks are
domain-independent: document, research, coding, and business operations use
the same transport with domain schemas and host admission. A relay transports requests; it grants no execution authority.

Test-time capabilities: decision jobs and the conversation `judgment` feedback carry the router's side of the [judgment budget](../../docs/essays/2026-09-29-test-time-capabilities.md#5-judgment-budget); the `start_eval` and `publish_eval` offers, cards, and test-set draft carry the chat path, and execution jobs carry hosted eval runs ([mapping](../../docs/essays/2026-09-29-test-time-capabilities.md#how-the-protocol-carries-test-time-capabilities)).

## Families and transport

| Family | Request/control | Result/control answer | Feedback | Payload version |
| --- | --- | --- | --- | --- |
| Conversation | `25900` | `26900` | `27000` | Integer `1`. |
| Decision ([NIP-DEC](NIP-DEC.md)) | `25910` | `26910` | `27010` | `openagents.systemone.v1`. |
| Execution | `25920` | `26920` | `27020` | `openagents.execution.v1`. |

All kinds are ephemeral. Relays fan them out to matching subscriptions and
MUST NOT retain them as durable job state. Execution recovery uses durable
worker state and [RUN](NIP-RUN.md). Each family has its own parser; a handler
MUST reject another family's kind or payload rather than reinterpret it.

Every request has exactly one `p` naming the worker. Every response has
exactly one `p` naming the caller and one `e` naming the exact request or
control event it answers. Content is NIP-44 v2 encrypted to that recipient.
Bodies contain required `v` and `requires`, plus optional inert `meta`.
The caller subscribes before publishing. An optional NIP-40 `expiration`
limits delivery; where a body declares a deadline they MUST agree.

Verify NIP-01 event ID/signature, kind, signer, recipient, and request binding
before accepting a payload. Deduplicate verified event IDs. An unsigned
subscription label cannot identify a job. NIP-42 authenticates a relay
connection, not a forwarded event's execution authority. The verified request
signer maps to host principal/tenant policy outside the payload. Payloads
contain no bearer credential or self-asserted grant, with one exception:
caller-paid model calls, below.

Workers bound input bytes, output bytes, active jobs, spend, and elapsed time.
They validate request freshness under a declared skew/window policy. Bodies
with missing/unknown versions or features refuse. Display strings are data;
clients must escape terminal controls and active markup.

## Conversation jobs

A request contains `v: 1`, `requires`, `task` (string), `transcript` (ordered
array of `{role, content}`), and optional `instructions` and `client` strings.
Role is `user` or `assistant`; content is a string. Both caller and worker
bound the transcript. Instructions are caller-supplied guidance and cannot
override host policy. Client is informational and conveys no authority.
A request MAY also carry `router` (the name of a routing question set the
worker serves, such as `chat-router-v1`) and `context`, a bounded object of
`surface` (`phone`, `desktop`, or `terminal`), `computer_ready` (boolean),
and `app_build` (at most 64 bytes). Added 2026-09-30: `computer`, where
Coder runs for the chat, as `{place: "here", name?, engines}` when the
sending device is itself that computer (the desktop app, or the terminal on
a computer) or `{place: "paired", name, engines?}` when a phone is paired
with one (`engines` added 2026-10-01: the coding agents the computer's
[HOST](NIP-HOST.md) presence names, omitted when it names none; a worker
that predates it ignores it);
`name` is the label the person gave the computer (at most 64 characters),
and `engines` is at most 8 `{engine, state}` (at most 4 before 2026-10-01)
with `engine` a word of at most 16 lowercase ASCII letters, digits, `-`, or
`_` (such as `codex` or `claude`) and `state` `ready`, `not_signed_in`,
`limited`, or (added 2026-10-01) `not_enabled`, an agent installed or signed
in on the computer that the person's settings do not allow. `engines` names
every coding agent installed or signed in on the computer, the allowed ones
first. A worker leaves out an engine whose state it does not know and reads
the first engines up to its own bound. Also
`project`, the chat's project folder, as `{name, path?}`: `name` at most
128 bytes, and `path`, the folder's absolute path of at most 1024 bytes,
only beside `place: "here"` (a worker ignores a path from anywhere else).
Every string is printable; a value past its bound is left out, never cut.
The computer's name and the project folder are the person's own data: a
worker uses them only to tell its model where the chat runs and to fill its
own reviewed answers, and sends them to no other service. Added
2026-10-01: `coder_run`, the chat's Coder run once its turn has ended, as
`{ending, turn, engine?, model?, summary, files, commands}`: `ending` is
`finished`, `failed`, or `stopped`; `turn` the turn that ended; `engine` a
word as in `engines`; `model` at most 64 bytes; `summary` what Coder
reported, at most 4 KiB (line breaks allowed); `files` at most 32
`{path, status}` with `path` at most 512 bytes; and `commands` the turn's
last 16 commands, each its first line of at most 200 bytes. A client sends
it only after the turn ended, never while Coder works or waits for an
answer. These too are the person's own data: a worker gives the summary,
files, and commands only to its model's instructions, so the chat can
answer about the run, and lets its routing read only that the run ended and
how. Context carries
no credential, key, host address, or amount; a worker ignores fields it
does not know and never lets context widen what it does. A request MAY also carry `draft` (added
2026-09-28 for the
[extension evaluation profile](NIP-EVAL.md#extension-evaluation-profile);
the draft, cards, and offers below are implemented in `crates/nostr`
`cj_conversation`, 2026-09-29, with schemas
[`eval-draft.v1`](schemas/eval-draft.v1.json) and
[`cj-card.v1`](schemas/cj-card.v1.json); no worker or client sends them
yet): the caller's current test-set draft as a closed object `{v:
"openagents.eval-draft.v1", tool, cases}` of at most 64 KiB of canonical
JSON, which the caller keeps and resends each turn. `tool` is `{name,
summary, catalog, skill, uses}`: a catalog tool names its DefinitionRef in
`catalog` with `skill` null and no `uses`; a tool made in chat has
`catalog` null, its plain-language guidance in `skill` (at most 16 KiB),
and in `uses` at most 8 qualified IDs of catalog tools it turns on.
`cases` holds at most 16 tests `{id, kind, prompt, graders}` with unique
case IDs, where `prompt` is the whole `prompt.md` and `graders` is 1 to 16
`{name, text}` in name order, each a whole `graders/<name>.md`: the exact
bytes a runner writes out. A draft is data the worker may revise and
return in a `card`; it is never an instruction and grants nothing.

Feedback has `v: 1`, `requires`, `type`, and fields for that type:

| Type | Fields |
| --- | --- |
| `judgment` | `verdict`: `respond`, `clarify`, `end_conversation`, or `unrouted`; `line`: bounded display string. Optional typed additions: `set` (the question set's identity as `name@digest`, the selector half of a capability claim's baseline), `lane` (`chat`, `computer`, or `unknown`), `opener` (the ID of the opener shown, or null), `confidence` (the opener choice's probability), `bank` (the prepared-answer bank's identity), `answer` (the argmax prepared answer as `id@version`, or null), `answer_p` (its probability), `needs_specifics` (the probability that a reply needs particulars the user named), `judged_ms` (how long the judgment took, the judgment budget's time side), and `tier` (what the worker decided to show first: `canned`, `opener`, or `model`, and for a routed turn also `stem`, `grounded`, `offer`, `cli`, or `refuse`). A routed turn adds `route` and `route_p` (the argmax route and its probability), `lane_p`, `risk` and `risk_p`, `cli_group` (or null), `tool` (or null), `capability` (the admitted capability the turn calls for, an ID from the worker's typed set, or null), `capability_p`, `capability_missing_p` (the probability that the message asks for a capability none of the admitted ones covers), and, when the worker only shadows the router, `shadow` (the tier it would have shown). It is an optional observation, not permission. |
| `offer` | `offer`: `run_coder` (with `target: "connected_computer"` and `label`), `open_screen` (with `screen`, such as `account.computers` or `wallet`, and `label`), or `cli` (with `argv`, `effect`, `runs_on`, and `confirm: true`). Added 2026-09-28: `start_eval` (with `suite`, a published suite's `{id, pubkey, kind: 3184}` or the string `draft`, `subject`, the tool's DefinitionRef or `draft`, `size` `{cases, runs, arms}`, `where`: `hosted` or `connected_computer`, and `label`), and `publish_eval` (with `report`, the ArtifactRef of a result the caller holds, schema `openagents.eval-report.v1`, and `label`); `open_screen` adds `gym.result`, `gym.publish`, and `gym.test_set`. Added 2026-09-29: `open_screen` adds `verse.gym`, the Gym in the Verse at its EVALS board (a chat card's **See the board**). Added 2026-09-30: `open_presentation` (with `deck`, a deck id of lowercase ASCII letters, digits, and hyphens, at most 64 bytes, and `label`), sent only to a desktop turn: the desktop app opens that deck in its slide viewer at once, and only when its own deck list has the id; any other id gets a plain refusal. Added 2026-10-01: `open_screen` adds `routes.map`, the desktop app's route map, sent only to a desktop turn. Added 2026-09-30: `run_coder` may carry `engine`, the coding engine the person asked for, one of `codex`, `claude_code`, `grok_build`, `opencode`, or `devin`; absent means no preference. The worker sets it only from its own typed reading of the message, never from text it copies. It is a request, not permission: the computer that starts Coder puts that engine first and falls back to another only when it is not signed in, at its usage limit or out of capacity, or not allowed by the owner's settings, and says which and why. An `engine` word a client doesn't know refuses the offer, like any other unknown value. Added 2026-10-02: `run_coder` may carry the router's dispatch plan: `runs`, 2 to 5 distinct engine words, starts one run on each in parallel instead of one run (each pinned to its engine, with no fallback to another); `read_only: true` starts the runs under a boundary that writes nothing in the worktree and seals Git, whatever the computer's access setting; `summarize: true` asks for one combined summary of the runs' results once they all end, written by the chat model, not by another run. Absent, they mean one run that may change files, as before. A client that cannot start several runs offers none rather than one. A request MAY then carry `context.runs`, at most 5 runs each shaped as `coder_run` (2026-10-02): the plan's results, for the worker's model to summarize; the worker answers such a request from them alone, with no routing or offer. A hosted `start_eval` stays within the hosted runner's 8 cases, 3 runs, and 2 arms. Offers are closed: an offer, screen, effect, or field a client doesn't know refuses, and a label is at most 80 characters. An action the client MAY render for the user to tap; it is an observation, never permission, and the worker takes no action for it. Tapping `start_eval` makes the client send its own signed execution request; tapping `publish_eval` opens a confirmation first. |
| `card` | Added 2026-09-28. `card`: a closed, typed display record the client renders with its own controls, never as model text: `tool` (`name`, `summary`, `definition` or null, and `latest`, its latest verified result `{publication, headline, verdict}` or null), `draft` (`draft`, the returned `openagents.eval-draft.v1`), `run` (`request`, the execution request's `{id, pubkey, kind: 25920}`, `where`, and `completed` of `planned` runs), `result` (`headline` `{subject_passed, baseline_passed, total}`, `verdict`, `report` ArtifactRef, and `publication` or null), `news` (`items`: 1 to 5 `{title, line, event, path}`, each citing exactly one source event or repository path), `check` (a result waiting for a check: `tool`, `publication`, `headline`, `verdict`, `confirms`, and `disputes`), or `credit` (`total` and up to 50 `awards` `{status, role, xp, title, award}` from the reader's ledger, a `confirmed` one citing its `3193` and a `pending` one none; `total` is the confirmed XP). Added 2026-09-29: `capability` (`status`: `missing`; `closest`: the admitted capability nearest the request as `{name, summary, reach}` with `reach` `chat` or `coder`, or null; `add`: `author`, the chat's authoring interview can make one, or `gym`, open the Gym), which carries nothing of the message. Every number in a card comes from a record the worker verified; a card cites it. A card type or field the client doesn't know refuses; `schemas/cj-card.v1.json` has every card. |
| `partial` | `seq`: nonnegative integer starting at zero; `delta`: string. |
| `status` | `status`: `queued`, `processing`, or `error`; error requires `code` and `message`, with optional nonnegative `retry_after_ms`. |

A worker MAY send `status: processing` as soon as it admits a turn, before any
model answers, so a caller hears it within one relay round trip. A request
MAY ask for a first response: `judge: true` asks for `judgment` feedback, and
`opener: true` asks for that and an opener. A worker that judges the turn
beside its model call MAY then send the chosen opener as partial `seq` 0 when
the judgment arrives before the model's first delta; the result's `text` then
begins with that same opener, so a client that replaces partials with the
result shows the same words. A worker MAY instead, under the same timing,
answer the turn with a prepared answer it holds (a reviewed text, not model
output): partial `seq` 0 carries the whole answer, the result's `text` is
exactly that answer, and the result names it with `model: "bank:<bank id>"`,
`tier: "canned"`, and `answer: "<id>@<version>"`; the worker then drops its
model call. A worker shows nothing before the model's own words when its
judgment is not sure enough of either. A request that asks for neither gets the model's
text unchanged, which a caller that parses the result as structured output
relies on. A judgment never delays generation, and one that arrives after the
model has started adds feedback only.

A request with `router` asks for routing and implies `opener`. The worker
MAY then answer from a reviewed bank in more shapes, each still starting at
partial `seq` 0: a whole answer (as above); a bank stem at `seq` 0 closed at
`seq` 1 by a validated continuation or the stem's reviewed ending; a refusal
from the bank; or a sentence with `offer` feedback. In each of those it drops
its model call. It MAY instead retrieve reference passages and answer with
the model grounded in them, or show a bank line above the model's reply. A
routed result adds `tier`, `route`, `bank` (the bank as `name@digest`), and
optionally `answer` (`id@version`), `followups` (up to a few `{id, label}`
suggestions whose label the client MAY send as the user's next message),
`citations` (`{id, title, source}` for a grounded reply), and `commit`. Its
`model` names who wrote the text: `bank:<bank name>` when no model did, the
continuation's model for a personalized stem, `kb:<corpus>` for a knowledge
base's reviewed answer. A routed turn is admitted and metered as any turn.

A request with `type: "rank"` asks for an ordering instead of a turn. It
carries `candidates` (1 to 16 `{id, label}`, IDs 1 to 64 bytes and not
`none`, labels at most 200 bytes), an optional `draft` string, and the
conversation's `transcript`. The worker admits and meters it as a turn and
answers with one result: `type: "result"`, `text` (the most likely ID),
`ranked` (every candidate as `{id, p}`, most likely first), `model`, and
`set`. A worker without a judge refuses it `unavailable`; malformed candidates
refuse `malformed`.

Workers increment `seq` once per emitted partial. After event-ID deduplication,
a client renders only the next contiguous sequence number. A gap, repeat with
a different event ID, or out-of-order partial ends incremental rendering;
the client waits for the complete result. Feedback never proves completion.

A result has `v: 1`, `requires`, `type: "result"`, nonempty `text`, optional
`usage: {input, output}` (nonnegative token counts), and optional `model`
(nonempty identifier). Missing usage is unknown, not zero. A model name is
an attribution claim, not proof of immutable weights. Added 2026-10-09: a
result MAY carry `switched` (`{provider, model, why}`) when the model
provider the worker asked first did not answer the turn before its first
words and another model wrote the reply: `provider` is `openagents`,
`openrouter`, `vercel`, or `other`; `model` the id that provider was running
(1 to 128 printable bytes); `why` is `error`, `timeout`, or `refused`. The
client says so in one short line beside the answer, naming the result's
`model` as the one that answered; an unknown word drops the field. The first valid result
or terminal error ends observation; subsequent events do not change it.

A worker that answers callers it has not admitted by name meters them and
refuses with `rate_limited` or `quota_exhausted` (each with `retry_after_ms`)
or `limit_exceeded`; a metered caller's refusal is not an outage.

Conversation jobs have no durable retransmission identity. Another request is
a new invocation. A dropped socket does not stop remote work. Effects requiring
recovery, cancellation, or exact implementation attribution use execution jobs.

### Caller-paid model calls

Added 2026-10-02. A conversation request MAY carry the caller's own model
provider keys, so the caller pays for the job's model calls. Such a request
names `payer.keys` in `requires` and carries `payer: {keys: <ciphertext>}`,
where the ciphertext is the JSON array `[{provider, key}]` (1 to 3 entries,
`provider` one of `openrouter`, `vercel`, `typesafe`, each at most once, each
key 1 to 512 bytes) NIP-44 v2 encrypted under the request's own conversation
key, separately from the body, so a decrypted body never holds a key in
plain text.

A worker that serves `payer.keys`:

- runs every model call of that job (the reply, personalization, decision
  subcalls, and the embeddings and judgments of any retrieval it grounds the
  reply in) only on those keys, and refuses the job rather than answering any
  part of it on keys of its own; a retrieval those keys cannot run is skipped
  for that job, never run on the worker's keys;
- uses the keys for that job only, never stores, logs, or publishes them,
  and records at most each key's provider and a fingerprint (the first 8 hex
  characters of its SHA-256 digest);
- treats them as payment only: they never widen admission, delegation, or
  execution, and a caller admitted only for conversations stays so.

A worker that does not serve `payer.keys` refuses the request
`unsupported_feature`, as for any unknown feature, so a caller's job is never
answered silently on the worker's keys.

## Typed decision jobs

The decision family (`25910` request or cancel, `26910` result, `27010`
status, `v: "openagents.systemone.v1"`) is specified in
[NIP-DEC](NIP-DEC.md): the request and answer shapes, EntryType
instructions and criteria, object `state`, bounds, model aliases, the
HTTP-gateway equivalence and status table, and ATIF recording. The
transport rules above apply to it unchanged, and it is the same wire this
section defined before 2026-09-30: every request valid then is valid now.
Execution results keep separate receipts for any decision subcalls.

## Execution jobs

This family carries an admitted operation or
program, typed task input, context references, and durable outcomes. It does
not use conversation text as an executable command or require an LLM to select
a program. CAP/PRG/EXT/RUN and the shared contracts are normative for this
family. Each handler MUST validate its own kind and schema before admission.

| Kind | Name | Direction |
| --- | --- | --- |
| `25920` | Execution request or control | Caller → worker |
| `26920` | Execution result or control answer | Worker → caller |
| `27020` | Execution admission/progress | Worker → caller |

These are draft OpenAgents assignments and are ephemeral. Durable state lives
in the worker and optional NIP-RUN retention service. A client subscribes before
publishing. Requests have exactly one `p` worker; responses have exactly one
`p` caller and one `e` for the request/control event they answer. Payloads are
NIP-44 v2 encrypted. Verify kind, signer, recipient, and exact request binding
before interpreting content; NIP-42 connection identity is not a forwarded grant.

### Execute payload

The body contains `v: "openagents.execution.v1"`, `requires`, `type: "execute"`,
`request`, `attempt`, `run`, `target`, `lock`, `input`, `context`,
`requirements`, `bounds`, `deadline`, `retain_until`, and optional `parent`.

[NIP-LAB](NIP-LAB.md) defines the required feature
`openagents.labor-binding.v1` for a commercially bound execution. A worker
that does not implement that feature refuses it as `unsupported_feature`.
A LAB worker validates and durably binds the separately authenticated order
linkage before dispatch. The marker grants no authority and does not add a
new job family, executable prompt convention, or implicit order field.

[NIP-ENV](NIP-ENV.md) similarly defines required feature
`openagents.environment-binding.v1`: an exact execution attachment must be
reserved, admitted by every required participant, and activated before
dispatch. A generic CJ worker refuses this feature if unsupported. The
attachment references an already signed execute request; the request need
not refer to its future attachment. LAB and ENV checks are independent, and
an execution requiring both must satisfy both before any effects.

| Field | Meaning |
| --- | --- |
| `request` | Random logical request ID stable across retries. |
| `attempt` | Positive integer; retransmission preserves it, a permitted new attempt increments it. |
| `run` | Logical remote run ID, distinct from parent run ID. |
| `target` | Exact operation, program, or OPT AI implementation DefinitionRef. |
| `lock` | ArtifactRef of the complete dependency lock. |
| `input` | Schema-valid bounded typed value, or `{artifact: ArtifactRef}` when the target schema specifies artifact input. |
| `context` | ArtifactRef of a recipient-specific context manifest. |
| `requirements` | ArtifactRef of requested effects, assurance, and disclosure constraints; not a grant. |
| `bounds` | Whole-attempt ceilings under the caller's parent reservation and worker policy. |
| `deadline` | Required Unix-second latest completion time; mirror in `expiration`. |
| `retain_until` | Required recovery horizon beyond deadline, subject to worker admission. |
| `parent` | Optional `{run, step, iteration, attempt}` attribution; no inherited authority by assertion. |

The request signer maps to the worker's principal/tenant policy independently
of payload claims. Admission resolves the target/lock, validates context
recipient and source scope, intersects authority, verifies enforceability,
reserves quota, and durably claims the pair before any effect. Missing or
unavailable referenced content refuses under bounded fetch policy. Credentials
and local absolute paths are never supplied as portable authority.

The worker validates `created_at` against its documented freshness/skew window
and requires an unexpired deadline. An `expiration` tag that differs from the
deadline is malformed. Retention must extend beyond the deadline. A deadline
is enforced by the worker as well as checked by the relay; NIP-40 expiration
does not terminate a subprocess. Valid retransmissions retrieve known state
without dispatching again, even if execution's deadline has since passed.

### Identity, admission, and retransmission

The idempotency key is `(worker, principal, request, attempt)`. Its fingerprint
is SHA-256 of JCS(the complete execute body), including target, input, lock,
context, requirements, bounds, deadline, and retention horizon. Different
transport events with the same key and fingerprint are retransmissions:
return the recorded admission/result and do not reserve or execute again.
Changed content under the same key is `idempotency_conflict`.

The worker binds a request to one run and monotone attempt sequence. A new
attempt is admitted only after reconciling the preceding outcome and applying
the target's retry contract. No automatic retry of an unknown effect is
permitted. Resending to a different worker has no cross-worker deduplication
guarantee and requires explicit reconciliation/admission.

Before a `27020` `type: "accepted"` response, persist the admitted claim,
enforcement/reservation plan, and NIP-RUN root. Accepted includes `request`,
`attempt`, `run`, `input_digest`, `lock_digest`, `record` (exact encrypted
NIP-RUN EventRef or retained record ArtifactRef), `mailbox`, `retain_until`,
and `controller`. It binds to the execute event with `e`. The worker may refuse
an unsupported retention request; it MUST NOT silently promise a shorter one.

A relay `OK` is delivery admission only, not worker acceptance. Missing
accepted feedback does not prove that the worker did nothing. The worker
records dispatch intent before dispatch; crash after intent is unknown until
reconciled. Claim/dispatch storage and fencing enforce at-most-once dispatch
for a known attempt where supported; the protocol does not promise exactly
once effects across crashes or external systems.

### Progress and results

`27020` progress has version/features, `type: "progress"`, request/attempt/run,
`seq`, and `status` (`queued`, `running`, or `reconciling`). Sequence is a
monotone progress counter, separate from the authoritative run journal.
Gaps stop incremental rendering until status/replay; progress never establishes
completion. Optional view/evidence ArtifactRefs remain recipient-scoped.

`26920` `type: "result"` includes request/attempt/run, common `outcome`,
`dispatched`, `output` (typed value or null), `artifacts`, `receipts`,
`verification`, `integration`, and latest `record` reference. A refusal before
dispatch also contains a typed `code` and `message`. Refusals distinguish
unsupported semantics, permission, stale inputs, limits, busy capacity,
revocation, unavailable content, and identity conflict. Unknown spend is null,
never zero. Decision subcalls retain separate execution receipts.

Persist results before reporting them as recoverable. One logical terminal
result can be delivered repeatedly, bound to each retransmission's event ID;
clients deduplicate by request/attempt and verified outcome identity. Conflicting
terminal results require reconciliation, not first-arrival selection. A result
may contain only artifacts and no prose; nonempty conversation text is not a
requirement here. Execution completion does not imply verified acceptance.

### Status, replay, and cancellation

Controls use kind `25920`, the same version/features, and `type` of `status`,
`replay`, or `cancel`. They contain request/attempt/run and one `e` referring
to an accepted execute event. `replay` additionally contains `after_seq` (null
for the root) and `max_records`; `cancel` contains a bounded reason.
Require the original caller or an independently authorized control principal;
knowledge of run ID or mailbox is not authority.

[NIP-CTRL](NIP-CTRL.md) defines one such independently admitted client role.
Its task rights do not transfer the original caller's execution or spending
authority. Apply its current grant and revision checks before control admission.

Controls never create new execution. The worker answers with `26920`, `e`
bound to the control event, type `status_result`, `replay_result`, or
`cancel_result`, and the same logical identities. Status includes the current
state and latest record/result reference. Replay includes ordered retained
record references, `next_seq`, and `complete`. Gaps, truncation, or expired
retention are explicit. A missing/expired record answers `unknown` or
`content_unavailable`, not proof of unattempted work.

Cancellation persists `cancel_requested`, prevents queued dispatch, propagates
to children/supervised processes, and reports confirmed outcome or unknown.
Before dispatch it may resolve `cancelled` with dispatched false; after
dispatch it MUST preserve evidence of effects and unresolved accounting.
`cancel_result` acknowledges the control, not guaranteed stop. Late outcomes
remain available for reconciliation even when the UI stops displaying them.

Workers retain idempotency state/results or tombstones through `retain_until`.
An expired/stale execute event is refused and MUST NOT recreate forgotten
work. A new attempt after expiry requires explicit reconciliation policy;
absence of a tombstone is not permission to repeat an effect.

### Optimization and compiled execution

An execution target may be an [OPT](NIP-OPT.md) AI implementation or a
registered optimization, materialization, or evaluation operation. Its typed
input pins study/candidate/trial references; its lock pins the complete
functional closure. The worker verifies those identities before measuring or
attributing a result and returns materialization and evaluation artifacts.
A generic conversation response cannot substitute for that evidence.

The same admission, disclosure, reservations, retries, and durable recovery
apply to proposal, reflection, student, and judge work. New candidate bytes
require a new candidate/trial identity; retransmission cannot hot-swap them.
No dedicated optimizer job family or arbitrary code payload is required.

## Conformance

Required cases cover family cross-delivery, malformed schemas, sequence gaps,
signer/principal binding, wrong answer keys/types, invalid probability mass,
repeated/conflicting fingerprints, lost admission/results, crash boundaries,
restart/status/replay, retention expiry, cancellation races, stale inputs,
unknown spend, duplicate/forked results, unauthorized controls, candidate
substitution, and unbound reflection calls. Relay fanout alone cannot establish
worker execution, materialization, or recovery conformance.
