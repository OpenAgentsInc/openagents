# Deployment authority map

This directory routes operators to the owning deployment contract for each
OpenAgents product. A runbook explains how to operate an already-authorized
surface. It does not grant another surface authority, signing identity, or
support status.

The Electron OpenAgents Desktop application was deleted at owner direction on
2026-08-04 (#9325). Omega replaced it. There is no Desktop release lane, no
Desktop ReleaseSet, and no Desktop update feed, and the two Desktop release
runbooks that used to sit in this directory were deleted with it. Omega
releases from the Omega repository, and `openagents.com` publishes its download
entry from one signed Omega manifest.

| Surface              | Owning contract                                                                                                 | Operator runbook                                                         | Authority boundary                                                                                          |
| -------------------- | --------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| OpenAgents mobile    | Mobile product contracts and the [mobile production release runbook](./openagents-mobile-production-release.md) | [Mobile production release](./openagents-mobile-production-release.md)   | Expo/mobile build and OTA authority                                                                          |
| Agent Computer       | Agent Computer admission contracts and production runbooks | [Production](./agent-computer-production.md), [image cadence](./2026-07-24-agent-computer-image-update-cadence-runbook.md) | Google Cloud runtime placement and admission, not end-user app distribution                                 |
| OpenAgents Audio     | Audio retention contracts and [retention runbook](./openagents-audio-retention.md)                              | [Audio retention](./openagents-audio-retention.md)                       | Encrypted audio/session retention only                                                                      |
| Web and API services | [`apps/openagents.com/AGENTS.md`](../../apps/openagents.com/AGENTS.md) and its deployment invariants            | Service-local sanctioned deploy commands                                 | Google Cloud service deployment and Cloudflare DNS. Never Omega signing or update authority                 |

The repository-wide production infrastructure boundary remains Google Cloud
with Cloudflare as DNS authority. GitHub Actions, GitHub-hosted runners,
GitHub Releases, object storage, and TLS are not release authorities for any
channel.
