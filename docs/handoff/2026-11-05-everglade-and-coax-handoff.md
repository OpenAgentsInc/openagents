# Handoff: Everglade multiplayer and Coax compiler work (2026-11-05)

This handoff covers the state at about 17:00 Chicago on 2026-11-05. It is for
the next agent taking over the issue burn-down. Read it with `AGENTS.md`,
`INVARIANTS.md` and `/Users/christopherdavid/work/NEEDS_OWNER.md`.

## The goal

The owner's goal: resolve every open issue in `OpenAgentsInc/openagents`
except #12864, #12865, #12866 and #12867. Those four are left for the
owner's Codex agent.

Since 15:45 there's no cap on agents: every issue an agent files gets its own
agent right away. At 16:55 the owner said to start no new agents, so the
agents below are the last ones started.

## Agents still running at handoff

None. Every agent started this session has finished.

## Production state (most important)

Everything is live as of about 18:15 Chicago on 2026-11-05 (#12936, closed):

- **API:** `oa-api-00107-l9z`, build `54575b412a`, migrations through
  `0135_character_checkpoints_v2.sql`.
- **Rooms:** all six shards run `6414ba56c4`. With 16 players a room they ran
  at 30 updates a second, with bytes 1.06–1.16 times the old build's and 16 of
  16 seats opening. The regression was a fixed work budget per room on a
  1-vCPU instance; `b17aaf5cd1` shares a pool among the live rooms.
  `deploy.sh` now moves traffic to each new revision (`13e0cb0376`).
- **Web:** release `7a679e651e`. Roll back with `coax/web/rollback.sh
  20261106T005750Z`.
- **Live checks:** `/worlds/everglade` seats a player, SETTINGS shows, and the
  pair run passes except for two checks filed as #12946: Reverse Gravity's
  second press, and BLOCK.
- **Owner generations:** `OA_ROOM_OWNER_GEN=1` can be set a day after this
  release, from 2026-11-06 at 18:15 Chicago (see `NEEDS_OWNER.md`).
- **Not authorized:** the relay fix (#12909), a second region and other
  database changes.

## The owner's local playtest stack (don't touch)

- Worktree: `/Users/christopherdavid/work/openagents-ownerroom`, detached at
  `f74da763f9`.
- Room: `coax/web/room/bin/room --port 8843 --dev
  /private/tmp/claude-501/-Users-christopherdavid-work-openagents/7212f8f4-e1c3-408c-b1be-66dbf716b6be/scratchpad/ownerplay`.
- Page: `coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:8876 --api
  8843 --static --cache 0`. Open http://127.0.0.1:8876/worlds/everglade.
- Audio Lab: the same serve command on 8913, with no `--api`.
- Agents must never use ports 8830, 8843, 8872, 8876 or 8913, and must never
  edit or build in that worktree.

To rebuild it, after an agent lands a fix the owner wants to test:

```sh
cd /Users/christopherdavid/work/openagents-ownerroom
git fetch -q && git checkout -q --detach origin/main
G=/private/tmp/claude-501/-Users-christopherdavid-work-openagents/7212f8f4-e1c3-408c-b1be-66dbf716b6be/scratchpad/build-gate.sh
$G coax/web/tool.sh coax/web/room/worker.coax apps/openagents-web/build/everglade-room.wasm   # run from the repo root
$G coax/web/room/build.sh
cd apps/openagents-web && m=../../verse && $G ../../coax/web/wasm.sh everglade5e.coax everglade5e-face.c build/everglade5e.wasm everglade-step.coax $m/grid.coax $m/everglade/river.coax $m/everglade/guard-guide-policy.coax everglade5e-rules.coax
```

Then stop the room and page processes by their exact PIDs and start them
again with the commands above. `serve.sh` rebuilds `coax/web/build/serve`
when `serve.c` is newer.

## What landed on 2026-11-05 (highlights)

The owner's playtest bugs, all closed:

- #12928 (`befc8fa3c2`): the local serve proxy now carries room WebSockets,
  and dev tickets name `everglade-1`. Before this, the page never sat in the
  local room and fell back to offline play.
- #12929 (`cd16f2a52f`): Repair tells you why when it can't start or stops.
- #12930 (`9201cc27ba`): the Thunderwave, Burning Hands and Cone of Cold
  cubes include the caster's own row.
- #12931 (`fc187ca626`): shouts expire, clear when the speaker calms down,
  falls or leaves, and avoid name labels. Two neighbouring guards' shouts
  overlapping each other isn't covered by a test yet.
- #12932 (`f74da763f9`): the gate leaves burn, fall and can be repaired.

Fire blame: #12920 (`11e703434b`) and #12923 (`f9c2037818`). Each fire
remembers who started it, and raid fires blame no one. Room fire bugs, closed
after the handoff was first written: #12933 (`2c5936ba70`, fires no longer step
once per caster), #12934 (`0db7bc8ead`, fire damage reaches the rules, as
bludgeoning for now) and #12935 (`22308dd321`, tree spread charges the
starter).

