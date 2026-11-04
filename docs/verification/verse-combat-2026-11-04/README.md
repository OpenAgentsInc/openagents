# Recorded WoW hunts, 2026-11-04

Owner request: repeat the XP hunt, improve it, and retain evidence for later
analysis. Issue [#12799](https://github.com/OpenAgentsInc/openagents/issues/12799).
Foob, Classic Era, Deviate Delight, Coldridge Valley, native 2548×1428.

## Actual results

| Trial | Kills / XP | Elapsed | Selection inputs | Lowest health | Images verified |
| --- | --- | --- | --- | --- | --- |
| V2 with complete recording | 3 / 121 | 164.876 s | 15 clicks, 31 Tabs | 83/88 | 343 |
| V3 visual survey | 3 / 104 | 131.712 s | 4 clicks, 0 Tabs | 81/88; levelled to 5 | 281 |
| V3 survey, ASan/UBSan | 4 / 112 | 272.210 s | 24 clicks, 0 Tabs | 95/103 | 497 |
| Final V3, ASan/UBSan | 3 / 70 | 64.236 s | 3 clicks, 0 Tabs | 94/103 | 102 |
| Installed native, finish during combat | 1 / 14 | 9.379 s | 1 click, 0 Tabs | 103/103 | 20 |

Fourteen verified XP kills, 421 XP total; level 4 at 1898/2100 to level 5
at 219/2800. No death observed in these trials. A kill needs both a matching
new player kill event and a positive XP delta. Three interrupted attempts
before a pull are retained separately; the broker refused physical input.

These are field trials, not a controlled speed benchmark. Creature levels,
movement, spawn availability, player level, resources, starting view and
recording load differ. The longer V3 survey run still spent 220 seconds in
search and used 24 clicks. The final short result does not establish that
every future three-kill hunt will take 64 seconds.

## What changed from evidence

The baseline spent 31 inputs on Tab and 13 clearing an unbound target. Those
targets could not authorize combat until visually selected. V3 scans by
turning and follows the same Aethyr-backed circuit without Tab. It allows
a wider central selection band, retaining public target admission and a
fresh recheck before effects. A visibly bound pre-pull approach uses 300 ms,
then requires another visual binding. Encounter movement retains its old
policy. Logging/down-call time now consumes the pulse's remaining sleep.

The baseline's maximum key interval upper bound from wire timestamps was
384 ms; the final sanitized run's was 201 ms. These runs contain different
requested pulses. This is evidence about transport and scheduling, not an
exact physical key duration or a claim of a controlled latency reduction.

## Retained evidence

The persistent local root is
`~/.local/share/verse-os/combat/2026-11-04/`.

- `baseline-01`, `improved-04`, `qualified-01`, `final-01`, and
  `native-finish-01`: every observation and recheck as lossless checksummed
  PPM.zst, typed JSONL, every candidate set, decision reasons/state, actual
  effects and exact desk requests/replies with monotonic timestamps.
- `improved-01` through `improved-03`: failed acquisition attempts, including
  explicit refusals and zero held input after closure.
- Each run's `.analysis` / `.audit`: decoded-image comparisons, SHA-256 image
  manifest, resource/combat/search timing, wire audit and sampled visual
  replay. Every referenced image was decompressed and compared against all
  logged combat/navigation sensor fields. No mismatches were found.
- `qualified-01.video`: 1440 frames over 360 s; `final-01.video`: 996 frames
  over 249 s. These are separate native-resolution H.264 recordings targeted
  at 4 fps, each with full public sensor facts and exact capture identities
  per frame. Each reported one late frame. The ten-second probe recorded
  40 frames with none late. Video also includes idle time around the hunts.
  The qualified survey video covers its whole controller interval. The final
  video starts 1.890 s after that controller's first observation; its lossless
  observation/recheck archive still covers the whole run. Coverage checks
  are retained explicitly in `03-video-coverage.json` / `04-video-coverage.json`.
  Wait for the video's first `frames.jsonl` row before starting a future hunt.
- `.build` directories: executable hashes, source list/hashes, source archive,
  base revision and tracked diff. Final sanitized executable and generated
  C are preserved. Large images/videos stay locally; repository copies keep
  reports, complete controller/wire journals, manifests and selected images.
- `client-combat-log.txt`: 15,854 bytes of the client's own combat events,
  covering the final three-kill run and native finish fixture. The file
  stayed at zero bytes while the client remained open and populated on a
  clean exit. Earlier trials lack this additional event stream. Logging
  was then disabled and the game reopened; the controller does not consume
  this file to steer or admit attacks.
  The independent CSV oracle matches all four final/native kill NPC/spawn
  identities against the client's own player PARTY_KILL records.
  The clean restart also required a Battle.net update; the launcher was
  restarted and native fullscreen presentation restored before re-entering
  Foob. A fresh world sensor and visible image confirmed the final state.

The recorder obtains ordinary broker screenshots and never acquires an
input lease. It is a game-window capture, not a headless game or a recording
of the owner's other desks. Sampled lossless captures and 4-fps video do not
contain every rendered frame. No microphone or desktop audio is collected.
An eight-GiB per-run archive budget requests encounter drain; it does not
reserve disk or prune prior recordings. Video export temporarily expands
observations to PPM and removes only its generated replay staging directory.

## Verification and limits

Final controller: 169 Coax checks. Relevant unchanged gates from
`942814a10c` remain sensor 134/134, combat 343/343, vision 117/117 and drain
21/21 entry-source mutants killed. Changed gates: hunt 120/120, evidence
timing/capacity 13/13, analysis intervals 73/73, zero survivors. Analyzer:
111 checks; passive video: 99; shared-client compatibility: game 87,
navigation 148. Installed native finish fixture requested a pause during
combat observation 6, verified exactly one XP kill, then checked a living
out-of-combat player with zero held keys/buttons.

Integrity fixtures accepted a real recorded image and cleanly rejected
truncated, missing, misbound and orphaned evidence, all with exit 1 rather
than a crash. Protobuf compilation, native CLI builds, shell syntax and Nix
package evaluation passed. This is not a full NixOS image build. tools-check
reports unrelated pre-existing/concurrent reason-header violations, retained
in its report: 19 before rebase and 20 in the rebased tree. The qualified
controller import closure was unchanged by the rebase. ASan/UBSan/leak checks cover the controller/host adapter, not
the external game or its GPU driver.

The owner saw character select while I initially claimed world entry. A
fresh capture showed a disconnect. Retried, verified the public world sensor
and the actual desk-2 image, and kept later claims tied to fresh evidence.
The owner then confirmed nothing was held and clicked/released ground;
acquisition still refused. Operator-only key/button release events cleared
the stale virtual state. It recurred during the clean restart and required
another release batch. Both are retained. The normal broker guard remains
in place; unattended input-state reconciliation is not qualified.

Combat remains a level 2–5 starter-warlock profile in the declared patch.
Arbitrary terrain planning, emergency retreat, automatic resurrection,
quests and a learned combat model remain outside this qualification.
Elephant retains its separately qualified local waypoint role.
