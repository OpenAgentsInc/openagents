# 19. Familiars and Wild Shape

> **Status: partly implemented.** Authoritative room summoning, Wild Companion,
> Wizard ritual casting, and familiar snapshot replication (`M1`/`m1`/`m2`)
> live in [`everglade/server/familiar.coax`](../server/familiar.coax) and
> [`everglade/server/familiar-wire.coax`](../server/familiar-wire.coax).
> Druid Wild Shape and Shapechange presentation live in
> [`everglade/shapeshift.coax`](../shapeshift.coax) and
> [`rules/srd/shapechange.coax`](../../rules/srd/shapechange.coax), while
> glade wildlife presentation lives in
> [`everglade/client/wild.coax`](../client/wild.coax). A standalone runnable
> book example and dedicated Wild Shape beast mesh pack remain to be extracted.

This chapter answers the player's question: "How does my owl follow me, and
how do I turn into a wolf?"

Both familiars and Wild Shape add or swap a creature under SRD 5.2.1 rules.
Neither is allowed to be client-only decoration: the room decides whether the
caster still has a Wild Companion use, a ritual turn, or a Wild Shape charge,
spends the resource only once placement succeeds, and replicates the creature
to every nearby player.

## What this chapter teaches

- How a familiar occupies a dedicated creature slot after the room's seats and
  monsters (`rs_familiar_of(b, seat)`).
- How Druid Wild Companion (`RS_FT_WILD_COMPANION`) and Wizard *Find Familiar*
  rituals share one server-checked placement path (`M1`/`m1`/`m2`).
- How blocking and long rests clear or hide a familiar without leaking state.
- How Wild Shape swaps beast stats and restores the caster cleanly when HP or
  duration ends.

## What exists today

| Piece | Role | Where |
| --- | --- | --- |
| Server familiar authority | Checks geometry, spends `RS_USE_WILD` or completes Wizard ritual, dismisses on Long Rest | [`everglade/server/familiar.coax`](../server/familiar.coax) |
| Familiar wire codec | Encodes/decodes `M1` requests, `m1` acknowledgements, and `m2` snapshots | [`everglade/server/familiar-wire.coax`](../server/familiar-wire.coax) |
| Battle creature slots | Reserves per-seat familiar slots after `EGB_BEAST0 + EGB_BEASTS` (`egb_enable_familiars`) | [`everglade/server/battle.coax`](../server/battle.coax) |
| Shapeshift & Wild Shape presentation | Maps `RsCreature.wild_form` / `shape_form` to body forms and transitions | [`everglade/shapeshift.coax`](../shapeshift.coax) |
| Glade wildlife placement | Places and animates non-combat creatures (the glade cat) over ground and colliders | [`everglade/client/wild.coax`](../client/wild.coax) |

## The method

### Pay only when placement succeeds

A familiar request (`M1 SEQ FORM SPIRIT`) arrives on the room socket. Before
spending a Wild Companion use (`RS_USE_WILD`) or marking a Wizard ritual
complete, the server checks that the seat is placed, has its Action open, and
passes room geometry. If geometry refuses placement (for instance, a solid wall
at the target point), the familiar stays inactive and `uses[RS_USE_WILD]` is
unchanged. Once placement succeeds, the server assigns `familiar_form` (such as
`MON_OWL`) and `familiar_spirit` (`RS_FAMILIAR_FEY`), replies `m1` to the owner,
and broadcasts `m2` snapshots to visible seats—withholding the familiar from
any peer who has blocked the owner (`im_block`).

### Wild Shape ends cleanly before Shapechange

When a Druid takes a Beast form, the SRD rules store the form on
`RsCreature.wild_form`. Taking a Long Rest or casting *Shapechange*
([chapter 33](ch033-lets-build-a-dragon.md)) ends the Beast form first
(`rs_wild_shape_end`) so two forms never stack stats or temporary Hit Points.

## Worked calculation: familiar slot layout

In a standard 16-seat Everglade II room (`EGB_SEATS = 16`, `EGB_BEASTS = 96`):

```text
Seats (players)       = indices 0 .. 15     (EGB_SEAT + seat)
Monsters & guards     = indices 16 .. 111   (EGB_BEAST0 + q)
Player familiars      = indices 112 .. 127  (EGB_BEAST0 + EGB_BEASTS + seat)
```

Each player owns at most one active familiar slot at a deterministic index, so
re-summoning replaces the prior familiar in place without leaking entity IDs.

## The checks

```shell
coax/bin/coax test coax/web/room/room.coax --only "e2 familiar" --steps 4000000000
coax/bin/coax test --library everglade/client/client.coax --only cat
```

- `everglade/server/familiar.coax`, `#test "e2 familiar: room checks geometry and spends one saved Wild Companion use"`: verifies refused geometry spends 0 uses, valid placement spawns `MON_OWL` (`RS_FAMILIAR_FEY`), blocked peers receive `m2 <tick> 0`, and Long Rest dismisses the familiar cleanly.
- `everglade/server/familiar.coax`, `#test "e2 familiar: Wizard ritual continues on saved turns and pays only on room placement"`: verifies multi-turn ritual progress survives turns and spends resources only on placement.

## What comes next

Part VI puts an armed NPC on the forest path. [Chapter 20](ch020-a-guard-walks-the-path.md)
walks the guard along his beat.
