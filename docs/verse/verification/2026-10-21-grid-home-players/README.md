# Multiplayer Grid on `/home` (#12406)

Recorded on 2026-10-21 on a macOS (Apple silicon) contributor machine: a
local `openagents-web` (`--everglade`, `--pylons`), a scratch `nostr-relay`
on `ws://127.0.0.1:7912`, and three headless Chrome profiles, each started
by `openagents browser run`. All three players were signed out, each with
its own browser-local key.

```sh
scripts/dev/check-grid-home-players.py --url http://127.0.0.1:4912/home \
    --relay ws://127.0.0.1:7912 --out OUT --players 3
```

| Check | Result ([players-check.json](players-check.json)) |
| --- | --- |
| Overview with live players | Player 0, still watching, saw both joined players with their tags ([overview.png](overview.png)). |
| Tags | Every page showed every other player as `npub1` and six characters, with no name line. |
| Spawns | Gaps between the three spawns: 4.3 m, 5.0 m, and 8.5 m. |
| Collision | Player 0 walked straight at player 1 from 5.0 m for 3 s: closest 0.900 m (two radii), stopped there, 0 m drift over the last second ([collision.png](collision.png)). |

`scripts/dev/check-grid-home.py` still passed on `/home`, with the page's
one WebSocket going to the relay. The module is 25,445,923 bytes
(7,516,129 with brotli; production on 2026-10-21 serves 25,477,066 and
7,527,417), and `bench/verse/2026-10-20/module-size/first_frame.py` at
20 Mbit/s drew the first frame in 3283–3301 ms (3403 ms in #12319).
