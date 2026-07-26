# FA-07 gate table on Omega `0.2.0-rc15`

- Status: producer record. rc15 does not close omega#26.
- Owner: OpenAgents
- Date: 2026-07-25
- Issue: [omega#26](https://github.com/OpenAgentsInc/omega/issues/26) (`OMEGA-FA-07`)
- Predecessors: `2026-07-25-omega-rc13-installed-observation-proof.md`,
  the rc13 gate report on omega#26

I am the producer. I did not review this record. I must not sign it.

## Why rc15 exists

`0.2.0-rc13` shipped, and `0.2.0-rc14` was cut, on `omega-effectd` **rc.9** — an
engine in which **a Full Auto run nobody is watching never reaches its second
turn**. Driving gate 2 against rc13 measured one real Codex turn and then
eighteen minutes of nothing, with the turn count stuck at one.

`rc14` could not have fixed it. It is a one-line commit that bumps
`OMEGA_RC_VERSION` and leaves `OMEGA_EFFECTD_RELEASE_TAG` on `rc.9`, whose
source commit `28c877d0b8` **predates** the fix in openagents `5d260cbc00`. It
was also never built or released. Installing rc14 and re-driving gate 2 would
have reproduced rc13's red exactly — the mistake rc11 made, at the cost of
another sign-and-notarize cycle.

`rc15` pins `omega-effectd-v0.1.0-rc.10`, built from openagents `e3280b4795`,
which contains `5d260cbc00`.

## Candidate

Every observation below binds to this candidate and to nothing else.

| Binding | Value |
| --- | --- |
| Version | `0.2.0-rc15` |
| `CFBundleShortVersionString` / `CFBundleVersion` | `0.2.0` / `20260726.013712` |
| omega source commit | `b70ce721b2309672c66acc51d55d4bcdbbcad0d9` |
| DMG sha256 | `6ac6f0007a1cc35be3112ad36a5f4d3b5477540a1f868f31b5aef798e7f84cd5` |
| Installed host binary sha256 | `cca81f5dff127f790e289aa8a400e72765dcefac8704a74fd852c757871b2858` |
| CDHash | `4260f65a15f021fcf6226735b9c7a55cdfe8edcb` |
| Gatekeeper | `accepted`, `source=Notarized Developer ID` |
| `xcrun stapler validate /Applications/Omega.app` | `The validate action worked!` |
| Engine | `omega-effectd-v0.1.0-rc.10`, source `e3280b4795` |
| Engine files vs. its own `component-manifest.json` | all four recomputed, all equal |

`git diff e3280b4795..origin/main -- packages/omega-effectd/src/` is **empty**,
so every source-tier test in that package tests exactly the engine bytes rc15
ships.

## Gate table

| # | Gate | rc15 | Evidence |
| --- | --- | --- | --- |
| 1 | Incident replay | green (source tier, engine source byte-identical to rc15's bytes) | `server.fa07-incident-replay.test.ts` |
| 2 | Multi-turn unattended run | **green — flipped from red** | `2026-07-25-omega-fa07-rc15-gate2-unattended.json` |
| 3 | Restart reconciliation | green | gate 6's restart leg and gate 2's return leg |
| 4 | Control matrix + `cap_reached` | green | `…rc15-gates-5-6-7-4-9.json` |
| 5 | Cross-provider handoff | **red — host refuses the note** | same receipt, and see below |
| 6 | Offline / Sync gap | green | same receipt |
| 7 | Mobile typed outcomes | green | same receipt |
| 8 | No model-initiated start | green | no start path was created, and gate 2 replays the launcher's wire form |
| 9 | Redaction | green | `…rc15-gates-5-6-7-4-9.json` |
| 10 | Independent assurance | **blocked — producer may not sign** | `…rc15-producer-claim.json` |

## Gate 2 — performed by an agent, and recorded as one

The owner lifted the observation reservation on 2026-07-25. That permits an
agent to **perform** the run. It does not convert an agent-performed run into a
person's observation, and the receipt does not claim otherwise:
`performedBy: "agent"`, `performedByIdentity: "emulated.operator.fa07-gate2"`,
`ownerObservation: false`. No `owner_observation` attestation was written.

| Observation | Value |
| --- | --- |
| Silent window | **240,005 ms with 0 framed calls of any kind** |
| Write calls after the start | **0** |
| Turns before / after the silent window | 0 / **5** |
| `turnsAdvancedDuringSilentWindow` | **true** |
| Engine dispatches before / after | 1 / **12** |
| Real Codex executions | **12**, every one exit 0 |
| `everyTurnHadAProvider` | true |
| `multiTurn` (≥ 3) | **true** |
| Final state | `cap_reached` — the cap still bounds an unattended run |
| Survived a restart at a new generation | run, turns, objective and report all intact |

**An Omega Full Auto run now reaches many turns on its own.** The same driver
against rc13's engine reports `multiTurn: false` with the turn count stuck at 1.

### A measurement trap worth recording

The first gate 2 run on rc15 used `--turn-cap 4` and reported `turnCount: 0`
with `multiTurn: false`, while simultaneously reporting **four real Codex
executions, all exit 0**, and `engineDispatchedDuringSilentWindow: true`. Read
carelessly that looks like the fix failing.

It is a reading artifact: `turnCount` comes from `detail.turns` on `get_run`,
and the driver takes that reading *after* the window, by which point a run whose
cap equals its dispatch budget has already gone terminal. Re-driven with a cap
the run could not exhaust, the same engine reports `turnCount: 5`. Both receipts
are kept — `…rc15-gate2-cap-bounded.json` is the cap-4 run — because a gate that
can be flipped by the caller's cap is a gate a reviewer should know how to
mis-drive.

`turnCount` (5) still lags `successfulAttempts` (12). Recording that as an open
observation rather than a claim: gate 2 asks whether the run advances unattended,
and it does, but the recorded-turn count and the attempt count do not agree and
somebody should find out why.

## Gate 5 — the engine emits, the host refuses

The handoff machinery is correct on rc15. Lane rebound `codex-local` →
`claude-local`. A handoff **while running** is refused. An **unknown target
lane** is refused. The durable registry and `report.providerTransitions` agree
record-for-record. The objective never enters the durable handoff. All three
note observations are **true**: `systemNoteEmitted`,
`systemNoteAddressedToTargetThread`, `systemNoteNamesBothLanes`.

The note reaches the host. **The host refuses it.**
`agent_ui::omega_host_bridge::append_system_note` (line 1059 on `origin/main`)
returns `unavailable("Agent threads do not expose an owner-visible system-note
authority.")`. So an owner reading the thread still cannot tell a different
model took over, which is the whole of what gate 5 asks for. `AgentThreadEntry`
has six variants and none of them is a system note, so this is a new transcript
entry kind plus its render, not a one-line unblock. `crates/agent_ui` is held by
the wire-up lane, so this is requested on omega#76 rather than collided with.

**Second observation, not attributed:** `bothLanesExecuted` is `false` and all
six turns ran on `codex-local`, even though the lane rebound and
`dispatchCountAfterHandoff` is 6. I cannot cleanly separate the engine routing
post-handoff dispatches to the old lane from my own host stand-in defaulting an
unregistered thread via `laneByThread.get(threadRef) ?? CODEX_LANE`. Stating it
as an open question for whoever takes gate 5 next, rather than picking the
reading that flatters either side.

## Gate 6 — green, with one changed reading

Started and dispatched while offline, with `publishBlocksDispatch: false`.
Publish refused `sync_unavailable` with the typed reason
`omega_khala_sync_session_unavailable`, distinguishable from `run_not_found`.
Run and report survive a real supervisor restart with Sync still honestly
unavailable. The objective is absent from every sync surface.

`offlineControlIntentStatus` reads `rejected` on rc15 where rc13 read `applied`.
That is the autonomy clock changing the scenario's timing, not a regression in
offline control: the run now reaches `cap_reached` under its own power before
the intent lands, and a control against a terminal run is correctly refused.
Recorded because a reviewer diffing rc13 against rc15 will see it.

## Provider-home safety

No `codex login` ran anywhere. Codex ran from an isolated `CODEX_HOME`, and the
driver refuses outright if `--codex-home` resolves to `~/.codex`.

Digest recipe:

```
for f in auth.json .credentials.json config.toml
do shasum -a 256 "$HOME/.codex/$f"
done | awk '{print $1}' | shasum -a 256
```

```
before: af604aa5c6e356028317c416ffae96a143751a3eac68da8191a5afaab3129cce
after:  af604aa5c6e356028317c416ffae96a143751a3eac68da8191a5afaab3129cce
```

Byte-identical, with `auth.json` at
`c6d3e9cf5e00a0d89c495832d6d0210b7a4e7b8129fb743aded228eb30ef9091` — the same
value the rc11 and rc13 lanes both recorded. The owner's live session is
untouched. The combined digest differs from those lanes' `a7ac5eeba3…` because
the recipe differs, not the files. `auth.json` is the directly comparable value.

## Gate 10 — what the reviewer must reproduce

I produced this evidence and may not sign the review. The reviewer must be an
execution identity that took no part in this work, holding the key at
`~/work/.secrets/omega-independent-reviewer.env` (pubkey `0326d8f9…`).

**Part A — candidate identity.**

```
OMEGA_REVIEWER_KEY_FILE=~/work/.secrets/omega-independent-reviewer.env \
script/review-omega-candidate \
  --candidate-dmg target/omega-rc/Omega-v0.2.0-rc15-macos-arm64.dmg \
  --app /Applications/Omega.app \
  --producer-claim <openagents>/docs/omega/2026-07-25-omega-fa07-rc15-producer-claim.json \
  --obligation omega#26 --output <receipt.json>
```

**Part B — the gates, re-driven, not read.** The reviewer must not take these
receipts as input.

```
node --import tsx packages/omega-effectd/scripts/fa07-gate2-unattended.ts \
  --app /Applications/Omega.app --live-providers --turn-cap 12 --silent-window-ms 240000 …
node --import tsx packages/omega-effectd/scripts/fa07-installed-gates.ts \
  --app /Applications/Omega.app --only 5,6,7,49 --live-providers …
```

Expected disagreement-free: gates 4, 6, 7 and 9 green. Gate 2 `multiTurn: true`
with `framedCallsDuringSilentWindow: 0`. Gate 5 `systemNoteEmitted: true` with
the host still refusing the note.

**A reviewer who gets `multiTurn: false` on rc15 with a cap above the dispatch
budget should refuse** — it would mean the candidate is not the one this
evidence binds to. Equally, **a reviewer who gets `multiTurn: true` on rc13
should refuse**, for the same reason in the other direction.
