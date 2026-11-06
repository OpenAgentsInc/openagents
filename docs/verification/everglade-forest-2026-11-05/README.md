# Authoritative forest qualification

The room advances forest fires, spread and motion once per world step. Its
ordinary entity replication carries occupied forest records to every seat.
Room pages apply that state and render fire without advancing tree damage
or spread. Session profile v5 preserves the published v4 contract.

The new room-tick regression failed before the authoritative forest step
was added (`server-red.txt`). The passing checks cover spread, burn-down
and attribution to the fire's starter rather than the most recent caster.
The wire fixture checks every field independently. Room checks cover late
joiners, blocked starters, a full forest snapshot with a lost multipart
frame, rebuild and record reuse. Page checks cover delayed state, disabled
local simulation, removal and reuse across sparse records.

`shared-fire.txt` records two headless pages in one native room at 250 ms
RTT, 30 ms jitter and 2% simulated read loss. A real Produce Flame command
ignited tree 1783. Both pages observed fire, then one burnt tree, no active
fire or falling trunk, and settled digest 897021801 on three successive
polls. Raw screenshots and page diagnostics remain in
`/tmp/oa-12940-current-forest250/`. This final rehearsal uses the current
artifacts and merged browser host. The previous sampler receipt is
`pre-movement-shared-fire.txt`.

## Input coverage and resend recovery

The rehearsal exposed a driver error: synthetic keyboard events targeted
`Window`, so the accessibility listener's `Element.contains` threw before
forwarding the key. `input-event.txt` checks that failure and the corrected
canvas target in headless Chrome. The drift comparison now also requires
at least 3 m of movement on each page before the scripted hold.

The first corrected full 250 ms run passed (`250-before-resend-pass.txt`).
The second recorded an 889 mm correction during Reverse Gravity
(`250-correction-red.txt`). A separate
run also drifted in position and townsfolk conditions. A deterministic
room test then reproduced input loss when a 640 ms TCP resend delivered
20 legitimate movement inputs together. The former queue held 16, and
clock credit retained only 500 ms. The queue now holds 32 and credit
retains one second. Credit is still earned by the server clock; each tick
spends at most four steps, and the four-step excess allowance is unchanged.
The exhausted-credit and fast-clock checks remain enforced.

The earlier #12941 receipts correctly exercised autonomous NPC timing and
raids, but their ignored keys did not establish movement or keyboard
spell coverage. The corrected-input qualification must be used for those
claims.

The first three post-recovery 250 ms runs passed, as did local and 100 ms
profiles (`pre-sampling-*`). The later tree sampler change preserves the
exact shared pose-buffer listing and samples tilt and fire age separately.
Its regression failed with missing auxiliary sampling, then passed before,
between and after state rows. All 211 source-linked listings pass without
book edits. Earlier binaries remain in `/tmp/oa-12940-resend-artifacts/`.

Two runs of the final sampler passed, but a third passed gameplay and
failed the raid's hotbar observation (`250-raid-observer-red.txt`). The
observer assumed a coherent LEVEL 1 kit after a fixed delay and could
also choose a timestamp before LEVEL 1 when LEVEL 2 followed quickly.
`raid-window-red.txt` reproduces the invalid timestamp. The driver now
requires a stable, observed LEVEL 1 kit before combat, bounds the sample
to that interval, and saves the level and slot traces. The original
failure did not retain those traces, so its precise cause is not inferred.
New receipts establish an eight-slot LEVEL 1 kit and a nine-slot LEVEL 2
kit, with Barkskin added. No gameplay or drift tolerance was raised.

Three consecutive full 250 ms runs passed every gameplay and raid
check (`pre-clearance-250-run1.txt` through `pre-clearance-250-run3.txt`). Worker tick p95 was 385, 375
and 360 microseconds against the unchanged 11,000-microsecond limit.
All three report zero offline stalls, zero dropped milliseconds, no
large corrections and no credit drops. The final room's slot and level
traces are retained in `raid-250-run*-room.json`. Those earlier raw browser records
remain under `/tmp/oa-12940-kit250-1/` through
`/tmp/oa-12940-kit250-3/`.

