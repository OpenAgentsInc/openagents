# Shared casting target outcomes

Date: 2026-11-08, America/Chicago. Baseline `98de0c631e`.
This is partial progress on [#13038](https://github.com/OpenAgentsInc/openagents/issues/13038)
in the reordered [Everglade slice](../../dnd/srd-compliance-roadmap.md).
The commit containing this record and [source hashes](source-hashes.txt) identify
the change. No issue is claimed complete by this target-only fix.

## Behavior

Under SRD 5.2.1 p.106, a cast at a creature that cannot be affected can still
spend its used slot. Hold Person (p.141) on a Skeleton now consumes the action
and slot, but applies no condition and rolls no save. It reports an apparent
successful save without inventing a die result: `RS_EV_SAVE`, amount 0, success
in the event's damage field. Its log says only that the target succeeded.

Malformed target indices, dead targets and out-of-range requests retain the
existing refusal contract. Their slots, action budgets, dice, events,
concentration and affinity-use flag remain unchanged. The affinity flag used
to be reset before target preflight; it now resets only after preflight succeeds.

A new concentration cast ends the caster's previous concentration normally.
The current single-target Hold Person has nothing to sustain on an invalid
target, so its new concentration is cleared. Legal Humanoids still make normal
saves. Quickened invalid-type casts pay both the slot and two Sorcery Points.
The outcome helper itself preserves unrelated concentration and target state.

## Checks and mutation scope

| Command | Result | Evidence |
| --- | --- | --- |
| `coax/bin/coax test --library aethyr/rules/srd/battle.coax` | 92 passed, zero failed | [Shared battle checks](battle-tests.txt) |
| `coax/bin/coax test --library apps/openagents-web/fivebar-test.coax` | 107 passed, zero failed | [Existing hotbar/seat integration](fivebar-tests.txt) |
| `coax/bin/coax mutate --library aethyr/rules/srd/cast-target-checks.coax --unit aethyr/rules/srd/cast-target.coax` | 7 killed, zero survivors; exit 0 | [Core target decision](target-policy-mutate.txt) |
| `coax/bin/coax mutate --library aethyr/rules/srd/cast-target-checks.coax --unit aethyr/rules/srd/cast-target-outcome.coax` | 24 killed, zero survivors; exit 0 | [Core unaffected-cast outcome](target-outcome-mutate.txt) |
| `coax/web/tool.sh aethyr/rules/srd/compliance-check.coax` | Exit 1: five PASS, four FAIL | [Focused witnesses](focused-witnesses.txt) |
| `coax/web/tool.sh tools-check` | Passed | [Tools check](tools-check.txt) |
| `git diff --check` | Passed | Whitespace review |

The new checks were first run against the old behavior: all three initial
target scenarios failed. The old Skeleton test asserted request refusal;
it now asserts spent resources and no Paralysis. No source expectation was
weakened to preserve that incorrect behavior.

Initial unit mutation runs killed every selected mutant but exited nonzero
because the compiler's whole-entry execution coverage found nine existing
public wrappers unused by the battle entry's checks. `cast-target-checks.coax`
adds meaningful caller-owned-buffer/turn/casting/logging and Monk shared
Bonus Action/Dash scenarios that exercise those interfaces. Both final scoped
mutation commands exit zero with no uncovered procedures. No equivalent-mutant
exemption or coverage suppression was used. The mutated units are the two new
core target units; this is not a claim that every unchanged procedure in the
legacy engine was mutation-qualified here.

The four current witness failures are class access, effect capacity, zone
capacity and Polymorph. The first three remain active slice work; Polymorph is
outside the active spell-level limit. Passing this target witness does not
qualify full casting, upcast/multiple target selection, components, preparation,
every condition or world integration. The existing hotbar checks are native
engine integration evidence, not a browser-rendering claim.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