Goldens and builds (#12918, #12922): `golden-check.sh --since 5c6db93d2f
--ref origin/main` exits 0. Nine worlds still have no golden. They're reported
as NEW REALM, not failures.

Aethyr (#12856 closed; #12845 partly done):

- the room side of the lifecycle contract;
- tick budgets per host (native room 0.84 ms, browser worker 0.6 ms);
- the party contract in `aethyr/session/party.coax`;
- the realm profiles in `aethyr/net/profile.coax`.

Compiler:

- #12924, read-only views, closed by Codex at 21:31 UTC.
- #12927 (`a059472dfb`, `d5e28e528f`): `add_checked`, `add_sat` and
  `add_wrap` (with sub and mul versions) on every integer width, the same in
  debug, release, the evaluator and native code. Fields marked `#counter`
  refuse plain arithmetic (E0840). The room and ledger counters use them.
- #12925: build timings, a cache for each step, and two emitter fixes. A
  gameplay edit went from 56 s to 44 s, and a test-name edit from 46 s to 6 s.
- #12938 (`85cfc9d1b1`, `2ac55b62bc`, `9215df3ad4`): big render units are
  emitted as up to 16 cached C parts, and the release link runs in 8
  partitions. A gameplay edit to everglade5e takes about 20 s in release and
  11 s in dev on coderos-4080. About 9 s of that is still Coax to C, mostly
  authority checks.
- #12926: capacity refusals name the limit, its top users and a fix. The
  traversal limit can now be raised with `#traversal N` or `--traversal N`.
- #12937 and #12939 (`199402193d`): the job pool's load notices print only
  to a terminal, so test output no longer depends on machine load.
- The order of work is recorded in
  `coax/audits/2026-11-05-mmorpg-language-and-compiler.md`.

Books: #12610 stays open and paused. Approved books work is #12947 and the runnable Coax section in #12948.

## Open issues and who owns them

| Issue | Owner or status |
| --- | --- |
| #12944 | Closed by `6414ba56c4`. The room server fails to link on main, with 21 duplicate `cb_*` symbols, since `010c25bbed` (Codex lane). This blocks room builds, the API room checks, the drift check and the #12936 release. The #12936 agent knows. |
| #12945 | Closed by `c8319bb8ac`. The cache oracle uses the current staging signature, split C includes integer helpers, and reviewed evidence bindings are current. All 1,358 cache-policy checks, 209 room-rules checks, 211 Aethyr listings and the default split page build pass. No traversal budget was raised. |
| #12946 | Closed by `e00925f958`. The smoke counted doors and boats as gravity cylinders, rejected a valid landing on the 4.5 m palisade, and counted the invited visitor as the blocked player. It now observes each spell's kind and caster, grounded landings, and the blocked seat. Verification is below. |
| #12941 | Closed by `8ba4376dc1`. A delayed world header dated server ticks at receipt, leaving buffered states ahead after clock synchronization. It now recovers the header's authored time from its server day. The original drift receipts qualify autonomous NPC timing and raids; corrected-input movement and spell qualification is under #12940. All 773 page checks pass. Verification is below. |
| #12940 | Closed by `cb8aa439fa`, with resend recovery in `0a660b7d7f`. The room steps and replicates forest fire, spread and motion; pages apply that state. The room regression and two-page burn-down pass. Corrected-input local, 100 ms and three consecutive 250 ms gameplay/raid runs pass. Verification is below. |
| #12947 | Completed: current-code overviews for Aethyr 49, 50, 58, 84 and 85; contradictory follow-up status text corrected; Coax 6, 19 and 41 updated. Verification below. |
| #12948 | Completed by `0265cd16df` and `d53cc8a8b9`: nominal integer IDs, engine/room/API/ledger adoption, generation admission, chapter 8 and qualified seeds. The books, room/API checks, drift and Wasm budgets pass. Verification below. |
| #12956 | Completed by `d53cc8a8b9`: canonical entity handles and checked ID exhaustion; no slot is allocated on refusal. |
| #12957 | Completed by `9ceca725fa`: the raid client and API fault fixture share seeded character identities; the native save-fault slice passes. |
| #12958 | Completed by `3b5548bf98`: seed comparison uses stage0; the native math oracle uses a final compiler. Mac and Linux pass. |
| #12951 | Completed by `b08d3a16a4`: cold, warm and check-level reuse preserve near-limit diagnostics in Coax 0.51.1. Verification below. |
| #12961 | Completed by `eae6049b87`: Coax 0.51.2 measures peaks against each phase's limits. Retained live storage still counts; sequential, parallel and reused notes agree. Verification below. |
| #12960 | Completed: a real shared door-rule edit reaches the native room and page in 899.5 ms median over ten runs on the merged host. Both replaced and full builds pass all three drift profiles with raids; failed checks appear in the session and block the guarded commit. Verification below. |
| #12838, #12840–#12843, #12868, #12869 | The owner's Codex agent. Don't take them. Its room changes share files with ours, so use small commits and rebase often. |
| #12845 | Next for this burn-down agent after #12940. It still needs commands, accepted results and replication behind shared interfaces, profiles for collision and per-recipient fields, Orbit League sharing more than the timing profile, and an adoption inventory. Book edits remain outside this run's scope. |
| #12910 | Nostr chapter 104 on signet. Blocked on the signet faucet's captcha, which is in NEEDS_OWNER.md. |
| #12718, #12833 | Epics. Close each when its children close. |
| #12610 | The book epic, paused by the owner. |
| #12864–#12867 | Excluded; left for Codex. |

## #12940 verification (2026-11-06, 01:52 Chicago)

The new room-tick test fails before the fix, then burns down and spreads a
tree as the original starter's crime, independently of the latest caster.
Session profile v5 carries whole finite forest records through ordinary
entity replication. Published v4 is unchanged. Room pages render the state
without advancing tree damage or spread. Tests cover lost multipart
snapshots, late joiners, blocked starters, rebuild and sparse-record reuse.

The page gameplay suite passes 782 checks and the room suite passes 670.
The current physical page build passes 1,377 render-unit checks; the shared
movement update passes 317. Forest policy kills 103 mutants and input
policy kills eight, both with zero survivors. The rehearsal driver passes
139 checks and its background-player helper passes 233.

The corrected keyboard driver requires real movement, observed spell
conditions and 32 m clearance from all three background players. Its
condition comparison matches the same person while both pages have that
row; it does not assert identical interest sets. All 32 gameplay and raid
checks pass at local and 100 ms, and in three consecutive 250 ms runs.
Worker tick p95 is 785, 615, 720, 840 and 970 microseconds respectively,
below the unchanged 11,000-microsecond limit. All five report zero offline
stalls, dropped milliseconds and credit drops. The raid observes eight
LEVEL 1 slots before combat and nine LEVEL 2 slots afterward.

On the final merged browser host, two pages at 250 ms RTT, 30 ms jitter
and 2% simulated read loss both see fire, then one burnt tree and settled
digest 897021801 on three successive observations. The page is 2,019,949
bytes against 2,150,000; the worker is 998,983 against 1,060,000. Native
startup reports zero fields off their defaults. `tools-check` passes.

The concurrent `599558ee03` locomotion change leaves a stale chapter 9
source listing; the current Aethyr structure check reports that failure.
The earlier 211-listing pass covers the source before that change. Book
files remain untouched under the paused scope. Receipts distinguish these
versions and retain the failed rehearsals that led to the driver fixes.

Evidence and source/artifact hashes are in
[the forest verification record](../verification/everglade-forest-2026-11-05/README.md).
Binaries and compilers remain in `/tmp/oa-12940-qualified-artifacts/`.
The remote build worktree is removed. No production deployment ran.

## #12941 verification (2026-11-05, 21:09 Chicago)

Before the fix, the third of three 250 ms runs failed at normal load:
townsfolk walked in place on 24% of walking frames, versus 0% offline and
the unchanged 15% allowance. The condition and raid checks passed. A new
delayed-header check then failed against the receipt-time tick anchor.
That same anchor and ping correction are present in reference commit
`3582fac170`; this is a pre-existing timing error, not a fire-code change.

`8ba4376dc1` dates a world header from its existing server-day field,
choosing the nearest 48-minute cycle around the synchronized clock. It
preserves receipt-time handling for headers without a valid server day.
The checks cover 0, 125 and 750 ms of delivery delay, both directions across
the day boundary, and a later ping correction. No wire format, interpolation
bound or drift tolerance changed.

The full page suite passed 773/773 at the default traversal limit; both
strengthened clock checks also passed separately. Three fresh, consecutive
250 ms runs passed every gameplay and raid check. Their worker tick p95
values were 395, 345 and 375 microseconds, against the existing 11,000
microsecond budget. Local and 100 ms profiles also passed with raids. All
five runs reported zero offline stalls and zero dropped milliseconds.
Recorded load readings during the three 250 ms runs were about 2–9.

Coverage correction from #12940: the old synthetic keys targeted `Window`.
The accessibility listener threw before forwarding them. These original
receipts establish autonomous NPC timing and raids, but not movement or
keyboard spells. The corrected-input rehearsals and positive movement
checks under #12940 supersede those coverage claims. The delayed-header
regression and its clock fix remain valid.

The page Wasm is 1,990,798 bytes, within its 2,150,000-byte budget. The
native room and 988,275-byte worker retain the unchanged simulation and
qualification from #12945; the page's clock reading is the only runtime
change. `tools-check` passed. Durable receipts are in
`docs/verification/everglade-drift-2026-11-05/`; each profile receipt names
its raw browser-record directory under `/tmp`. Qualified binaries and hashes are retained in
`/tmp/oa-12941-artifacts/`. No production deployment was performed.

## #12947 verification (2026-11-05)

The Aethyr structure check passes with 211 source-linked listings, including
chapter 89. All 48 runnable programs pass in debug and release (96 runs).
The full `coax/book/check-examples.sh` passes, including checks, mutation,
native hosts and structure. `book_claims.py` passes 10 tests and
`book_registry.py` passes 11. Chapter 3's overflow and counter examples were
already registered; chapter 6's read-only examples were already present and
pass both modes. Chapter 41 adds a checked traversal-budget example.

The Wasm cache oracle passes cold/warm reuse, changed inputs, split parts and
uncached byte parity. Small actual Wasm builds, phase/declaration reports,
split output, a traversal refusal and a near-limit note were recorded while
writing chapter 41. The initial measurements use checked compiler fingerprint `037278f2a55d`.
After rebasing onto the concurrent compiler fix, the full Coax suite and
all 96 Aethyr runs pass with `b8802e5b501a`; the latter uses a fresh
`COAX_CHECK_CACHE` so a reused verdict cannot hide a diagnostic. No new
whole-app performance claim is made. Tools-check and
whitespace checks pass. No deployment or owner action is needed.

Two prerequisites were filed and resolved: #12949 (reviewed evidence hashes;
concurrent `c8319bb8ac` also refreshes them) and #12950 (authority-check
headroom). Commit `80ec7cabbc` records the review and explicit budgets.
A subsequent cold-cache run also exposed near-limit checks in forge/doc.coax
and scatter-hosts.coax; both now have explicit measured headroom. Property
domains and assertions are unchanged. #12951 tracks the separate cache
diagnostic-parity bug that a warm pass initially masked. `aethyr/book/examples.json`
keeps its existing 48 programs: the rewritten overviews link to existing
implementation chapters rather than inventing duplicate examples.

