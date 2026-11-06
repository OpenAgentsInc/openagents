# Coax checked state changes — 2026-11-06

Coax 0.52 implements Everglade issues [#12952](https://github.com/OpenAgentsInc/openagents/issues/12952),
[#12953](https://github.com/OpenAgentsInc/openagents/issues/12953),
[#12954](https://github.com/OpenAgentsInc/openagents/issues/12954) and
[#12955](https://github.com/OpenAgentsInc/openagents/issues/12955), building on
#12948's nominal IDs. The syntax, guarantees and initial limits are in
[`coax/checked-state.md`](../../../coax/checked-state.md). No deployment was run.

## Final gameplay and size qualification

| Host | Local p95 | 100 ms p95 | 250 ms p95 | Issue limit |
| --- | ---: | ---: | ---: | ---: |
| Native | 480 µs | 630 µs | 430 µs | 840 µs |
| Browser worker | 486 µs | 530 µs | 491 µs | 600 µs |

Every paired gameplay probe passes, including movement, observed jumps, spells,
conditions, status text, camera behavior and corrections. The native local room
runs in its own process, separately from the paired room and its three bots.
Native observations are in `native-final.txt` and `native-cost-windows.txt`.
Worker results are in `worker-final-local-raid.txt`, `worker-final-100-raid.txt` and
`worker-final-250-raid.txt`; their raw samples are `worker-final-ticks-*.csv`.

The worker gate covers completed cost batches from boot through the paired
script: 2,078 / 2,044 / 2,047 ticks. The browser rounds observations upward to
microseconds; Coax computes the nearest-rank percentile. Missing, malformed or
overflowing observations refuse qualification. These runs use muted headless
Chrome with background throttling disabled and a 1280×800 viewport. They qualify
this authored scenario on this Mac; capacity retains its separate load checks.
Own builds and other checks were stopped during timing.

Each extended local / 100 ms / 250 ms run passes all four raid verdicts: stables
collapse on both pages, the room reaches level 2 through XP, and its hotbar changes
with the level. Each 600 µs observation covers the paired script before its raid.
The focused 250 ms forest rehearsal passes: both pages see the fire and burnt
tree, and their settled forest digests match (`forest-final.txt`).

| Ordinary release artifact | Bytes | Limit |
| --- | ---: | ---: |
| SRD page served at `/worlds/everglade` | 1,999,484 | 2,150,000 |
| Classic page | 1,784,949 | 1,910,000 |
| Local room worker | 1,053,804 | 1,060,000 |
| Orbit League | 289,987 | 300,000 |

`current-builds.json` retains artifact hashes and checked-build observations.
The served SRD build executes 1,404 checks. The earlier complete evaluator
reports pass 674/674 room checks and 777/777 page checks; final checked builds
cover the integrated sources, whose root inventories are 724 and 801. The
12 room-town regressions pass with the fixture lifetimes supplied by the real
host. API room qualification passes, including its 203 room checks.

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
| Complete Coax checker | 13,534 / 13,534 | 0 |
| Shared network / Orbit / input-policy integration | 900 / 900 | 0 |

These are full component inventories, without equivalent labels. Library and
realm runs use `--root-only` while retaining dependency checks. The complete
checker run covers all 1,401 checks and its full mutation inventory. Raw reports
are retained beside this record.

The independent entity oracle preserves the old column/flush behavior. Native
development and release runs exercise actual seat leave/reuse, contact fire,
replicated forest ownership, accepted delayed impacts, repair outbox refusal,
third-target area/fire refusal and ledger refusal after its first write.
Expired fires can keep burning; they cannot blame the replacement player.
Evaluator and native transaction checks cover aliases, nested scopes, cleanup,
owned refusal values, cyclic graphs and depth limits. Excessive depth traps.
Actual clock and I/O insertions in both shared rule sources fail with E0941 and
a call path.

The compatibility CLI rejects removed/inserted fields, changed defaults,
withdrawn persisted markers, changed union alternatives and changed C ABIs.
Defaulted appends and safe view variance pass. The production gate compares
explicit message/checkpoint facades against retained production `1da0654445`.
Its six-case probe admits private receive-cache growth while refusing changes
to published messages and withdrawn input/owned-hello markers. The ordinary
`coax compat` command still checks complete published module surfaces.
Baseline comparison suppresses unchanged old faults and rejects new ones.
Release/deploy syntax and revision-selection checks pass; no live Cloud Run
mutation was performed.

Shared host reactors build. Source-linked books pass: Aethyr 48 programs /
211 listings; Psionic 357 programs plus four native libraries / 1,865 listings;
Nostr 89 programs / 787 listings. The 17 reviewed book capability bindings
changed only by explicit effect declarations; their audit retains byte equality
after removing that exact annotation.

## Compiler, integration and measurement corrections

The full bootstrap ran with `COAX_MUTATION_FULL=1`. Cached gates retain their
input identities; full checker mutation and final compiler checks completed.
`provenance.record`, `source-digest` and `binary-identities.txt` bind the installed
compiler. Application gates use a development compiler with the same compiler
source inputs. No compiler inputs changed during final application integration.

The integration preserves session-v5 forest/input work through `e5c3b69fac` and
Valley Lab `f255037e9c`, then the shared-network integration through `cff8a2cf2b`.
Valley's 21 newly required effect declarations in eight files preserve its bodies;
the Valley world compiles. The network integration adds 50 effect declarations
in 13 files while preserving their merged bodies. Ten affected authority units
kill all 900 mutants. Orbit's consumer body matches main after removing effect
declarations, and its fresh Wasm build passes the byte gate. Release
metadata passes; tools-check reports 1,833 tracked tool files and zero new files
without a reason.

The admitted worker compiles only the river C unit at `-O3`; other units
remain `-Os`. River C emission enables guarded loop versioning, retaining
checked arithmetic, development branches, debug metadata and the checked
fallback. Its profile participates in the C-cache key. The actual axis report
is `river-emission-profile.json`; source keys and artifact hashes are retained.
Strict floating-point flags remain enabled. Whole-city and combined
worker-wrapper speed candidates exceeded the size gate and were excluded.
On the merged network base, a river-only `-O2` attempt measured 615 / 606 µs
for local / 100 ms. Its failed report and samples are retained; river-only
`-O3` passes all three full profiles within the unchanged byte and tick limits.

Final application builds use `COAX_NEAR_LIMIT=100` to avoid serial replays for
informational near-limit notes. Work, storage and traversal limits stay intact;
earlier default-threshold notes remain in the prior reports.

Failed observations remain reviewable:

- Periodic and initial complete-workload worker failures are in
  `worker-periodic-attempt.txt` and `worker-workload-first.txt`.
- Later background-scheduled failures (660, 680 and 601 µs) retain their reports
  and raw samples. Final runs use the existing unthrottled headless observer.
- `native-startup-stale-page.txt` records rebuilding the classic target while
  accidentally serving the old SRD target; that attempt supplies no qualification.
- `native-control-poll-gap.txt` records independent development hosts applying
  the same hold almost two seconds apart. Atomic development control-file
  publications now reload promptly; production keeps its API polling cadence.
- `native-coalesced-jump.txt` and key records show press/release collapsing into
  one rendered frame. Taps now wait for both input sequences to observe the
  press. A jump witness rejects two missed jumps agreeing with each other.
- The movement-origin regression first fails, then passes: a shared physical
  origin preserves real gaps and avoids inventing gaps after convergence.
- The first extended raid filled the old report bound and omitted the last two
  verdicts. Its report and observations are retained. The report now holds 64
  verdicts and refuses qualification on overflow; a regression retains a late failure. The final
  extended run prints and passes all four raid verdicts.

Existing drift tolerances and the issue's tick limits remain unchanged.

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
coax/web/tool.sh tools-check
# After building the SRD page, worker and native room; run separately:
coax/web/tool.sh everglade-drift --profiles local,100,250 --offline native --no-raid --tick-us 840 --port 8980
coax/web/tool.sh everglade-drift --profiles local --offline worker --tick-us 600 --port 8990
coax/web/tool.sh everglade-drift --profiles 100 --offline worker --tick-us 600 --port 8990
coax/web/tool.sh everglade-drift --profiles 250 --offline worker --tick-us 600 --port 8990
coax/web/tool.sh everglade-drift --profiles 250 --offline native --forest-only --port 8980
```

Native integration bodies are also exercised as ordinary development C at
`-O0` and Coax `--release` C at `-O2`. Retained C wrappers only dispatch to the
Coax bodies; they contain no assertion or game logic.

Regions, native-boundary enforcement, concurrent transactions, callback types
and general memory safety remain outside this initial profile. Store backing
and view lifetimes remain caller obligations. Saved commits stay in services;
in-memory rollback supplies no crash atomicity. Spec §3 keeps its existing claim.
