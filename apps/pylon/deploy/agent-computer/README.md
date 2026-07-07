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
- Require the placement echo contract from #8476: one work-context ref per
  Agent Computer, no cross-context reuse, SCM-broker-only credentials,
  credential scanner before closeout/writeback, and reclaim receipts proving
  both scratch wipe and microVM destruction.

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

The manifest intentionally records #8476's public enforcement requirements. A
signed image/control-plane receipt must prove the same requirements before
#8503 can close.

## Proven Substrate (2026-07-07)

The Firecracker-on-GCE substrate is proven end-to-end on `agent-computer-gce-1`
(`openagentsgemini`, `us-central1-a`, `n2-standard-4`, nested-virt enabled,
`/dev/kvm` present, host kernel `6.17.0-1020-gcp`):

- Firecracker + jailer `v1.16.1` installed at `/usr/local/bin`.
- Firecracker CI baseline kernel + rootfs staged under `/srv/openagents/cloud-vm/`
  (digests recorded in `agent-computer-image.manifest.json` `substrateProof`).
- A real microVM booted in ~1s with its own guest kernel `5.10.223` (distinct
  from the host's `6.17` — a true separate-kernel isolation boundary), 2 vCPUs.
- Real commands ran inside the microVM (ssh-on-tap), HTTPS egress worked
  through host NAT (fetched real bytes from a public repo), an artifact was
  copied microVM -> host, and the microVM was destroyed with its per-run scratch
  rootfs wiped.

## In-microVM coding turn PROVEN (2026-07-07)

The baked agent-computer rootfs now runs a real coding turn **inside** the
microVM. Proof script `/tmp/vsock-turn-proof.py` (host-local, root) reports
`TURN-PROOF-RESULT: PASS`:

- microVM boots from the baked rootfs
  (`sha256:78861d5a033657ec5a0752ee328d5ca568bee815122fba0797da5b6cb5a339eb`);
  the vsock guest agent (`agent-guest.service`, port `1024`) is ready in ~3s.
- `turn-runner` runs inside the guest: real depth-1 checkout of
  `octocat/Hello-World@7fd1a60b01f9` (in-guest git-fetch egress works) + a real
  1-file staged diff, with Khala-shaped `runtime_event`s streamed over vsock.
- `result.json` is extracted microVM -> host; `baseCommit` matches the pinned
  commit; the microVM is SIGKILLed and its per-run scratch rootfs is wiped.

Egress fix (recorded in the manifest `inMicrovmTurnProof.egressFix`): the baked
image enabled `systemd-networkd` with an empty `/etc/systemd/network/`, which
claimed `eth0` and cleared the kernel `ip=`. Disabling + masking
`systemd-networkd.service`/`.socket`/`-wait-online`, masking `systemd-resolved`,
and writing a static `/etc/resolv.conf` (then `e2fsck -fy`) restores in-guest
egress under the TAP+NAT + kernel `ip=` boot config.

This is a real coding turn, but **not** an LLM turn: `turn-runner` performs a
deterministic checkout + staged-diff step. A real model-token usage receipt
(`/api/khala/cloud/runtime-turn-usage`) still requires a Codex/Claude OAuth
login baked into the image or routing the turn through the hosted Khala gateway.

## Remaining Work Before The #8503 DoD Turn

Ranked, with the owning repo:

1. ~~Baked agent-computer rootfs~~ — DONE (digest pinned in `guestImage`,
   in-microVM turn proven above).
2. **Control-plane guest transport (`cloud/` `crates/oa-codex-control/src/cloud_vm.rs`)**
   — port the proven vsock protocol into `guest_exec`/`guest_copy_out` and make
   `wait_guest_ready` poll the real vsock readiness signal.
3. **placement -> firecracker -> executor integration (`cloud/`)** — today
   `POST /v1/placement` binds a run to a Codex runner lane, not a Firecracker
   microVM. Add the Agent Computer placement path that boots a microVM from the
   baked image, runs the executor for the admitted work context, streams
   `runtime_event`s into the thread scope, and emits `cloud.gce.*` lifecycle +
   `openagents.resource_usage_receipt.v1` receipts carrying `workContextRef`,
   `scratchWipeReceiptRef`, and `microvmDestroyReceiptRef` (the Worker's
   `validateAgentComputerPlacement` already requires these).
4. **Model-token receipt** — bake a Codex/Claude OAuth login into the image (or
   route the turn through the hosted Khala gateway) so the turn produces a real
   model-token usage receipt, not just the deterministic coding step.
5. **Arm + dispatch** — arm staging flags against that control plane and run one
   real mobile-dispatched turn.

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