## #12946 verification (2026-11-05, 20:05 Chicago)

The page suite passed all 770 checks. The added cylinder regression also
passed; keyboard, ticket restoration and the actual remote-body renderer
checks passed. The headless local pair passed, including the BLOCK click
with an unrelated visitor still present. The local API does not store
blocks, so reload persistence was checked through the native ticket path.
The complete drift run held at local, 100 ms and 250 ms, including raids.
Its old keyboard driver had the coverage limitation recorded under #12941;
use #12940's corrected-input receipts for movement and keyboard spells.
The trusted-CDP pair and BLOCK checks above are unaffected.
The page was 2,021,551 bytes against 2,150,000; the room worker was 988,275
against 1,060,000. `tools-check` passed.

The default split page build exposed a separate missing integer-runtime
prelude in generated C. This verified page used `COAXIAL_SPLIT=0`; #12945
subsequently fixed and verified the default build. No production
deployment was performed. Local logs are `/tmp/oa-12946-*.log`, with drift
receipts in `/tmp/oa-12946-drift/`.

## #12945 verification (2026-11-05, 20:19 Chicago)

The obsolete three-argument staging oracle failed with E0316/E0317 before
the fix; all 1,358 checks now pass. The split-cache fixture reproduced an
undeclared `coax_add_sat_s32` before the emitter fix and now passes, including
cached and uncached byte comparisons. The five integer oracle tests pass
for checked/release evaluation, native C, ASan/UBSan and actual Wasm. The
two spatial fixture checks pass.

