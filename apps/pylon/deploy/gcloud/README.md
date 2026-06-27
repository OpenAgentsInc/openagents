# GCloud Pylon Setup

This directory contains the GCE setup path for the always-on Pylon fleet rung
described in `docs/2026-06-10-always-on-fleet-plan.md`.

The script creates or starts a Google Compute Engine VM, optionally copies a
serialized isolated Pylon home, copies a local env file over IAP/SSH, and runs
the existing Linux installer:

- `apps/pylon/scripts/install-cloud-node.sh`

It deliberately does **not** put owner tokens, provider keys, wallet material,
or Pylon identity secrets into instance metadata or startup scripts.

## Prerequisites

- A `gcloud` account with Compute Engine create/list/start, IAP tunnel, SCP, and
  SSH permissions in the target project.
- An env file that contains any owner-granted runtime credentials needed by the
  Pylon, typically:

```sh
OPENAGENTS_AGENT_TOKEN=...
ANTHROPIC_API_KEY=...
```

Keep that file outside Git. Do not paste its contents into issues, docs,
commit messages, or terminal transcripts.

## Own-Capacity Codex / Claude VM

Use this path for issue #6433-style migration off the owner's desktop: one
lightweight VM per already-linked account home, with systemd restarting the
standing Pylon and the no-spend own-capacity supervisor.

First package only the isolated Pylon home that should move to the VM. Keep the
archive outside Git and transfer it only over the IAP/SSH path:

```sh
tar -C ~/.pylon-fable -czf ~/work/.secrets/pylon-fleet/codex-1-pylon-home.tar.gz .
```

Then provision a VM and install the persistent supervisor:

```sh
apps/pylon/deploy/gcloud/setup-pylon.sh \
  --instance oa-codex-control-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --machine-type e2-standard-2 \
  --env-file ~/work/.secrets/openagents-artanis-agent.env \
  --pylon-home-archive ~/work/.secrets/pylon-fleet/codex-1-pylon-home.tar.gz \
  --supervisor codex
```

For a Claude account, pass `--supervisor claude`. For a VM that intentionally
hosts both linked account families, pass `--supervisor both`. The remote
installer writes `openagents-pylon.service` plus the requested
`openagents-codex-supervisor.service` and/or
`openagents-claude-supervisor.service`. The supervisor reuses the restored
`PYLON_HOME`; it never runs `codex login`, `pylon auth codex`, or
`pylon auth claude`.

Scale this by repeating the command with one archive and one instance per
account, for example `oa-codex-control-1` through `oa-codex-control-7`. Use
`SUP_MAX_SLOTS` and `SUP_PER_ACCOUNT` in the env file only when the operator
intentionally wants to override the supervisor defaults.

## CPU Pylon

```sh
apps/pylon/deploy/gcloud/setup-pylon.sh \
  --instance pylon-gcloud-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --subnet oa-lightning-us-central1 \
  --machine-type e2-standard-4 \
  --env-file ~/work/.secrets/openagents-agent-claim-christopher-codex.env
```

The default network posture is IAP-only: `--no-address` is used unless
`--with-address` is passed. The OpenAgents project default subnet is
`oa-lightning-us-central1`; pass `--subnet ""` only if you intentionally want
the project default network.

When `~/.ssh/google_compute_engine` exists, the script passes it to
`gcloud compute ssh/scp` explicitly. Use `--ssh-key-file <path>` to override
that key.

## Repurpose An Existing VM

If you reuse a stopped VM that previously belonged to another workload, clear
its startup script before booting it as a Pylon. This prevents the old workload
from racing the installer on first start:

```sh
apps/pylon/deploy/gcloud/setup-pylon.sh \
  --instance gswarm508-clean2-20260325044551-contrib \
  --project openagentsgemini \
  --zone us-central1-b \
  --clear-startup-script \
  --tags psion-swarm-contributor-host,pylon-hosted,openagents-pylon \
  --env-file ~/work/.secrets/openagents-agent-claim-christopher-codex.env
```

For existing instances, `--tags` is applied with `gcloud compute instances
add-tags` before start. Preserve any old tags that still provide required
network access until you have verified IAP SSH and private egress on the new
Pylon posture.

## GPU / Proven-Engine Pylon

For the Khala #6089 serving lane, choose a GPU-compatible VM type and pass an
accelerator:

```sh
apps/pylon/deploy/gcloud/setup-pylon.sh \
  --instance pylon-gcloud-l4-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --machine-type g2-standard-4 \
  --accelerator nvidia-l4=1 \
  --env-file ~/work/.secrets/pylon-serving.env
```

This script gets the Pylon node online. It does not by itself install vLLM or
SGLang, run a model server, arm paid routing, move money, or claim a
`PYLON_SERVING_REAL_GPU_*` benchmark. A live #6089 closeout still needs the
real proven-engine endpoint and the exact-parity/canary/replay serving receipt.

## Dry Run

```sh
apps/pylon/deploy/gcloud/setup-pylon.sh \
  --instance pylon-gcloud-dry-run \
  --env-file ~/work/.secrets/openagents-agent-claim-christopher-codex.env \
  --dry-run
```

Dry-run output prints command shapes but not env-file contents.

## Verification

After setup:

```sh
gcloud compute ssh pylon-gcloud-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command 'sudo systemctl --no-pager --full status openagents-pylon'
```

For supervisor VMs, also verify:

```sh
gcloud compute ssh oa-codex-control-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command 'sudo systemctl --no-pager --full status openagents-codex-supervisor'
```

Then verify the live Pylon projection through the public OpenAgents API and
issue-comment only public refs: registration ref, heartbeat ref, capability
refs, serving benchmark refs, and any canary/replay receipt refs.

## Remote Khala Issuer Authorization Smoke

The GCE Pylon env file may also carry:

```sh
OPENAGENTS_AGENT_TOKEN=...
PYLON_OPENAGENTS_BASE_URL=https://openagents.com
```

Do not print either value. From the VM, first run the negative authorization
smoke. It reaches the Khala gateway with the remote bearer token but deliberately
targets a Pylon ref that is not linked to that OpenAuth account, so no assignment
is created and no money path is exercised:

```sh
pylon khala request \
  --prompt "Remote-token capacity authorization smoke" \
  --workflow codex_agent_task \
  --pylon-ref pylon.not_linked.authorization_smoke \
  --json
```

Expected result: a non-zero exit with JSON containing
`target_pylon_not_authorized` (or the server's equivalent not-linked target
reason) and no `assignmentRef`. Running the same command locally with the same
token should produce the same authorization result; origin/IP is not authority.

For an owner-approved positive smoke, replace `--pylon-ref` with a real
caller-owned, heartbeat-fresh Codex Pylon. The successful response should include
`assignmentRef`, `durableRequestId`, and `durableStreamUrl`; the assignment still
stays on the Khala coding `unpaid_smoke` path unless a separate paid-capacity
change explicitly arms spend.
