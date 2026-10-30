# Managed Pylon fleet watch: deployment and grants

The operator deploys one private Cloud Run Job, `pylon-fleet-watch`, and one
five-minute Cloud Scheduler in `openagentsgemini/us-central1`. The supplied
image contains the Coax policy and bounded platform effects described in
[Fleet watch](fleet-watch.md). This file covers deployment, IAM and maintenance;
it is not evidence that a particular execution passed.

The job receives an explicit six-hour window. Before that window and after it
expires, the runtime policy refuses recovery. Deployment starts paused and dry.
Activation is a separate operator action after the complete expected fleet has
passed real requests. An online advertisement is not an execution test.

## Exact grants

The deployment wrapper uses the operator's existing authorized GCloud account.
It creates no Google keys and grants no project-wide runtime roles. The two
new service accounts are:

- `pylon-watch-runtime@openagentsgemini.iam.gserviceaccount.com`;
- `pylon-watch-scheduler@openagentsgemini.iam.gserviceaccount.com`.

| Principal | Resource | Permission / role |
| --- | --- | --- |
| Runtime | VM `psionic-rtx6000-pylon`, `us-central1-b` | `compute.instances.get` |
| Runtime | VM `oct30-att-tdx-clef`, `us-central1-a` | `compute.instances.get`, `compute.instances.start` |
| Runtime | Existing Job `pylon-spot-recovery`, `us-central1` | `run.jobs.run` |
| Runtime | Private services `oa-mesh-elephant`, `oa-mesh-mammoth`, `us-central1` | `roles/run.invoker` |
| Scheduler | New Job `pylon-fleet-watch`, `us-central1` | `run.jobs.run` |

The three custom roles contain one permission each, in
`psionic/platform/fleet-watch-{read,start,invoke}-role.yaml`. They are bound to
the exact resources above. Job invocation has no execution-override permission.
There is no direct GPU-start grant: the watcher dispatches the existing bounded
[Spot recovery](spot-recovery.md) job. Its configuration and Scheduler state are
not changed. Neither account receives create, stop, delete, reset, instance-list
or service-administration permissions from this deployment.

These are this deployment's explicit grants. Resource bindings alone do not
prove the absence of inherited organization/project access. Operators auditing
effective access must inspect those policies as well. Metadata tokens and
service-account OAuth are keyless; credentials must not be copied into image
layers, logs or command arguments.

## Deploy and prove the Scheduler path

Requirements: an authorized existing GCloud account, Bash, `jq`, and the
qualified immutable watcher image. The script refuses mutable tags, a different
registry/repository, an unrelated resource sharing its name, or an existing
custom role with different permissions.

```sh
START=$(date +%s)
END=$((START + 21600))
IMAGE='us-central1-docker.pkg.dev/openagentsgemini/openagents/pylon-fleet-watch@sha256:QUALIFIED_IMAGE_DIGEST'
GPTOSS_PROVIDER='VERIFIED_64_CHARACTER_LOWERCASE_PUBLIC_KEY'

bash psionic/platform/pylon-fleet-watch.sh deploy \
  "$IMAGE" "$START" "$END" "$GPTOSS_PROVIDER"

bash psionic/platform/pylon-fleet-watch.sh verify
```

`deploy` creates a one-task, no-retry job with a 240-second task deadline,
`PYLON_WATCH_DRY_RUN=1`, and a paused Scheduler. It is a deployment wrapper, not
a builder. An initial dry deployment may omit the provider argument while the
expected public key is being qualified; activation requires it.

`verify` refuses active mode. It dispatches through the actual Scheduler OAuth
identity, waits at most five minutes for a new completed execution, checks its
creator and success, and writes `/tmp/pylon-fleet-watch-scheduler.json` (or
`PYLON_WATCH_RECEIPT`). It restores a previously paused Scheduler even on
failure. A successful dispatch alone is not claimed as a successful execution.
Inspect that execution's structured runtime logs for its actual test results.

After all four expected roles—Elephant, Mammoth, GPT-OSS and the independently
verified confidential lane—have passed their respective real requests, use the
agreed beginning of that evidence window:

```sh
bash psionic/platform/pylon-fleet-watch.sh activate \
  "$START" "$END" "$GPTOSS_PROVIDER"
bash psionic/platform/pylon-fleet-watch.sh status
```

Activation sets the exact window and provider, clears dry mode and resumes only
this Scheduler. A fresh provider public key must come from the qualified
deployment, not an arbitrary matching display label. The confidential endpoint
can rotate after restart and requires fresh vendor/workload verification.

The job's runtime service account can request a TDX start or dispatch GPU
recovery but cannot inspect Compute operations. A logged `start_requested` is
not a running VM, a warm model, a fresh beacon, or a successful inference.
Subsequent observation and end-to-end probes establish those separately.
Auth/quota failures from a public endpoint are not proof of broken GPU health.

## Pause before maintenance

```sh
bash psionic/platform/pylon-fleet-watch.sh pause
```

This pauses its Scheduler, makes future executions dry, cancels this job's
active executions and requires a 30-second quiescent drain window. Previously
accepted Compute starts or dispatched recovery jobs can still settle. Do not
intentionally stop a VM until those effects have settled.

For GPU maintenance, pause and drain the fleet watcher **first**, then follow
the existing Spot recovery runbook to pause and drain that controller too.
The fleet watcher alone does not disable the older controller. For TDX
maintenance, confirm the fleet watcher is paused/dry and any accepted start has
settled before stopping the fixed CPU VM. Neither maintenance action is
performed automatically by this wrapper.

At the six-hour deadline, the Coax window stops recovery decisions; the operator
should pause the Scheduler to stop idle invocations. This does not stop healthy
Pylons. Starting another observation window requires deliberate activation.

## Focused deployment checks

```sh
bun test psionic/platform/fleet-watch-deployment.test.ts
```

The tests drive a recording fake GCloud executable through the actual wrapper.
They check exact resource grants, no broader existing-role mutation, immutable
images, window bounds, foreign-job refusal, Scheduler attribution, failure
handling, pause restoration and drain behavior. They are infrastructure effect
checks; Coax policy checks and mutation testing remain with the runtime source.