The rebuilt compiler passed both bootstrap stages, seed/final reproduction
and provenance verification with `COAX_MUTATION=0`, the release build path.
Compiler policy bodies did not change; this is not a new full mutation
qualification. Its source fingerprint is
`b8802e5b501a95f0c5f682aff916b73e3c0c48d2b588b3fdc496b41632357302`.

Chapter 89 was already reconciled by `625a21c253`. The current structural
gate initially stopped on stale evidence bindings. The graphics status
update was reviewed against its named product-snapshot receipt, and the
spatial fixture change removes only the obsolete load-notice workaround.
The existing language/graphics claim scopes are preserved; the Aethyr
structure gate now passes all 211 listings. All 209 room-rules checks pass
without traversal overrides. The unchanged page-suite traversal result is
the 770/770 receipt under #12946.

The default split page, including the incoming action-packet capacity
export, builds at 1,990,711 bytes against 2,150,000. Logs are
`/tmp/oa-12945-*.log`. The native room's startup check passed and the rebuilt
worker is 988,275 bytes against 1,060,000. The incoming rebase changed none
of their Coax source closures. Runtime artifacts and checksums are retained
in `/tmp/oa-12945-artifacts/` for #12941. `tools-check` passed. No production
deployment was performed.

## Owner rules that still apply

