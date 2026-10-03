# WoW bridge

A supervised, headless Rust client for the private WoW gym. It pins
`benilla-protocol` at `cf891dc3` and Rust 1.98.1, with no renderer or Bevy.

```sh
scripts/build-wow-bridge.sh
export VOYAGER_WOW_ACCOUNTS="$HOME/wow-gym/accounts.json"
"$HOME/work/openagents-target-agent1/release/wow-bridge"
```

Send one JSON object per line. Replies carry the same `id`, `ok`, and either
`result` or `code`/`error`; unsolicited observations carry `event`.

```json
{"id":1,"op":"join","args":{"auth":"100.74.238.61:3724","account":"GYM1","character":"Voyagera","create":{"race":1,"class":1},"reset":true}}
{"id":2,"op":"state","args":{"radius":40}}
{"id":3,"op":"goto","args":{"x":-8947,"y":-132.5,"z":83.5,"seconds":10}}
{"id":4,"op":"say","args":{"text":"Ready"}}
{"id":5,"op":"wait","args":{"seconds":1}}
{"id":6,"op":"disconnect"}
{"id":7,"op":"shutdown"}
```

The credential file is a private JSON array of account/password objects,
readable only by its owner. Passwords never travel in requests or replies.
`reset` deletes and recreates only the named character on the selected account.
One helper serves one session; start a new helper for the next episode.

State includes map, position, health, powers, level, XP, copper, quest log,
backpack slots, item counts, and nearby entities. GUIDs are decimal strings to
preserve all 64 bits. Scans are capped at 100 yards and 256 results. Actions
have a 120-second limit; route lists contain at most 32 waypoints. Movement
follows straight lines with heartbeats and a final stop; it has no terrain
planner. World-stream reads keep packet framing intact on a dedicated bounded
channel. EOF cancels an active action, and the supervisor can kill a stuck
helper. Disconnect waits for the server's logout confirmation.

Ordinary sessions refuse `gm`, the setup account, and dot-command chat. A
separate trusted process with `WOW_BRIDGE_SETUP=1` may send `gm` for manifest
setup. Agent programs must never receive that process or its operations.

The live smoke verified Northshire login, descriptor state, three-yard movement
independently saved by the server, chat, GM refusal, logout, and shutdown. Its
disposable character was removed through character select. The
[realm runbook](../docs/wow/realm.md) covers the private server.

Combat and quest operations are `target`, `attack`, `cast`, `loot`, `quest`,
`use`, and `vendor`. See [the episode runbook](../docs/wow/episodes.md) for argument
shapes and deterministic grading. `attack` returns whether the selected target
died; kill credit comes from the server. `loot` opens, takes, and releases a loot
window. `quest` confirms acceptance and reward against observed state. Vendor
sales support only server-quality-zero backpack items. Every operation remains
bounded by at most 120 seconds.
