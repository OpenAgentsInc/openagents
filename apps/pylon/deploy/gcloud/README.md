# GCloud Pylon Setup

This directory contains the GCE setup path for the always-on Pylon fleet rung
described in `docs/2026-06-10-always-on-fleet-plan.md`.

The script creates or starts a Google Compute Engine VM, copies a local env file
over IAP/SSH, and runs the existing Linux installer:

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

Then verify the live Pylon projection through the public OpenAgents API and
issue-comment only public refs: registration ref, heartbeat ref, capability
refs, serving benchmark refs, and any canary/replay receipt refs.