## Rehearsal geometry and hold timing

The subsequent local run failed on townsfolk conditions
(`local-condition-red.txt`). Recording actual background-player positions
found that the old 32 m orbit specified goals, without ensuring clearance:
a bot came within 2.55 m (`bot-orbit-positions.txt`). Background players
now move on the empty south road. The rehearsal waits for all actual gaps
to reach 32 m and fails if any becomes smaller or unknown. This keeps
additional active sessions under load; it does not qualify visible remote
body rendering, which has its separate #12946 pair receipt.

A run with failed bot startup also showed the condition mismatch. That run
does not qualify multiplayer load. It helped rule out bots as the only
cause. The original cast position let a townsperson's patrol cross Reverse
Gravity's radius. Moving the cast only slightly still failed the new
12 m placement check (`gravity-margin-red.txt`). The script now moves to
the open road near the patrol's center, verifies its placement on both
pages, and requires a townsfolk condition before the later Hold Person.
The centered local run passed the condition comparison, then exposed
movement across independently delivered hold expirations
(`held-release-position-red.txt`). The movement attempt now stays inside
the hold and ends before release; both releases precede the next step.

The negative route and hold-window checks are retained. The driver passed
136 checks and the background-player helper passes 233. These changes
preserve the existing condition tolerance, timing windows,
position bounds and correction limits. The page and room binaries are
unchanged from the qualification below. Final rehearsals with the added
positive placement, fall and clearance checks are recorded separately.

The complete local rehearsal passed at 385 microseconds worker tick p95,
with the eight-to-nine-slot raid transition. The following 100 ms run
passed the raid but failed the aggregate townsfolk-condition count
(`condition-visibility-100-red.txt`). Guard 34 left the offline page's
replicated view 3.4 seconds before leaving the native-room page's view.
The other observed condition bits matched. The two row masks and per-person
bits are retained in `condition-visibility-*-red.txt`.

The condition probe now compares exact bits for the same person while both
pages have that person's row, within the existing wall-clock delivery
window. An absent row is not a condition cleared. Missing or truncated
observations fail, and at least one matching conditioned person must be
observed. A regression fails with the old count comparison, accepts the
different views, and rejects changed or swapped conditions even when totals
agree. Observations stop at the captured frame boundary. All 137 driver
checks pass, including the capture boundary and positive-coverage cases.
This qualifies conditions on shared entities; it does not establish identical
interest sets or identical WANTED decay times in the two worlds.

## Startup and scheduled observations

The first subsequent 250 ms attempt failed the positive background-player
check: its bots connected before the native room accepted connections
(`startup-250-red.txt`, `startup-bots-red.txt`). It is not a qualifying run.
The driver now waits for a successful `/ready` response before starting
background players. A gameplay failure skips the raid and still closes
the processes it owns.

The following 250 ms gameplay run passed, but raid setup failed before
combat (`raid-setup-red.txt`). That receipt does not identify the failed
readiness stage, so its exact cause is not claimed. Inspection found
another observation hole: an end mark was scheduled a few input steps
ahead, but a fixed 300 ms wait did not guarantee it had appeared on both
pages. A missing mark produced a zero full-bar baseline and a silent
refusal. `end-mark-red.txt` reproduces that invalid baseline. The driver
now observes both end marks, refuses a missing baseline explicitly and
records each raid readiness stage. All 138 driver checks pass.

## Concurrent shared movement update

Commit `599558ee03` adds the Valley locomotion option to shared movement
types and code. The page gameplay and room suite source lists have no
intersection with that commit's changed files. Their earlier 782/670
results are retained. The physical page render unit and room artifacts do
load shared movement, so they were rebuilt. The updated movement unit
passes all 317 checks; the native room's startup check reports zero fields
off their defaults. The concurrent authority mutation results are retained
in `docs/verification/valley-lab-echo-2026-11-05/` rather than repeated.

