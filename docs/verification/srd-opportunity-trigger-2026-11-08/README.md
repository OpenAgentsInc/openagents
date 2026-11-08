# Opportunity trigger boundary and retained Addle

Partial #13040/#13044 work on f9d51994a2, cached compiler 0.52.0.
Pinned SRD 5.2.1 Opportunity Attacks p.15 and Open Hand Technique p.51.

The authority `opportunity-trigger.coax` provides a read-only trigger query:
a living creature leaves a capable reactor's melee reach while seen, using its
own movement, Action, Bonus Action or Reaction. A spent Reaction, retained
Addle, teleportation or the mover's active-turn Disengage prevents triggering.
Being moved without those own resources (e.g. explosion/falling) does not
trigger. The exact boundary is old distance <= reach, new distance > reach;
zero/extended reach are represented without assuming every creature has five
feet. Logical actor/self and grid-span constraints precede geometry. A negative
reach naturally cannot satisfy nonnegative old-distance <= reach.

Caller facts are explicit and fail closed by default: visibility including
special senses, selected attack's melee identity/reach and movement resource
provenance. They must be bound by an authoritative producer. This is not a
weapon/target/charm authorization or movement request validator. It does not
choose a reactor, move anyone, spend resources, roll, damage or create an
opportunity event. It does not impose a team restriction that the 2024 source's
"a creature" trigger does not state.

Open Hand Addle previously only wrote a message. It now retains
`opportunity_blocked` on the recipient; the actual turn-start hook retires it.
Clock passage cannot consume it and it does not spend/prevent all Reactions.
The query consumes that state and the prior Disengage state without clearing
either one. No ordinary movement call fires attacks yet.

## Evidence

Initial fixtures failed on missing API/state. Final full entry: 195 checks
pass. The policy gate kills 88 mutants, zero survivors/no exemptions. Independent
fixtures exercise all 16 combinations of own-resource causes, teleport/sight,
reach equality, every physical span edge including valid zero coordinates,
logical unused backing records, spent Reaction/incapacity/death, Disengage's
owner and Addle's real-turn lifetime. Full hotbar entry: 224 passes. Selected
Experimental combat: 29 passes. Actual native tool prints
PASS OPPORTUNITY-TRIGGER. Tools-check/diff check pass. Logs/hashes retained.

```
coax/bin/coax test aethyr/rules/srd/opportunity-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/opportunity-checks.coax --library --unit aethyr/rules/srd/opportunity-trigger.coax --jobs 4
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat'
coax/web/tool.sh aethyr/rules/srd/opportunity-native.coax
```

## Required next execution work

A shared Reaction strike must preflight one admitted melee attack and retained
riders before spending, run immediately before departure and preserve the
mover's true active-turn budgets. Simply borrowing the reactor's turn changes
the context used by the defender's own-turn Shield/slot/Quickened policy and is
not sufficient. Strike-local once-per-turn state must stay distinct from that
true budget context, including off-turn feature interactions. Movement must
then re-evaluate any interruption without undoing an accepted Reaction attack.
Authoritative visibility/attack-choice producers and ordinary world adapters
must call that boundary. None of those are replaced by this predicate, and
opportunity attacks still do not execute. No #13040/#13044 closure or gameplay
qualification is claimed; the earlier native room host predates this change.
