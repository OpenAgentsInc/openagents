# Player testing handoff — 2026-11-18

Stopped at the owner's explicit five-minute stop-and-handoff request. **Phase 1
is not qualified.** Implementation, product fixes and the spec are on main;
the remaining work is the complete live acceptance cohort, any failures it
reveals, and publication of its actual results. No test processes are left running.

## Current release and executable

- Live eight-game client: <https://gamebuilder.org/player-releases/phase1-20261117-v11>.
- Client source/tag: `5d8ec3fba149045c2b0ae0e7cf12879b9507ab53`,
  `player-testing/phase1-20261117-v11`.
- Release manifest SHA-256: `da8e4db5de774674d4216e6222dd0e4fe3de7bd1048652175fc7549b3a36ccdc`.
- Client OpenAgents pin: `3ff038f301291bf2e6ec4ceea0ed534938d35509`.
- Frozen runner source/tag: `e6243c95bb9c435eceae706faf4a9e6505f40959`,
  `player-testing/phase1-20261117-v11-runner8`.
- Existing executable: `build/player-release/phase1-20261117-v11/player-test-qualification-r8`.
  SHA-256: `49c2a1aaa47d2db9e9b9a16baa3a353324d6c0da186f37bb38e394af8dcc5b6c`.
- Native input fixture SHA-256: `fb043d5426eaa6841ade6342b4eb1a01884917a8c9b93210b9fe5ecab96e8b79`.
- Expected live service policy: `gamebuilder-accounts-27686bc39a-46685296b2`.

All tags are pushed. Never overwrite a client release, tag, frozen executable
or evidence directory. The snapshot covers third person, first person,
top-down, side-scroller, village, crates (three rooms), platformer (eight
checkpoints) and racing (three laps). Main's later drift, account storage,
publishing and other additions are outside this approved named snapshot.

## Verified before stopping

The v11 live diagnostics below passed every attempted cell, all four quality
gates (provenance, validated recording, hardware GPU, native input) and private
upload. These partial scopes cannot count as full-matrix repetitions.

| Local evidence directory under `build/player-tests/` | Passed | Detail |
| --- | --- | --- |
| `snapshot-v11-create-desktop-001` | 10 | Real service creation, stand/step off, strict selected-object resize/removal, three independent Decide edits |
| `snapshot-v11-create-phone-001` | 10 | Same journey with native Chromium touch; second QA account |
| `snapshot-v11-create-tablet-001` | 10 | Same journey with native Chromium touch |
| `snapshot-v11-visible-001` | 5 | Observer-free creation and visible geometry from six PNGs, standing in two views, stepping off, removal |

Their `private-upload.txt` files give the exact private evidence locations.
The frozen runner passed 592 coordinator checks, 591 checks in each gameplay
calibration suite, and native input fault conformance. The eight-game v11
chat/JSON source passed all 220 loaded checks. Main at `bcdb1cd6279b` passed
`scripts/build.sh` (1,256 core checks, 370 exports, 172 golden calls, hosted
and account checks, unsigned iOS/macOS/macOS Dev builds and CLI). Its player
checks passed 600/660/660. Logs are ignored under `build/player-v11-*.log`.

## Interrupted and failed evidence

Both fresh v11 queues were stopped during their first desktop child at the
owner's request. No full report exists and **zero v11 full repetitions qualify**:

- `build/player-tests/qualification-v11-ordinary-001/round-1-chrome-mac-ordinary`
- `build/player-tests/qualification-v11-entitled-001/round-1-chrome-mac-entitled`

Each root has `coordinator-stop.txt`. Raw evidence is retained; partial video
is encoded separately, never presented as a completed recording qualification.
The owned browsers, coordinators, runners, recorders, telemetry and watchdogs
were stopped. Their temporary browser profiles were moved into the private
`~/.openagents/gamebuilder/player-accounts/quarantine/` directory, not evidence.
No credentials or generated evidence are committed. Both partial videos
were validated and uploaded with an explicit sanitized log allowlist under
`gs://openagentsgemini-gamebuilder-player-evidence/diagnostic/owner-stopped-v11-20261118/`
(`ordinary/` and `entitled/`). Bucket public-access prevention and uniform
access were verified before upload. Each interrupted queue root has a
`partial-upload.txt` receipt. These are diagnostic archives, without a
qualifying `report.json`.

