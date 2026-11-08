# One admitted opportunity strike, before departure

SRD 5.2.1 p.15 calls for one melee weapon attack or Unarmed Strike as a
Reaction immediately before reach departure. Source is the pinned PDF with
SHA-256 `8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

`rs_opportunity_attack` consumes the checked trigger, validates represented
weapon identity/reach and owned Charmed effects, preflights exactly one
strike's storage, and spends the Reaction even on a miss. Refusal rolls back
all state, including the borrowed frame, rolls and events. It neither moves
the victim nor grants an Attack Action, Extra Attack or Cleave.

A borrowed strike context preserves the actual mover's Action, Bonus Action,
movement, once-per-turn marks and spell budget. Own-turn strikes retain their
marks. Shield observes the actual owner's spent-slot/Quickened restrictions
and retains its correct next-turn duration. Off-turn weapon riders use the
reactor's upcoming turn without an extra borrowed turn. An opportunity kill
releases Hex for that upcoming turn. Returned strike state exposes follow-up
movement credit; it is not a second authoritative action budget.

## Evidence

Tests were written before implementation. The additional Shield-duration
regression failed on the borrowed-context implementation before correction
(`shield-red.log`). The composed entry passes all 202 checks; a subsequently
added actual Hex-kill fixture passes separately (203 total listed). The frame
entry passes 198 checks. Hotbar passes 227 checks and selected Experimental
combat passes 29 checks. Native execution prints PASS OPPORTUNITY-ATTACK,
including all four actual Shield slot/Quickened combinations and duration.

Core gates: reaction-frame 53/53 killed, reaction-slot 57/57 killed. The
unchanged opportunity executor's earlier gate killed 42/42; no exemptions.
Tools-check passes (1,897 tracked tool files); diff check passes. Compiler
0.52.0 reports the existing newer-source advisory; no bootstrap was run.

```
coax/bin/coax test aethyr/rules/srd/opportunity-attack-checks.coax --library
coax/bin/coax test aethyr/rules/srd/reaction-frame-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/reaction-frame-checks.coax --library --unit aethyr/rules/srd/reaction-frame.coax --jobs 4
coax/bin/coax mutate aethyr/rules/srd/reaction-frame-checks.coax --library --unit aethyr/rules/srd/reaction-slot.coax --jobs 4
coax/web/tool.sh aethyr/rules/srd/opportunity-attack-native.coax
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat'
coax/web/tool.sh tools-check
```

## Remaining integration

Ordinary movement does not yet call this executor. Authoritative consumers
must supply visibility, own-resource movement provenance and attack choice,
resolve before departure, and re-evaluate movement after accepted damage or
interruption without rolling back the Reaction. Natural attacks and extended
monster weapon records require their typed entry; this API covers only the
represented ordinary melee weapon bank and Unarmed Strike. Caller facts do
not establish unimplemented sight/sense rules. General saved-seat/clock
lifetime qualification remains separate; normal non-borrowed timing semantics
are preserved. No #13040/#13044 completion or world qualification is claimed.
