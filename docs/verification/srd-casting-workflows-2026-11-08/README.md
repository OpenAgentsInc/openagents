# Rituals and you-can-see targets (#13038)

Source: SRD 5.2.1, pinned PDF SHA-256
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

## Rules checked

| Rule | SRD 5.2.1 | Engine | Check |
| --- | --- | --- | --- |
| A Ritual takes 10 minutes longer and expends no slot; it must be prepared | p.104 Rituals; p.188 Ritual | `ritual.coax` `rs_rite_admits`; `ritual-cast.coax` | `cast-workflow-checks.coax` "a prepared Ritual takes ten more minutes…" |
| A Wizard may cast a spellbook Ritual without preparing it | p.78 Ritual Adept | `rs_rite_book_has` and the Wizard/Ritual Adept facts | "Ritual Adept reads a spellbook Ritual…" |
| A 1 minute or longer casting takes the Magic action each turn and needs Concentration; broken Concentration fails it and expends nothing | p.105 Longer Casting Times; p.179 Concentration | rite holds Concentration under the spell's ID; `rs_rite_turn`; `rs_end_concentration` hands a rite to `rs_rite_fail` | "a missed Magic action, failed Concentration save, incapacitation, Rage or new Concentration spell fails the Ritual…" |
| Starting a Concentration casting ends earlier Concentration | p.179 | `rs_ritual_begin` | "beginning a Ritual ends earlier Concentration…" |
| A Ritual can't be cast at a higher level | p.188 Ritual | completion only at slot 0; Quickened refused | "only the due rite's caster casts it free…" |
| Source Ritual tags and casting times | catalog | `RsSpell.ritual` on Speak with Animals, Water Walk, Water Breathing | "every executable record carries the source Ritual tag and no executable spell has a timed casting" |
| A spell's description says what it targets; "a creature you can see" | p.106 Targets; p.177 Blinded; p.184 Invisible | `cast-sight.coax` (22 spells + Vicious Mockery's see or hear) | "the reviewed sight list matches every executable description…", "an unseen creature or point refuses…" |

## Decisions

- A Ritual's final Magic action is the spell's normal casting time. The world
  calls `rs_cast` at slot 0 then, so the ordinary target, component, effect and
  zone preflight applies; a refused completion (for example a bad target)
  keeps the rite and spends nothing.
- During a rite the caster's Concentration is the spell's own ID. Damage
  saves, incapacitation, Rage and a new Concentration spell already end
  Concentration; `rs_end_concentration` turns that into a failed Ritual and
  never removes effects an earlier copy of the spell made.
- A turn without the Magic action fails the Ritual at the next turn's start.
  Beginning, continuing and completing a rite each count as casting a level 1+
  spell that turn (Quickened Spell, p.68).
- Unseen choices refuse before any cost. The caster cannot choose a creature
  it cannot see, so this is a malformed request; p.106's invalid-target
  spending is for a chosen target the spell can't affect.
- The sight model is attack-sight.coax's open-grid flags (Blinded caster,
  Invisible target; the caster is not hidden from itself). Total Cover,
  obscurement and alternate senses are world facts not yet admitted.

## Evidence

- `cast-workflow-checks.coax`: 334 checks pass (includes the battle library).
- Mutation, zero survivors, no exemptions: `ritual.coax` 25/25,
  `cast-sight.coax` 50/50 (both through `cast-workflow-checks.coax`),
  `concentration.coax` 89/89 (through `concentration-checks.coax`).
- Regression entries pass unchanged: concentration 375, cast-target 357,
  hotbar (fivebar-test) 365, battle 323, slice spells 336, kit cantrips 330,
  kit spells 333, kit high 333, condition rolls 340, effect rolls 332,
  components 364, spell state 325, saving throws 334, HP interaction 419,
  spell-access source 363, compliance probes 339, class kit 325; 41 selected
  Experimental combat checks.
- Lab Wasm (`fivelab`): 2,566,015 bytes of its 2,575,000 budget, SHA-256
  `a59803b9b7c1c13da25279f5286f625d06be19cae64448ea74aa373f5d14dbbd`.

```sh
coax/bin/coax test aethyr/rules/srd/cast-workflow-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/cast-workflow-checks.coax --library --unit aethyr/rules/srd/ritual.coax --jobs 2
coax/bin/coax mutate aethyr/rules/srd/cast-workflow-checks.coax --library --unit aethyr/rules/srd/cast-sight.coax --jobs 2
coax/bin/coax mutate aethyr/rules/srd/concentration-checks.coax --library --unit aethyr/rules/srd/concentration.coax --jobs 2
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/lab.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
```

## Not covered

No world control begins or continues a Ritual yet: Everglade's bars cast these
three spells as ordinary slot casts. A Wizard's need to read from the book is
not modeled. Feather Fall's sight lives in its Reaction trigger, which the
world supplies. Hunter's Mark and Hex moves are not sight-checked.