- **Sign-offs:** none from the owner. Agents run the checks, rehearsals and
  measurements themselves and close the issue. Only production deploys wait
  for the owner, and the owner approved today's.
- **Main checkout:** never edit or build in
  `/Users/christopherdavid/work/openagents`. Use your own worktree from
  `origin/main`, push with `git pull -q --rebase origin main && git push -q
  origin HEAD:main`, and remove the worktree when done. Stage only your own
  files. Never stash or reset another agent's work.
- **Commits:** plain messages, ending with `Co-Authored-By: Claude Opus 5.5
  <noreply@anthropic.com>`.
- **Builds:** heavy ones go through the build gate (four slots, nice 5, 6
  check jobs). coderos-4080 has 28 cores and can take up to 4 heavy builds at
  nice 15. Leave its GPU models alone. Run independent work in parallel.
- **Processes:** kill only the exact PIDs you started. Never `pkill` by name.
  Don't leave `until pgrep` or `until grep` loops behind.
- **Browsers:** headless only. `serve.sh` only with `--static`. Revert any
  changelog churn.
- **Production:** no production database, secret or bucket access outside a
  deploy script. For gcloud, use
  `CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config`.
  Total GCP spend stays under $1,500 a month. Never print secrets.
- **Out of scope:** the Psionic repo is reference only, and Psionic and books
  work is paused. Bitcoin Lab, Granite, TwIL and One-Shot are out of scope.
