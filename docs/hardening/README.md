# Hardening

This directory owns the design for the OpenAgents Bitcoin OSS hardening
program: the coordination layer that Episodes 263–265 concluded was missing,
built as a public project inside our own Nostr relay.

Status: design and roadmap. Nothing here is admitted implementation work, a
schedule, or a public claim. Today the honest position is one measured
experiment, one upstream PR to Loupe, and zero confirmed vulnerabilities in
anyone else's code.

| Document | Contents |
| --- | --- |
| [`2026-08-04-nostr-native-hardening-program.md`](2026-08-04-nostr-native-hardening-program.md) | The architecture spec: the substrate that exists today, the program mapped onto the All Work NIPs, five new hardening NIPs (SP, SC, FD, SI, BT), the relay/SDK/web/Omega surfaces, the coverage-map disclosure question, and a five-phase roadmap with the Episode 266 demo cut line |
| [`2026-08-04-gpui-on-web-addendum.md`](2026-08-04-gpui-on-web-addendum.md) | Addendum A: could the public projection be Omega's GPUI components compiled to WebAssembly rather than separately authored web components? Measured research — the `gpui_web` backend in our tree, a built wasm bundle, WebGPU reach, the missing accessibility adapter, and where GPUI-on-web is worth spending on instead |

## Where the surrounding material lives

- **The argument.** [`../loupe/2026-08-01-coordination-not-scanners.md`](../loupe/2026-08-01-coordination-not-scanners.md)
  — why the missing layer is coordination rather than another scanner.
- **What to hunt.** [`../loupe/2026-08-01-hardening-against-ai-assisted-attacks.md`](../loupe/2026-08-01-hardening-against-ai-assisted-attacks.md)
  — the ten classes, ranked by whether the attacker gets a free public oracle.
- **The measurement.** [`../loupe/2026-08-01-coldcard-prefix-experiment.md`](../loupe/2026-08-01-coldcard-prefix-experiment.md)
  (pre-registered) and [`../loupe/2026-08-01-coldcard-prefix-experiment-results.md`](../loupe/2026-08-01-coldcard-prefix-experiment-results.md)
  (prediction refuted; submodules were the whole difference).
- **The incident.** [`../coldcard/`](../coldcard/) — postmortems, independent
  analyses, generator reproduction, and the checked-in benchmark.
- **The protocol program.** [`../nips/PROPOSED.md`](../nips/PROPOSED.md) — the
  25 All Work NIPs this program organizes itself with, and where the five
  proposed hardening NIPs would join them.
- **The relay.** `~/work/immortal` — the Rust relay serving
  `relay.openagents.com`, and its `nips/` lanes (official, Block, OpenAgents).
- **The workbench.** `~/work/omega/crates/omega_forensics/` plus
  `packages/forensic-contract/` and `packages/forensic-loupe-adapter/` in this
  monorepo.
