# Four slice spells and Shield against Magic Missile

Partial #13043 work on `1058e2dbf`. Source: pinned SRD 5.2.1 (PDF identity in
`docs/dnd/srd-compliance-ledger.md`); expected values come from the pages cited.

## Behavior

Four spells gain executable records in the existing closed step set; no new
step kind or interpreter branch was needed for their effects:

| Spell | Page | Program |
| --- | --- | --- |
| Inflict Wounds | 143 | Touch; Con save, 2d10 Necrotic, half on a success; +1d10 a slot level above 1 |
| Acid Splash | 107 | 60 ft point, 5-ft Sphere; Dex save or 1d6 Acid; Cantrip Upgrade via the shared tier |
| Magic Missile | 146 | 120 ft; three darts of 1d4 + 1 Force, no attack roll; +1 dart a slot level; darts split by explicit per-dart picks |
| Mass Healing Word | 148 | Bonus Action, 60 ft; up to six chosen creatures regain 2d4 + modifier; +1d4 a slot level above 3 |

Shield (p.161) now also answers Magic Missile: `magic-missile-ward.coax`
(`#authority`) decides that an unwarded Magic Missile target is offered the
existing Shield Reaction once, before any dart strikes, and that a warded
target takes no dart damage. The battle reports each blocked dart as a
zero-damage Force event. Shield's slot/Reaction admission is the existing
`rs_shield`; nothing else changes for other spells.

Records carry their source page, components and text. Class tables match the
independently imported tables (Cleric; Sorcerer/Wizard; Sorcerer/Wizard;
Bard/Cleric). `executable-source-ids.coax` and the regenerated
`data/spell-index.coax` add the four IDs (78-81, `RS_MORE_END` 82).

Six existing checks used the literal 78 as "the first unassigned ID"; they now
use `RS_MORE_END`, so adding a record no longer breaks them. The casting-ID
range check in `cast-request.coax` keeps a deliberate literal pin (now 81).

## Evidence

```
coax/bin/coax test --library aethyr/rules/srd/slice-spells-checks.coax   # 244/244
coax/bin/coax mutate aethyr/rules/srd/slice-spells-checks.coax --library --unit aethyr/rules/srd/magic-missile-ward.coax   # 2/2, 0 survived
coax/bin/coax mutate aethyr/rules/srd/cast-target-checks.coax --library --unit aethyr/rules/srd/cast-request.coax   # 23/23
coax/bin/coax mutate aethyr/rules/srd/cast-target-checks.coax --library --unit aethyr/rules/srd/spell-access.coax   # 186/186
coax/bin/coax mutate aethyr/rules/srd/cast-target-checks.coax --library --unit aethyr/rules/srd/cast-self.coax      # 14/14
coax/bin/coax mutate aethyr/rules/srd/concentration-checks.coax --library --unit aethyr/rules/srd/concentration.coax # 85/85
coax/bin/coax mutate aethyr/rules/srd/component-checks.coax --library --unit aethyr/rules/srd/cast-components.coax  # 143/143
coax/web/tool.sh aethyr/rules/srd/spell-index-build.coax --check aethyr/rules/srd/data/spell-index.coax  # matches
```

Composed suites on the final tree: spells 68, cast-target 269, compliance
probes 251, components 276, spell-access source 275, spell state 237, short
rest 289, spell coverage 243, hotbar `fivebar-test` 271 and
`everglade5e-room-rules` 374 (which asserts every executable ID has a raid
look), all passing. Fixed dice exposed two preset facts the checks now state
rather than hide: the level 5 Wizard preset is an Evoker (Potent Cantrip halves
a saved cantrip's damage, SRD level 3 feature) and the Cleric preset is Life
Domain (Disciple of Life adds healing), so Mass Healing Word is checked with a
Bard. The preset full-caster slot table stops at level 6, the slice's limit;
the upcast check grants its 4th-level slot directly.

## Limits

Spell coverage at levels 0-3 rises from 43 to 47 of 183 distinct spells.
"You can see" targeting, Shatter's Construct Disadvantage, and every other
source-only spell remain #13043 work. Presets and world bars are unchanged:
these spells are castable by any creature whose selected loadout holds them,
but no world offers them yet (#13046). Not a qualification of whole spells
beyond the clauses tested.
