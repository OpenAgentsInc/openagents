# Nostr adoption audit coverage

This is the package disposition appendix for the [audit](README.md), at source
`b152f145e5`. It is not a per-line code review or a claim that each package was
executed. The [manifest inventory](inventory.json) supplies the exact 44-package
enumeration. Finding IDs refer to the audit.

## Workspace packages

| Package | Observed boundary | Recommended disposition |
| --- | --- | --- |
| [atif](../../../crates/atif) | Append-only local trajectory and document format. | Keep pure/local; a host exporter binds exact source records into RUN/CTX artifacts. A01. |
| [capability](../../../crates/capability) | Local registry, exact host binding trust, supervised probes; Nostr definitions supported. | Add admitted relay catalog acquisition; preserve local probe authority. A05. |
| [coder](../../../crates/coder) | Real conversational CJ; local task/program/package runtime; decision relay profile refused; generic execution dispatch unconnected. | Complete decision caller, one remote operation, and durable continuity. A01–A03, A05–A06. |
| [coder-boundary](../../../crates/coder-boundary) | Filesystem boundary and independent snapshots. | Keep enforcement local; ENV/CAP describes supported remote use. A06. |
| [coder-connect](../../../crates/coder-connect) | Private Nostr retained-history reader and QR bootstrap. | Preserve it; add separate managed-session and reader-state profiles, not implicit execution permission. A01, A13. |
| [coder-control](../../../crates/coder-control) | Nostr grants, commands, finite task views, and durable retry state. | Reuse as the control seam; extend RUN evidence and actual mobile consumption. A01, A13. |
| [coder-history](../../../crates/coder-history) | Bounded local Codex/Claude discovery and transcript readers. | Keep host-side import; expose only owner-granted sources through the existing connection. A13. |
| [coder-labor](../../../crates/coder-labor) | Persisted free-order MKT/LAB host and transport fixtures. | Add independent provider operations and separately admitted settlement. A11. |
| [coder-mobile](../../../crates/coder-mobile) | Rust-owned history/cache and shared Verse; Nostr through connection/session/Gym dependencies. | Add task-control/session clients, permitted read-state sync, and chosen world parity. A13–A14. |
| [coder-mobile-probe](../../../crates/coder-mobile-probe) | Separate synthetic platform feasibility probe. | Keep local; do not mistake the probe for a missing live mobile transport. |
| [coder-one](../../../crates/coder-one) | HTTP decisions/generation, local executors, real optional Wasm guests, local studies/traces. | Wrap admitted operations and export exact RUN/EVAL/OPT evidence; do not add relay sockets to each heuristic. A02–A05, A10. |
| [coder-project](../../../crates/coder-project) | GitHub source adapter, local project controller, explicit local executor. | Add private projections then exact-revision WORK/COORD commands over the same owner. A06. |
| [coder-scheduler](../../../crates/coder-scheduler) | Pure planner and locally locked durable reservations. | Keep local serialization; add an admitted coordinator and AUTO occurrence adapter. A06. |
| [coder-terminal](../../../crates/coder-terminal) | Terminal widgets, palette re-export, layout, and interaction. | Keep transport-free rendering; consume application projections. |
| [coder-ui](../../../crates/coder-ui) | Application theme. | Keep pure/local and outside Rust Native. |
| [coderbench](../../../crates/coderbench) | Local subprocess measurements; can measure a relay-backed Coder through its configured capability. | Add attributable EVAL export and an explicitly admitted evaluation job profile. A04. |
| [discovery](../../../crates/discovery) | Bundled documentation, cards, and fixed plugin package bytes. | Retain HTTP/MCP discovery; bind executable/package listings to exact signed releases. A05. |
| [gateway](../../../crates/gateway) | HTTP admission/services plus real CJ decision worker and CAP publisher. | Keep one admission owner; add caller parity, Nostr account operations, jobs, feedback, and usage adapters. A02, A07–A11. |
| [gym](../../../crates/gym) | Local result chains, manifests, traces, annotations, comparisons, and TUI. | Publish/retrieve complete EVAL artifacts and authenticated review records; reuse existing caches. A04, A10. |
| [gym-bridge](../../../crates/gym-bridge) | Private Nostr live boards and separately granted fixed recipes. | Preserve entry-scoped observation and grants; persist client launch recovery and add complete evidence retrieval. A04. |
| [jev](../../../crates/jev) | Provider-compatible HTTP decision SDK, jobs, and account client. | Preserve SDK compatibility; place a shared CJ decision interface above or beside it. A02. |
| [kev](../../../crates/kev) | Model inference, artifacts, and HTTP serving. | Keep inference direct; expose the admitted service through the existing decision worker. A02, A10. |
| [knowledge](../../../crates/knowledge) | Local retrieval, signed public cache, private/snapshot file bundles, evidence and study rules. | Add private network delivery, freshness, complete report verification, and explicit adoption. A12. |
| [laya](../../../crates/laya) | Local model variants and HTTP serving. | Keep compute local; publish actual served identity/capabilities at the service boundary. A02, A10. |
| [lev](../../../crates/lev) | On-device model bridge, supervised JSON IPC, and serving/measurement code. | Keep device inference/IPC local; admit remote door calls separately. A02. |
| [microcoder](../../../crates/microcoder) | Real KB/XP relay commands, frozen local knowledge inputs, HTTP models, repository task adapter. | Reuse public exchange; connect admitted execution, private artifact delivery, and verified adoption. A02–A03, A12. |
| [microluna](../../../crates/microluna) | Provider HTTPS and bounded native tools; `Remote` is a tool-environment trait. | Keep credentials and tools on the admitted host; expose operations via CJ/ENV, not provider secrets. A03. |
| [nostr](../../../crates/nostr) | Pure protocol primitives and contract subsets. | Keep no storage/network; close precise signer/EXT/report-validator gaps before broader adoption. B03–B04, A12. |
| [nostr-relay](../../../crates/nostr-relay) | Event storage, ACLs, subscriptions, groups, media, and selected Block roles. | Preserve transport/authority separation; address reconciliation concerns and unsupported client/server roles. B05–B06. |
| [nostr-transport](../../../crates/nostr-transport) | Bounded AUTH sockets and exact private-event publication/fetch. | Add reusable admitted connection/recovery and artifact services without silent failover disclosure. B01–B02. |
| [oak](../../../crates/oak) | HTTP CLI plus stdio/HTTP MCP. | Keep external interoperability; expose the selected decision transport and authenticated Nostr identity explicitly. A02, A07. |
| [openrouter](../../../crates/openrouter) | Direct provider SDK. | Preserve provider transport; record actual model, usage, and disclosure in host evidence. |
| [plugin](../../../crates/plugin) | Bounded Wasmtime execution with pure/snapshot-read profiles. | Keep guests offline; add signed release/adoption outside the sandbox. A05, B04. |
| [plugin-code-search](../../../crates/plugin-code-search) | Search within a granted snapshot. | Keep local bounded guest; share exact release and evaluation evidence. A05. |
| [plugin-outline](../../../crates/plugin-outline) | Diagnostic/outline guest. | Keep pure; use the same package/adoption path. A05. |
| [plugin-pdk](../../../crates/plugin-pdk) | Shared guest packet ABI and host-call wrappers. | Keep protocol-neutral ABI; do not introduce ambient Nostr/network authority. |
| [plugin-repo-map](../../../crates/plugin-repo-map) | Bounded snapshot map. | Keep local guest; package its exact bytes and declared bounds. A05. |
| [plugin-test-report](../../../crates/plugin-test-report) | Parses supplied test output. | Keep parsing local; never promote parsed output to independent verification solely through a signature. A05. |
| [receipts](../../../crates/receipts) | Transport-neutral digested receipts and consent-aware export helpers. | Add host publication/projection; preserve original schema, provenance, and unknowns. A01, A04, A09. |
| [rust-native](../../../crates/rust-native) | Generic typed UI/style/surface contracts. | Keep free of Nostr and OpenAgents product details; application adapters own synchronization. |
| [supervise](../../../crates/supervise) | Process groups, timeouts, cancellation, and bounded output. | Keep local enforcement; remote requests cannot bypass it. A03, A06. |
| [tenancy](../../../crates/tenancy) | Durable accounts, budgets, billing, skills, training, and admission owners. | Preserve one authority; add verified Nostr principal and artifact adapters. A07, A10–A11. |
| [verse](../../../crates/verse) | Real world/chat/XP relay paths, shared renderer, private Gym board; desktop agent/replay paths remain partly local. | Add durable agent conversations, selected mobile parity, and signed world/evidence manifests. A14. |
| [voyager](../../../crates/voyager) | Game bridge, local skills and episodes, HTTP decisions, Nostr guild chat, relay management, and trusted quest labels. | Keep tick/action loop local; publish exact episode/skill evidence and admit remote operations later. A15. |

