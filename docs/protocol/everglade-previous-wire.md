# The previous wire: a room speaks two versions for one release

Issue: [#13398](https://github.com/OpenAgentsInc/openagents/issues/13398).
Code: `everglade/server/wire-prev.coax` (the rule and the room's hooks),
`everglade/server/wire-prev-wire.coax` (the old frames' one codec).
Profile: `proto/verse/everglade-wire-prev-v6.json`, checked by
`coax/web/tool.sh coax/web/room/wire-prev-profile.coax proto/verse/everglade-wire-prev-v6.json`.

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

The engine, rules and content pins have the same rule beside this one: a room
seats the release before's pins for one release
([the previous pins](everglade-previous-pins.md), #13408).

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

## Wire 6 under wire 7

Wire 7 (#13276, #13245) made Everglade II's familiars replicated rows
(`WK_FAMILIAR`), their requests and the Healer's Kit's `E` (use) kinds, and
the room's answers `e` entries of `D` told to the asker
([the familiar words](everglade-familiar-v2.md)). A wire 6 page reads and
writes frames of their own instead. A wire 7 room speaks them to a wire 6
seat, and sends that seat no familiar's row.

| Hook | Wire 6 |
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

When the wire rises to 8, delete with these writers: the frames' nine
profiles and docs, `everglade/server/kit-wire.coax`, the old cases of
`familiar-profile.coax`, the `m2` and `u1` routes in `RM_WT_ROUTES`, and the
room checks in `everglade/server/familiar.coax` that say `H 6`.

## Wire 5 under wire 6 (ended)

Wire 6 made the monsters and the path guard rows. A wire 6 room sent a wire
5 seat `g2`, `g3`, `h2` and `h3` in their place. Those writers ended when
the wire rose to 7.
