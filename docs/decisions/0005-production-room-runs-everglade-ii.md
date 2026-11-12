# 0005. Production rooms run Everglade II only

Date: 2026-11-12
Status: Accepted
Source: [Everglade and Aethyr against AAA MMORPG practice](../audits/2026-11-11-everglade-aethyr-aaa-audit.md), section G1; issue #13267 (P1.1); [0001](0001-one-game.md)

## Context

The room binary served two games: Everglade v1 (the city, its town, raid and watch, stepped by `everglade_room_tick` on the city's C units) and Everglade II (`everglade2_room_tick`, chosen per room by the `.e2` flag). v1 is frozen, but every room build still compiled and linked its code, and every room change had to keep both paths working. P1.1 (#13267) needs a room binary that loads no v1 file. That can't happen while production rooms might still be asked to run v1.

## Decision

- Production rooms run Everglade II only. No production room runs the v1 room, and the default room build (`coax/web/room/build.sh`, `stage.sh`, `compile.sh`) carries no v1 code.
- v1's room is frozen at one commit. Its room server, its offline room for the browser and its drift check build by name from that commit's own tree with that tree's own compiler: `everglade/server/legacy/v1.sh room|worker|drift`.
- v1 keeps building by name this way until P1.3 (#13269) retires v1, Experimental and the labs from the default build.

## Consequences

- The default room loses the city's C units, `everglade_room_tick`, the town and the v1-only paths. Its tick, seat record and battle layout become Everglade II's own (#13267).
- A v1 page can't play against a production room. Online v1 ends with the next room release. Offline v1 needs the frozen build.
- The drift check compares v1 offline play with v1 room play, so it runs on the frozen tree (`v1.sh drift`). Its result can only change if that commit changes, which it doesn't.
- v1's history stays in version control, not in the build ([0001](0001-one-game.md)).
