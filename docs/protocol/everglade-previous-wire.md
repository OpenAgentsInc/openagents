# The previous wire: a room speaks two versions for one release

Issue: [#13398](https://github.com/OpenAgentsInc/openagents/issues/13398).
Code: `everglade/server/wire-prev.coax` (the rule and the room's hooks),
`everglade/server/wire-prev-wire.coax` (the old frames' one codec).
Profile: `proto/verse/everglade-wire-prev-v5.json`, checked by
`coax/web/tool.sh coax/web/room/wire-prev-profile.coax proto/verse/everglade-wire-prev-v5.json`.

## The rule

A room speaks two session wire versions: its own (`WIRE_VERSION`) and the one
before it (`WIRE_MIN`, which is always `WIRE_VERSION - 1`). A page speaks
`WIRE_VERSION` alone.

A release moves its rooms before its page, and a page a player has open is the
previous release's until they reload. So a room at wire N+1 seats a page at
wire N and writes that page what it reads. Without this, every player is
refused between the two steps of a release.

The previous version's writers live for exactly one release. The change that
raises `WIRE_VERSION` to N+2 deletes the writers for N and adds the writers
for N+1.

| Room | Page | Result |
| --- | --- | --- |
| wire N+1 | wire N+1 | plays |
| wire N+1 | wire N | plays: the room writes the old frames to that seat |
| wire N+1 | wire N-1 or older | refused, `r 0 version` |
| wire N | wire N+1 | refused, `r 0 version`: a page never ships ahead of its rooms ([the release set](../deployment/release-set.md)) |

A room reports the range on its vitals: `build engine=1 rules=1 content=1 wire=5-6 schema=2`.

The engine, rules and content pins are not covered by this rule. A page and a
room whose pins differ never meet, in any version.

## How to raise the wire version

Do all four in the commit that raises `WIRE_VERSION`:

1. Raise `WIRE_VERSION` in `everglade/shared/everglade-wire.coax`. `WIRE_MIN`
   follows it.
2. Rewrite `everglade/server/wire-prev.coax` and `wire-prev-wire.coax` for the
   version you leave behind. Set `WPREV_SERVES` to the old version and
   `WPREV_UNDER` to the new one. Delete the writers that were there.
3. Replace `proto/verse/everglade-wire-prev-vN.json` and its row in
   `proto/registry.json` with the old version's frames, and update
   `coax/web/room/wire-prev-profile.coax` to check them.
4. Keep the old frames' WebTransport routes in `RM_WT_ROUTES`
   (`coax/web/room/room-wt.coax`).

The room has five hooks. Each is called at one place, for a seat whose hello
said `WIRE_MIN`:

| Hook | Called from | What it does |
| --- | --- | --- |
| `wprev_pick` | `eg_send` | Withholds the rows the old page has no reader for |
| `wprev_entry` | `eg_send` | Sends a `D` entry the old wire said another way as its old frame |
| `wprev_after` | `rm_eg_send` | Sends the frames the old wire had beside `D` |
| `wprev_heard` | `eg_heard` | Takes a frame the old page says that the new wire no longer has |
| `RM_WT_ROUTES` | `room-wt.coax` | Gives the old frames their WebTransport channels |

A change that deletes a page-to-room frame (as the familiar and kit change
planned at wire 7 does for `M1` to `M7` and `U1`) keeps the old reader behind
`wprev_heard`, so the old page's requests still work.

## What fails when a version is raised without this

| Check | What it catches |
| --- | --- |
| The room's check "previous wire: these writers serve the version just below this room's" | `WPREV_UNDER` is not the new `WIRE_VERSION`. The room does not build |
| `wire-prev-profile.coax` (run by `wire-check`'s profile checks and before each push) | The profile or codec still serves the older version |
| `everglade-wire-profile.coax` | The session profile's `min` is not `WIRE_MIN` |
| `coax/remote/check.sh e2 PROD`, the line `e2 prev-wire` | Production's own protocol bots, run from production's tree, are refused by this commit's room, or seat fewer, or fight no monster where production's room let them. The after-landing check runs it on every head of `main` |

## Wire 5 under wire 6

Wire 6 (#13276) made Everglade II's monsters and guard replicated rows
(`WK_MONSTER`, `WK_GUARD`) and their blows `e mon` and `e guard` entries of
`D`. A wire 5 page (production's `01bdb405f0`) reads four frames beside `D`
instead. A wire 6 room sends them to a wire 5 seat, and sends that seat no
monster or guard row and no `e mon` or `e guard` entry.

| Frame | Fields | Sent |
| --- | --- | --- |
| `g2` | `TICK LIFE STATE X Z YAW HP MAX SWING FOE` | The guard: each tick he changes, else once a second. `X Z` in mm, `YAW` in thousandths of a radian |
| `g3` | `TICK KIND SEAT AMOUNT` | A blow: the `e guard` entry's fields |
| `h2` | `TICK COUNT [M LIFE KIND STATE X Z YAW HP MAX ACT ACT_TICK FOE]...` | The monsters within 120 m: each tick they change, else once a second. `X Z` in cm |
| `h3` | `TICK KIND M SEAT AMOUNT [FX FZ TX TZ FLY_MS]` | A monster's event: the `e mon` entry's fields |

Wire 5 has no page-to-room frame that wire 6 lacks, so `wprev_heard` takes
none.
