# A caster inside its own area, and the declared spare-caster house rule

Work on `187959e5e` toward #13043/#13045. Source: pinned SRD 5.2.1, Areas of
Effect (Rules Glossary) and the spells' own words, e.g. Fireball p.131 "Each
creature in a 20-foot-radius Sphere centered on that point".

## Problem

The cast-time area loop and the transactional effect preflight both skipped
the caster for every area shape, citing "each creature (other than you)".
That is right only for areas whose point of origin is the caster (a Cube
originating from you, a Line from you). A Sphere, Cylinder or Cube centered on
a chosen point includes the caster when it stands inside, so a Fireball,
Shatter or Acid Splash at the caster's feet silently spared it.

## Change

`aethyr/rules/srd/area-origin.coax` (`#authority`) decides whether an area
leaves out its own caster: always for `RS_A_CUBE_SELF` and `RS_A_LINE`; for an
area at a point only under the declared house profile `RS_SPARE_CASTER_AREAS`.
The profile follows the timing profile's contract: a new battle is strict,
`rs_set_areas` admits exactly the two declared kinds before the clock is
sealed, and `rs_reset` restores strict. Both the cast loop (`battle.coax`) and
`rs_cast_effect_room` (`effect-admission.coax`) consult it, so a caster inside
its own area also needs effect room and a full caster refuses before spending.
Sculpt Spells unchanged: an Evoker spends one of its `1 + level` choices on
itself, as the SRD allows.

Every world that selected `RS_ACTION_TIMING` keeps its existing play by also
declaring `RS_SPARE_CASTER_AREAS` at the same call: the MMO battle, the Lab's
action mode, the Everglade 5E single and room rules, Experimental and the
hotbar fixture. Strict battles (the Lab's round mode, the one-shot table and
all engine checks) now follow the SRD.

## Evidence

The new behaviour check failed before the change (the caster took no damage)
and passes after it.

```
coax/bin/coax test --library aethyr/rules/srd/slice-spells-checks.coax   # 256/256
coax/bin/coax mutate aethyr/rules/srd/slice-spells-checks.coax --library --unit aethyr/rules/srd/area-origin.coax         # 22/22
coax/bin/coax mutate aethyr/rules/srd/cast-target-checks.coax --library --unit aethyr/rules/srd/effect-admission.coax    # 164/164
```

Composed suites: cast-target 278, compliance probes 260, components 285, spell
state 246, strike reactions 328, short rest 298, Everglade II rules 255,
Everglade 5E room 383, hotbar 280 and the 5E Lab 834, all passing. Before the
profile, three world checks failed (the Everglade room's own-blast seats and
two hotbar seat checks in which the hero's Mega Thunderbolt killed the hero);
the declared profile restores them. The checked Lab Wasm build passes with
unchanged budgets (traversal 94%, value slots 86%).

## Limits

Later zone triggers (a caster entering or starting its turn in its own
Moonbeam or Spike Growth) were not changed here. The existing preflight check
now places its caster outside the Sphere for the cases it tests and asserts
the in-area caster separately. No general profile format is introduced.
