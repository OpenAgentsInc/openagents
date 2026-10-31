# Pylon decisions in Everglade

Owner direction, 2026-10-30: use Everglade guards as the first low-pressure
game consumer. This replaces the proposed Belt 2200 starting point. The goal
is a complete, bounded request-to-game path whose absence never breaks play.
This is a delivery plan; an outlined milestone is not an implemented capability.

## First behavior

A calm guard may choose **quiet**, **a courteous greeting**, or **an observation
about the weather**. The game supplies the exact authored lines. The decision
model selects an enum; it cannot generate dialogue, award items, move a guard,
invent a crime, change wanted status, choose an attack or override a spell.

Start with Elephant, keeping its configured original checkpoint identity.
Mammoth is a later measured comparison, not an implicit substitute. Current
guard beats remain deterministic functions of the shared clock. Existing crime,
combat, interruption and return-to-duty rules always take precedence. Ambient
state is separate from combat shout indices and never mutates combat state.

The first candidate is one nearby, healthy, calm guard, with a living player who
is not wanted, talking, in an editor/map view or otherwise interrupting duty.
Only one guard request is pending in the realm, not one per actor. A remote
answer cannot make a fallen, removed, replaced or now-hostile guard greet.

## Current foundation and gaps

Chapters 19–21 implement Pylon service, fleet routing and explicit-trust mesh
checks. [21a](../../psionic/book/ch021a-a-public-mesh.md) implements the public
edge at `/api/v1/systemone`. Its [production evidence](../verification/psionic-public-edge-production-2026-10-30.md)
records real Elephant and Mammoth calls, approximately 5.6–6.0 seconds including
a deliberate five-second discovery wait. These are recorded observations, not
a new latency or liveness measurement.

At plan creation the fleet retained a relay connection and peer table but
waited the discovery window on every request. M0 is now implemented on main;
the measured public fleet still uses the older discovery wait. Warm routing must first process
current-connection catch-up and withdrawals; a cached online beacon alone is
insufficient after reconnect. Two learned provider keys share one Spot GPU VM,
so current placement is not independent remote failover.

The public demo allows 12 admitted requests/minute and 120/day per identity,
1,200/day globally, one active call per identity and four globally. Guest play
has no SystemOne session merely because it has a Verse player ID. Use the
existing same-origin HttpOnly session; 401, 429 and unavailable capacity are
ordinary fallbacks. Do not mint identities to escape limits or forward cookies
to a Pylon. New access/budget policy is separate explicit work.

At plan creation the browser HTTP host had eight shared response slots and no
cancellation/generation fence. M1 now provides bounded cancellation and fencing. A consumer timeout cannot simply free/reuse an
active slot: a late response could reach its successor. Deadline measurement
must use a real monotonic effect, not game time, which pauses and changes speed.

## Delivery order

| Milestone | Work | Completion evidence |
| --- | --- | --- |
| M0. Warm routes without stale authority | A current-connection catch-up barrier and bounded pending-frame drain, then immediate selection of an eligible fresh peer. Cold start, reconnect and absence retain bounded discovery. | Focused signed native peers prove no repeated full discovery delay, no job after a newer withdrawal, expiry and reconnect behavior, and bounded malformed traffic. No whole-model CPU runs. |
| M1. A decision arrives while play continues (21b) | A reusable Coax ticket/lifecycle gate and generic HTTP cancellation/deadline/late-response fencing. Fixed capacity, one pending guard request, exactly-once consumption and explicit refusal reasons. | Authored Coax checks; focused authority mutation; native debug/release and sanitizer checks; controlled asynchronous browser/effect tests. |
| M2. An Everglade guard asks (21c) | A small Coax observation/request/response adapter, exact model/question/options validation, and an opt-in shadow integration at the existing guard-frame boundary. | Synthetic responses and actual emitted Wasm exercise request, callback, invalidation and fallback; default/golden modes make no model calls. |
| M3. Real end-to-end and quality | Signed-in shadow requests through the existing public edge; compare recorded choices to local rules on a held-out guard-context suite. Use chapters 156–158 and 164 for calibrated abstention, outcomes and promotion. | Exact checkpoint/build/policy identities, end-to-end latency distribution, fallback/coverage and judged appropriateness. A reference probability match is not game-quality evidence. |
| M4. Ambient application | Explicit opt-in ambient mode displays one authored line after the same admission gate; normal patrol and combat retain priority. No default activation from a confidence number alone. | Calm acceptance, alarm/death/edit/leave invalidation, late/duplicate/old callbacks, malformed answers and service refusals. Browser behavior and frame-time checks on the changed path. |
| M5. Operational reliability | A narrow consumer-focused slice of 117/120/124: service cancellation, bounded scheduling, capacity budgets and separately hosted qualified same-checkpoint providers with explicit route admission. | Kill/disconnect/overload drills, no stale application, independent-provider recovery and no frame stalls. Provider errors retain useful typed causes. |
| M6. Other consumers | Other Everglade ambient characters, then discrete decisions in other games. Voice uses its existing epoch gate; continuous vehicle control stays local. | Each consumer has its own legal action set, observation scope, validity conditions, fallback and quality gate. |

M0 and M1 can proceed in parallel after this plan is pushed. M2 consumes M1's
fixed API. M3 qualifies real shadow usefulness before M4 applies a cosmetic
line. M4 stays opt-in; broader activation needs the same quality evidence;
M5 gates remote availability claims. Finishing unrelated model/training chapters
is not a prerequisite for these slices.

## Ticket and application contract

