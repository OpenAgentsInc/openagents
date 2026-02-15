# Lightning / L402 documentation

Docs are organized into subfolders:

| Folder | Contents |
|--------|----------|
| **runbooks/** | Operational runbooks: Aperture deploy, staging reconcile, observability rehearsal, EP212 buyer rehearsal. |
| **plans/** | Plans and roadmaps: EP212 demo plan, L402 paywall infra plan, LND/Neutrino/Electron, deep integration roadmap. |
| **reference/** | Reference and guides: Voltage → L402 connect, Breez/Spark comparison, Lightning agent tools. |
| **status/** | Status snapshots and worklogs: current EP212 deployed status, liquidity bootstrap log, wallet executor/aperture deploy logs, historical desktop/LND Neutrino work. |
| **deploy/** | Aperture image build (Dockerfile, Cloud Build). See runbooks for config and Cloud Run deploy. |
| **scripts/** | Config templates and scripts (aperture-voltage-config*.yaml, voltage-api-fetch.sh). |

**Operator checklist (what you need to do now):** `status/20260215-current-status.md`.
