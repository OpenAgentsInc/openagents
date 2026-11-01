# Block (Buzz) extension NIPs

This index is maintained by OpenAgents. The 20 specifications and two JSON
fixture files beside it are byte-exact copies of
[the pinned Block source](https://github.com/block/buzz/tree/24a370606044c0c7be787cc2fd98ec0bfe1d606b/docs/nips)
at `24a370606044c0c7be787cc2fd98ec0bfe1d606b`. Pins and file hashes are recorded in the
[manifest](../manifest.json) and [upstream lock](../upstream-lock.json).

| Spec | Scope |
| --- | --- |
| [NIP-AA](NIP-AA.md) | Agent Authentication |
| [NIP-AE](NIP-AE.md) | Agent Engrams |
| [NIP-AM](NIP-AM.md) | Agent Turn Metrics |
| [NIP-AO](NIP-AO.md) | Agent Observability |
| [NIP-AP](NIP-AP.md) | Agent Personas |
| [NIP-AR](NIP-AR.md) | Channel Artifacts |
| [NIP-CL](NIP-CL.md) | Channel Identity and Discovery |
| [NIP-CW](NIP-CW.md) | Channel and Thread Windows |
| [NIP-DV](NIP-DV.md) | DM Visibility |
| [NIP-ER](NIP-ER.md) | Event Reminders |
| [NIP-FA](NIP-FA.md) | Federated Identity Adapter |
| [NIP-FI](NIP-FI.md) | Federated identity authorization — stateless core |
| [NIP-GS](NIP-GS.md) | Git Object Signing with Nostr Keys |
| [NIP-IA](NIP-IA.md) | Identity Archival |
| [NIP-MP](NIP-MP.md) | Multi-Repository Projects |
| [NIP-OA](NIP-OA.md) | Owner Attestation |
| [NIP-PL](NIP-PL.md) | Push Leases |
| [NIP-PMA](NIP-PMA.md) | Private Managed-Agent Aggregate |
| [NIP-RS](NIP-RS.md) | Cross-Device Read State Sync |
| [NIP-WP](NIP-WP.md) | Workspace Profile |

[NIP-MP.fixtures.json](NIP-MP.fixtures.json) and
[NIP-MP.fold-fixtures.json](NIP-MP.fold-fixtures.json) are upstream test data,
not OpenAgents-authored schemas. Their bytes are checked with the specs.

The [sync review](../../docs/protocol/2026-10-27-upstream-nip-sync.md) explains
what changed. No runtime conformance is claimed by this source mirror. In
particular, PMA remains a reservation subject to its privacy and transactional
gates; AR/CL/FA and the revised AP/FI contracts need actual consumer work.
Historical implementation inventories from before the repository reset are
not evidence that those services exist in the current checkout.
