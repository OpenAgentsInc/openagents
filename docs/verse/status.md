# Current Verse capabilities

Status reconciled October 6, 2026, after audit recommendations V01–V28.
The [audit](../audits/2026-10-04-verse-engine-audit.md) retains the original
October 4 baseline and each remediation's acceptance. The
[architecture](engine/architecture.md) and [roadmap](engine/roadmap.md) include
proposed boundaries and dated milestones; use this page for current capabilities.
Completing the recommendations establishes the bounded implementations below.
AAA MMORPG production qualification still requires population, device, content,
and operating targets beyond these receipts.

## Source-owned versions and features

The generated [runtime contract](runtime-contract.json) records current wire,
save, character, rules, social, replay, and public-content versions, the pinned
compiler, and Cargo feature declarations and local dependency edges. Each value
names its source. It describes source declarations, not an executable's resolved
feature union or permission to read an older format. Migration and admission
code decide compatibility; replay binds the actual execution profile.

Regenerate it after changing an owning constant or manifest, then check it:

```sh
python3 scripts/verse-runtime-contract.py
python3 scripts/verse-runtime-contract.py --check
```

This manual documentation check uses Python 3.11 or later and the standard
library. It adds no product runtime or GitHub automation. The generator refuses
ambiguous or missing source patterns rather than selecting a plausible value.

## Implemented paths and acceptance

In this table, **portable** means shared Rust authority or presentation values;
it does not establish physical phone or browser acceptance. **Native measured**
means the contributor-machine workload named by the linked receipt. Earlier
tests and measurements remain evidence of their recorded sources; subsequent
correctness checks do not refresh a benchmark automatically.

