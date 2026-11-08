# Simultaneous saving-throw damage

Partial progress on #13039/#13043, based on SRD 5.2.1 p.16, Damage Rolls /
Saving Throws and Damage / Damage against Multiple Targets. Pinned PDF SHA-256:
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.
Baseline `45069b20ee`, compiler 0.52.0.

## Behavior

One request-local scope retains a damage roll per effect instruction across
simultaneous save recipients. Every recipient still makes its own save and
applies its own resistance/immunity/vulnerability. Flat, affinity, Agonizing
and Enfeeblement modifiers belong to the roll before save halving. The scope
is allocated per cast, Ice Knife burst or zone pulse, never on global battle
state, so nested calls cannot overwrite it. Attack rays and separate trigger
invocations remain independent; separate damage instructions retain separate
rolls. Single-recipient legacy APIs allocate their own scope.

The original Fireball witness failed: two saves plus two 8d6 rolls consumed
18 dice instead of ten. The fixed witness checks failed/successful saves and
ten dice. A level-six Sorcerer witness checks affinity, Enfeeblement and
save-then-resistance. Policy fixtures check instruction independence, attack
independence, fresh scopes, zero floor and random-stream d8 parity. A hotbar
Moonbeam regression checks two consecutive pulses, each with two saves and
one damage roll. Existing chosen-ray checks remain in the full composition.

## Verification

- `coax/bin/coax mutate aethyr/rules/srd/shared-damage-checks.coax --library --unit aethyr/rules/srd/spell-damage-rolls.coax --jobs 4`: all baseline checks passed; 20 mutants killed, zero survivors. An initial d8-to-d9 survivor prompted a varying-seed independent d8 fixture; no exemptions.
- `coax/bin/coax test apps/openagents-web/fivebar-test.coax --library`: 193 checks passed.
- `coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat'`: 21 selected checks passed.
- `coax/web/tool.sh aethyr/rules/srd/compliance-check.coax`: 18 native witnesses pass, one deferred Polymorph witness fails; exit 1 retained. This existing suite does not independently witness the new shared-damage behavior.
- `coax/web/tool.sh tools-check`: zero new tool files without a reason.

All commands finished below fifteen minutes. The retained red is the original
source-derived failure; final mutation baseline, hotbar and world logs describe
the fixed behavior. No full engine mutation or complete SRD qualification is
claimed. Source hashes bind the measured source.

## Remaining

These issues remain open. This does not qualify area geometry, every spell's
source program, simultaneous HP/event scheduling, all feature timing, or the
existing affinity-on-persistent-spells policy. Per-creature Spirit Guardians
entry/turn triggers are independent invocations, not one perpetual shared roll.
No spell IDs, sound bindings or supported catalog scope changed.

## Rebased composition

Rebased onto `9379d66b5f`, preserving the independent Dodge lifecycle change
`3009a8557b` and Everglade networking work. The changed shared composition then
passes 205 core checks and 196 hotbar checks. Re-running those checks is warranted
by the incoming battle/Dodge/hotbar changes. The roll policy source is unchanged;
its passing 20-mutant receipt is reused. Original source hashes are retained
separately from the final combined-tree hashes.
