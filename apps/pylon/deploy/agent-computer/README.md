# Agent Computer Host Deployment

This directory describes the public, non-secret part of the Khala Code Agent
Computer substrate from `docs/khala-code/2026-07-06-agent-computers-strategy.md`.

An Agent Computer is not a hosted Pylon. It is an isolated Firecracker microVM
on OpenAgents-owned GCE capacity, assigned to one admitted work context
(`user + thread + repo binding`) and reclaimed after the lifecycle policy says
the work context is idle or expired. The Pylon runtime and coding agents are
software inside the image; the provisioned and metered unit is the Agent
Computer.

## Public Responsibilities

- Create a nested-virtualization-capable GCE host in `openagentsgemini`.
- Verify `/dev/kvm` on the host before any Firecracker lane is armed.
- Keep the host IAP/private-egress by default; do not expose inbound services.
- Install only host prerequisites here. Private topology, kernels/rootfs paths,
  capability broker internals, control-plane tokens, SCM tokens, and user repo
  content stay out of the public repo.
- Let the private `cloud/` repo's `oa-node` / `oa-codex-control` provisioner
  own Firecracker lifecycle, scratch wipe, quarantine, and refs-only receipts.

## Host Bootstrap

Dry-run the command shape first:

```sh
apps/pylon/deploy/agent-computer/setup-gce-host.sh \
  --instance agent-computer-gce-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --machine-type n2-standard-4 \
  --dry-run
```

Live host creation:

```sh
apps/pylon/deploy/agent-computer/setup-gce-host.sh \
  --instance agent-computer-gce-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --machine-type n2-standard-4
```

The script creates a VM with `--enable-nested-virtualization`, no external IP by
default, and then verifies `/dev/kvm` over IAP SSH. It deliberately accepts no
env file and no bearer token because secrets belong in Secret Manager and the
private control-plane deployment.

## Image Manifest

`agent-computer-image.manifest.json` records the public image contract:
contracts, runtime tools, isolation promises, and the owner-gated rootfs/kernel
digest placeholders that must be filled by the private image build. The manifest
is not a signed image receipt by itself.

## Owner-Gated Receipts

#8503 is not complete until the owner records public-safe receipts for:

- the nested-virt GCE host with `/dev/kvm` verified;
- the signed or digest-pinned Agent Computer kernel/rootfs image;
- `CLOUD_CODING_SESSIONS_ENABLED=true`,
  `OA_CODEX_GCE_PROVISIONER=live`, and `OA_CLOUD_CONTROL_URL`/token configured
  against the real control plane;
- one mobile-dispatched Khala Code turn that ran inside a Firecracker microVM;
- lifecycle receipt refs for provision/active/idle/reclaim and an
  `openagents.resource_usage_receipt.v1` compute receipt;
- exact token receipt refs for the same turn.

Do not paste control tokens, SCM credentials, raw GCE instance identifiers,
guest IPs, SSH keys, prompts, repo content, or private traces into issues or
docs. Issue comments should contain only public-safe refs and command receipts.
