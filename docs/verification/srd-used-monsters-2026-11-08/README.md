# Used-monster stat blocks, 2026-11-08

Issue: [#13034](https://github.com/OpenAgentsInc/openagents/issues/13034), the
used-monster subset of the Everglade levels 1–6 slice (#13030). Source: SRD 5.2.1,
pinned in the [ledger](../../dnd/srd-compliance-ledger.md).

## What the worlds field

| World | Role | Stat block | Page |
| --- | --- | --- | --- |
| Experimental village and 10x dungeon | imp | Imp | 300 |
| Experimental | gremlin (Puglin) | Goblin Warrior | 290 |
| Experimental dungeon | Puglin Brute | Ogre | 312 |
| Experimental dungeon | Imp Fiend | Quasit | 317 |
| Experimental dungeon | Puglin Warlord | Hill Giant | 298 |
| Everglade 5E room | Conjure Animals wolf | Wolf | 364 |
| Everglade 5E room | Finger of Death's risen | Zombie | 343 |
| Everglade 5E room raid, Forge | Cultist, Cultist Fanatic | npcs.coax | 278 |
| Everglade 5E room watch, Forge | Guard, Warrior Veteran, Knight, Scout | npcs.coax | 296, 337, 302, 322 |

Everglade II fields no monsters (`everglade2-cast.coax`). The manifest is
`RS_USED_MONSTERS` in `aethyr/rules/srd/monsters.coax`; a check requires every
entry to name a block the engine builds.

## What the engine now runs

- Size, and a Large or Huge creature's space reaching one square further
  (`monster-traits.coax`).
- Pack Tactics, Magic Resistance on saves against spells and magical effects,
  Nimble Escape's Disengage, and the Goblin's Advantage damage rider.
- Ranged and thrown stat attacks with normal and long range, second damage on
  a hit, and Prone and Poisoned riders, with room checked before the Action is
  spent (`stat-attack.coax`).
- Multiattack: the Hill Giant's two attacks, each picked again from where the
  two creatures stand (`stat-multiattack.coax`).
- Opportunity attacks apply the record's rider and the attacker's space.
- Experimental's body AI shoots the Shortbow and throws the Javelin past
  reach, and contact runs the block's whole Action.

## Checks

| Command | Result |
| --- | --- |
| `coax test aethyr/rules/srd/monster-checks.coax --library` | 348 checks, 347 pass |
| `coax test aethyr/rules/srd/condition-roll-checks.coax --library` | 313 pass |
| `coax test aethyr/rules/srd/disengage-checks.coax --library` | 304 pass |
| `coax test apps/openagents-web/fivelab.coax --library --only experimental` | 80 pass |
| `coax test apps/openagents-web/fivebar-test.coax --library` | 337 pass |
| `coax test apps/openagents-web/everglade5e-room-rules.coax --library` | 437 checks, 435 pass |
| `coax test aethyr/rules/srd/kit-spells-checks.coax --library`, `kit-spells-high-checks.coax` | 308, 308 pass |
| `coax/web/tool.sh tools-check` | 0 new without a reason |

The one failure, "typed opportunity retains shared critical follow-up without
giving the mover that movement", runs out of its step budget in
`stat-opportunity-checks.coax`. It fails the same way on unmodified main
`d91c0c69f5`, so this change doesn't cause it. The two room failures ("a squad
lays siege by the shared tactics..." and "Cult Priests bless the cult and
command the player...") also fail the same way on `d91c0c69f5`.

The checked Lab Wasm build (`coax/web/wasm.sh apps/openagents-web/fivelab.coax
...`) was 2,559,477 bytes on `d91c0c69f5` plus this change, against the
2,560,000 budget then (2,554,381 without it). The blocks are compact `u16` rows
rather than struct literals to stay inside it. Rebased on `596b288c6a`, which
raised the budget to 2,575,000 for new spells, it's 2,560,786 bytes.

Mutation, every `#authority` file touched, zero survivors, no exemptions:

| Unit | Entry | Mutants killed |
| --- | --- | --- |
| `monster-traits.coax` | `monster-checks.coax` | 54/54 |
| `stat-attack.coax` | `monster-checks.coax` | 204/204 |
| `stat-multiattack.coax` | `monster-checks.coax` | 85/85 |
| `stat-opportunity.coax` | `monster-checks.coax` | 45/45 |
| `condition-rolls.coax` | `condition-roll-checks.coax` | 70/70 |
| `disengage.coax` | `disengage-checks.coax` | 86/86 |

The `stat-opportunity.coax` run raised the pre-existing check's step budget
locally so the baseline could pass; that edit wasn't committed.

## Not done

These block parts aren't used by any world, so the engine doesn't run them:

- Fly speed (Imp), senses, languages and skills.
- The Imp's and the Quasit's Invisibility and Shape-Shift, the Quasit's Scare,
  and the Goblin's Hide (Nimble Escape's other choice).

Approximations and house mappings:

- The battle keeps one square per creature. A Large or Huge creature reaches
  one square further but doesn't fill its whole space.
- Everglade 5E's wolves and zombies strike on the room clock, outside turns.
- Conjure Animals fielding Wolf stat blocks is a house mapping. The SRD spell
  is a pack of spirits that forces a Dexterity save.
- The raid Cultists' world-granted cantrips are a house deviation.
- The Cultist Fanatic's Spiritual Weapon 2/Day and Light/Thaumaturgy and the
  Guard's thrown spear aren't in the engine yet.
