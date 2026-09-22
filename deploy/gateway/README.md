# Gateway deployment assets

Configuration for the tested deployment lanes. Install a config as
`/etc/openagents-gateway/gateway.json`, edit `registry`, `doors`, and
`cors_origins` for the host, and keep the installed copy — do not edit
these examples in place on a live host.

| Path | Lane |
| --- | --- |
| `gateway.shared.json` | Shared multi-tenant: public listener behind TLS termination, shared door to a localhost backend, declared browser origins. |
| `gateway.dedicated.json` | Dedicated tenant: workspace membership required, one tenant's door to its own backend. |
| `gateway.self-hosted.json` | Self-hosted: localhost listener, local registry, small bounds. |
| `gateway.on-device.json` | On-device: localhost listener, Lev backend through the local bridge, the smallest bounds. |
| `openagents-gateway.service` | The hardened systemd unit — one dedicated user, immutable release directory under `current`, localhost-only networking. |
| `gateway.env.example` | The environment template — deliberately empty of secrets today. |
| `stub-backend.py` | A minimal backend for install verification; not a door you deploy. |

The runbook — prerequisites, artifacts, TLS boundary, secrets, health,
backup, upgrade, rollback, and the verified fresh-install procedure —
is [`docs/decision-models/service/deployment.md`](../../docs/decision-models/service/deployment.md).
`../../scripts/verify-gateway-install.sh` is the verification itself.