## Additional surfaces

| Surface | Audit disposition |
| --- | --- |
| [iOS host](../../../bins/coder-ios/host) | Thin SwiftUI/UIKit, Keychain, Metal, QR scanning, and motion adapters remain local. Rust owns Nostr application state. No automatic sensor/private-chat publication. |
| [Minecraft bridge](../../../mc-bridge) and [Swift model bridge](../../../swift/lev-bridge) | Separate host/game and Apple runtime boundaries; remote requests should reach an admitted host, not replace these protocols. |
| [Capabilities](../../../capabilities), [programs](../../../programs), [questions](../../../questions), [sources](../../../sources), and [methods](../../../methods) | Source-controlled local definitions are useful. Add signed exact publication/resolution for portable definitions; preserve host-local commands, trust, licenses, and independent evidence. |
| [Client plugins](../../../plugins) | Keep Claude/Codex/MCP packaging for interoperability; share exact release provenance with the extension path. |
| [Terminal-Bench tooling](../../../bench/terminal-bench/tbench) | Keep Harbor, Docker, provider, and public-reference adapters. Replace manual cross-machine result copying with optional EVAL/artifact import/export; no changes to retained trials. |
| [Scripts](../../../scripts), [deployment](../../../deploy), and [migrations](../../../migrations) | Keep operator deployment, system services, database transactions, backups, and platform distribution. Nostr operations must call enforcing hosts; relay retention is not a backup strategy. |
| [NIP lanes](../../../nips) | Official and Block snapshots plus OpenAgents contracts define intended roles. Use observed source and narrower verification evidence to assess implementation, not inventory counts. |
| [Documentation](../../README.md) and retained evidence | Cross-check current guides with code; flag stale status claims. Transcripts, historical failures, and third-party produced artifacts are not rewritten or counted as live product code. |

The absence of a dedicated CoderOS, production Android, or broad Rust web client
package is a roadmap gap, not an existing non-Nostr implementation to convert.
