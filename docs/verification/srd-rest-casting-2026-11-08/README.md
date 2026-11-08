# Accepted casting interrupts Long Rest before payment

SRD 5.2.1 p.185: casting a spell other than a cantrip interrupts Long Rest;
a period of at least an hour grants Short Rest benefits. Pinned PDF SHA-256:
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.
This advances #13040 and casting integration under #13038, not completion.

Ordinary casts finish admission, including components, target, effect and zone
capacity, before applying the interruption. Shared Reaction slot payment does
the same after its spell/component/Reaction/true-turn checks. Recovery happens
before spending the slot: refilling Pact slots after payment would refund the
spell. Empty-target hotbar casting and the hotbar Reaction path share this
ordering. Refusals and cantrips retain rest without recovering banks. Damage
and casting share the owned interruption consumer; casting does not consult
the house damage floor. Repeated causes do not grant recovery twice.

The empty-target path also now pays feature/free spells as ordinary casting
does: it consumes the feature use where required, preserves unlimited Fiendish
Vigor, leaves slot zero intact and does not mark a slot spent. It previously
subtracted from slot zero and marked a slot payment for these free casts.

## Evidence

Checks were added first. The ordinary Pact-casting regression failed at the
retained phase/slot assertion before implementation. The empty-target free
spell regression also failed before the payment fix.

- 330/330 composed Long Rest checks; 311/311 hotbar checks.
- Authority scopes: rest-casting 4/4, shared rest-damage/cause 70/70,
  reaction-slot 58/58 mutants killed; zero survivors or exemptions.
- The Reaction gate uses reaction-frame-checks to retain its existing true-owner
  budget witnesses. Initial narrower scope missed those cases. An unnecessary
  numeric level constant produced a surviving equivalent substitution; the
  Reaction path now calls the common non-cantrip cause directly. No mutant was
  marked equivalent or exempted.
- Compiled native: 24 transactions (ordinary, empty-target, Hellish Rebuke and
  hotbar Feather Fall at each level 1–6), plus accepted/refused true-owner
  borrowed Reaction admission. Recovery precedes payment; another actor's turn
  remains intact. No Reaction renewal is granted.
- Checked Lab Wasm passes in 42.0 seconds, 2,550,713 bytes; SHA-256
  `1b9d6b651bb9ae9f7741cb8164099de866a5ca83016ee6c967d26370fb5dc230`.
  Existing source-version/value-use advisories remain. This temporary artifact
  has no shipping size-budget entry; no budget was raised.
- Tools policy: 1897 tracked files, 35 vendored, zero missing reasons.

```sh
coax/bin/coax test aethyr/rules/srd/long-rest-resources-checks.coax --library
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax mutate aethyr/rules/srd/long-rest-resources-checks.coax --library --unit aethyr/rules/srd/rest-casting.coax --jobs 2
coax/bin/coax mutate aethyr/rules/srd/long-rest-resources-checks.coax --library --unit aethyr/rules/srd/rest-damage.coax --jobs 2
coax/bin/coax mutate aethyr/rules/srd/reaction-frame-checks.coax --library --unit aethyr/rules/srd/reaction-slot.coax --jobs 2
coax/web/tool.sh aethyr/rules/srd/rest-casting-native.coax
coax/web/tool.sh tools-check
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/srd-rest-casting.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
```

## Remaining integration

As with damage, this consumes an already synchronized journal. Scheduled ticks
provide synchronization; manual owners must observe/consume earlier boundaries
before calling admission. No world rest control enables scheduling yet.
Initiative, non-damage death, actual movement/activity/settled observations,
queued optional recovery choices, controller-owned same-time ordering (p.187),
source loss/nutrition producers, inventory dropping and rejoin persistence
remain required. Reaction triggers themselves remain the caller's obligation;
the bar witness does not invent a fall trigger or qualify all Reaction spells.