- **UI and docs:** no system-internal words, walls of prose or dead controls
  in the UI. Docs follow Google developer style. Show times in Chicago time.
- **NEEDS_OWNER.md:** lives in the workspace repo (`/Users/christopherdavid/work`,
  `AtlantisPleb/workspace`). Commit and push only that file.

## Machine state

- **Disk:** about 170 GB free after cleanup at about 16:30.
  - The owner's recent Downloads and all document files are in
    `~/Desktop/Downloads-recent`.
  - Every iOS simulator is deleted. The next iOS run creates a fresh one.
  - `~/work/film-out` keeps `reviews`, `everglade-mayor`, `promo`, `clips`
    and the Episode 290 cuts.
  - The owner is downloading Valley of the Ancient (about 205 GB) in the
    Epic launcher, so free space will shrink.
- **Build gate:**
  `/private/tmp/claude-501/-Users-christopherdavid-work-openagents/7212f8f4-e1c3-408c-b1be-66dbf716b6be/scratchpad/build-gate.sh`.
  It lives in that session's scratchpad. Copy it somewhere permanent if that
  session's temp directory is cleaned up.

## #12948 verification (2026-11-05)

Coax 0.51.0 adds `struct @id` and opt-in `@id(ordered)`. IDs compare only
within their declared domain. Construction and integer extraction are
explicit. Generic keys and read-only views preserve the type; named integer
arithmetic and codecs consume `.value`. C headers expose distinct records
and include ordering in a new compatibility schema. Device output refuses
E0962. Aethyr keeps liveness and generation admission; entity IDs remain
session-local.

