# Enabled class-kit spell manifest, checked against the SRD

Partial-to-complete #13043 work on `9c58d90c20`. Source: pinned SRD 5.2.1
(PDF identity in `docs/dnd/srd-compliance-ledger.md`); every expected number is
read from the page cited, with fixed dice (each die lands on half its faces).

## The manifest

`apps/openagents-web/class-kit-spells-checks.coax` declares the enabled spell
manifest of the levels 1–6 slice: the 23 spells the twelve class kits show at
any selector level 1–6 (`xc_bar_level`, played by Experimental and Everglade
II) plus Shield, which the Sorcerer and Wizard heroes cast as their own
Reaction. A check holds the manifest equal to what the kits enable at every
class and level, so a kit change must update it. Barbarian, Fighter, Monk and
Rogue kits enable no spells; Hellish Rebuke is not enabled for the kit Warlock.

| Check file (`aethyr/rules/srd/`) | Spells |
| --- | --- |
| `kit-cantrips-checks.coax` | Fire Bolt, Chill Touch, Eldritch Blast, Starry Wisp, Produce Flame (flame and hurl), Vicious Mockery, Sacred Flame |
| `kit-spells-checks.coax` | Healing Word, Thunderwave, Entangle, Ray of Sickness, False Life, Hex, Guiding Bolt, Divine Smite, Hunter's Mark, Shield |
| `kit-spells-high-checks.coax` | Blindness/Deafness, Hold Person, Scorching Ray, Fireball, Spirit Guardians, Call Lightning, Hypnotic Pattern |

Each spell has a check of its damage or healing dice, attack or save, success
and failure, cantrip tier or upcast, range refusal, duration or repeat save,
and riders (no-heal, glow, Disadvantage, Advantage, push, Restrained and its
escape, Paralyzed/Blinded repeat saves, extra targets and rays, the Emanation's
enter/end-turn saves and Speed). The manifest check also holds every record's
level, page, components, Concentration, casting time, range and duration to
its canonical source entry, and gives each spell its own sound
(`xc_spell_sound`: no two share cast, layer and pitch) and colour.

## Defects found and fixed

| Spell | Defect | Fix |
| --- | --- | --- |
| Hypnotic Pattern p.141 | Every creature in the Cube was dazed, even on a successful save | The DAZE step applies only on a failed save |
| Hypnotic Pattern p.141 | A Blinded creature was affected; no one could shake a creature out | `RsSpell.sight` leaves out a Blinded creature; `rs_shake` (an action, within 5 feet: the SRD names no range, an interpretation) ends the stupor |
| Entangle p.128 | Its caster, inside its own square, could be Restrained ("each creature (other than you)") | `RsSpell.others`, decided in `area-origin.coax` for the cast loop and the preflight |
| Divine Smite p.125 | The extra 1d8 applied to an Undead only, not a Fiend | `RsCreature.fiend`; Experimental's Imp and Imp Fiend are Fiends |
| Spirit Guardians p.164 | A creature could not save on the battle's first turn (`guard_turn` 0 equalled `turn_no` 0); in Experimental a monster walking into the Emanation never saved | `guard_turn` starts at -1; `xc_guardians_step` runs the shared save when a monster enters the Emanation, whoever moved |
| 11 records | Source pages one past the canonical entry (Produce Flame, Starry Wisp, Healing Word, Thunderwave, Entangle, Chill Touch, Ray of Sickness, False Life, Eldritch Blast, Hold Person, Scorching Ray) | Pages match the imported entries |
| Shield | Cast as the player's Reaction in Experimental with no sound or look | Its cast sound and the 5E Lab's barrier play |
| Fireball, Hypnotic Pattern, Call Lightning, Spirit Guardians, Hold Person, Blindness/Deafness | Drawn as the generic streak in Experimental and Everglade II | `xc_look_of` uses the 5E Lab's existing recipes, sized to the SRD areas |

## Evidence

```
coax/bin/coax test --library aethyr/rules/srd/kit-cantrips-checks.coax      # 303/303
coax/bin/coax test --library aethyr/rules/srd/kit-spells-checks.coax        # 306/306
coax/bin/coax test --library aethyr/rules/srd/kit-spells-high-checks.coax   # 305/305
coax/bin/coax test --library aethyr/rules/srd/slice-spells-checks.coax      # 309/309
coax/bin/coax test --library aethyr/rules/srd/cast-target-checks.coax       # 330/330
coax/bin/coax test apps/openagents-web/class-kit-spells-checks.coax --library   # 357/357
coax/bin/coax test --library apps/openagents-web/fivelab.coax --only "experimental combat"   # 34/34
coax/bin/coax test --library apps/openagents-web/fivebar-test.coax          # passes
coax/bin/coax mutate aethyr/rules/srd/slice-spells-checks.coax --library --unit aethyr/rules/srd/area-origin.coax           # 24/24
coax/bin/coax mutate aethyr/rules/srd/kit-spells-high-checks.coax --library --unit aethyr/rules/srd/effect-admission.coax   # 167/167
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c OUT.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax   # 2,554,381 bytes, budget 2,560,000
```

Zero survivors, no exemptions.

## Limits

The manifest is the slice's enabled spells, not all 183 cantrips and 1st–3rd
level spells (`spell-coverage.coax` still counts those). Preset loadouts
prepare 20 more 1st–3rd level spells that no kit shows (for example Cure
Wounds, Bless, Moonbeam); they are not enabled in play and are outside this
manifest. Clauses the battle has no model for are narrative here: Thunderwave's
unsecured objects and boom, Fire Bolt and Fireball igniting objects, Call
Lightning's existing-storm bonus, Spirit Guardians' Necrotic form for an evil
caster, and Sacred Flame's cover clause (the battle grants no cover to saves).
Unreachable at character levels 1–6: Hunter's Mark's 8- and 24-hour durations
(3rd+ slots) and Call Lightning's later bolts after a 4th+ slot (they keep the
3rd-level damage).
