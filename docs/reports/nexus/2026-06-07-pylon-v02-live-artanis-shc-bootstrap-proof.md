# Pylon v0.2 Live Artanis SHC Bootstrap Proof

Date: 2026-06-07
Issue: `OpenAgentsInc/openagents#4551`

## Result

The live account-backed Artanis SHC bootstrap gate is satisfied for the Pylon
v0.2 release candidate.

The successful run completed through the Artanis bootstrap API with wallet
authority disabled:

```text
POST /v1/artanis/bootstrap/start
```

Public-safe run identifiers:

| Field | Value |
| --- | --- |
| SHC run id | `artanis.bootstrap.pylon-launch.20260607141825` |
| Omega external run id | `shc-codex:oa-shc-katy-01:artanis.bootstrap.pylon-launch.20260607141825` |
| Runner | `oa-shc-katy-01` |
| OpenAgents release-candidate commit | `229895ded4aacecb417db5daf068124081852a30` |
| Cloud commit | `65972fe286ebe25866f49569901b36925fc0e7dc` |
| Wallet authority | `false` |

Omega callback state:

| Field | Value |
| --- | --- |
| `agent_runs.status` | `completed` |
| `agent_runs.event_cursor` | `63` |
| `agent_runs.started_at` | `2026-06-07T14:18:27.804Z` |
| `agent_runs.completed_at` | `2026-06-07T14:21:34.562Z` |
| Final event | sequence `63`, `cloud.run.completed`, `OpenCode/Codex run finished with status completed.` |

Provider-account grant state:

| Field | Value |
| --- | --- |
| Requested action | `artanis_pylon_bootstrap` |
| Grant status | `used` |
| Grant created | `2026-06-07T14:18:25.000Z` |
| Grant used | `2026-06-07T14:18:29.863Z` |

The provider account ref, grant ref, auth material, bearer material, and
workroom-private state are intentionally omitted from this public report.

## Required Artifacts

The SHC workroom recorded all eight required launch artifacts.

| Artifact | Content digest |
| --- | --- |
| `result.md` | `sha256:e8357517316d3d01030486b2d1a676a31a4766609ac4043372eca81fdbc284f7` |
| `artanis-source-map.json` | `sha256:9074d043d77570c7e2330c6fe451abfecdd955cdf65fa27e6fb88ff9b2db8d68` |
| `pylon-launch-plan.json` | `sha256:5b65a23ca611ea5446f9e794bb02351777be6f299c39dd23e614d2ff43b81029` |
| `continual-learning-plan.json` | `sha256:2df36e7c96355f7e7bf4d8589c48c02306a245fe7001b6b987bc5a54604cb175` |
| `signature-mining-plan.json` | `sha256:3aad1af1fd94830c927e8dd99f0cd82c542f41dc1b56b7b4123ccb2e9a009e2d` |
| `work-order-drafts.json` | `sha256:663b3f23e209628435d633dd31595cc27c5f4870f86be45f5c4907d736ce2237` |
| `artifact-manifest.json` | `sha256:d0718dc1c12ce69c4a3b564524f6fd9b4d2f96620f64dc0435f10079249ad63e` |
| `proof-bundle.json` | `sha256:7f966603ec58a3ec3c7ca5cbae3c9a205acd3c35d33c95ac33a8eaaa7996aa8e` |

The closeout manifest digest is:

```text
sha256:f3b87779f9046a0dd6a29ab7d4498dcf527725094c3e788c30535e5ccae40d80
```

Artifact receipt digests:

```text
sha256:8b751a94e3cf6f96fe13dbe1d1511dd87bcf2905cf3155a1490470b6dba14425
sha256:521f348e754c335c32935c8c99eefaa645d56d9fa525ddaff896925cc50de688
sha256:258652424e5773eb9e3dfd95afb4a9190fb6fe63d6c012e88394cb707ac5d461
sha256:333c0387e55b834f8b22718e3526ed81e0a534dfcf5981557c345c8b37f18d66
sha256:1a78c20dff7aea07589f1967e83f79809edbe56822636eb456c700b17d474ccf
sha256:997472c51a7c1080c47060557189e453e17733e016fcc5a6bfb3fcb6fd1b5f6c
sha256:0be642b2a72cbc7e4794ea5852ccdce4b3e848745295ece03160b6258873270d
sha256:d207673a2df701a5cde10c74d18028806363b0ea96f15c045419ad4610b98104
```

## Verification Commands

Public-safe evidence came from:

```bash
ssh -o BatchMode=yes ubuntu@23.182.128.195 'hostname && systemctl is-active oa-codex-control'
```

Expected result:

```text
oa-shc-katy-01
active
```

Omega D1 verification:

```sql
SELECT id,status,event_cursor,external_run_id,started_at,completed_at,failed_at
FROM agent_runs
WHERE id='artanis.bootstrap.pylon-launch.20260607141825';

SELECT sequence,type,summary,status,artifact_refs_json,created_at
FROM agent_run_events
WHERE run_id='artanis.bootstrap.pylon-launch.20260607141825'
ORDER BY sequence DESC
LIMIT 10;

SELECT status,requested_action,created_at,used_at,expires_at
FROM provider_account_auth_grants
WHERE runner_session_id='artanis.bootstrap.pylon-launch.20260607141825'
ORDER BY created_at DESC
LIMIT 3;
```

SHC artifact verification:

```bash
RUN=artanis.bootstrap.pylon-launch.20260607141825
ROOT=/var/lib/openagents/codex-control/$RUN
STATE=$(find "$ROOT" -path "*/state/artifact-state.json" -print -quit)
MANIFEST=$(find "$ROOT" -path "*/state/closeout-manifest.json" -print -quit)
jq '{required_artifacts, artifacts, closeout}' "$STATE"
jq '.' "$MANIFEST"
```

## What Failed Before This Passing Run

Three earlier attempts found integration problems that are now documented for
release hardening:

- A stale connected ChatGPT/Codex provider account produced a provider-side
  `token_invalidated` failure. The passing run used a different approved
  account-backed grant.
- A direct SHC call without an Omega `agent_runs` row caused Omega callback
  ingestion to reject events. The passing run had the Omega run row registered
  before SHC execution.
- A prior assignment shape let the workroom try absolute SHC state paths when
  source files were unavailable. The passing run supplied embedded public-safe
  assignment context and explicitly forbade absolute state-path probing.

## Caveat

This proof demonstrates the live account-backed Artanis SHC bootstrap boundary,
Omega callback ingestion, artifact capture, grant consumption, and no-wallet
policy for the Pylon v0.2 release gate. It does not by itself prove public
Pylon release install behavior or post-release paid-work settlement. Those
remain separate release-runbook gates.
