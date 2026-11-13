# The previous wires: a room speaks every version from production's page to its own

Issue: [#13398](https://github.com/OpenAgentsInc/openagents/issues/13398).
Code: `everglade/server/wire-prev.coax` (the rule and the room's hooks),
`everglade/server/wire-prev-wire.coax` (the old frames' one codec).
Profiles: `proto/verse/everglade-wire-prev-v5.json` and
`everglade-wire-prev-v6.json`, one for each version served, each checked by
`coax/web/tool.sh coax/web/room/wire-prev-profile.coax PROFILE`.

## The rule

A room speaks its own session wire version (`WIRE_VERSION`) and every older
one down to `WIRE_MIN`. A page speaks `WIRE_VERSION` alone.

`WIRE_MIN` is the wire production's page speaks (`WIRE_LIVE` in
`everglade/shared/everglade-wire.coax`), and never newer than the version
before `WIRE_VERSION`.

A release moves its rooms before its page, and a page a player has open is the
last release's until they reload. So a new room seats production's page and
writes that page what it reads. Without this, every player is refused between
the two steps of a release.

The wire can be raised more than once between two releases. Each raise keeps
the writers for every version back to `WIRE_LIVE`, so production's page is
always served. Until 2026-11-13 the rule was "the version before, for one
release": wire 7 landed while production still spoke wire 5, and no release
could ship from `main`.

| Room | Page | Result |
| --- | --- | --- |
| wire N | wire N | plays |
| wire N | any version from `WIRE_LIVE` to N-1 | plays: the room writes the old frames to that seat |
| wire N | older than `WIRE_LIVE` | refused, `r 0 version` |
| wire N | wire N+1 | refused, `r 0 version`: a page never ships ahead of its rooms ([the release set](../deployment/release-set.md)) |

A room reports the range on its vitals: `build engine=1 rules=1 content=1 wire=5-7 schema=2`.

The engine, rules and content pins have a rule of their own beside this one:
a room seats the release before's pins for one release
([the previous pins](everglade-previous-pins.md), #13408).

## How to raise the wire version

Do all of these in the commit that raises `WIRE_VERSION`:

1. Raise `WIRE_VERSION` in `everglade/shared/everglade-wire.coax`. Leave
   `WIRE_LIVE` alone, so `WIRE_MIN` stays on production's wire.
2. In `everglade/server/wire-prev-wire.coax`, add a constant for the new
   version (as `WPREV_FAMILIARS :: 7`) and the codec for the frames the old
   wire had. Set `WPREV_UNDER` to the new version.
3. In `everglade/server/wire-prev.coax`, add a branch to each hook for a seat
   below the new version (`if w < WPREV_...`). Keep every older branch.
4. Add `proto/verse/everglade-wire-prev-vN.json` for the version you leave
   behind and its row in `proto/registry.json`, and teach
   `coax/web/room/wire-prev-profile.coax` to check it.
5. Keep the old frames' WebTransport routes in `RM_WT_ROUTES`
   (`coax/web/room/room-wt.coax`).

Never lower `WIRE_LIVE`, and never hand-edit `WIRE_MIN` or a profile's `min`.

## After a production release

In the first change after production's page moves to wire N:

1. Set `WIRE_LIVE` to N.
2. Delete what served the versions below N: their branches in the hooks,
   their frames in the codec, their profiles and registry rows, and their
   routes. Set `WPREV_SERVES` to the new `WIRE_MIN`.

The landing check fails on a head whose range leaves production's wire out,
so `WIRE_LIVE` can't be raised before the release it names.

## The hooks

The room has five hooks. Each is called at one place, for a seat whose hello
said a version below the room's (`wprev_wire`). A seat gets every older form
from its own version up: a wire 5 seat is served as wire 5 and as wire 6.

| Hook | Called from | What it does |
| --- | --- | --- |
| `wprev_pick` | `eg_send` | Withholds the rows the old page has no reader for |
| `wprev_entry` | `eg_send` | Sends a `D` entry the old wire said another way as its old frame |
| `wprev_after` | `rm_eg_send` | Sends the frames the old wire had beside `D` |
| `wprev_heard` | `eg_heard` | Takes a frame the old page says that the new wire no longer has |
| `RM_WT_ROUTES` | `room-wt.coax` | Gives the old frames their WebTransport channels |

A change that deletes a page-to-room frame (as wire 7 did for `M1` to `M7`
and `U1`) keeps the old reader behind `wprev_heard`, so the old page's
requests still work.

## What fails when a version is raised without this

| Check | What it catches |
| --- | --- |
| The room's check "previous wire: these writers serve every version from the live page's to the one below this room's" | `WPREV_UNDER` is not the new `WIRE_VERSION`, or `WPREV_SERVES` is not `WIRE_MIN`. The room does not build |
| `wire-prev-profile.coax` (run by `wire-check`'s profile checks and before each push) | A profile is for a version the room does not serve, or a served version has no profile |
| `everglade-wire-profile.coax` | The session profile's `min` is not `WIRE_MIN` |
| `coax/remote/check.sh e2 PROD`, the line `e2 prev-wire` | Production's wire is outside this commit's `WIRE_MIN` to `WIRE_VERSION`. Or production's own protocol bots, run from production's tree, are refused by this commit's room, or seat fewer, or fight no monster where production's room let them. The after-landing check runs it on every head of `main` |

## Wire 6 and wire 5 under wire 7

Wire 7 (#13276, #13245) made Everglade II's familiars replicated rows
(`WK_FAMILIAR`), their requests and the Healer's Kit's `E` (use) kinds, and
the room's answers `e` entries of `D` told to the asker
([the familiar words](everglade-familiar-v2.md)). A page of wire 6 or wire 5
reads and writes frames of their own instead. A wire 7 room speaks them to a
seat below wire 7, and sends that seat no familiar's row.

| Hook | A seat below wire 7 |
| --- | --- |
| `wprev_pick` | Every `WK_FAMILIAR` row is withheld |
| `wprev_entry` | `e fam SEQ ORDER FORM` is said as `m1 SEQ FORM` (order 0), `m4 SEQ ORDER` (1 to 3), `m5 SEQ`, `m6 SEQ` or `m7 SEQ`. `e fam-cast SEQ STATUS` is said as `m3 SEQ STATUS`. `e kit SEQ LEFT` is said as `u1 SEQ LEFT` |
| `wprev_after` | `m2 TICK COUNT [OWNER FORM X Z HP MAX]...`, the familiars the seat may see, and `m8 MODE`, whose senses it has: each on a change, else once a second |
| `wprev_heard` | `M1`, `M3` to `M7` and `U1` are read as the `E` kinds they are on wire 7 (9 to 15), and reach the same asks |

The rules behind a request are the same on either wire. One rule differs in
what a seat is sent: `m2` keeps a familiar from a page its owner blocks as
well as from a page that blocks its owner, and a row follows the row layer's
one rule (who sees a familiar is who sees its owner).

The old frames keep their own profiles
(`proto/verse/everglade-familiar*-v1.json`, `everglade-find-familiar-v1.json`
and `everglade-healers-kit-v1.json`), which
`coax/web/room/familiar-profile.coax` holds to `wire-prev-wire.coax`.
`everglade-wire-prev-v6.json` pins how each wire 7 entry is said to a wire 6
seat and how each wire 6 request is read.

Delete these with their writers once production's page speaks wire 7
(`WIRE_LIVE` is 7): the frames' nine profiles and docs,
`everglade/server/kit-wire.coax`, the old cases of `familiar-profile.coax`,
the `m2` and `u1` routes in `RM_WT_ROUTES`, and the room checks in
`everglade/server/familiar.coax` that say `H 6`.

## Wire 5 under wire 6 and after

Wire 6 (#13276) made the monsters and the path guard rows (`WK_MONSTER`,
`WK_GUARD`) and their blows `e mon` and `e guard` entries of `D`. A wire 5
page (production's `01bdb405f0`) reads four frames instead.

| Hook | A seat below wire 6 |
| --- | --- |
| `wprev_pick` | Every `WK_MONSTER` row and the `WK_GUARD` row are withheld |
| `wprev_entry` | `e guard` is said as `g3 TICK KIND SEAT AMOUNT`, `e mon` as `h3 TICK KIND M SEAT AMOUNT [FX FZ TX TZ FLY_MS]`, field for field |
| `wprev_after` | `g2 TICK LIFE STATE X Z YAW HP MAX SWING FOE`, the guard, and `h2 TICK COUNT [M LIFE KIND STATE X Z YAW HP MAX ACT ACT_TICK FOE]...`, the monsters in view: each on a change, else once a second |
| `wprev_heard` | Nothing: wire 5 has no page frame that wire 6 lacks |

The guard's `X Z` are millimetres and his `YAW` thousandths of a radian. A
monster's `X Z` are centimetres. `everglade-wire-prev-v5.json` pins the four
frames with examples and refusals.

These writers were deleted when the wire rose to 7 (c311ce48d2), while
production still spoke wire 5. They are back, and stay until `WIRE_LIVE`
passes 5. Delete with them: the four routes `g2`, `h2`, `g3` and `h3`.

Not covered: a zone that declares more than 16 seats (`OA_ROOM_ZONE_SEATS`,
#13275). Seats past the sixteenth have their rows in slots a page from before
that change has no room for, and no check here plays an old page in such a
zone. Keep production's rooms at 16 seats until its page is one built after
#13275.
