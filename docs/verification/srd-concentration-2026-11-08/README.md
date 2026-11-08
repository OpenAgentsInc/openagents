# Concentration interruption, dismissal and owned teardown

Source: SRD 5.2.1 Concentration, printed p.179; pinned PDF SHA-256
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.
Incapacitated or death ends Concentration. Paralyzed, Petrified, Stunned and
Unconscious include Incapacitated consequences. Ending Concentration voluntarily
requires no action and is unavailable while Incapacitated.

At the starting commit only selected spell branches and HP loss ended concentration.
Accepted Stunned and Hypnotic Pattern effects could leave an old spell active.
The new actual Bless/Stunned regression first failed on that retained ownership
(`red.log`). Tests preceded implementation. A later Stunning Strike fixture
initially chose the default Monk Wisdom and tied the save DC; it now declares
the legal level-4 Wisdom ASI instead of the default Dexterity ASI and checks the
resulting failed save. No production save threshold was changed to make it pass.

## Behavior

Accepted effect insertion now uses the same immunity/Mindless-Rage mask policy
as storage preflight, then synchronizes concentration only after retention.
Immunity and empty conditions are completed no-record outcomes; full storage,
invalid targets and EF_NONE refuse without changing concentration, dice or logs.
The empty condition case no longer reserves a phantom record contrary to its
preflight demand. Direct script callers retain explicit capacity refusal.

During this work, `c20f499fbb` independently landed `effect-lifecycle.coax`,
including immediate concentration/Rage/Turn Undead interruption, and
`73a82fb5d4` landed improved death saves. The rebase preserves both. Final
insertion and interruption use that one shared lifecycle file; the duplicate
insertion path was removed from the new concentration file. The shared insertion
gains logical-index bounds and delegates condition filtering to the existing
admission mask. New concentration reconciliation dispatches that shared hook,
so it retains the incoming feature cleanup rather than implementing a fork.

The shared sync hook ends active concentration when the actor is incapable,
including implicit incapacitating conditions, death or zero HP. It is used by
effect insertion and as a clock/start-turn reconciliation fence. Halt alone
leaves concentration and voluntary dismissal available. Authoritative world
writers that directly change condition/death state must call the hook before
continuing gameplay; this change does not intercept arbitrary structure writes.

Teardown clears the owner and deadline before releasing its tagged zones and
records. Other owners and different spell tags survive. Entangle leases preserve
another zone's restraint, unrelated Guiding Bolt records remain, Faerie Fire
outlines owned by another caster remain, and Spirit Guardians decrements once
with a floor at zero. Repeated cleanup emits no duplicate end event. Release
primitives already handle empty/off records, so redundant active-record tests
were removed rather than exempting unobservable mutants.

Condition messages precede their concentration-end messages. Self-owned
incapacitating concentration effects release themselves immediately. Existing
`rs_choice_continue` policy now guards ordinary recipient loops and effect
program entry/steps too, preventing later dice or retained effects after loss.
Its unchanged 94-mutant receipt from `8415fee4f2` is reused; no source change to
that policy is claimed. The existing damage/DC save resolver is unchanged.

`rs_dismiss_concentration` and `fb_seat_dismiss` need neither an Action, Bonus
Action, Reaction, component nor the actor's own turn. The saved-seat fixture
closes its turn and gives another actor spent budgets; dismissal preserves both
budgets and resource counts. Incapacitated, absent concentration, invalid actors
and unknown executor identities refuse. Unknown IDs no longer produce a
fallback spell's end event. This is not an import/repair API for forged actors.

## Evidence

- Final focused concentration/casting/lifecycle entry: 217 checks pass, including actual Bless,
  Hypnotic Pattern, Stunning Strike, two Entangle owners, separated Faerie Fire
  owners, Spirit Guardians counters, self-interruption, zero HP/death fences,
  immunity/capacity refusal, hidden backing storage and no further program work.
- Final concentration authority: 85 mutants killed; integrated effect lifecycle:
  37 killed. Both have zero survivors and no exemptions. Earlier survivors
  prompted valid executor-identity validation in place of numeric sign-only
  guards. The earlier 115-mutant receipt belongs to the pre-rebase insertion
  layout, not the final two-file policy split.
- Combined hotbar: 189 checks pass. Incoming death saves: 193. Pre-rebase
  probes: 180 and ordinary saves: 175. Their historical logs remain explicit.
  The final core/native builds also include an additive composition fixture:
  raw zero HP ends self-owned Bless before the next battle death save, so no
  stale self-Bless die can leak into that new shared resolver. This also supplies
  whole-entry execution coverage for it. Gameplay procedures did not change
  after the combined hotbar/death checks; unchanged checks were not rerun.
- Experimental: 21 selected scenarios pass; room Cult Priest checks pass.
  Selected checks do not claim whole-entry authority coverage.
- Native CONCENTRATION-LIFECYCLE passes: retained Stunned cancels actual Bless
  benefits immediately; a subsequent tick does not duplicate cleanup; actual
  Fog Cloud dismissal releases its zone without additional action/slot/die costs.
  The final combined strict suite has 18 PASS and one parked Polymorph FAIL,
  exit 1 retained.
- Tools-check and source/compiler hashes accompany the logs. Cached compiler
  0.52.0 is used; no compiler upgrade or complete SRD qualification is claimed.

Commands, all local and below fifteen minutes:

```sh
coax/bin/coax test aethyr/rules/srd/concentration-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/concentration-checks.coax --library --unit aethyr/rules/srd/concentration.coax --jobs 4
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test aethyr/rules/srd/compliance-probes.coax --library
coax/bin/coax test aethyr/rules/srd/saving-throw-checks.coax --library
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat'
coax/bin/coax test apps/openagents-web/everglade5e-room-rules.coax --library --only 'Cult Priests'
coax/web/tool.sh aethyr/rules/srd/compliance-check.coax
coax/web/tool.sh tools-check
```

## Remaining acceptance

#13039 is open. This does not qualify every condition/immunity relationship,
complete Rage/Turn Undead lifecycles beyond the shared interruption, overlapping Faerie Fire provenance,
simultaneous/visibility/area targeting, all compound spell effects or non-spell
concentration features. Longer/ritual/Ready casting workflows and world input
remain #13038/#13044/#13046 work. Dismissal APIs are not a new visible button or
general voice/agent interface. Raw world state writers still need migration to
shared lifecycle methods. No owner action is required.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
