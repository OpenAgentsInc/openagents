# Printed melee stat attacks in the shared executor

Prerequisite progress for #13034/#13046, baseline `287eb5b201`, compiler 0.52.0.
Shared source rules: pinned SRD 5.2.1 damage/HP pp.16–18 and monster attack
notation p.185. Fixture attack numbers are explicit declared test records,
not claims of importing a canonical monster. PDF SHA-256:
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

## Contract

`RsStatAttack` carries source/world record ID and name, printed attack bonus,
damage dice and flat modifier, damage type, attack ability and melee reach.
`rs_stat_attack` admits a logical NPC actor in its own turn, with a free Action,
legal capability, a live distinct target in reach and valid metadata. Failure
changes no budget, dice or events. An admitted miss still spends the Action.
Player-class actors cannot use the NPC entry to supply their own printed bonuses.
Record provenance belongs to a trusted catalog/world-profile caller, not UI input.

The existing ordinary weapon strike is now a wrapper over the same resolved
executor. Typed stat attacks supply printed modifiers rather than manufacture
scores or a weapon-catalog ID. An internal nonweapon marker prevents them being
presented as Unarmed Strikes; zero-dice printed damage has no phantom one-point
base. The Sacred Weapon boundary remains limited to an actual melee weapon.
The shared target AC, conditions, saves, reactions, resistances, temporary HP,
concentration/zero-HP lifecycle and existing continuations remain in that executor.

This entry models one simple natural melee attack. It supplies no weapon mastery,
Finesse, secondary damage or record-producing rider, so it needs no rider slot
reservation. Ordinary weapon callers retain their existing capacity preflight.
No new large attack record is stored in every creature or battle slot.

Metadata work bounds are explicit: 0..100 dice, 0..100 sides (positive for dice),
0..1000 feet reach, a valid ability/type and nonempty ID/name. These are executor
limits, not new SRD rules. Printed signed modifiers come from trusted records.
The record ID remains caller metadata; current attack events retain the printed
name rather than a new serialized attack-ID field.

## Verification

The first fixtures precede code; red records the absent typed entry. Tests cover
printed-vs-ability modifiers, critical dice without doubling flat modifiers,
flat zero-dice damage, extended melee reach, Action payment on miss, index zero
and malformed logical actors, wrong turns, incapable actors, dead/self/distant
targets, invalid/boundary metadata and exact printed-name events. Shared target
fixtures exercise resistance/immunity, Uncanny Dodge, prepared Shield, Deflect
Attacks, temporary HP, zero-HP concentration cleanup and critical continuation.

Local commands, all below fifteen minutes:

- `coax/bin/coax mutate aethyr/rules/srd/stat-attack-checks.coax --library --unit aethyr/rules/srd/stat-attack.coax --jobs 4`: 80 killed, zero survivors.
- Same entry with `--unit aethyr/rules/srd/strike-policy.coax`: 8 killed, zero survivors. Initial survivors prompted representation cleanup and boundary/naming fixtures; no exemptions.
- `coax/bin/coax test aethyr/rules/srd/stat-attack-checks.coax --library`: final combined 210 checks pass.
- `coax/bin/coax test apps/openagents-web/fivebar-test.coax --library`: 214 checks pass.
- `coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat'`: all 22 selected checks pass; no filtered authority coverage claim.
- `coax/web/tool.sh aethyr/rules/srd/stat-attack-native.coax`: actual native `PASS STAT-ATTACK`, exit 0. Native and evaluator share a golden scenario, not an independent oracle.
- `coax/web/tool.sh tools-check`: zero new tools without a reason.

Rebased onto `87bd5aa7ec`, preserving incoming shared ability checks/training and
Initiative/escape improvements. The combined core/hotbar/world/native results
above exercise that changed composition. The two policy units remain unchanged,
so their zero-survivor receipts are reused. Source/compiler hashes bind final files.

## Remaining migration

#13034/#13046 remain open. Physical hostiles still call their legacy roll/HP path;
this API is not wired into body contacts yet. The trace also found the player's
per-cast synthetic HP/reset-and-transfer calculation. The next adapter work must
route physical contacts through typed shared attacks, make class HP/temp HP
canonical, and update the HP display/respawn behavior with explicit house policy.
Used-monster source records, Multiattack/secondary riders, ranged/reach-mode
choices, hit visibility, authoritative profile admission and full world flows are
not qualified by this simple melee entry. No catalog or full-SRD completion claim.
