# Knockout, Healer's Kit, condition owners and HP interactions, 2026-11-08

Issue: [#13039](https://github.com/OpenAgentsInc/openagents/issues/13039), the
checks/conditions/damage/healing/death part of the Everglade levels 1–6 slice
(#13030). Source: SRD 5.2.1, SHA-256
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`, pinned in the
[ledger](../../dnd/srd-compliance-ledger.md).

## What changed

- **Knocking Out a Creature** (pp.17, 185), `aethyr/rules/srd/knockout.coax`.
  When the melee attack the strike executor is resolving would reduce a living
  creature above 0 HP to 0 (after Temporary HP), and the attacker chose
  `RsCreature.knockout`, the creature stays at 1 HP. An `EF_KNOCKOUT` record owns
  its Unconscious condition; it is Prone and drops a held focus; it starts a
  Short Rest. Massive damage, Monster Death and Undead Fortitude don't apply,
  because the creature never drops to 0. The condition ends when that Short Rest
  finishes, when the creature regains any HP, or on successful first aid. Ranged
  attacks, Reactions the attack provokes, spells and other sources never knock out.
- **Healer's Kit** (p.97), `rs_healers_kit` in `stabilization.coax`: a Utilize
  action and one of the kit's uses stabilize an Unconscious creature at 0 HP with
  no Wisdom (Medicine) check, resetting both death-save counts and arming the
  1d4-hour recovery.
- **First aid on a knockout** (p.185): `rs_first_aid` also admits a knocked-out
  creature; success ends the knockout instead of stabilizing.
- **Charmed** (p.177): a Charmed creature's Attack action at its charmer is
  refused before the Action is spent (`rs_charmed_by`).
- **Grappled** (p.182): Disadvantage on attack rolls against any target other
  than the grappler (`rs_grapple_down`).
- **Petrified** (p.186): Immunity to the Poisoned condition at admission.
- **Experimental**: a KNOCK OUT box under REST (the sixth box) toggles your choice.
  With it on, a blow that would fell a monster leaves it at 1 HP; KNOCKED OUT
  rises over it, your target line reads UNCONSCIOUS, and it neither moves nor
  attacks. Off, the blow kills as before.

## Who owns each condition in the slice

A record owns its bits, and ending it clears only bits no other owner holds
(`effect-storage.coax`). Checked by "condition owners" in
`aethyr/rules/srd/hp-interaction-checks.coax`.

| Condition | Slice producers | Owner | Ends |
| --- | --- | --- | --- |
| Blinded | Blindness/Deafness | `EF_COND` by the caster | repeat save, duration, concentration |
| Charmed | Hypnotic Pattern | `EF_DAZED` (with Incapacitated) | damage, shaken awake, concentration |
| Deafened | Blindness/Deafness | `EF_COND` | as Blinded |
| Exhaustion | the cumulative Exhaustion API; no slice spell or monster imposes it | the `exhaustion` level, not a record | Long Rest removes one level |
| Frightened | Turn Undead | `EF_TURNED` (with Incapacitated) | damage, one minute |
| Grappled | none in the slice | `EF_COND` by the grappler | — |
| Incapacitated | Hypnotic Pattern, Turn Undead; implied by Paralyzed, Petrified, Stunned, Unconscious | those records | their owners |
| Invisible | none in the slice | raw bit | Faerie Fire clears it |
| Paralyzed | Hold Person | `EF_COND` | repeat save, concentration |
| Petrified | none in the slice | — | — |
| Poisoned | Ray of Sickness, Cunning Strike, Poisoned stat riders | `EF_COND` | its timer or save |
| Prone | 0 HP, sleep, knockout, Topple, Trip, Wolf bite, Unconscious ending | raw bit, never a record | standing (`prone-movement.coax`) |
| Restrained | Entangle | `EF_ZONE_RESTRAIN` zone lease | leaving/escaping the zone, the zone ending |
| Stunned | Stunning Strike | `EF_COND` | the attacker's next turn start |
| Unconscious | 0 HP; natural sleep; knockout | zero HP while `hp==0`; `EF_REST_SLEEP`; `EF_KNOCKOUT` | healing; waking; Short Rest end, healing or first aid |

## HP interaction fixtures

`hp-interaction-checks.coax` runs the source's worked examples through the shared
entry points: 8 healing at 14/20 restores 6 and never Temporary HP; 5 Temporary HP
and 7 damage lose 2 HP; Temporary HP keep the larger grant (False Life over 0, 5
and 10); Temporary HP at 0 HP don't wake and absorbed damage still costs a death
save; maximum 12 at 6 HP dies from 18 and survives 17; a monster dies at 0 and a
character falls Unconscious and Prone; damage at 0 HP costs one failure, two on a
Critical Hit, death at the HP maximum; three successes stabilize, three failures
kill, a 1 is two failures, a 20 regains 1 HP and leaves Prone; a Stable creature
makes no death saves until damaged; 28 Fire through a -5 aura, Resistance to all
and Vulnerability to Fire is 22; resistances never stack; Immunity wins.

## Checks

| Command | Result |
| --- | --- |
| `coax test aethyr/rules/srd/hp-interaction-checks.coax --library` | 399 pass |
| `coax test aethyr/rules/srd/short-rest-checks.coax --library` | 367 pass |
| `coax test aethyr/rules/srd/condition-roll-checks.coax --library` | 327 pass |
| `coax test aethyr/rules/srd/monster-checks.coax --library` | 355 checks, 354 pass |
| `coax test apps/openagents-web/fivebar-test.coax --library` | 342 pass |
| `coax test apps/openagents-web/fivelab.coax --library --only experimental` | 81 pass |

The checked Lab Wasm build (`coax/web/wasm.sh apps/openagents-web/fivelab.coax ...`)
is 2,563,469 bytes, under its 2,575,000 budget (2,560,786 before).

The one monster-checks failure is the opportunity-attack step-budget check that
already fails on unmodified main `d91c0c69f5`.

Mutation, every `#authority` file touched, zero survivors, no exemptions:

| Unit | Entry | Mutants killed |
| --- | --- | --- |
| `knockout.coax` | `hp-interaction-checks.coax` | 73/73 |
| `stabilization.coax` | `hp-interaction-checks.coax` | 175/175 |
| `hit-points.coax` | `hp-interaction-checks.coax` | 137/137 |
| `effect-storage.coax` | `hp-interaction-checks.coax` | 133/133 |
| `command-halt.coax` | `hp-interaction-checks.coax` | 43/43 |
| `short-rest.coax` | `short-rest-checks.coax` | 235/235 |
| `condition-rolls.coax` | `condition-roll-checks.coax` | 90/90 |
| `effect-admission.coax` | `cast-target-checks.coax` | 170/170 |

`effect-admission.coax` had one survivor from `3458557eed` (a Blinded creature
in a sight-required area); a new check kills it.

## Not done

- Done later (#13030): targeted spells and stat attacks refuse the charmer, and
  Frightened reads line of sight to its source and refuses a willing step closer
  (`condition-roll-checks.coax`). Social Advantage for the charmer has no
  producer in the slice.
- Unconscious "drop whatever you're holding": the engine models only a held
  focus. Sleep and the knockout drop it; dropping to 0 HP doesn't, and no weapon
  is dropped.
- Done later (#13030): a knockout through an Opportunity Attack starts its Short
  Rest when the Reaction ends (`knockout-checks.coax`). An interrupted Short Rest likewise leaves it
  Unconscious until healed or first aid, which the glossary text allows.
- A creature immune to Unconscious, or with all six effect records in use,
  can't be knocked out; the damage applies as dealt.
- Hit Point maximum 0 death has no producer in the slice.
- Petrified: no producer; an existing Poisoned is not removed on petrification.
- Healer's Kit: no inventory acquisition, hand or Thief's Fast Hands rules; in
  Experimental you fight alone, so there is no one to use it on, and it has no
  control there.