| Capability | Implemented path | Platforms and retained acceptance | Remaining limit |
| --- | --- | --- | --- |
| Intent-only authority and clocks | [`verse-world::play`](../../crates/verse-world/src/play.rs), [`physics`](../../crates/physics/src/lib.rs) | Portable 30 Hz command schedule and 120 Hz body paths; [V18 native campaign](../../bench/verse/2026-10-05/battle-scale/README.md). | Admission caps are not population or cross-architecture determinism guarantees. |
| Ordered durable rewards and mutations | [`persistence::writer`](../../crates/verse-world/src/service/persistence/writer.rs), [`rewards`](../../crates/verse-world/src/service/rewards.rs) | Native host; [ordered durability](../../bench/verse/2026-10-04/ordered-durability/run.json). | Local exclusive writer and bounded backpressure; no distributed database or unlimited receipt history. |
| Save migration and compatibility | [`persistence::migration`](../../crates/verse-world/src/service/persistence/migration.rs), [`save`](../../crates/verse-world/src/service/save.rs) | Native offline operator; [migration receipt](../../bench/verse/2026-10-04/content-migration/run.json). | Reviewed migrations only; unsupported rules or schemas are refused. |
| Authenticated transport and work admission | [`net`](../../crates/verse-world/src/service/net.rs), [`reach`](../../crates/verse-world/src/service/reach.rs), [`net::admission`](../../crates/verse-world/src/service/net/admission.rs) | Native TLS and REACH TCP/WebSocket; browser REACH WebSocket; [admission receipt](../../bench/verse/2026-10-04/admission/run.json), [platform checks](../../bench/verse/2026-10-05/platform-clients/README.md). | Configured trust/enrollment; world access and a character role are separate. Work limits do not prove hardware throughput. |
| Duplex requests, relevance, and acknowledged deltas | [`client_runtime`](../../crates/verse-world/src/service/client_runtime.rs), [`replication`](../../crates/verse-world/src/service/replication.rs), [`worker`](../../crates/verse-world/src/service/worker.rs) | Shared clients; [spatial replication](../../bench/verse/2026-10-04/spatial-replication/run.json), [V18 campaign](../../bench/verse/2026-10-05/battle-scale/README.md). | Conservative bounds, bounded histories and queues; no unlimited interest sets or population claim. |
| Movement prediction and reconciliation | [`prediction`](../../crates/verse-world/src/prediction.rs), [`native session`](../../crates/verse-imported/src/imported/chamber_session.rs) | Desktop, Rust mobile mount, browser shared session; [V04 acceptance](../audits/2026-10-04-verse-engine-audit.md#v04-confirmed-intervals-establish-bounded-delayed-movement-acceptance), [V18 campaign](../../bench/verse/2026-10-05/battle-scale/README.md). | Flat-ground and battle profiles differ. FIFO delayed TCP chunks do not simulate arbitrary packet loss or physical input-to-display latency. |
| Independent realm instances and transfers | [`realm`](../../crates/verse-world/src/service/realm.rs), [`realm::transfer`](../../crates/verse-world/src/service/realm/transfer.rs) | Native multi-listener host; [transfer receipt](../../bench/verse/2026-10-04/realm-transfer/run.json). | One bounded local realm; no multiprocess character handoff or fleet qualification. |
| Accounts, character selection, logout, and replacement keys | [`realm::lifecycle`](../../crates/verse-world/src/service/realm/lifecycle.rs), [`accounts`](../../crates/verse-world/src/service/accounts.rs) | Native realm; [V09 acceptance](../audits/2026-10-04-verse-engine-audit.md#v09-persistent-accounts-and-characters-have-a-recovery-contract). | Local authenticated account book; no general identity-provider or MMO billing service. |
| Hosted social worlds and Studio seat placement | [`play::social`](../../crates/verse-world/src/play/social.rs), [`social::studio`](../../crates/verse-world/src/social/studio.rs) | Portable rules and native projection; [social receipt](../../bench/verse/2026-10-04/social-authority/run.json). | Walking grants no Studio panel authority. Explicit instance bindings; public world discovery convergence remains proposed. |
| Profiling and quality budgets | [`verse-pbr`](../../crates/verse-pbr/src/lib.rs), [`quality`](../../crates/verse-engine/src/quality.rs) | Native GPU/CPU attribution and tiers; [frame attribution](../../bench/verse/2026-10-04/frame-attribution/run.json), [renderer budgets](../../bench/verse/2026-10-04/renderer-budgets/run.json). | GPU execution, CPU work, capture, and presentation are separate measurements; no physical display-latency result. |
| Streaming residency and texture mips | [`streaming`](../../crates/verse-engine/src/streaming/mod.rs), [`mips`](../../crates/verse-engine/src/mips.rs), [`renderer streaming`](../../crates/verse-pbr/src/streaming.rs) | Native renderer with portable admission; [residency](../../bench/verse/2026-10-05/streaming-residency/README.md), [mip semantics](../../bench/verse/2026-10-05/mip-semantics/README.md). | Bounded resource/profile admission; not a continent-scale content service or minimum-device memory qualification. |
| Scene broadphase and navigation crowds | [`queries`](../../crates/physics/src/queries.rs), [`navigation`](../../crates/physics/src/navigation.rs) | Portable physics; [broadphase](../../bench/verse/2026-10-05/scene-broadphase/README.md), [crowd checks](../../bench/verse/2026-10-05/navigation-crowd/README.md). | Measured authored scenes and declared actor populations; no arbitrary crowd throughput claim. |
| Shared caster catalog and tuning | [`content`](../../crates/verse-world/src/content.rs), [`play`](../../crates/verse-world/src/play.rs) | Portable authority; [caster evidence](../../bench/verse/2026-10-05/actor-casters/README.md). | Bounded authored spells and encounter definitions; no complete class/content balancing claim. |
| Shared engine and dedicated host | [`verse-engine`](../../crates/verse-engine/README.md), [`verse-content`](../../crates/verse-content/README.md), [`verse-host`](../../crates/verse-host/README.md) | Native dedicated TLS host, native renderer, portable/browser checks; [two-world evidence](../../bench/verse/2026-10-05/engine-boundaries/README.md). | REACH remains application integration; the independent host does not link it or the renderer. |
| Battle capacity | [`battle harness`](../../crates/verse/examples/battle_scale.rs) and production authority/session | Native, 20 players/40 hostiles; [accepted campaign](../../bench/verse/2026-10-05/battle-scale/README.md). | One renderer plus nineteen headless workers on one machine; low quality, 1280 × 720, bounded delayed route. No realm-scale claim. |
| Operator health, recovery, and backups | [`operator`](../../crates/verse-world/src/service/operator.rs), [`persistence::backup`](../../crates/verse-world/src/service/persistence/backup.rs) | Native scratch host; [operator evidence](../../bench/verse/2026-10-05/operator-recovery/README.md). | Local reviewed restore/rollback and bounded telemetry; no coordinated deployment fleet or live failover. |
| Content authoring and sealed previews | [`authoring`](../../crates/verse-content/src/authoring/mod.rs) | Rust compiler/workbench and native authority preview; [authoring evidence](../../bench/verse/2026-10-05/content-authoring/README.md). | Validated closed scene/rules vocabulary; no arbitrary scripts or full commercial art editor. |
| Locomotion, contacts, aim, and animation diagnostics | [`locomotion`](../../crates/verse-engine/src/locomotion.rs), [`animation graph`](../../crates/verse-engine/src/animation_graph.rs) | Portable values and native evaluation; [character evidence](../../bench/verse/2026-10-05/character-locomotion/README.md). | Declared rigs and terrain; no general retargeting, finished AAA art, or physical-device motion qualification. |
| Lighting and material semantics | [`shading`](../../crates/verse-pbr/src/shading.rs), [`lighting`](../../crates/verse-engine/src/lighting.rs) | Shared semantics and native GPU references; [lighting evidence](../../bench/verse/2026-10-05/lighting-contract/README.md). | Tiered local lamps and authored references; no full-zone shadow cost or minimum-device result. |
| Audio banks, mixing, streaming, and captions | [`audio`](../../crates/verse-engine/src/audio.rs), [`audio_stream`](../../crates/verse-engine/src/audio_stream.rs) | Native callback output; shared captions on mobile/browser; [audio receipt](../../bench/verse/2026-10-05/audio-contract/run.json). | Phone/browser output adapters and physical-device latency, thermal, and hearing checks remain in owner verification. |
| Native, mobile, and browser clients | [`platform clients`](platform-clients.md) and [`mobile mount`](../../crates/coder-mobile/src/verse_app.rs) | Native loopback, Rust mounting/lifecycle, linked Wasm and software WebGL2; [platform evidence](../../bench/verse/2026-10-05/platform-clients/README.md). | Combat subset on mobile/browser; no physical phone, hardware browser, gamepad, or screen-reader acceptance. Software rendering adapts resolution. |
| Parties, guilds, progression, gear, and trades | [`realm::services`](../../crates/verse-world/src/service/realm/services.rs), [`game_services`](../../crates/verse-world/src/service/game_services.rs) | Portable state and native two-instance TLS; [game-service evidence](../../bench/verse/2026-10-06/game-services/README.md). | Bounded local durable books and typed client views; no marketplace, full commerce UI, or automatic party combat attribution. |
| Creator releases and account safety | [`public release`](../../crates/verse-content/src/authoring/release.rs), [`publication`](../../crates/verse-content/src/authoring/publication.rs), [`realm::safety`](../../crates/verse-world/src/service/realm/safety.rs) | Native offline export/review and realm TLS; [publication/safety evidence](../../bench/verse/2026-10-06/publication-safety/README.md). | Operator-controlled publication and moderation, bounded requests; no distributed moderation, live content eviction, or phone/browser review UI. |
| Operational input replay | [`replay`](../../crates/verse-world/src/replay.rs) | Native Linux x86-64 exact executable; portable Wasm compile; [68-tick replay](../../bench/verse/2026-10-06/input-replay/README.md). | Diagnostic SDK, not automatic TLS/REACH, realm, Studio, or storage-completion recording. Exact execution profile; no cross-build/architecture claim. |

NIP-MV plaza presence remains a separate signed-pose projection, not combat
authority. See [networking](networking.md) for implemented channel/admission
paths and the proposed public discovery convergence. Separately loaded Lagrange
and Physics Lab remain consumers with their own documented rules; their tests
do not establish chamber multiplayer acceptance.

## Reading measured evidence

V18's final integrated campaign passes three combined repeats and a ten-minute
soak. Its accepted profile includes durable storage, twenty authenticated players,
forty hostiles, reconnects, and actual authority/session rendering. The soak
records simulation p99 upper bound 28.667 ms, CPU frame p95 5.248 ms, GPU p95
5.772 ms, and snapshot age p95 223.896 ms. Its source, executables, hardware,
route, workload, and gates are pinned in the
[campaign evidence](../../bench/verse/2026-10-05/battle-scale/README.md).
Earlier failed campaigns remain retained and labeled. A later
[blocked-write soak](../../bench/verse/2026-10-05/battle-scale-blocked-write/soak-run.json)
fails journal append/sync after 3,700 workload ticks and records eighteen movement
expiries while the shared disk is nearly full. Exhaustion is a likely cause;
the receipt does not prove the errno attribution. The accepted campaign remains
evidence of its pinned revision. Uninterrupted operation on a newer revision
requires a healthy rerun that measures and resolves those expiries. Later
game-service, safety, and replay receipts establish
their own correctness profiles and do not repeat that throughput campaign.

[Owner verification](../../NEEDS_OWNER.md) retains physical phone/browser,
audio, interruption, thermal, and presentation checks. Fleet deployment,
cross-machine realm population, broader persistent soak, operating cost, and
minimum-device targets need separate qualification. Preserve retained evidence,
source notices, and transcripts when updating these guides.
