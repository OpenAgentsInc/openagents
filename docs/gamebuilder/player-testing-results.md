# Live player qualification

Phase 1 is **not yet qualified**. Acceptance requires three clean full runs
for every supported target and account role. Diagnostic passes below do not
replace that matrix. The owner requested a stop on 2026-11-18; both v11
queues were interrupted before their first full report. See the
[handoff](player-testing-handoff.md) for the exact state and restart commands.

The owner approved qualification of a named release while main continues
changing. The candidate is
[phase1-20261117-v11](https://gamebuilder.org/player-releases/phase1-20261117-v11),
with eight entrances: third person, first person, top-down, side-scroller,
medieval village, crates, platformer and racing. Later main deployments,
including drift, require their own qualification. The current source register
retains those newer obligations; it cannot silently certify them with an
older release's evidence.

| Identity | Value |
| --- | --- |
| Client tag | `player-testing/phase1-20261117-v11` |
| Client source | `5d8ec3f` |
| Client OpenAgents pin | `3ff038f301291bf2e6ec4ceea0ed534938d35509` |
| Release manifest SHA-256 | `da8e4db5de774674d4216e6222dd0e4fe3de7bd1048652175fc7549b3a36ccdc` |
| Runner source tag | `player-testing/phase1-20261117-v11-runner8` |
| Runner source | `e6243c9` |
| Runner SHA-256 | `49c2a1aaa47d2db9e9b9a16baa3a353324d6c0da186f37bb38e394af8dcc5b6c` |
| Observer-free runner | Same frozen runner8 executable; v11 visible diagnostic passed |
| Expected service policy | `gamebuilder-accounts-27686bc39a-46685296b2` |
| Native input fixture SHA-256 | `fb043d5426eaa6841ade6342b4eb1a01884917a8c9b93210b9fe5ecab96e8b79` |

The publisher refuses existing names and preserves every failed candidate.
The browser receipts must match the immutable WASM, shader and village-pack
hashes in the served catalog. Login, chat and build jobs use real first-party
production services, whose behavior remains live while client bytes are frozen.
No request interception, cookie installation, controlled clock or gameplay
state assignment is used.

Required targets are headless Chrome desktop (1440 × 900, DPR 1), phone
emulation (412 × 915, DPR 2) and tablet emulation (1024 × 1366, DPR 2).
Touch targets also exercise landscape. The owner approved Chromium emulation
for phase 1; these are not Safari or physical-device results. The environment
is Chrome 157.0.8103.79 on macOS 26.4 (25E246), with a non-fallback Apple
Metal WebGPU adapter. No test activates a foreground browser, Safari or Simulator.

The snapshot has 53 cases and **155 applicable target/case cells**. Account
obligations are checked separately: both fixtures require 53 desktop cells and 51 cells on
each touch target. The complete 18-run cohort therefore requires 930 passing role/case
repetitions, the same runner hash and client manifest, complete recordings,
verified input, hardware GPU and provenance in every report.

Full reports must prove all three crate rooms, the platformer's eight
checkpoints and finish, all three racing laps and restart, as well as every
entrance and the shared account, create, editing, settings, input, layout,
lifecycle, recovery and exploration journeys. Both signed-in accounts must
create a platform through the real service, jump onto it, stand, step off,
edit the selected object through chat while retaining its identity, width,
depth and position, remove it, use Decide and confirm a
bounded build-agent job through the UI.

Production now admits all GameBuilder accounts to chat and build jobs, as
specified in OpenAgents commits
[27686bc39a](https://github.com/OpenAgentsInc/openagents/commit/27686bc39a620d2d505fa7feb41dd07aaa09c6d5)
and [46685296b2](https://github.com/OpenAgentsInc/openagents/commit/46685296b2df1d5cf3623b0b908614544d61f5b2).
Both fixture aliases remain for independent credentials and pacing. Runner6
binds reports to `gamebuilder-accounts-27686bc39a-46685296b2`; the merger
rejects old or missing service-policy contracts. `agent.refusal` now proves
the anonymous UI sign-in gate in a separate fresh profile, at the lane's
touch viewport where applicable. Later main's guest builder is outside this frozen client.

Independent diagnostics have verified:

- Phone and tablet picker pictures and all wrapped card labels, including
  native scrolling to choices below the viewport.
- Real phone and tablet agent creation, standing on the created platform,
  selected-object editing, and three separate Decide turns on v8.
- Each v9 phone/tablet editor run's 15 attempted cells, including direct placement,
  material changes/resets, height and native drag editing, all four shapes,
  both placement modes, every world setting, gravity, double jump, flight,
  orientation, typing focus, visibility loss, frame blur and movement after
  restoration. Provenance, video, hardware GPU and input checks all passed;
  sanitized evidence was uploaded privately.
- Native input conformance for the frozen v9 runner, including covered-target
  rejection, delayed count updates and overshoot, stale/malformed frames,
  dead browser, missing mail, late model response and killed-policy watchdog
  release. Replacing a temporary build-cache executable cannot change the
  frozen runner used by child processes.
- The v9 observer-free create/play repeat: all five attempted cells passed,
  with no diagnostic observer, accepted native movement and independent
  geometry from six PNGs proving appearance, two standing views, stepping off
  and removal. Recording, input, GPU, provenance and private upload passed.
  This scope reuses a legitimate QA session and cannot qualify login or a
  full-matrix repetition.

Earlier failures remain failures. Tests exposed clipped picker text and
overlapping chat/selection controls on phones; those candidate releases were
replaced by new immutable candidates. The v9 product fix prevents wrapped
settings rows shrinking until their reset buttons overlap the next row.
The direct-edit test now uses the shared engine camera and viewport to find
an uncovered face of its selected box, followed by actual native input and
a strict same-object position check. Missing selection coordinates can never
count as motion. Each shape is tested in an independent open build setup.
These diagnostic-assisted gestures are separate from screenshot-only policy
exploration and corroboration.

Two PNG-based observer-free repeats failed the movement-image freshness
guard, including an isolated fresh-login repeat. Their failures remain in
evidence. Supplemental runner `visible3` uses native JPEG captures for those
short movement guards and retains the 300 ms bound, complete video and six
full-resolution PNG checkpoints. Actual live captures took 69–80 ms. The
supplemental runner cannot contribute full-matrix repetitions. The final
full cohort will use one frozen runner consistently.

Runner2's first full ordinary report passed 46/47 cells, failing exploration
when the policy issued three real moves followed by stop despite a playable
world. Its video, input, GPU and provenance passed; the report remains failed.
Runner4 retains the four-action and measured-motion requirements, numbers
each request explicitly, and distinguishes non-modal chat notices from a
blocking screen. An early policy stop is unqualified/inconclusive rather
than an invented pass. Its conformance suite and focused refusal-context
diagnostics precede a new full cohort with one frozen executable hash.

The numbered policy passed three separate actual refusal-context diagnostics,
each with four accepted native actions, measured motion and complete evidence.
Runner5 additionally captures its pre-background image before holding input
and rejects any watchdog intervention through actual visibility loss. Desktop,
phone and tablet lifecycle diagnostics each passed all six attempted cells,
including that causal guard, recovery, frame blur and private upload.

Runner2's entitled desktop report passed 51/52 cells, with all quality flags
and private upload, failing one duplicate at count 927. The attempted press
was 5,018 ms after the near-full notice appeared; the notice's five-second
dismissal shifts the panel. Runner5 waits for that actual transition to
finish and records the button geometry before continuing, retaining the
1,000-object/1,001st-refusal requirement. The incomplete runner4 ordinary run
was stopped when the visibility oracle was strengthened; its sanitized
partial video/logs were archived privately, its owned browser closed and its
temporary profile quarantined outside evidence. It has no qualifying report.

Runner5's ordinary full run failed its obsolete beta-refusal expectation
when production returned a successful edit. Its entitled full run failed
resume when the created block physically obstructed backward movement.
Those reports remain failed. Runner6 requires the new account policy and
tries at most four ordinary movement directions on resume, aborting any
native-input or freshness failure and preserving the world's object count.
A real-agent wall regression measured 19 cm backward (blocked) and then
271 cm forward, with resume and blur passing. The ordinary account's live
create/platform/selected-edit/Decide diagnostic and independent anonymous
sign-in gate passed, with all quality gates and private uploads. Its separate
build-agent diagnostic also passed: no start before yes, exactly one real
202 receipt, visible done status and unchanged world. These diagnostics
retain blocked cells outside their scope and cannot count as full repeats.

The frozen runner passed 592 coordinator checks and 591 checks in each
platformer/racing calibration suite. Native input fault conformance passed.
Main's newer entrance helper waits 84 frames rather than this snapshot's
240; its offline controller test is now aligned with the actual walkthrough's
reset followed by five seconds of settling. Both keyboard and touch pass
that production-start condition. This main-only calibration does not alter
the frozen executable or any live cohort.

Runner6 completed two clean desktop reports (53/53 each) and an ordinary
phone report (51/51). Its entitled phone report passed 48/51, failing material
reset, selected-object chat editing and a broad world-default check; all four
quality gates and private upload passed. That report remains failed, and the
partial cohort is unqualified. Both coordinators stopped before tablet runs.

The material screenshot retained bounce at 0 instead of its default 0.15.
The world panel then listed that same changed material: its own world rows
were reset, so the broad selector counted the earlier material failure again.
A traced repeat recorded a 66 CSS-pixel shift in the first slider's position.
Runner7 waits for an uncovered control centre to stay still for 250 ms
(bounded to 2.5 seconds) before each gesture. Each single reset must then
remove its own changed marker and reset button within two seconds. No failed
press is retried. The final check enumerates the exact registry rows exercised
by that group, retaining any material failure in its own case. A native fixture
moves a target ten times; the waiter then permits exactly one successful press.
A second v10 phone repeat still failed its first material change (14/15
attempted cells, all quality gates and private upload). Its before/after
measurements and retained video locate the 66-pixel move at dismissal of the
five-second "no room there" placement notice. Runner8 waits for an existing
limit notice to actually disappear, bounded to ten seconds, before the stable
geometry wait. A separate timed-notice fixture must wait through that later
layout change before one native press. No failed gesture is retried.

The failed selected-edit observation retained a literal `route: build/block`
in the visible answer and unchanged platform geometry, despite prose claiming
it was taller. The response body was not retained, so its exact formatting
is unknown. A regression reproduces that no-op with leading whitespace and
CRLF around a valid header. V10 trims that formatting and indented plan lines,
retaining the route allowlist: unknown and unrouted commands still cannot edit.
All 144 chat checks pass. Telemetry now records only whitelisted route/verb
enums and counts, leading-whitespace/CR flags and the earlier structural
metadata; these fields cannot grant a gameplay pass. Native screenshots and
read-only objective fields remain distinct from screenshot-only exploration.

Earlier candidates also exposed insufficient recording allowance, native
input acknowledgement delays, pointer-lock assertion timing and build-job
identifier collisions. Fixes retain complete recording/input gates and the
actual first-party job receipt requirement. An occasional earlier HTTP 200
chat reply produced no usable answer; its cause is not asserted as fixed.
The collector now records only sanitized reply structure, lengths, token
counts and whitelisted finish reasons to diagnose any recurrence. Response
text, headers, cookies and service identifiers never enter that artifact,
and telemetry cannot grant a gameplay pass.

The v10 runner8 cohort stopped after its ordinary desktop report passed
52/53 cells: Decide received HTTP 200 with one choice but missing assistant
content, and the UI correctly said no answer came back. Its exact finish
reason and response body were not retained; the sanitized collector classed
the finish as other. All four evidence quality gates passed. The entitled
desktop report passed 53/53. These reports cannot contribute to v11 repeats.

V11 reads the final completion with the shared JSON codec and permits one
bounded app retry only for an empty text completion with no error, refusal,
tool continuation, filtered/truncated finish or unknown finish. Missing/null
finish reasons and recognized successful endings are eligible. The exact
original request is resent while the turn remains busy; history and world
are unchanged until a nonempty answer arrives. Two blanks stop visibly.
Nonempty answers, including no-op answers, are never retried. This does not
assert that the unretained v10 finish was eligible. Unit checks prove the
bound, unchanged bytes, error/refusal handling and exactly one applied edit.
The frozen eight-game source passed all 220 loaded chat/JSON checks; the
main build passed the unsigned iOS and both Mac builds. All three live v11 create preflights passed, each with three independent
Decide edits and all evidence quality gates/private upload; the observer-free
v11 repeat also passed all five attempted cells. The owner stopped the two
full queues before either first desktop report completed. All 18 fresh full
reports remain required.

At the owner's instruction to lift the exhausted test-mailbox limit, narrow
administrative transactions reset only the dedicated QA email/day counters.
Global, hourly and IP policies, auth challenges, codes, sessions and
entitlements were unchanged. Every full run still uses a fresh browser
profile and ordinary UI email-code login, with a single-run account lease
and thirteen-minute code pacing. Diagnostic session reuse uses only a
legitimate UI-created test profile and cannot qualify login coverage.

Evidence remains private under `build/player-tests/` and
`gs://openagentsgemini-gamebuilder-player-evidence/`. Uploads use a sanitized
allowlist with native screenshots and videos; credentials, profiles,
secret-entry captures and raw recording frames are excluded. Passed evidence
expires after seven days; failed and diagnostic evidence after thirty.
The [runbook](player-testing.md) defines reproduction, scopes and acceptance.
No recurring automation has been created.
