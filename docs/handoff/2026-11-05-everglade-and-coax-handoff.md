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

Books: #12610 stays open and paused. The only books work approved is #12947.

## Open issues and who owns them

| Issue | Owner or status |
| --- | --- |
| #12944 | Closed by `6414ba56c4`. The room server fails to link on main, with 21 duplicate `cb_*` symbols, since `010c25bbed` (Codex lane). This blocks room builds, the API room checks, the drift check and the #12936 release. The #12936 agent knows. |
| #12945 | Closed by `c8319bb8ac`. The cache oracle uses the current staging signature, split C includes integer helpers, and reviewed evidence bindings are current. All 1,358 cache-policy checks, 209 room-rules checks, 211 Aethyr listings and the default split page build pass. No traversal budget was raised. |
| #12946 | Closed by `e00925f958`. The smoke counted doors and boats as gravity cylinders, rejected a valid landing on the 4.5 m palisade, and counted the invited visitor as the blocked player. It now observes each spell's kind and caster, grounded landings, and the blocked seat. Verification is below. |
| #12941 | No agent. The drift check held at 250 ms during the release, so this may have been load. Close it if three runs in a row hold. The drift check's 250 ms profile failed twice after the fire fixes, on Hold Person timing, conditions and townspeople walking in place. It may be real or caused by load, and it gates release.sh. Check it first if #12936's release is stuck. |
| #12940 | No agent. Tree fires never burn or spread on the room server; only pages step them. |
| #12947 | Completed: current-code overviews for Aethyr 49, 50, 58, 84 and 85; contradictory follow-up status text corrected; Coax 6, 19 and 41 updated. Verification below. |
| #12948 | In progress after #12947: nominal integer ID types (audit §6.2, P0), then room, API and ledger adoption. |
| #12838, #12840–#12843, #12868, #12869 | The owner's Codex agent. Don't take them. Its room changes share files with ours, so use small commits and rebase often. |
| #12845 | Open, with no agent. It still needs commands, accepted results and replication behind shared interfaces, profiles for collision and per-recipient fields, Orbit League sharing more than the timing profile, and the book's adoption inventory. |
| #12910 | Nostr chapter 104 on signet. Blocked on the signet faucet's captcha, which is in NEEDS_OWNER.md. |
| #12718, #12833 | Epics. Close each when its children close. |
| #12610 | The book epic, paused by the owner. |
| #12864–#12867 | Excluded; left for Codex. |

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
