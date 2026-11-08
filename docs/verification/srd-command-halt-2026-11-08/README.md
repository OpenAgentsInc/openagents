# Command Halt: target-turn ownership

Pinned SRD 5.2.1 Command, printed page 116, gives the instruction on the target's
next turn. Halt prevents movement, Actions and Bonus Actions; it does not impose
Incapacitated or prohibit Reactions. Source PDF SHA-256 remains
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

The previous executor immediately blocked actions, reactions and passive
features and used elapsed time to retire the instruction. Pending records now
reserve storage until target-turn start promotes them. Turn end retires active
records only; a newly queued instruction is preserved for the following turn.
Clock advancement cannot consume either phase. Independent records retain their
owners. The displayed deadline is an estimate, never the expiry authority.

General capability is distinct from permission to act on the commanded turn.
Shield/Rebuke, defensive reactions, Aura of Protection, Danger Sense and Rage
use general capability. Attack, spell Action/Bonus Action, feature actions and
escape still respect Halt. The shared hotbar and room admission preserve this
distinction. The existing generic Reaction handler now pays through shared slot
history/readiness/Rage guards; its fixture supplies a trigger and does not
qualify Feather Fall's physical falling behavior or trigger detection.

## Evidence and limits

Checks preceded implementation: `red.log` demonstrates premature action denial.
`mutation.log` records all 31 new command-halt authority mutants killed, zero
survivors and no exemptions. Functions were unchanged after this passing gate;
a later capacity check and an alive lifecycle fixture improve composed coverage.
Final combined logs and source hashes record integration with independent HP
changes from `dc86ad5b8f`; its unchanged HP/storage mutation evidence remains in
the sibling hit-points verification directory.

Combined results: 179 core, 164 hotbar, 165 HP, 154 roll, 157 probe and 40
composed spell-entry checks passed. Experimental passed 17 selected checks;
the room's Cult Priest Command fixture passed. These selected world runs do
not claim whole-entry authority coverage. Native: 14 PASS, one parked Polymorph
FAIL, exit 1 retained. The compiler is the hashed cached 0.52.0 release.

Commands: `coax/bin/coax test ENTRY --library` for cast-target-checks,
fivebar-test, hit-point-checks, effect-roll-checks, compliance-probes and spells;
`coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat'`;
`coax/bin/coax test apps/openagents-web/everglade5e-room-rules.coax --library --only 'Cult Priests'`;
`coax/web/tool.sh aethyr/rules/srd/compliance-check.coax`;
`coax/bin/coax mutate aethyr/rules/srd/cast-target-checks.coax --library --unit aethyr/rules/srd/command-halt.coax --jobs 4`;
`coax/web/tool.sh tools-check`. Each command completed within 15 minutes.

The strict native witness remains intentionally failing overall for parked
Polymorph; COMMAND-HALT-TURN passes. Bare engine.coax remains a composition
fragment without spell-record definitions; its documented check entry is now
spells.coax. No cyclic load or suppressed authority check was introduced.

Command's Approach/Flee need movement and turn control, Drop needs held-item
ownership, and Grovel still needs its own faithful executor. These options,
general action/movement rules, other conditions, complete casting workflows,
world choice surfaces and profiles remain open. This closes neither #13040 nor
#13043 and claims no complete spell or world qualification.
