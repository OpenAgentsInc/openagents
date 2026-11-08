# Shatter and spell save riders

Partial #13043 work on `0bb972d3d`. Source: pinned SRD 5.2.1 p.161 (Shatter).

## Behavior

Shatter executes: a 60-foot point, a 10-foot-radius Sphere, a Constitution save
for half of 3d8 Thunder, 1d8 more a slot level above 2. Class tables (Bard,
Sorcerer, Wizard) match the imported source. Damage to unattended objects is
the world's, as for other records.

"A Construct has Disadvantage on the save" is a save rider:
`save-riders.coax` (`#authority`) maps a SAVE step's `c` field to extra
Disadvantage sources, and `rs_saving_throw_with`/`rs_save_edge_with` join
them with the target's other sources before cancellation, so Rage's Advantage
on a Strength save cancels rather than being overridden. `rs_saving_throw`
and `rs_save_edge` are unchanged wrappers. Creatures gain a `construct` type
flag beside `humanoid`/`undead`; no current stat block is a Construct, so the
flag awaits the used-monster subset (#13034).

The checks found and fixed one pre-existing gap: on `main`, the
`condition-rolls.coax` gate kept one survivor (the Exhaustion save penalty's
sign). A new check fixes saving-throw totals at Exhaustion 0-3 with and
without a rider.

## Evidence

The Construct check failed with the rider unwired (one d20 instead of two)
and passes with it.

```
coax/bin/coax test --library aethyr/rules/srd/slice-spells-checks.coax   # 258/258
coax/bin/coax mutate aethyr/rules/srd/slice-spells-checks.coax --library --unit aethyr/rules/srd/save-riders.coax             # 5/5
coax/bin/coax mutate aethyr/rules/srd/condition-roll-checks.coax --library --unit aethyr/rules/srd/condition-rolls.coax       # 67/67 (66/67 on main before)
coax/bin/coax mutate aethyr/rules/srd/cast-target-checks.coax --library --unit aethyr/rules/srd/cast-request.coax             # 23/23
coax/web/tool.sh aethyr/rules/srd/spell-index-build.coax --check aethyr/rules/srd/data/spell-index.coax                       # matches
```

Composed suites: spells 69, cast-target 279, compliance probes 261,
components 286, spell-access source 285, saving throws 256, condition rolls
258, spell coverage 253, Everglade 5E room 384, hotbar 281 and the 5E Lab 835,
all passing. Level 0-3 coverage: 48 of 183.
