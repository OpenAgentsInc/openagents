# Rituals, targets, objects and choices (#13038)

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
| An object's AC by substance, HP by size (fragile or resilient), Poison and Psychic Immunity | pp.177-178 Breaking Objects | `object-target.coax` | `object-target.coax` unit checks; `cast-object-checks.coax` "Fire Bolt hits a wooden barrel…" |
| A damage threshold stops a smaller instance and takes all of one that meets it | p.180 Damage Threshold | `rs_object_damage` | "a carried or stone object doesn't catch, a threshold stops a small hit…" |
| Eldritch Blast, Fire Bolt, Produce Flame's hurl and Starry Wisp may attack an object; Fire Bolt ignites an unattended flammable one; burning deals 1d4 Fire | pp.127, 132, 156, 165; p.178 Burning | `object-cast.coax` `rs_cast_object`, `rs_object_burn` | "Eldritch Blast's beams…", "Starry Wisp lights an object…", "the reviewed object list matches every executable description that targets a creature or an object" |
| A spell that can't take the object, a destroyed or distant object, a spent action or a blocked component spends nothing | p.106 Targets; pp.105-106 Components | `rs_cast_object` before costs | "a spell that can't take an object… refuses with nothing spent" |
| Always-prepared grants come from named features | pp.35, 40, 46, 54, 56, 58, 70, 73, 76 | `rs_preset_spell_grants` | `cast-provenance-checks.coax` "every level 1-6 preset's always-prepared spell is a listed SRD feature grant…", "the listed grants follow their features' rules" |
| Material components come from the class's focus, a pouch or an exact receipt; priced or consumed ones only from a receipt | pp.105-106 Material | `cast-components.coax` | "every spell a level 1-6 preset can cast has its components…" |
| Experimental's hotbar sends the player's target, option and slot level; a refused option spends nothing | p.106 Targets; Hex p.140, Blindness/Deafness p.113 | `fivebar.coax` `fb_option`, `fb_cast_levels`; `experimental-combat.coax` `xc_choose`, `xc_pick_at` | Experimental combat "(#13038)" checks |

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

- An object is a world fact handed to `rs_cast_object` by slice; it has no
  turn, so the world calls `rs_object_burn` once a round. Objects fail saves,
  but no executable save spell takes an object yet.
- The Experimental hotbar marks a spell's choices on its own icon: the slot
  level digit in the top-right corner and the option label along the bottom.
  A tap there steps the choice instead of casting. A spell aimed at a creature
  sends that one creature through the chosen-target path, so the engine's
  option, sight and capacity preflight applies.
- Fiendish Vigor's False Life is this Warlock preset's level 5 invocation and
  Magical Discoveries' Fireball and Spirit Guardians are the Lore Bard preset's
  choices; both are checked against their features' rules.

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
- `cast-object-checks.coax` 349 checks; `object-target.coax` mutation 45/45
  killed, no exemptions. `cast-provenance-checks.coax` 339 checks.
- Experimental's choices: 4 new checks; 45 selected Experimental combat checks,
  hotbar (fivebar-test) 375 and class kit 338 pass.
- Lab Wasm (`fivelab`): 2,566,015 bytes after Rituals and sight; 2,576,249
  bytes after the hotbar choices (the chosen-target admission is now live in
  the lab), SHA-256
  `ab49c37339b86519cdbe7e3a720e92243d3268bb2e179bcfa8c72f85d5c48709` before
  the last call-site merge; budget raised to 2,585,000.

```sh
coax/bin/coax test aethyr/rules/srd/cast-workflow-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/cast-workflow-checks.coax --library --unit aethyr/rules/srd/ritual.coax --jobs 2
coax/bin/coax mutate aethyr/rules/srd/cast-workflow-checks.coax --library --unit aethyr/rules/srd/cast-sight.coax --jobs 2
coax/bin/coax mutate aethyr/rules/srd/concentration-checks.coax --library --unit aethyr/rules/srd/concentration.coax --jobs 2
coax/bin/coax test aethyr/rules/srd/cast-object-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/cast-object-checks.coax --library --unit aethyr/rules/srd/object-target.coax --jobs 2
coax/bin/coax test aethyr/rules/srd/cast-provenance-checks.coax --library
coax/bin/coax test --library apps/openagents-web/fivelab.coax --only '#13038'
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/lab.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
```

## Not covered

No world control begins or continues a Ritual yet: Everglade's bars cast these
three spells as ordinary slot casts. A Wizard's need to read from the book is
not modeled. Feather Fall's sight lives in its Reaction trigger, which the
world supplies. Hunter's Mark and Hex moves are not sight-checked. No world places objects or
offers an object target yet; Cover, obscurement and GM-set object
vulnerabilities are not admitted. Everglade II's bar does not yet carry the
choices (#13046 adopts the shared kit). Character origin (background and
species) grants have no engine records.
