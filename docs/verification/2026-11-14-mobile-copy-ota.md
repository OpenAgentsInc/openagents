# Mobile copy reduction and OTA

2026-11-14. Application source: `2143f180c2`. Comparison: `5fc963163c`,
the app through TestFlight 1085. No native host or protocol changes.

## Rendered copy audit

The same eleven fixed application states contain **73 words versus 292**,
a **75% reduction**. The world-library module contains **7 versus 44**,
an **84% reduction**. These counts cover application chrome, field names and
empty states, excluding accessibility-only icon labels and authored
conversations/documents. They are not a claim about every possible screen or
provider error response. The library's native host header is outside its
module count and remains present.

| State | Before | After |
| --- | ---: | ---: |
| Email login | 25 | 7 |
| Email code | 34 | 6 |
| Home | 17 | 9 |
| Observed conversation | 21 | 4 |
| Project files/notes | 36 | 10 |
| Project draft review | 38 | 7 |
| Publication confirmation | 20 | 8 |
| Usage without computers | 32 | 5 |
| Connect Claude account | 49 | 6 |
| Settings | 12 | 7 |
| Empty Projects | 8 | 4 |
| World library | 44 | 7 |

Checks and reproducible counters:
[mobile](../../apps/openagents-ios/home/mobile-copy-checks.coax),
[worlds](../../apps/openagents-ios/worlds/world-copy-checks.coax),
[counter](../../apps/shared/copy-budget.coax).
The budgets require at least a 50% reduction. Icons use bindings already
installed in 1085; accessible names remain. Public publication and irreversible
deletion still require a separate, clearly labelled confirmation.

## Qualification

- Local Coax compilation/checks and the rendered budgets passed. The new
  JavaScript artifacts match Wasm frames/actions in the independent oracle.
- **12 native/UI checks passed**, zero failures, on iPhone 17 Pro/iOS 26.5
  Simulator: nine native program checks and three installed UI flows covering
  chat/drawer, project review/publish/reopen, and Claude usage/login/selection.
- Qualification used a private copy of the existing 1085 simulator SDK/test
  binaries with the new resources, rather than rebuilding unchanged hosts or
  world producers. Screenshots were exported and visually reviewed. No physical
  phone or real provider account was exercised in this pass.
- `tools-check`, `wire-check`, `layer-check`, and the push's parity,
  state-keep, agent-reference and WGSL gates passed. The hook's additional
  remote Coax suite reached its 120-second limit and was deferred by the hook;
  it is not reported as a passing suite here.
- The broad `components-check` still reports the pre-existing
  `components/core/json-fields.coax` → `coax/lib/json/json.coax` layer violation.
  Both files and the gate are unchanged by this release. Missing local compiler
  path failures were resolved by pointing this worktree at the cached compiler.

## Delivery

Signed generation **2**, 20 resources, for installed runtime
`2d68783d8dcf339c1914ea28688a0baa0ce00c83a695288286aa93aeb3d5b750`
(TestFlight **1085**). Only the app, extras and worlds portable modules and
their Wasm references change. Editor code and world asset inventories retain
their frozen 1085 bytes. No new TestFlight binary is required.

The production Ed25519 signature was independently verified with CryptoKit.
Both public origins serve generation 2, and all twenty artifact sizes and
SHA-256 hashes were verified from each origin after publication.
Public feed/artifact verification is retained with the release artifacts at
`~/.openagents/releases/openagents-ios-1085-copy-ota/` alongside the manifest,
signed envelope, source identity, XCTest summary, screenshots and logs.

The personal gcloud login required reauthentication. Publication used the
repository's existing deployment credential profile without changing the
owner's active CLI account.