The preceding v10 ordinary desktop report passed 52/53, failing Decide when
production returned HTTP 200 with missing assistant content. Its body and exact
finish were not retained; sanitized telemetry classified the finish as other.
V10's entitled desktop and phone passed 53/53 and 51/51. All remain historical;
they cannot be merged into v11 qualification.

V11 uses the shared JSON decoder and permits at most one app retry for an
eligible empty final completion, resending identical bytes while busy and
before any edit/history change. Errors, refusals, tool continuations,
filtered/truncated/unknown finishes and nonempty/no-op answers never retry.
Two blanks fail visibly. Unit tests prove the bound and exactly one applied
edit. The unknown v10 finish is not asserted to have been eligible. No test
assertion was relaxed or live response faked.

## Resume the acceptance cohort

Use headless desktop Chrome and Chromium phone/tablet emulation; the owner
approved these in place of Safari and physical devices. Do not open or focus
windows. Use the dedicated credentials through the existing wrapper, never a
personal browser profile, cookie injection or test login bypass.

Verify the executable and input fixture hashes above. Start two new output
roots, one per account, using the existing frozen executable:

```sh
export GB_PLAYER_RUNNER="$PWD/build/player-release/phase1-20261117-v11/player-test-qualification-r8"
scripts/player-test.sh --matrix --role ordinary \
  --origin https://gamebuilder.org/player-releases/phase1-20261117-v11 \
  --out build/player-tests/qualification-v11-ordinary-002
scripts/player-test.sh --matrix --role agent-entitled \
  --origin https://gamebuilder.org/player-releases/phase1-20261117-v11 \
  --out build/player-tests/qualification-v11-entitled-002
```

Run the two commands in independent background sessions so both accounts can
progress. Each queue performs nine fresh-profile UI logins and full runs;
allow roughly 3.5 hours with real gameplay and inbox pacing. It stops on the
first unclean child. Keep failed reports intact; investigate before restarting.
A role-only queue's final merge cannot qualify the whole matrix by itself.

Merge all 18 completed child directories with that same frozen executable's
`--merge RUN_DIR... --out NEW_DIR`, ordered by round. Require
`phase1Complete: true`, `invalidInput: false`, 155/155 target/case cells,
three clean runs for each of six target/account groups, 930 passing role/case
repetitions, all quality gates, validated videos and every private upload.
Then update `docs/player-testing-results.md` with the actual six-group score
table and matrix receipt, commit and push main. Never mix releases, runner
hashes, old v10 reports, diagnostics or interrupted children. A rebuilt runner
requires its own frozen identity, conformance and wholly consistent cohort.

The spec/runbook is `docs/player-testing.md`; current findings and history are
`docs/player-testing-results.md`. The ignored local journal
`build/player-qualification-notes.md` has detailed earlier investigations.
Production account services remain live while the named client is immutable.

## Environment and authorization

The owner authorized narrow resets of dedicated QA email/day counters after
the inbox exhausted its limit. Those resets have already been made. Global,
hourly/IP policies, challenges, codes, sessions and entitlements were unchanged.
The wrapper keeps account leases and thirteen-minute email-code pacing. Reclaim
only leases whose owner PID is dead; diagnostic session reuse cannot qualify
fresh login. Preserve evidence privacy and the bucket's seven-day passed /
thirty-day diagnostic-and-failed retention.

The active main dependency is the clean pinned SDK at
`/Users/christopherdavid/.codex/worktrees/live-player/openagents`, commit
`f5a5677d3205449877bbaae67f415bd525ebe8d2`, with its compiler restored.
`../openagents-gamebuilder` links to it. **Do not archive or remove it** while
it remains the pinned dependency. Never modify the shared OpenAgents checkout.
Fetch/rebase owner changes safely before the next main push; do not change
source or pins while a build is running. No recurring automation was created.