The Aethyr listing gate now stops at chapter 9's locomotion profile,
changed by that concurrent commit (`incoming-listing-drift.txt`). The
earlier 211-listing pass qualifies the source before that update. Book
files remain untouched under the paused book scope.

The Mac page build was stopped before its 15-minute limit when the shared
worker pool admitted only one worker. The remote rebuild at nice 15 used a
provenance-verified Coax 0.51.2 cache and finished in 757.7 seconds. Its
physical render-unit gate ran 1,377 checks with no failures. The phase
report is retained in `page-build-current-timings.json`; 741.3 seconds were
authority checks, including 555.5 seconds collecting verdicts. The
redundant local resume was stopped once that artifact was available.
No compiler budget was raised. The native room and worker use the checked
Mac compiler named in `artifacts.sha256`. Both compilers and the exact
product-source bundle are retained with the binaries.

## Complete patrol containment

Two simultaneous browser-pair rehearsals on the rebuilt artifacts failed
(`south-patrol-250-red.txt`, `short-move-local-red.txt`). Key timestamps
against the recorded input sequence show that the short backward hold
lasted 13 steps on one page and 15 on the other. Ordinary frames can take
more than one input step; the driver cannot inject a DOM event midway
through that frame. Final browser-pair qualifications therefore run
sequentially.

The condition failure also exposed a geometry error: Tilda walks north to
z = -45 m, beyond the southern cast's cylinder. The earlier placement
check covered Bram's current point. The new failing regression covers
both entire patrols. The script now stops at the closed gate's west edge
and omits the backward move. Its preflight checks both guards' observed
positions and all seven authored patrol endpoints, leaving at least
0.5 m inside the 15.24 m radius. Straight patrol legs remain inside that
circle because it is convex. No condition, position, correction or
delivery-window allowance changed. All 139 driver checks pass.

The final three consecutive 250 ms runs use the rebuilt artifacts and this
complete-patrol preflight. Each passes all 32 gameplay and raid checks.
Worker tick p95 is 720, 840 and 970 microseconds. Each reports zero offline
stalls, zero dropped milliseconds and zero credit drops; the correction
checks pass. Raw records remain in `/tmp/oa-12940-patrol250-1/` through
`/tmp/oa-12940-patrol250-3/`. The final local profile also passes all 32
checks, at 785 microseconds worker tick p95, with no offline stalls or
dropped milliseconds. Its raw records are in `/tmp/oa-12940-patrol-local/`.
The final 100 ms profile passes all 32 checks at 615 microseconds worker
tick p95, with zero offline stalls, dropped milliseconds and credit drops.
Its raw records are in `/tmp/oa-12940-patrol-100/`. Both profiles observe
the eight-slot LEVEL 1 kit before combat and the nine-slot LEVEL 2 kit
afterward. `input-credit.txt` retains all five credit-drop readings.

The final rebase onto `021c7e7c8a` adds the other agents' renderer and
development body-replacement integration. It changes none of the
Everglade Coax simulation sources or the forest bridge. Ordinary native
builds leave the new development hooks disabled; the browser's new blend
selection is opt-in through an export this page does not provide. The
shared forest rehearsal below exercises the merged browser host.

## Evidence scope

The full page suite passed 782/782 with compiler SHA-256 `c3928a30bcdc…`.
The final room suite passed 670/670. The input policy killed all eight
mutants, with zero survivors (`input-policy-mutation.txt`).
The original forest policy mutation result remains valid for the unchanged
source hash in
`artifacts.sha256`: 103 killed, zero survivors. Logs retain exact counts
and artifact sizes.

The current page is 2,019,949 bytes against 2,150,000; the current room
worker is 998,983 against 1,060,000. Their successful builds enforce those
unchanged budgets. `tools-check` passes with 1,827 tracked tool files and
no new file missing a reason. Current binaries and their manifest are in
`/tmp/oa-12940-qualified-artifacts/`; the earlier sampler artifacts are
preserved in `/tmp/oa-12940-pre-movement-artifacts/`.

Mac checks ran through the shared build gate; the physical page rebuild
ran on coderos-4080 at nice 15. Browsers were headless and muted. No
production deployment was performed.
