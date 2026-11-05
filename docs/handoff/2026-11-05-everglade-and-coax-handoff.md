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

Each one works in its own worktree, pushes to `main` and closes its issue
with a comment. Check the issue and `git log origin/main` for their results.

| Issue | Work | Worktree |
| --- | --- | --- |
| #12936 | Fix the production room regression, then finish the release (rooms, then web) | `../openagents-roomperf` |
| #12927 | Checked, wrapping and saturating integer operations; migrate the room counters | `../openagents-coax-int` |
| #12938 | Split the carrier C unit so a gameplay edit rebuilds in under 20 s | `../openagents-carrier` |

If one of them stops without closing its issue, read its last issue comment
and its worktree's `git status` and `git log`. Pick up from there; don't
redo work that's already on `main`.

## Production state (most important)

The owner authorized a production deploy of current `main` at about 14:20.
Here's what happened:

- **API: live.** `oa-api-00107-l9z`, build `54575b412a`, migrations through
  `0135_character_checkpoints_v2.sql`. They only add tables, columns and
  functions.
- **Rooms: rolled back.** All six shards are back on `oa-room-00037-4kj` and
  `oa-room-K-00016-*` (build `1fee391fc9`, 2026-11-03). In production, the
  new rooms ran at 23–26 updates a second with 16 players a room; the spec
  needs 28. Each player also got 7 times more bytes, and 2 of 16 first joins
  returned 404. A local room on the same build holds 30 updates a second, so
  the problem only shows up over real internet links.
- **Web: held.** Production pages are still release `20261104T010141Z`,
  because the new pages need the new rooms.
- The room deploy's check loop always fails on `sim-check`. That check
  expects a closed player to leave within 800 ms, but the room now holds a
  seat for 15 s.

The #12936 agent is told to find the cause and fix it, test it, and then
redeploy the rooms. Its release bar: at least 28 updates a second, bytes
within about 1.5 times the old build, and 16 of 16 seats, measured against
the old revision the same way. Only then does it run
`coax/web/release.sh` for web. If its result isn't on #12936, start there.

The authorization covered the API, rooms and web. It doesn't cover setting
`OA_ROOM_OWNER_GEN=1`, which waits a day after all six shards run the new
build, or the relay fix (#12909), a second region, or other database changes.

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
- #12925: build timings, a cache for each step, and two emitter fixes. A
  gameplay edit went from 56 s to 44 s, and a test-name edit from 46 s to 6 s.
- #12926: capacity refusals name the limit, its top users and a fix. The
  traversal limit can now be raised with `#traversal N` or `--traversal N`.
- #12937 and #12939 (`199402193d`): the job pool's load notices print only
  to a terminal, so test output no longer depends on machine load.
- The order of work is recorded in
  `coax/audits/2026-11-05-mmorpg-language-and-compiler.md`.

Books: #12610 stays open. The owner said "no more books work". Don't work it.

## Open issues and who owns them

| Issue | Owner or status |
| --- | --- |
| #12936, #12927, #12938 | A running agent (see above) |
| #12941 | No agent. The drift check's 250 ms profile failed twice after the fire fixes, on Hold Person timing, conditions and townspeople walking in place. It may be real or caused by load, and it gates release.sh. Check it first if #12936's release is stuck. |
| #12940 | No agent. Tree fires never burn or spread on the room server; only pages step them. |
| #12838, #12840–#12843, #12868, #12869 | The owner's Codex agent. Don't take them. Its room changes share files with ours, so use small commits and rebase often. |
| #12845 | Open, with no agent. It still needs commands, accepted results and replication behind shared interfaces, profiles for collision and per-recipient fields, Orbit League sharing more than the timing profile, and the book's adoption inventory. |
| #12910 | Nostr chapter 104 on signet. Blocked on the signet faucet's captcha, which is in NEEDS_OWNER.md. |
| #12718, #12833 | Epics. Close each when its children close. |
| #12610 | The book epic, paused by the owner. |
| #12864–#12867 | Excluded; left for Codex. |

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
