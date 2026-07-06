# GCloud Pylon Setup

This directory contains the GCE setup path for the always-on Pylon fleet rung
described in `docs/2026-06-10-always-on-fleet-plan.md`.

For Khala Code mobile-only MVP Agent Computers, use
`apps/pylon/deploy/agent-computer/` instead. Agent Computers are isolated
Firecracker microVMs on OpenAgents-owned GCE capacity; the Pylon runtime is only
software inside the image, not the provisioned or billed unit.

The script creates or starts a Google Compute Engine VM, copies a local env file
over IAP/SSH, and runs the existing Linux installer:

- `apps/pylon/scripts/install-cloud-node.sh`

It deliberately does **not** put owner tokens, provider keys, wallet material,
or Pylon identity secrets into instance metadata or startup scripts.

For the own-capacity Codex/Claude fleet migration tracked in public issue
#6433, use this installer for the base VM + Pylon service, then copy the
pre-authenticated isolated Pylon account home to the VM and install the
Codex supervisor systemd service described below.

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

## Own-Capacity Codex VM Migration (#6433)

Target shape: one lightweight GCE VM per distinct Codex account. Each VM owns one
isolated Pylon home, advertises one Codex slot by default, runs `pylon node` as
`openagents-pylon.service`, and runs the Codex supervisor as
`openagents-codex-supervisor.service`. This frees the operator desktop from
being the durable executor while preserving the same own-capacity/no-spend
authorization boundary.

1. Create or start the VM with the base Pylon node:

```sh
apps/pylon/deploy/gcloud/setup-pylon.sh \
  --instance oa-codex-codex-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --subnet oa-lightning-us-central1 \
  --machine-type e2-standard-4 \
  --env-file ~/work/.secrets/openagents-artanis-agent.env
```

2. Copy exactly one already-authenticated isolated account home. Do not copy or
   mutate the default `~/.codex` home.

```sh
tar -C "$HOME/.pylon-fable/accounts/codex" -czf /tmp/codex-1.tgz codex-1
gcloud compute scp /tmp/codex-1.tgz oa-codex-codex-1:/tmp/codex-1.tgz \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap
gcloud compute ssh oa-codex-codex-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command 'sudo mkdir -p /var/lib/openagents-pylon/accounts/codex && sudo tar -C /var/lib/openagents-pylon/accounts/codex -xzf /tmp/codex-1.tgz && sudo chown -R pylon:pylon /var/lib/openagents-pylon/accounts'
rm -f /tmp/codex-1.tgz
```

3. Install the Linux supervisor service with one slot on that VM:

```sh
gcloud compute ssh oa-codex-codex-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command 'sudo SUP_MAX_SLOTS=1 SUP_PER_ACCOUNT=1 PYLON_HOME=/var/lib/openagents-pylon /opt/openagents-pylon/apps/pylon/scripts/install-codex-supervisor-systemd.sh'
```

4. Verify readiness and service persistence:

```sh
gcloud compute ssh oa-codex-codex-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command 'sudo systemctl --no-pager --full status openagents-pylon openagents-codex-supervisor && sudo -u pylon env PYLON_HOME=/var/lib/openagents-pylon bun /opt/openagents-pylon/apps/pylon/src/index.ts codex accounts list --json'
```

Expected: the copied account reports `readiness.state: "ready"`, the service is
active, and the supervisor log under
`/var/lib/openagents-pylon/.codex-supervisor/supervisor.log` shows heartbeat
lines. Repeat the same shape for `oa-codex-codex-2`, `oa-codex-codex-3`, etc.
Use distinct underlying ChatGPT/Codex accounts for real throughput; aliases of
one account may share a rate budget.

Safety boundaries:

- Never run `codex login` or `pylon auth codex` on the VM against `~/.codex`.
- Never place the account tarball, agent token, or auth JSON in instance
  metadata, startup scripts, docs, issue comments, or logs.
- Keep VMs IAP-only unless there is an explicit network reason to add an
  external address.
- The supervisor is still own-capacity/no-spend: no payout claim, no settlement,
  and proof comes from exact `token_usage_events` rows plus owner-only traces.

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