Local pending metadata binds a session/world epoch, actor identity and generation,
decision sequence, context revision, option-set/policy identity and monotonic
expiry. The existing `openagents.systemone.v1` wire is preserved: local lifecycle
metadata need not become caller-controlled provider or model routing fields.
Any new published protocol projection starts in `proto/` with a distinct version.

At the frame boundary the Coax consumer checks the pending request, permitted
response, exact model/question/options, current actor eligibility and expiry.
It consumes an answer at most once. An ordinary clock tick does not invalidate
the request; a relevant transition does. Recheck proximity and eligibility when
applying, including after a player walks away. Alarm, combat, fall/death, actor
replacement, respawn, rebuild, relevant world edit and realm exit revoke intent.

Timeout or cancellation revokes game authority immediately, independently of
whether the remote model has stopped. Generic host cancellation must reclaim
the owned HTTP slot and fence its late completion before reuse. Browser abort
is not proof that server/Pylon work was cancelled; service-side propagation is
separate M5 evidence. A failed or absent answer leaves the original behavior.

## Initial budgets and privacy

Remote guard behavior is off by default. Explicit development/query modes select
shadow behavior; ambient remains disabled until the later promotion gate.
Golden/clip/replay checks suppress remote calls.
Start with at most one pending call, a 60-second minimum dispatch interval and
six attempts per realm visit. This local cap supplements the existing public
quota; it does not replace account admission. Authentication/quota failures
back off rather than retrying every frame. The initial consumer deadline is
eight seconds, measured monotonically; it is a bound, not a latency promise.
These initial local bounds are implemented and checked; real latency/quality
measurements still decide later revisions.

Send only bounded, game-authored observations: calm/nearby state, time/weather
category and the fixed options. No player speech, chat, account identity, precise
private user history, secrets, hidden opponent knowledge or arbitrary world text.
Providers can read their requests. Log counters, reason codes and policy/model
identities; avoid request/answer content in ordinary telemetry. An opt-in owned
synthetic replay fixture is separate from production content logging.

## Checks and promotion

Write checks before implementation. New admission/application policies are
`#authority` and get focused zero-survivor mutation; preserve imported baseline
checks without re-mutating unchanged crypto/model libraries. Native ASan/UBSan
checks exercise actual emitted C and buffers. Browser checks are muted/headless.
Each command stays below 15 minutes and each pushed slice below two hours.

The central failure drill is: disconnect all Pylons while Everglade runs. Guards
keep their ordinary beats and combat behavior; a delayed answer cannot revive
an old greeting or overwrite newer intent. Include 401/429/503/504, timeout,
malformed/oversized response, wrong model/options, duplicated callback, slot
reuse after abort, tab hide/resume, world reset and guard state changes.

Run the held-out game corpus through the complete service when judging quality.
Measure API→fleet→relay→Pylon→application latency, not just kernel time. Compare
quiet/local-authored behavior and Elephant on the same contexts; a larger model
must earn its cost. Keep uncertainty, coverage, missed deadlines and inappropriate
lines visible. A mesh check-pass or a model confidence never grants game authority.

## Work ownership for the first slices

The coordinating agent owns this plan, book/index reconciliation, integration
review and pushes. Separate agents inspect/implement the shared consumer and
generic HTTP effects, the Everglade adapter, and the warm-fleet path with disjoint
file ownership agreed before editing. Agents share API contracts and evidence;
they do not stash/reset others' files, run broad gates or deploy experimental
behavior by default. A later record names actual completed milestones and leaves
unqualified operational/quality work explicit.

## First implementation status (2026-10-30)

M0 and M1 are implemented. M2 has a runnable local shadow integration and
actual-Wasm/headless synthetic-edge qualification. The book now has
[21b](../../psionic/book/ch021b-a-decision-arrives-while-play-continues.md) and
[21c](../../psionic/book/ch021c-a-guard-asks-a-pylon.md). See the
[verification record](../verification/pylon-guard-decisions-2026-10-30.md) for
726 selected mutants killed, native/sanitizer/browser scope and measured warm
routing improvement. Independent review corrected both an empty-history
routing regression and same-frame observation ordering before qualification.

M3 now has real signed-in shadow calls, bounded latency/outcome instrumentation
and a fixed context corpus; its usefulness gate remains open (below).
M4 ambient lines remain disabled. Production
availability, remote cancellation and independent-provider failover remain M5;
Belt still follows later. No live-model or production rollout claim follows
from the local synthetic checks.

## M3 measurement result (2026-10-30)

[Chapter 21d](../../psionic/book/ch021d-measure-a-guard-decision.md) and the
[measurement record](../verification/pylon-guard-shadow-2026-10-30.md) deliver
bounded telemetry, Coax-owned evaluation and actual public/cookie evidence.
The warm corpus has 16/16 timely pinned replies, median 5925 ms including CLI
startup/signing. The normal cookie browser pilot admits in 5724 ms; pending blur
revokes safely. The initial 503 is retained separately and remains unexplained.

Every corpus answer was quiet. It has no observed usefulness gain over the free
local quiet baseline, so M4 remains disabled. Measurement/contract mechanics are
qualified; calibration and player benefit are not. Do not tune the request to
these observed labels and present it as held-out improvement.

Next work has two independent tracks: give a new candidate a meaningful bounded
guard purpose and fresh held-out contexts; stage the existing M0 warm-fleet
patch with real cold/catch-up/withdrawal qualification and compare public
latency. The deployed fleet still waits five seconds per call. M5 continues to
own independent providers, broader availability and remote cancellation.
