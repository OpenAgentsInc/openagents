# Fixed Pylon Spot recovery

This infrastructure controller can inspect and request a start for exactly
`openagentsgemini/us-central1-b/psionic-rtx6000-pylon`. That one 96 GB RTX PRO
VM hosts both real Elephant and Mammoth providers. The retired
`psionic-l4-elephant` VM is deliberately excluded. No VM, GPU or reservation
is created by recovery; Spot capacity can still be unavailable.

The private Cloud Run Job is `pylon-spot-recovery` in `us-central1`.
Cloud Scheduler invokes its Google API every five minutes using OAuth and a
keyless service account. A custom role containing only `run.jobs.run` is
bound to that job. Its separate runtime account has only
`compute.instances.get` and `compute.instances.start`, bound directly to the
single instance. There are no exported service-account keys, project-wide
compute grants, stop/delete permissions or operation-poll permission.

The decision is authored in `psionic/lib/ops/spot-recovery.coax` and compiled
to the small native adapter in the infrastructure image. RUNNING is a no-op;
transitional states wait; TERMINATED dry mode only reports `would_start`;
TERMINATED active mode requests a start. Unknown states, modes and targets
refuse. Its three checks and all 20 root mutants pass with zero survivors.
The curl/jq HTTP effect layer reads a keyless metadata token and the fixed
Compute resource. There is no Python or Google SDK in this support runtime.

## Deploy and verify

The operator wrapper requires Bash, gcloud, jq and shasum. Its local check
runner also uses the qualified Coax compiler, a C compiler and Bun.

Use an existing authorized GCloud admin account; do not broaden the ordinary
automation account or create Google keys. The deployment reuses the existing
`oa-mvp-automation` Cloud Build identity required by this project's org policy.
Only these controller resources and the fixed VM/job IAM bindings are changed.

```sh
psionic/platform/spot-recovery-check.sh
psionic/platform/pylon-spot-recovery.sh deploy
psionic/platform/pylon-spot-recovery.sh verify
```

Deployment initially pauses Scheduler and sets `PYLON_RECOVERY_DRY_RUN=1`.
`verify` enables the trigger if paused and dispatches it using the real
Scheduler identity, still in the job's current mode. Inspect the resulting
Cloud Run execution and structured `action=noop` / `status=RUNNING` log for a
healthy VM; do not stop a healthy worker to test recovery.

After verification, enable actual recovery:

```sh
psionic/platform/pylon-spot-recovery.sh resume
psionic/platform/pylon-spot-recovery.sh status
```

A successful asynchronous start is reported only as `start_requested`, not
as a running/healthy provider. The controller intentionally cannot poll zone
operations. Capacity/API errors fail the task and remain in its logs. The
next scheduled observation will see the actual VM state. Cloud Run uses one
task, no task retries, a 90-second task timeout, one CPU and 512 MiB; Scheduler
also has no HTTP retries. This is VM lifecycle recovery, not model-health or
confidential-execution verification. After a cold resume the enabled native
systemd backends/Pylons need time to upload their verified checkpoint cache
from persistent disk into the GPU and pass their doctors.

## Stop or maintain the VM

**Pause and drain recovery before any intentional STOP.**

```sh
psionic/platform/pylon-spot-recovery.sh pause
# Continue only after: recovery paused and dry; no active execution after drain window

gcloud compute instances stop psionic-rtx6000-pylon \
  --project=openagentsgemini --zone=us-central1-b
```

The pause command disables future triggers, makes the job dry to neutralize late
dispatches, cancels active executions of this job only, and requires a quiescent
drain window covering the 30-second Scheduler HTTP deadline. If draining fails, it exits nonzero;
do not stop the VM until it succeeds. An already accepted Compute start is
not canceled by killing a job: let any in-flight VM transition settle before
an intentional stop. Resume only after maintenance and deliberately choosing
to run the existing GPU again. Leaving Scheduler enabled would undo a manual
stop on a later observation.

## Observe

```sh
gcloud run jobs executions list --job=pylon-spot-recovery \
  --project=openagentsgemini --region=us-central1

gcloud logging read \
  'resource.type="cloud_run_job" AND resource.labels.job_name="pylon-spot-recovery"' \
  --project=openagentsgemini --limit=20
```

Cloud Run resolves the
built application image to an immutable digest for each execution. The stored
execution records identify its creator, runtime account, image and dry flag.

A read-only reservation inventory found no project reservations to consume.
Google checks on-demand capacity when creating a reservation; that provisions
billable capacity. Catalog/quotas alone do not establish available Standard
stock. No reservation or blue/green GPU was allocated for this controller.

Google references: [Cloud Scheduler authentication](https://docs.cloud.google.com/scheduler/docs/http-target-auth),
[Cloud Run jobs IAM](https://docs.cloud.google.com/run/docs/reference/iam/roles),
[Compute resource IAM](https://docs.cloud.google.com/compute/docs/access/managing-access-to-resources),
[reservation behavior](https://docs.cloud.google.com/compute/docs/instances/reservations-overview).

## Recorded live check, 2026-10-30

The final curl image `sha256:6d21aef8218ecf4ea5533431c30b3418d8ec8dc88b1069d3432b840540464a19`
passed a real Scheduler-created dry execution `pylon-spot-recovery-kjr2x`,
then ordinary active-mode execution `pylon-spot-recovery-57fms`. Both completed
one task and logged `noop` / `RUNNING`. The latter observation was at
2026-10-30 18:50:09 UTC. Pause/dry/quiescent-drain was also exercised; the
healthy GPU VM was never stopped. Scheduler is enabled in active mode.

The first SDK-based effect timed out and was replaced; it is not the active
controller image. The final effect uses bounded metadata/HTTPS calls, a
0600 token-header file and no SDK/Python. Actual preemption/start was not
forced; a future accepted start remains distinct from actual VM/model health.
The public source record is
`docs/verification/data/pylon-spot-recovery-2026-10-30/managed-job.json`.
