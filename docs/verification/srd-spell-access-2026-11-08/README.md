# Shared spell eligibility and explicit grants

2026-11-08. Baseline `d66abdcdde`. Partial progress on
[#13038](https://github.com/OpenAgentsInc/openagents/issues/13038).
The containing commit and [source hashes](source-hashes.txt) identify this change.

## Behavior and source

Ordinary and Quickened casting now require base-class eligibility or an explicit
creature-local extra-list grant. An unmodified Druid refuses Guiding Bolt without
spending or rolling. Shield and Hellish Rebuke reactions also require access;
their trigger/AI flags alone grant nothing. Rage blocks either reaction cast.
House records require explicit grants, regardless of their display class text.
Helper actions inherit their parent spell's eligibility. Grants accept only
implemented parent IDs and keep the reserved gap unavailable.

The preset grant policy is in the new core authority `spell-access.coax`:
polar Circle of the Land's Ray of Frost (p.46), Fiend patron spells (p.75),
Lore Bard's chosen Fireball and Spirit Guardians discoveries (p.35), Blessed
Warrior's chosen Sacred Flame (p.54), and Draconic Command (p.70). Fiendish Vigor
is effective only when explicitly chosen by a Warlock of level 2 or higher.
Class constructors call that shared policy; a raw hero does not infer subclass
choices. Existing Mage/Priest/Fanatic constructors grant their named implemented
stat-block spells rather than bypassing permission for every Monster.

Everglade's declared custom hero grants precisely the spells on its existing
shared bar, including extra-list and house spells. This preserves its custom
play without granting them to ordinary Druids. The new hotbar regression checks
every grant bit against the bar and verifies isolation from ordinary presets.
No bar slot IDs, sound bindings or persisted class identities changed.

An independent source test checks the class names of all 73 executable parent
records against the imported PDF's base class tables (not the spell description
class masks). It found two stale executable lists: Speak with Animals lacked
Warlock, and Wall of Stone lacked Druid. Those fields are corrected. Helpers
remain under their parents, and legacy `create-water` maps to source ID
`create-or-destroy-water`. This does not reinterpret the nine preserved source
discrepancies; none of the current executable parents has conflicting table
versus description class membership.

## Validation

The Guiding Bolt and reaction-flag regressions were each run against the old
behavior and failed before their admission changes. The source-table check also
failed before the two metadata corrections. Mutation survivors exposed missing
fixtures for the bitset's 64 boundary, the parent ID zero case, invocation
prerequisites and the spellcasting-ability sentinel. Concrete boundary checks
were added. A redundant second record assignment was removed instead of marking
its mutation equivalent. Preset grant rules were moved into the same authority
so its gate covers the new class/level policy as well as access.

| Command | Final result | Evidence |
| --- | --- | --- |
| `coax/bin/coax test --library aethyr/rules/srd/spell-access-source-checks.coax` | 108 passed, zero failed, exit 0 | [Source and core checks](source-and-core-tests.txt) |
| `coax/bin/coax mutate --library aethyr/rules/srd/cast-target-checks.coax --unit aethyr/rules/srd/spell-access.coax` | 132 killed, zero survivors, exit 0 | [Access/preset authority](access-mutate.txt) |
| `coax/bin/coax test --library apps/openagents-web/fivebar-test.coax` | 115 passed, zero failed, exit 0 | [Hotbar/seat checks](fivebar-tests.txt) |
| `coax/bin/coax test --library apps/openagents-web/fivelab.coax --only 'experimental combat'` | 14 selected checks passed, zero failed, exit 0 | [Actual Experimental entry](experimental-tests.txt) |
| `coax/web/tool.sh aethyr/rules/srd/compliance-check.coax` | Six PASS, three FAIL, exit 1 as required | [Native focused witnesses](focused-witnesses.txt) |
| `coax/web/tool.sh tools-check` | Passed, exit 0 | [Tools check](tools-check.txt) |
| `git diff --check` | Passed | Whitespace review |

The Experimental component is loaded through `fivelab.coax`; compiling the
component alone lacks the containing entry's types. Its selected checks do not
claim whole-entry execution coverage. The full source/core entry and scoped
mutation gate have no execution-coverage exemptions. The Experimental compiler
reported 93% compile-time value-slot use; it passed under the unchanged limit.
These are local engine/integration checks, not browser-rendering evidence.
The mutation scope is the new access/preset authority, not all unchanged legacy
engine procedures. Unchanged request/target mutation receipts remain at their
earlier commits and were not rerun.

## Remaining work

Eligibility is necessary, not sufficient for complete legal casting. This change
does **not** select or limit prepared/known spells or cantrips, model a Wizard's
spellbook, separate every grant's provenance, or finish components, rituals,
explicit upcast/multiple targets and per-spell mechanics. An eligible base-class
spell can still pass without a prepared selection. #13038 remains open.
The focused witness now passes CAST-CLASS and CAST-INVALID-TARGET; effect/zone
capacity remain active failures, and Polymorph remains a parked higher-level
failure. No other active or parked issue is claimed complete.

This work includes material from the System Reference Document 5.2.1 ("SRD
5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd,
licensed under Creative Commons Attribution 4.0 International License,
https://creativecommons.org/licenses/by/4.0/legalcode.
