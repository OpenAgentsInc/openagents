# Projection-cache closure

This record owns only `coax/lib/compiler/query-projection-cache.coax` and the
new external `coax/tests/results-projection-policy.coax`. Compiler input stays
separate from these tooling-only oracles. The complete original checker
baseline is retained in every scoped run.

**Final result: 286 mutants, 286 killed, zero survivors**, in 184.95 seconds.
All 1,201 baseline checks passed under the stock mutation command.

The earlier scope had 75 surviving mutants. Seven independent oracle groups
cover 87 cases: exact committed-prefix endpoints; source/memo identity misses;
individual malformed field-row components and refusal diagnostics; array/view
route and terminal-type admission; complete cold/hit witness publication; and
refresh of the exact two-entry cache key without evicting the other working
set. Excluded physical storage is initialized with otherwise valid values so
a missing committed-prefix bound cannot be hidden by a later tag check.
No check asserts a private return value after fatal refusal.

Checks preceded changes. `oracles-v1.stdout` records the initial focused green
checks. `oracles-v2-mutation` mutates the original production procedures with
the added independent oracles: 290 mutants, 279 killed, 11 surviving. The raw
report preserves those exact remaining sites.

The phase cleanup preserves every admission guard, resource call and work
charge. `CB_projection_hit` reports the already established fatal flag as its
unspecified private result after resource refusal. The caller checks fatal
before considering that result. Other fatal returns use the already available
field/type value, whose private value remains unspecified.

On the eligible cold route, `retain_fields` is true. The retained `CB_field`
branch has no ordinary not-found return: zero follows `CK_resource`, a failed
`CB_tick`, or `CB_at`'s fatal refusal. Testing the canonical fatal phase after
`CB_field` therefore preserves immediate stop before another field read or
record publication. The exact comparison and return were simplified without
removing this guard. `CB_query` checks fatal before using the projected type.
`phase-v3-baseline` records all 1,201 checks passing after that cleanup.

The final current-module scope is recorded separately with its raw result and
pins. Its procedures are copied verbatim into the physical root; struct
definitions stay in the loaded dependency. The new external oracles run first
and load all original checker checks afterward. The stock `mutate --root-only
--jobs 4` command uses no mutator changes, survivor exclusions, equivalent
labels, increased quotas, or changed borrow-path limit.

`record.json` binds the final result, exact production and external oracle
hashes, producer, commands, complete dependency manifest and empty process-group
readback. Normal matching-image/bootstrap and actual full-compiler cost
qualification belong to the coordinating root record after all source owners
freeze.

The final post-audit rerun drops only assertions about unused private hint
words after fatal refusal. Fatal/code/location checks remain. The stock full
module rerun still kills all 286 mutants; `post-audit-v5-mutation` is the final
receipt. Earlier snapshots remain bound to their original oracle bytes.
