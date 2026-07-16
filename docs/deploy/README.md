# Deployment authority map

This directory routes operators to the owning deployment contract for each
OpenAgents product. A runbook explains how to operate an already-authorized
surface; it does not grant another surface authority, signing identity, or
support status.

| Surface              | Owning contract                                                                                                 | Operator runbook                                                         | Authority boundary                                                                                          |
| -------------------- | --------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| OpenAgents Desktop   | [Cross-platform release ProductSpec](./openagents-desktop-cross-platform-release.md)                            | [Desktop production release](./openagents-desktop-production-release.md) | Signed Desktop ReleaseSet, native packages, Desktop update selection, and Desktop `/download` truth only    |
| OpenAgents mobile    | Mobile product contracts and the [mobile production release runbook](./openagents-mobile-production-release.md) | [Mobile production release](./openagents-mobile-production-release.md)   | Expo/mobile build and OTA authority; Desktop publication must preserve its feeds but never reinterpret them |
| Agent Computer       | Agent Computer admission contracts and the [production runbook](./agent-computer-production.md)                 | [Agent Computer production](./agent-computer-production.md)              | Google Cloud runtime placement and admission, not end-user app distribution                                 |
| OpenAgents Audio     | Audio retention contracts and [retention runbook](./openagents-audio-retention.md)                              | [Audio retention](./openagents-audio-retention.md)                       | Encrypted audio/session retention only                                                                      |
| Web and API services | [`apps/openagents.com/AGENTS.md`](../../apps/openagents.com/AGENTS.md) and its deployment invariants            | Service-local sanctioned deploy commands                                 | Google Cloud service deployment and Cloudflare DNS; never Desktop signing or update authority               |

The repository-wide production infrastructure boundary remains Google Cloud
with Cloudflare as DNS authority. GitHub Actions, GitHub-hosted runners, GitHub
Releases, Electron Updater metadata, object storage, and TLS are not release
authorities for any Desktop channel.
