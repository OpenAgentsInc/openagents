# Casting request structure

Partial progress on [#13038](https://github.com/OpenAgentsInc/openagents/issues/13038),
2026-11-08. Baseline `0c082d4d25`. The containing commit and
[source hashes](source-hashes.txt) identify this change.

Unknown executor IDs previously resolved to empty spell records and could be
accepted as cantrips. Requests now refuse before record lookup unless the ID
belongs to the current executable ranges (0–77 or the explicitly separate house
records 100–103). Catalog presence does not imply executable support. Cantrip
requests require slot zero; leveled spells permit slot zero only to reach the
existing feature/free-cast admission, or a sufficient slot from 1 through 9.
The new predicate itself grants neither class access nor a free cast.

The integration regression was written and run first against the old engine:
`coax/bin/coax test --library aethyr/rules/srd/cast-target-checks.coax --only 'malformed casting'`
failed at the assertion that the malformed request cannot cast. After the fix,
the full entry passed **97 checks, zero failed**, exit 0. The regression checks
ordinary and Quickened request refusal and preserves concentration, affinity,
events, dice, target HP, slots and action budgets. Exhaustive small tables cover
the ID gap/boundaries and all supported spell levels versus negative, zero,
sufficient, insufficient and overflowing slot requests.

`coax/bin/coax mutate --library aethyr/rules/srd/cast-target-checks.coax --unit aethyr/rules/srd/cast-request.coax`
passed: **23 mutants, 23 killed, zero survived**, exit 0, no execution-coverage
exemptions. This gate covers the new structural authority, not the full legacy
casting engine. The existing caller-lifecycle tests remain in the entry.

The [hotbar/seat integration](fivebar-tests.txt) passed **109 checks, zero failed**
with `coax/bin/coax test --library apps/openagents-web/fivebar-test.coax`, exit 0.
[Tools check](tools-check.txt) passed, exit 0, and `git diff --check` passed.
Unchanged target-policy mutation gates from `0c082d4d25` were not rerun.

This does not complete #13038. Legal class/prepared loadouts, full components,
ritual/long-cast behavior and explicit multiple-target choices remain open.
