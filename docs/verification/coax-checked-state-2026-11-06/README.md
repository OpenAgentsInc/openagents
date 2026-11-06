# Coax checked state changes — 2026-11-06

Implements Everglade issues [#12952](https://github.com/OpenAgentsInc/openagents/issues/12952),
[#12953](https://github.com/OpenAgentsInc/openagents/issues/12953),
[#12954](https://github.com/OpenAgentsInc/openagents/issues/12954) and
[#12955](https://github.com/OpenAgentsInc/openagents/issues/12955), on the nominal
IDs from #12948. The accepted syntax, contracts and limits are in
[`coax/checked-state.md`](../../../coax/checked-state.md). No deployment is part
of this verification.

## Functional and mutation evidence

| Component | Killed / total mutants | Survivors |
| --- | ---: | ---: |
| Generic handle store | 144 / 144 | 0 |
| Aethyr entity store | 221 / 221 | 0 |
| Everglade lifetime decisions | 37 / 37 | 0 |
| Source effect policy | 203 / 203 | 0 |
| Transaction runtime | 51 / 51 | 0 |
| In-memory ledger | 254 / 254 | 0 |
| Compatibility policy | 306 / 306 | 0 |
| Complete checker, including persisted values | 13,534 / 13,534 | 0 |

These are full component inventories, without equivalent labels. Library and
realm component runs use `--root-only` to mutate that component while retaining
dependency checks. The checker run covers all 1,401 checks and its full inventory.
Raw reports are retained beside this record.

The room suite passes 674/674 and the page suite 777/777. API room qualification
passes, including its 203 room checks. The independent entity oracle retains the
old column/flush behavior. Actual clock and I/O insertions in both shared rule
sources fail with E0941 and a call path.

Native debug and release runs exercise the actual seat leave/reuse path, repair
outbox refusal, third-target fire refusal and ledger outbox refusal. Evaluator
checks also cover the third-target area action, nested scopes, aliases, cleanup
order and owned error values. Native transaction graph checks cover cycles and
deep views; the excessive-depth fixture traps in both native profiles.

The compatibility CLI rejects field removal, insertion, changed existing
defaults, withdrawn persisted markers, changed union alternatives and changed
C ABIs. Defaulted appends, nested defaulted appends and safe view variance pass.
The regression baseline suppresses an unchanged old incompatibility while
refusing a new one. The actual production gate passes offline against retained
production source. Release/deploy syntax and pure revision-selection checks pass;
no live deployment or Cloud Run mutation was run.

The served SRD page Wasm is 2,005,680 bytes against 2,150,000. The classic page
is 1,790,730 against 1,910,000; the worker is 1,023,072 against 1,060,000. Shared host reactors build. Source-linked books pass
their structure checks: Aethyr 48 programs / 211 listings, Psionic 357 programs
plus four native libraries / 1,865 listings, Nostr 89 programs / 787 listings.

## Tick qualification

The native local room runs in its own server, separately from the paired room
and its three bots. Its reported p95 is 450 / 390 / 490 µs for local / 100 ms /
250 ms connections, all within the issue's 840 µs limit. Every gameplay probe
passes across these profiles.

Worker qualification collects every actual duration in completed cost batches
from boot through the paired script. Coax computes the nearest-rank percentile;
the browser only copies observations and rounds them up to microseconds. The
8,192-sample bound, malformed values and missing observations refuse. The raw
CSV samples are retained beside the result.

The earlier periodic attempt is retained in `worker-periodic-attempt.txt`: its
final one-second p95 was 935 / 535 / 510 µs, with the local observation failing
600 µs. That observation exposed the old harness's limited measurement window.
The complete-workload gate keeps the same 600 µs limit and collects every
completed cost batch in each fresh scenario, including startup and the final
window. The first full-workload trace (560 / 551 / 610 µs) remains recorded as
a failed pre-optimization observation. These measurements qualify the authored
paired scenario on this Mac; room capacity retains its separate load checks.

The compiler bootstrap ran with `COAX_MUTATION_FULL=1`. Cached gates retained
their input identities; the full checker mutation and final compiler checks
completed before the timing runs. The installed binary is bound by
`provenance.record`; development-compiler observations have the same compiler
source inputs. Own build/check processes were stopped during the timing runs.

## Reproduce focused gates

```sh
coax/build.sh
coax/tests/run.sh --only handles effect_contracts transactions compat
coax/bin/coax test coax/web/room/room.coax
coax/bin/coax test apps/openagents-web/everglade.coax
coax/bin/coax mutate aethyr/entity.coax --root-only
coax/bin/coax mutate apps/openagents-web/everglade-handles.coax --root-only
coax/bin/coax mutate aethyr/session/ledger.coax --root-only
coax/web/tool.sh coax/tests/everglade-safety-native.coax
coax/web/tool.sh coax/tests/ledger-safety-native.coax
coax/web/tool.sh coax/tests/fire-safety-native.coax
coax/web/tool.sh coax/tests/compat-facades.coax "$PWD/coax/bin/coax"
coax/web/tool.sh coax/tests/compat.coax "$PWD/coax/bin/coax"
coax/web/tool.sh tools-check
```

Each native integration source can also be emitted with `coax c --release
--library`, linked with the existing `tool-native.c` entry wrapper, and run.
The retained native debug/release logs are distinct observations.

Existing authority and boundary procedures receive explicit broad effect
bounds. Narrow shared rule bounds are checked transitively. The 17 reviewed
book capability bindings changed only by these declarations; their audit
retains byte equality after stripping that exact annotation. Prior independent
host/asset receipts retain their original source identities.

Regions, native boundary enforcement, concurrent transactions, callback types
and general memory safety remain outside this initial profile. Spec §3 retains
its existing memory-safety claim.

## Integration with current main

The final integration includes the upstream session-v5 forest replication and
input/drift fixes through `e5c3b69fac`. Forest and placed-object fires, grass
fires and town contact lookups now resolve the captured starter's lifetime;
the forest's replicated starter follows the same decision. The additional
ordinary native entry is `coax/tests/fire-safety-native.coax`.

The release gate uses explicit message/checkpoint facades. The production-source
comparison passes against `1da0654445`; a six-case CLI probe admits private
receive-cache growth and defaulted trailing fields, refuses removed/inserted
message fields, and refuses withdrawn input/owned-hello persisted markers.
The standalone compatibility command retains its complete published-module
checks. The versioned session profile continues to qualify encoded bytes.

Temporary timing counters located about 274 µs of the earlier worker tick in
town setup, mostly river physics (about 304 µs of stepping and 5 µs of float
output in a focused observation). Building only the river C unit with `-O2`
reduced the stepping observation to about 238 µs, with strict floating-point
flags unchanged. Its diagnostic worker grew by about 13 KB. Final qualification
uses the ordinary build without those temporary counters; diagnostic timings
are not a passing budget result.

The final ordinary builds execute the current room/page check closures; the
root suites list 682 and 788 entries after the main integration. The 12 room
town regressions also pass with explicit fixture lifetimes supplied as the
real host supplies them. The accepted-impact regression breaks a real town
piece after replacing its caster and observes an unowned break, rather than
the replacement's seat. Both ordinary native development C at `-O0` and Coax
`--release` C at `-O2` execute the same contact and delayed-impact Coax bodies.
The retained C wrappers only dispatch to those bodies; their generated C input
hashes and the final Wasm/native artifacts are in `current-builds.json`.

These final builds use `COAX_NEAR_LIMIT=100` to avoid serial replays that expand
informational near-limit notes. Compile-time work, storage and traversal limits
remain unchanged; the earlier default-threshold diagnostic notes are retained
in the prior reports. The compiler's qualified source/binary record remains
valid because this integration changes application and tool sources.

A startup-only native attempt used the prior SRD page after rebuilding the
classic target by mistake. Both pages refused the room's session version and
reloaded, so that attempt supplies no gameplay or timing qualification. Its
failure is retained in `native-startup-stale-page.txt`; `/worlds/everglade`
loads `everglade5e.wasm`, which is rebuilt separately for final qualification.
