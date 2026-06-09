# Issue 4550 Recovery Identity Live Audit

Date: 2026-06-07
Repository: `OpenAgentsInc/openagents`
Issue: `#4550`

## Summary

The keyless Nexus recovery identity is now provisioned and verified against the
live `nexus-mainnet-1` VM.

The recovery identity is:

- `nexus-recovery-operator@openagentsgemini.iam.gserviceaccount.com`

It is intended for approved operator recovery and audit flows without service
account keys and without project-owner credentials.

## Provisioned Authority

The provisioning script creates or updates:

- Service account:
  `nexus-recovery-operator@openagentsgemini.iam.gserviceaccount.com`
- Custom project role:
  `projects/openagentsgemini/roles/NexusRecoveryOperator`
- Custom role permissions:
  - `compute.instances.get`
  - `compute.instances.list`
  - `compute.instances.reset`
  - `compute.instances.getSerialPortOutput`
  - `compute.zoneOperations.get`
  - `compute.zones.get`
- Project roles:
  - `roles/iap.tunnelResourceAccessor`
  - `roles/compute.osAdminLogin`
- Organization OS Login role:
  - `roles/compute.osLoginExternalUser`
- VM service-account act-as role:
  - `roles/iam.serviceAccountUser` on
    `nexus-mainnet@openagentsgemini.iam.gserviceaccount.com`
- Optional impersonator grant:
  - `roles/iam.serviceAccountTokenCreator` on the recovery service account for
    the approved impersonator.

No service-account keys were created.

## Commands Run

```bash
NEXUS_RECOVERY_IDENTITY_DRY_RUN=true \
NEXUS_RECOVERY_IMPERSONATOR_MEMBER='user:chris@openagents.com' \
scripts/deploy/nexus/34-provision-recovery-identity.sh

NEXUS_RECOVERY_IMPERSONATOR_MEMBER='user:chris@openagents.com' \
scripts/deploy/nexus/34-provision-recovery-identity.sh

env -u CLOUDSDK_AUTH_IMPERSONATE_SERVICE_ACCOUNT \
gcloud compute ssh nexus-mainnet-1 \
  --tunnel-through-iap \
  --project openagentsgemini \
  --zone us-central1-a \
  --impersonate-service-account=nexus-recovery-operator@openagentsgemini.iam.gserviceaccount.com \
  --command='id && hostname'

CLOUDSDK_AUTH_IMPERSONATE_SERVICE_ACCOUNT=nexus-recovery-operator@openagentsgemini.iam.gserviceaccount.com \
scripts/deploy/nexus/33-audit-public-watchdog.sh
```

## Implementation Finding

The first impersonated SSH attempt could mint an access token and reach IAP, but
the VM rejected the OS Login public key. Guest SSH logs reported:

```text
google_authorized_keys: OS Login user sa_116527414006321536801 does not have login permission.
```

The missing live requirements were:

- `roles/compute.osLoginExternalUser` at the organization parent for the
  recovery service account.
- `roles/iam.serviceAccountUser` on the VM-attached
  `nexus-mainnet@openagentsgemini.iam.gserviceaccount.com` service account.

The provisioning script and runbook now include those requirements.

## Audit Helper Finding

The first full impersonated audit after SSH succeeded failed because
`33-audit-public-watchdog.sh` uploaded to a fixed remote path:

```text
/tmp/nexus-audit-public-watchdog.sh
```

That path could be owned by a previous SSH user. The audit helper now uploads
to a unique remote `/tmp/nexus-audit-public-watchdog-<timestamp>-<pid>.sh` path
and removes it after execution.

## Verification

Local verification:

```bash
bash scripts/deploy/nexus/test-public-watchdog-shell-guards.sh
bash scripts/deploy/nexus/test-recovery-identity-shell-guards.sh
bash -n scripts/deploy/nexus/33-audit-public-watchdog.sh \
  scripts/deploy/nexus/34-provision-recovery-identity.sh
git diff --check
```

Live verification:

- Minimal impersonated SSH succeeded:
  - user: `sa_116527414006321536801`
  - host: `nexus-mainnet-1`
- Full watchdog audit through
  `CLOUDSDK_AUTH_IMPERSONATE_SERVICE_ACCOUNT=nexus-recovery-operator@openagentsgemini.iam.gserviceaccount.com`
  succeeded.
- Live audit reported:
  - `nexus-public-watchdog.timer`: enabled and active.
  - `nexus-http-recovery-proxy.service`: enabled and active.
  - `nexus-cloudflared.service`: enabled and active.
  - `nexus-relay.service`: enabled and active.
  - VM-local origin health: HTTP `200`.
  - Recovery proxy health: HTTP `200`.
  - Latest watchdog receipt:
    - `status=healthy`
    - `reason=public_edge_ok`
    - `action=none`
    - `local_health_code=200`
    - `public_edge_code=200`
    - `consecutive_edge_failures=0`

## Result

The #4550 noninteractive recovery identity requirement is satisfied for the
public watchdog audit lane. The identity is keyless, least-privilege for this
scope, impersonable by the approved operator, and verified against the live VM.

This does not resolve the separate Treasury continuity issue tracked by
`#4548`.
