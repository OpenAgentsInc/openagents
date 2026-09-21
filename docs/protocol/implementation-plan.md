# Nostr protocol implementation plan

Status: target coverage and implementation backlog, 2026-09-21. The
[OpenAgents protocols](../../nips/openagents/README.md) revise CAP/PRG in place
as v1 and add EXT, RUN, shared artifact contracts, and execution jobs as v1.
This change defines specifications and work; it does not implement or advertise
those new runtime/relay paths.

The subsequent [TypeSafe agent addendum](../coder/design/typesafe-agent-protocol-addendum.md)
adds CTX, POL, COORD, and EVAL v1, the shared private artifact envelope `3188`,
and public evaluation declaration `3189`. These extend the target backlog;
the earlier issue descriptions and baseline below are not proof that the new
contracts are implemented.

## Scope and baseline

The baseline is repository revision `fdfe937d51b743d8bf6967423d34616b429da33b`.
The pinned [manifest](../../nips/manifest.json) supplies 98 official NIPs
(excluding the upstream index) and 15 Block NIPs. Official source commit is
`c53877571f96eb423661fc23c620d629d37b8f19`; Block source commit is
`8342dfcc5890b81a269a8ec3db73a8a56f76ce79`. This task does not sync or edit
upstream specifications. A future reviewed sync updates the inventory and its
completion evidence separately.

| Surface | Evidence available now | Remaining contract |
| --- | --- | --- |
| Pure protocol | `crates/nostr` has event/filter/replacement/deletion primitives, NIP-19/44, Block helpers, and decision-job wire validation. | Shared revised shapes, all new event families, and complete per-spec coverage. |
| Relay | Existing Postgres gateway, privacy, media, groups, management, and configured Block paths. | New public/private profiles and complete pinned optional/partial surfaces. |
| Local capability/program readers | Approved probes, four program step kinds, grants, bounded delegation, and protected checks. | Revised-v1 definitions/bindings, exact portable locks, typed composition, `invoke`, and modules. |
| Run storage | Standalone `coder::runstate` and ATIF records. | Runtime wiring, authoritative network journal, fencing, retention, and recovery transport. |
| Jobs | Conversation transport and worker; pure decision-job protocol. | Decision worker/client under existing #9469 and new durable execution family. |
| Extensions | Design documents and new NIP contracts. | Validators, installer/authoring service, PDK/host, publication, revocation, and measured activation. |

Existing server behavior is documented in [Block coverage](block-nips.md),
[protocol expansion](nip-expansion.md), and [media](media.md). They are evidence
of bounded implementations, not declarations that every pinned requirement is
complete. In particular, NIP-CW's full query profile, NIP-PL execution, NIP-29
private/hidden groups and subgroups, and NIP-77 synchronization need completion.
No missing applicable protocol may be dismissed as merely optional or old.

## Work ownership

The linked issue table is the delivery queue. Protocol parsing and relay work
extend the current crates. Coder integration extends the shared turn/runtime;
there is no second program engine or private catalog backend.

