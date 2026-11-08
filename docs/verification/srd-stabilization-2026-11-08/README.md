# First aid and stable recovery

2026-11-08, baseline `87bd5aa7ec`, final integration base `76c6b8624a`.
Partial #13039/#13040 progress. Neither issue
is complete. The containing commit and [hashes](source-hashes.txt) identify the
verified source/compiler. No owner action is required.

## Source and behavior

Pinned [SRD 5.2.1](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.1.pdf),
SHA-256 `8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`,
pp.17–18: Help can stabilize a creature at zero HP with a DC 10 Wisdom
(Medicine) check. Becoming Stable resets both death-save counts, prevents further
death saves, does not heal and leaves zero-HP Unconscious intact. Damage ends
stability even if temporary HP absorb the damage. Without earlier healing a
stable creature regains one HP after 1d4 hours. The public Basic Rules chapter
was also read; the pinned source remains the acceptance reference.

`stabilization.coax` owns Action admission, first aid and the recovery policy:

- `rs_first_aid` returns the shared check result so admission is distinct from
  success. Invalid actors, no physical contact, unavailable Actions and targets
  that are dead, above zero or already stable refuse before dice/resources/events.
  An admitted attempt spends its Action even on failure. It uses actual selected
  Medicine proficiency, conditions, Exhaustion and existing rescue spending;
  stabilizing does not require proficiency and is not healing.
- `RsFirstAidContext` supplies authoritative physical-contact and task/sense/
  circumstance facts. These are adjudicator/world facts, not a new geometry
  oracle. The Help skill/tool/attack Advantage workflows are separate work.
- A successful attempt resets both counters and starts a new recovery episode.
  The actual third battle death-save success does the same. Each episode rolls
  one d4 and stores its logical-clock deadline in internal `RsCreature.stable_due`;
  no spell-effect slot or fallible timed-effect insertion is required. This
  changes internal layout and later dice sequence consumption, not protocol fields
  or frozen Character Create save keys.
- `rs_add` arms an admitted stable creature, including one stabilized through the
  standalone primitive, and preserves an existing restored deadline. Direct
  borrowed-state/raw producers must call `rs_stable_arm` when they establish
  clock ownership; tick/turn reconciliation is a fallback, not knowledge of when
  an unreported transition happened. Restored deadlines require the same clock
  origin; cross-world clock conversion/persistent progression remain separate.
- Tick recovers after ordinary effects expire; actual turn start also reconciles.
  The deadline does not reroll on later ticks. Damage, death or earlier healing
  makes the timer inactive; a later new stable episode replaces the old deadline.
  No-healing effects postpone actual HP recovery without rerolling. Healing keeps
  another owner's Unconscious effect and Prone, and one successful recovery emits
  one HP event. It cannot revive a dead creature.
- `fb_seat_first_aid` uses the same policy with the player's saved Action budget,
  then restores the other actor's turn. It supplies an adapter API, not a new
  visible hotbar slot, keyboard binding, wire command or phone control.

## Evidence

- [Red recovery](red.log): the authored stable-recovery scenario failed before
  implementation. [Red admission](admission-red.log): admitting a standalone stable
  creature failed before connecting admission to the timer policy.
- [Core](core.log): 251 checks pass, composing the new scenarios with existing
  condition, HP, death-save and Exhaustion fixtures. Independent replay seeds
  `[1,6,2,13]` produce d4 faces `[1,2,3,4]`, computed using a separate BigInt
  SplitMix64 calculation on the high 32 bits; exact dice counts/state, deadline
  boundaries and one-time recovery are asserted. Context, refusal, failed attempts,
  resources, metadata, death/interruption, independent condition owners and
  restabilization are covered.
- [Stabilization mutation](stabilization-mutation.log): 112 killed, zero survivors.
  [Battle death-save mutation](death-mutation.log): 32 killed, zero survivors.
  No exemptions. Initial survivors exposed missing actor/recipient-zero controls,
  event damage metadata, return-value assertions on inactive/blocked recovery,
  and stale-episode reset coverage. The death-save gate composes the Exhaustion
  fixtures so natural-twenty rescue preservation is checked even after penalties.
  These receipts precede the final admission-only hook/fixture; both gated policy
  units and compiler remain byte-identical and their passing results are reused.
- [Native record](native.json), [summary](native.log): all fifteen root `#test`
  scenarios pass in ordinary emitted checked C at O0/O2. This runner does not
  establish sanitizer or rendered-world qualification.
- [Hotbar/saved seat](hotbar.log): 216 checks pass. [Experimental](experimental.log):
  15 selected combat checks pass of 737 on the final merged tree. Ordinary
  non-stable actor admission does not roll a recovery die.
- [Tools check](tools.log) passes; no compiler change or new non-Coax tool.

Rebase preserved the incoming printed NPC melee/strike work and both invariant/
ledger records. The [pre-rebase hashes](pre-rebase-source-hashes.txt) match all
eleven owned/dependency/compiler files except the expected shared battle change;
neither gated policy changed. Core, hotbar, native and selected Experimental
checks were freshly run on that composition. Unchanged mutation/tool evidence
is reused.

Commands have 180-second test/tool and 600-second mutation/native limits:

```sh
coax/bin/coax test aethyr/rules/srd/stabilization-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/stabilization-checks.coax --library --unit aethyr/rules/srd/stabilization.coax --jobs 4
coax/bin/coax mutate aethyr/rules/srd/stabilization-checks.coax --library --unit aethyr/rules/srd/death-save-roll.coax --jobs 4
python3 coax/tools/native_checks.py aethyr/rules/srd/stabilization-checks.coax --out /tmp/oa-stabilization-native-final.json
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat:'
coax/web/tool.sh tools-check
```

## Remaining acceptance

Healer's Kit acquisition/charges and its no-check stabilization option, nonlethal
knockout/Short Rest recovery, general Help benefits, full rest admission and world
interaction controls are not implemented by this slice. Full condition provenance,
visibility/alternate senses, remaining class bonuses/actions and all reachable
world lifecycles still need the parent issues. Coherent storage, legal scores/
resources and a monotonically advanced battle clock remain caller duties.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