The compiler and refreshed seeds are in `0265cd16df`; `bb5fc45978` records
the introducing release hash. `d53cc8a8b9` adopts seat, entity, generation,
character and item domains across the engine, room, API and ledger. Its
entity changes also resolve #12956. `9ceca725fa` resolves the stale seeded
raid fixture (#12957); `3b5548bf98` repairs the portability oracle (#12958).

The full parser and checker mutation gates kill 1,835 and 12,868 mutants.
The normal full-mutation bootstrap passes both stages and fixed points;
its installed provenance verifies and its survivor file is empty. The
qualified compiler passes six conformance methods and 19 release tests.
Both books pass, including all 96 Aethyr runs and chapter 89's listing.
The claims and registry tests pass. Entity, control, travel, ledger and
checkpoint mutation gates have zero survivors. Mac and Linux seed
comparison and the independent math oracle pass.

The room suite passes 662 checks. The sanitized API and real room cover all
15 room selectors: 203 assertions across bounded slices. The failed
save-fault slice is replaced by its complete passing receipt after the
shared identity fix; passing non-fault sections are not repeated. Drift
holds at local, 100 ms and 250 ms latency, including raids. Page Wasm is
1,997,831 of 2,150,000 bytes; worker Wasm is 988,180 of 1,060,000 bytes.
Tools-check passes. No deployment or owner sign-off runs.

The commands, scope, compiler fingerprint and raw receipts are in
[the verification record](../verification/nominal-ids-2026-11-05/checks.txt).
At completion of #12948, #12951 remains open as the separate cache
diagnostic-parity bug. Its later resolution is recorded below.


## #12951 verification (2026-11-05)

Coax 0.51.1 preserves capacity facts in whole-program authority verdicts
and renders them in the requested diagnostic format on reuse. The private
record uses a new schema and is parsed completely before publication.
The near-limit threshold participates in cache identity. Per-check reuse
keeps quiet passes; a diagnostic-bearing check runs in the parent to
refresh its detailed consumers and current source locations. Retained
storage still triggers the sequential fallback when necessary.

`b08d3a16a4` implements the fix, regressions, cache invariant and chapter 41
explanation. `1d8c1818a5` records the introducing release commit. The
qualified compiler passes all 21 capacity regressions; the same suite
passes on Linux GCC. The six new cases also pass under ASan/UBSan.
Authority-cache, check-reuse, sequential/parallel parity, traversal and
JSON diagnostic suites pass. The full-mutation bootstrap completes with
an empty survivor file, and installed provenance verifies.

The checks preserve visible near-limit notes and do not change budgets.
The books, release metadata, tools and registry checks are recorded in
[the verification record](../verification/cache-notes-2026-11-05/checks.txt).
Testing finds #12961, the separate phase-peak accounting bug listed above.
No deployment or owner sign-off runs.


## #12961 verification (2026-11-05)

`eae6049b87` resets evaluator storage measurements at phase boundaries in
Coax 0.51.2; `f8e5e55a89` records its release hash. Each new phase starts
with its retained live slots, containers and string bytes. Earlier temporary
peaks, holder snapshots and refusal snapshots are cleared. Retained values
keep their allocation locations, and capacity facts already collected keep
their original governing limits. Returning from authority checks restores
both compile limits and measurements, including after failures.

The regressions reject the old false note, retain real compile and check
notes, and still refuse a new table when it exceeds the budget together
with live constants. One and multiple workers, whole-cache reuse, edited
check reuse and direct test execution agree. All 23 capacity methods pass
on Mac Clang and Linux GCC. The 33 work/storage methods pass, including
the actual allocator under ASan/UBSan and passing/failing limit restoration.
Legacy cache, parallel and JSON diagnostic checks pass. The normal
full-mutation bootstrap passes, installed provenance verifies, and the
survivor file is empty. All 23 capacity methods also pass on its qualified
compiler; release metadata and 19 release tests pass. Both books pass,
including all 96 Aethyr runs and 211 source-linked listings. The claims,
registry and tools checks pass.

Commands, compiler qualification and book checks are in
[the verification record](../verification/phase-peaks-2026-11-05/checks.txt).
No deployment or owner sign-off runs. No new bug is found.

## #12960 verification (2026-11-06)

The development door-rule replacement reaches ordinary native collision calls
and a later page frame in 899.5 ms median across ten distinct source edits on
coderos-4080. Every sample is below one second. The page retains its engine,
state address and seat. The real rule changes; observation probes do not count
as ordinary gameplay calls.

The admission and publication gates kill 315 and 6 root mutants. Physical
process-lock checks pass on Mac and Linux, with ASan/UBSan/LSan on Linux.
All 662 room checks pass. The edited page passes 1,352 checks, reusing 1,197
and running 155 in the background. The replaced and clean full builds each
pass local, 100 ms and 250 ms drift with three bots and raids. The full release
Wasm is 2,030,740 bytes against its unchanged 2,150,000-byte budget.
After rebasing the shared browser renderer, fresh session identities and another
ten-edit measurement pass. Both images also pass local drift with raids on the
merged host. The unchanged Coax program reuses its complete checking verdict.

A deliberately broken body fails the real door check. The session displays
the failure, the guarded dry-run commit refuses, and HEAD stays unchanged.
The long checker and its browser observation run as separate bounded steps.
Unsupported context or interface edits require the ordinary full rebuild.
General R26/R27 replacement and migration remain separate.

The commands, raw samples, complete check reports and artifact hashes are in
[the verification record](../verification/fast-edit-2026-11-06/README.md).
No deployment or owner sign-off runs. No separate bug issue is filed.
