# Hardening

This directory owns the design for **Operation Diamond Hands**, the OpenAgents
Bitcoin OSS hardening program: the coordination layer that Episodes 263–265
concluded was missing, built as a public project inside our own Nostr relay.

Status: design, protocol drafts, and roadmap. Nothing here is admitted
implementation work, a schedule, or a public claim. Today the supported
position is one measured experiment, one upstream PR to Loupe, five drafted
hardening NIPs, and zero confirmed vulnerabilities in anyone else's code.
The first delivery slice is a basic project page at `/dh`. Its project record,
status, latest update, and recent activity come directly from
`relay.openagents.com` over a Nostr WebSocket opened inside the browser.

| Document | Contents |
| --- | --- |
| [`2026-08-04-nostr-native-hardening-program.md`](2026-08-04-nostr-native-hardening-program.md) | The architecture spec: the substrate that exists today, Operation Diamond Hands mapped onto the All Work NIPs, five hardening NIP drafts, the relay/client/web/Omega surfaces, the coverage-map disclosure question, and the roadmap beginning with the direct-relay `/dh` page |
| [`2026-08-04-gpui-on-web-addendum.md`](2026-08-04-gpui-on-web-addendum.md) | Addendum A: measured GPUI-on-web research; §11 records Omega's real `ui` components and Aiur theme running in a browser, §12 records the Rust/GPUI owner decision, and §13 fixes the first `/dh` delivery slice |

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
  25 All Work NIPs this program organizes itself with and the five drafted
  hardening NIPs: [`SP`](../nips/SP.md), [`SC`](../nips/SC.md),
  [`FD`](../nips/FD.md), [`SI`](../nips/SI.md), and [`BT`](../nips/BT.md).
- **The relay.** `~/work/immortal` — the Rust relay serving
  `relay.openagents.com`, and its `nips/` lanes (official, Block, OpenAgents).
- **The workbench.** `~/work/omega/crates/omega_forensics/` plus
  `packages/forensic-contract/` and `packages/forensic-loupe-adapter/` in this
  monorepo.
