# Zone admission and lifetime

2026-11-08. Baseline `698e29294a`. Partial progress on
[#13041](https://github.com/OpenAgentsInc/openagents/issues/13041).
The containing commit and [source hashes](source-hashes.txt) identify the work.

## Behavior

A lasting area cast now selects its storage slot before spending, rolling,
writing events, resetting the affinity flag or ending concentration. It can use
a free slot or one that replacing its own current concentration will release.
It cannot reclaim an unrelated caster/spell or release concentration for a
non-concentration cast. Accepted casts publish into that selected slot; the old
post-spend allocation could silently leave the accepted area unstored.

The preflight and publication remain inside one serialized cast. Ending the
previous concentration is the only release between those steps. Instantaneous
areas, burst effects and non-area spells do not reserve zone storage. Refusal
preserves action/Bonus Action/Quickened budgets, Sorcery Points, slots, dice,
events, concentration, existing zones and the affinity flag. These are bounded
implementation resources, not an SRD rule limiting zones to eight.

`rs_tick` now expires non-concentration zones at their stored deadlines. Their
terrain stops applying and storage becomes reusable; an end event is emitted
once. Previously they never expired. Shared release also handles concentration
replacement and its restraint binding. It preserves an independently recorded
Restrained condition and does not clear unrelated condition bits. This is not
a qualification of every spell's duration semantics or all overlapping-zone
restraints.

## Verification

| Command | Result | Evidence |
| --- | --- | --- |
| `coax/bin/coax test --library aethyr/rules/srd/cast-target-checks.coax` | 123 passed, zero failed | [Core checks](core-tests.txt) |
| `coax/bin/coax mutate --library aethyr/rules/srd/cast-target-checks.coax --unit aethyr/rules/srd/zone-admission.coax` | 19 killed, zero survivors, exit 0 | [Slot admission](admission-mutate.txt) |
| `coax/bin/coax mutate --library aethyr/rules/srd/cast-target-checks.coax --unit aethyr/rules/srd/zone-lifetime.coax` | 57 killed, zero survivors, exit 0 | [Release/expiry](lifetime-mutate.txt) |
| `coax/bin/coax test --library apps/openagents-web/fivebar-test.coax` | 128 passed, zero failed | [Hotbar/seat checks](fivebar-tests.txt) |
| `coax/bin/coax test --library apps/openagents-web/fivelab.coax --only 'experimental combat'` | 17 selected checks passed, zero failed | [Experimental checks](experimental-tests.txt) |
| `coax/web/tool.sh aethyr/rules/srd/compliance-check.coax` | Nine PASS, two FAIL, exit 1 | [Native focused witnesses](focused-witnesses.txt) |
| `coax/web/tool.sh tools-check` | Passed | [Tools check](tools-check.txt) |
| `git diff --check` | Passed | Whitespace review |

The [admission regression failed before implementation](admission-red.txt).
The final fixture uses an ordinary Druid's Entangle and a Quickened Sorcerer's
prepared Sleet Storm. Positive controls cover full-array concentration
replacement, old restraint release, preservation of unrelated slots,
instantaneous Fireball without storage and publication into the last free slot.
Policy checks cover empty/full spans, owner/spell mismatches and zero identities.

An independent [baseline tick regression](expiry-red.txt) was evaluated against
the `698e29294a` battle source with a temporary appended check: an active Ice
Storm zone with deadline 1 stayed active after `rs_tick(...,1)`. The temporary
source was removed. The new checks cover before/at-deadline expiry, single end
events, reclaimed capacity, independent condition carriers and unrelated bits.
Initial survivors prompted zero-identity and condition-mask boundary fixtures;
none was marked equivalent or exempted. Mutation covers the two new core units,
not all unchanged legacy engine procedures. Unchanged earlier gates were reused.

Rebasing over `8b8b4fe754`, then `ea390e80b8`, brought consumer changes, so the
Experimental check was rerun and includes three new presentation/attention
checks. Earlier passing results are retained [before rebase](experimental-before-rebase.txt)
and [after the first rebase](experimental-after-first-rebase.txt). Changed source
hashes matched after both rebases, so unchanged core gates were reused.

Experimental passed under its unchanged compiler limit at 32,647,413 of
33,554,432 value slots (97%). Selected integration checks do not claim full-entry
execution coverage or browser rendering. Full core and mutation entries have no
execution-coverage exemptions. The compiler binary's hash is retained alongside
the source hashes.

## Remaining work

CAP-ZONE now passes, but CAP-EFFECT still fails and Polymorph remains parked.
The six-entry effect API can still overwrite an independent effect. Its callers
need explicit outcomes and preflight for spell/attack/feature and multi-target
effects. Independent zone restraints still share the legacy single creature
binding; they need owned effect records rather than replacement of that binding.
Those requirements keep #13041 open. This change does not qualify full condition
stacking, all spell lifetimes or asynchronous world casting workflows.
