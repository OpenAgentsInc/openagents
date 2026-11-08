# Immediate interruption when an incapacitating effect takes hold

2026-11-08. Baseline `127e8032b4`; integration base `be4578fb4a`. Partial
[#13039](https://github.com/OpenAgentsInc/openagents/issues/13039) progress.
The containing commit and [source hashes](source-hashes.txt) identify the tree.

Sources: pinned [SRD 5.2.1](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.1.pdf),
Rage p.29, Mindless Rage p.30, Turn Undead p.37, Concentration p.179,
Incapacitated p.184 and the incapacitating conditions on pp.186, 189, 191.
The source PDF SHA-256 remains
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

`effect-lifecycle.coax` now owns battle effect insertion and its immediate
interruptions. After a record is retained, actual creature capability determines
whether concentration, its Turn Undead ownership and Rage end. Incapacitated,
Paralyzed, Petrified, Stunned and Unconscious all reach the same boundary.
Concentration release retires that caster's records, zones and outlines while
preserving other owners. Ending the condition later does not resume concentration.

Storage refusal and immune no-record outcomes do not run interruptions. The
existing condition immunity and Mindless Rage filtering behavior is retained.
Payload bits in Hex, boons and Command Halt are not interpreted as conditions.
The caller-specific timed-spell and Stunning Strike interruptions are removed;
Stunning Strike therefore no longer ends an immune target's Rage. Dropping to
zero HP also uses the shared interruption boundary, ending Rage immediately.

No protocol or saved-layout changes. Storage stays in its existing authority;
the new authority adds battle lifecycle around successful insertion rather than
making low-level storage recursively operate on a battle.

## Checks

- [Red](red.log): five failures reproduce condition concentration loss, Rage/
  Turn Undead release, actual Stunning Strike versus Bless, immune Stunned
  preserving Rage and immediate zero-HP Rage loss. Two negative scenarios pass.
- [Final core](core.log): all 168 checks pass after the
  component/grant rebase. The property enumerates all 15 condition bits through
  three condition carriers, including independent record/zone owners and
  outlines. Added cases check full storage, immunity, noncondition carriers,
  real Bless/Stunning Strike and nonresumption after condition removal.
- [Mutation](mutation.log): all 50 mutants in the new authority killed, zero
  survivors and no equivalent exemptions. Ten initial survivors prompted
  fixtures for every immune carrier, noncondition payloads with matching
  immunity, and the Mindless Rage carrier/mask/state matrix. The final policy
  bytes/compiler were unchanged by the upstream components/grants rebase;
  its passing mutation result is reused. Complete evaluator and native
  integration ran again because the component and creature dependencies changed.
- [Native](native.json), [text](native.log): all seven root `#test` scenarios
  pass at O0/O2 in ordinary emitted checked C. The 15-case `#forall` remains
  evaluator/mutation evidence, not native. No sanitizer claim.
- [Hotbar](hotbar.log): 183 checks pass; [components](components.log): 201 pass;
  [Experimental](experimental.log): 14 selected combat checks pass out of 707
  on the combined tree. No rendered browser/phone claim.
  [Tools check](tools.log) passes.

Commands (180-second checks/tools, 600-second mutation/native limits):

```sh
coax/bin/coax test aethyr/rules/srd/effect-lifecycle-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/effect-lifecycle-checks.coax --library --unit aethyr/rules/srd/effect-lifecycle.coax --jobs 4
python3 coax/tools/native_checks.py aethyr/rules/srd/effect-lifecycle-checks.coax --out /tmp/oa-effect-lifecycle-native-final.json
coax/bin/coax test aethyr/rules/srd/component-checks.coax --library
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat:'
coax/web/tool.sh tools-check
```

This does not complete #13039 or all condition clauses. Raw condition writes
outside battle insertion remain the caller's responsibility; a general loaded-
snapshot reconciliation boundary is not introduced. Full condition immunity/
compound dependencies, poison suspension while Petrified, speech/drop/falling
consequences, Dodge lifecycle, initiative penalties, Exhaustion, ability checks,
death-save modifiers and other HP/recovery acceptance remain. The existing
single-condition and compound immunity conventions are preserved, not newly
qualified as complete source semantics. Helpers assume admitted indices and
tracked creature/effect state. No owner action is required.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