<!-- protocol-issues:start -->
Tracker: [#9527](https://github.com/OpenAgentsInc/openagents/issues/9527).

| Issue | Work |
| --- | --- |
| [#9516](https://github.com/OpenAgentsInc/openagents/issues/9516) | nostr: implement shared v1 artifacts, locks, evidence, and context contracts |
| [#9517](https://github.com/OpenAgentsInc/openagents/issues/9517) | nostr: implement revised NIP-CAP v1 definitions and host bindings |
| [#9518](https://github.com/OpenAgentsInc/openagents/issues/9518) | nostr: implement revised NIP-PRG v1 typed programs and native invocation |
| [#9519](https://github.com/OpenAgentsInc/openagents/issues/9519) | coder: implement the NIP-PRG v1 Wasm packet host and Rust PDK |
| [#9520](https://github.com/OpenAgentsInc/openagents/issues/9520) | nostr: implement NIP-EXT v1 releases, descriptors, and revocation |
| [#9521](https://github.com/OpenAgentsInc/openagents/issues/9521) | nostr: implement NIP-RUN v1 encrypted journals and fenced recovery transport |
| [#9522](https://github.com/OpenAgentsInc/openagents/issues/9522) | coder: implement NIP-CJ execution v1 worker and client |
| [#9523](https://github.com/OpenAgentsInc/openagents/issues/9523) | nostr-relay: implement OpenAgents discovery, privacy, and retention profiles |
| [#9524](https://github.com/OpenAgentsInc/openagents/issues/9524) | nostr: complete implementation of the pinned official NIP lane |
| [#9525](https://github.com/OpenAgentsInc/openagents/issues/9525) | nostr: complete implementation of the pinned Block NIP lane |
| [#9526](https://github.com/OpenAgentsInc/openagents/issues/9526) | nostr: prove program and extension interoperability end to end |
<!-- protocol-issues:end -->

Existing work retains its owner:

| Issue | Scope retained |
| --- | --- |
| [#9469](https://github.com/OpenAgentsInc/openagents/issues/9469) | Decision-family worker/client, authenticated admission, and failure/settlement behavior. |
| [#9470](https://github.com/OpenAgentsInc/openagents/issues/9470) | Decision service capability publication/discovery using revised CAP. |
| [#9471](https://github.com/OpenAgentsInc/openagents/issues/9471) | Shared decision receipts and transport integration; RUN links these receipts. |
| [#9510](https://github.com/OpenAgentsInc/openagents/issues/9510) | Coder's local program lifecycle, budgets, and recovery; RUN adds the network contract. |
| [#9511](https://github.com/OpenAgentsInc/openagents/issues/9511) | Child-program runtime integration using revised PRG bindings. |
| [#9512](https://github.com/OpenAgentsInc/openagents/issues/9512) | Coder's portable resolver/trust/installation consumer using EXT and exact locks. |

## Order and definition of done

The additional protocol work fits the following existing workstreams. This
mapping adds acceptance scope to this plan; it does not assert that GitHub
issue bodies or completed implementations already include it.

| Additional contract | Protocol workstream | Host/client work that remains |
| --- | --- | --- |
| CTX task frames, representations, context requests, and history | [#9516](https://github.com/OpenAgentsInc/openagents/issues/9516) shared contracts; [#9523](https://github.com/OpenAgentsInc/openagents/issues/9523) private envelopes. | Evidence/context storage, instruction-aware selection, expansion, and invalidation from the TypeSafe roadmap's CTX slices. |
| POL instruction, approval, disclosure, and routing records | [#9516](https://github.com/OpenAgentsInc/openagents/issues/9516) artifacts; [#9517](https://github.com/OpenAgentsInc/openagents/issues/9517) bindings. | Trusted policy resolution, atomic approval consumption, provider adapters, confinement, and complete-task cost accounting. |
| COORD claims, reuse, and background findings | [#9521](https://github.com/OpenAgentsInc/openagents/issues/9521) journal/fencing; [#9522](https://github.com/OpenAgentsInc/openagents/issues/9522) execution transport. | Transactional coordinator, alias-aware resource scopes, scheduler priorities, stale-result checks, and protected integration. |
| EVAL reports and publication | [#9516](https://github.com/OpenAgentsInc/openagents/issues/9516) typed artifacts; [#9523](https://github.com/OpenAgentsInc/openagents/issues/9523) publication profiles; [#9526](https://github.com/OpenAgentsInc/openagents/issues/9526) interoperation. | Gym adapters, workload/partition evidence, independent comparisons, disclosure review, and explicit promotion. |

Include all four in schema fixtures and end-to-end acceptance. Keeping a
contract in an artifact rather than a new event kind does not remove its
parser, authority, or failure-case requirements.

1. Implement shared references, canonical digests, schemas, effects, evidence,
   context, and refusal fixtures. Establish the complete pinned-spec ledger.
2. Implement revised CAP and PRG validation; migrate current manifests and
   readers together. Implement local EXT resolution alongside the existing
   resolver work. Unsupported new shapes remain unavailable.
3. Implement the Wasm ABI/PDK, durable RUN journal and its local-store bridge,
   and public/private relay profiles. Integrate existing decision receipts.
4. Implement execution-family worker/client admission, status, cancellation,
   replay, and retention. Complete existing decision-family network work.
5. Finish the official and Block lanes against their own specifications, then
   prove complete program/extension flows across process and relay boundaries.

Every pinned specification needs a ledger row naming its exact source version,
domain/parser surface, client/server role, configuration, fixtures, exercised
acceptance path, remaining limitations, and owning issue. `unassessed`,
`partial`, `implemented`, and `configured-and-proven` are distinct. Client-only
specifications need real fixture-backed clients; generic event storage is not
a client implementation. Deprecated/unrecommended pinned contracts get exact
compatibility coverage without becoming new-design foundations.

Completion requires positive and adversarial fixtures, configured execution,
privacy checks across REQ/ID/COUNT/search/live paths, and honest NIP-11/worker
advertisement. Domain parsing alone does not complete a network service;
relay storage alone does not complete a runtime. Unknown effects, missing
retained content, revocation gaps, and unsigned claims cannot become success.

Run relevant manual checks for implementation changes under
[verification policy](../verification.md); no GitHub workflows or GitHub-billed
automation. Documentation-only specification changes need link/path/artifact
checks, not the Rust gate. Publishing these drafts does not complete their
fixture or implementation milestones.

## Pinned inventory

The lists below enumerate coverage scope, not implementation status. The lane
completion issues must maintain per-spec evidence rather than closing on an
inventory-only audit. README indexes are not separate specifications.

<!-- protocol-inventory:start -->
### Official lane

[01](../../nips/official/01.md), [02](../../nips/official/02.md), [03](../../nips/official/03.md), [04](../../nips/official/04.md), [05](../../nips/official/05.md), [06](../../nips/official/06.md), [07](../../nips/official/07.md), [08](../../nips/official/08.md), [09](../../nips/official/09.md), [10](../../nips/official/10.md), [11](../../nips/official/11.md), [12](../../nips/official/12.md), [13](../../nips/official/13.md), [14](../../nips/official/14.md), [15](../../nips/official/15.md), [16](../../nips/official/16.md), [17](../../nips/official/17.md), [18](../../nips/official/18.md), [19](../../nips/official/19.md), [20](../../nips/official/20.md), [21](../../nips/official/21.md), [22](../../nips/official/22.md), [23](../../nips/official/23.md), [24](../../nips/official/24.md), [25](../../nips/official/25.md), [26](../../nips/official/26.md), [27](../../nips/official/27.md), [28](../../nips/official/28.md), [29](../../nips/official/29.md), [30](../../nips/official/30.md), [31](../../nips/official/31.md), [32](../../nips/official/32.md), [33](../../nips/official/33.md), [34](../../nips/official/34.md), [35](../../nips/official/35.md), [36](../../nips/official/36.md), [37](../../nips/official/37.md), [38](../../nips/official/38.md), [39](../../nips/official/39.md), [40](../../nips/official/40.md), [42](../../nips/official/42.md), [43](../../nips/official/43.md), [44](../../nips/official/44.md), [45](../../nips/official/45.md), [46](../../nips/official/46.md), [47](../../nips/official/47.md), [48](../../nips/official/48.md), [49](../../nips/official/49.md), [50](../../nips/official/50.md), [51](../../nips/official/51.md), [52](../../nips/official/52.md), [53](../../nips/official/53.md), [54](../../nips/official/54.md), [55](../../nips/official/55.md), [56](../../nips/official/56.md), [57](../../nips/official/57.md), [58](../../nips/official/58.md), [59](../../nips/official/59.md), [5A](../../nips/official/5A.md), [60](../../nips/official/60.md), [61](../../nips/official/61.md), [62](../../nips/official/62.md), [64](../../nips/official/64.md), [65](../../nips/official/65.md), [66](../../nips/official/66.md), [67](../../nips/official/67.md), [68](../../nips/official/68.md), [69](../../nips/official/69.md), [70](../../nips/official/70.md), [71](../../nips/official/71.md), [72](../../nips/official/72.md), [73](../../nips/official/73.md), [75](../../nips/official/75.md), [77](../../nips/official/77.md), [78](../../nips/official/78.md), [7D](../../nips/official/7D.md), [84](../../nips/official/84.md), [85](../../nips/official/85.md), [86](../../nips/official/86.md), [87](../../nips/official/87.md), [88](../../nips/official/88.md), [89](../../nips/official/89.md), [90](../../nips/official/90.md), [92](../../nips/official/92.md), [94](../../nips/official/94.md), [96](../../nips/official/96.md), [98](../../nips/official/98.md), [99](../../nips/official/99.md), [A0](../../nips/official/A0.md), [A4](../../nips/official/A4.md), [B0](../../nips/official/B0.md), [B7](../../nips/official/B7.md), [BE](../../nips/official/BE.md), [C0](../../nips/official/C0.md), [C7](../../nips/official/C7.md), [CC](../../nips/official/CC.md), [EE](../../nips/official/EE.md), [F4](../../nips/official/F4.md).

### Block lane

[NIP-AA](../../nips/block/NIP-AA.md), [NIP-AE](../../nips/block/NIP-AE.md), [NIP-AM](../../nips/block/NIP-AM.md), [NIP-AO](../../nips/block/NIP-AO.md), [NIP-AP](../../nips/block/NIP-AP.md), [NIP-CW](../../nips/block/NIP-CW.md), [NIP-DV](../../nips/block/NIP-DV.md), [NIP-ER](../../nips/block/NIP-ER.md), [NIP-GS](../../nips/block/NIP-GS.md), [NIP-IA](../../nips/block/NIP-IA.md), [NIP-MP](../../nips/block/NIP-MP.md), [NIP-OA](../../nips/block/NIP-OA.md), [NIP-PL](../../nips/block/NIP-PL.md), [NIP-RS](../../nips/block/NIP-RS.md), [NIP-WP](../../nips/block/NIP-WP.md).

<!-- protocol-inventory:end -->
