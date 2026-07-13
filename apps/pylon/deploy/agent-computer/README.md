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
- Let in-repo `crates/oa-node` / `crates/oa-codex-control` provisioners own
  Firecracker lifecycle, scratch wipe, quarantine, and refs-only receipts
  (migrated from private `OpenAgentsInc/cloud`; see `docs/cloud/MIGRATION.md`).
- Require the placement echo contract from #8476: one work-context ref per
  Agent Computer, no cross-context reuse, SCM-broker-only credentials,
  credential scanner before closeout/writeback, and reclaim receipts proving
  both scratch wipe and microVM destruction.



## Source-controlled rootfs bake (CX-3 #8547 item 1)

The guest rootfs bake is no longer a hand-run recipe on the bake host — it is
`build-agent-computer-rootfs.sh` in this directory (root, Linux x86_64,
normally `agent-computer-gce-1`). It reproduces the proven recipe (debootstrap
jammy + git/python3/ca-certificates/openssh-client, pinned bun, the vsock
guest agent from the checked-in `guest-agent.py` + `agent-guest.service`,
compiled `turn-runner`, fixed `portable-session-control`, `oa-workroomd`, and the systemd-networkd/resolved
egress fix) and ADDS the pinned `codex` binary at `/usr/local/bin/codex`
(npm `@openai/codex` linux-x64 vendor musl build; version + digests pinned in
the script and in `agent-computer-image.manifest.json` → `guestImage.codex`).

```sh
# On the nested-virt bake host, as root, with staged guest binaries:
sudo ./build-agent-computer-rootfs.sh \
  --turn-runner /srv/openagents/stage/turn-runner \
  --portable-session-control /srv/openagents/stage/portable-session-control \
  --workroomd /srv/openagents/stage/oa-workroomd \
  --output /srv/openagents/cloud-vm/agent-computer-rootfs-codex.ext4
```

It bakes to a NEW image (never overwrites the validated one), fsck-verifies,
seals the sha256, and writes a refs-and-digests-only bake receipt JSON. Re-pin
`guestImage.rootfsDigest` in the manifest only after the microVM boot smoke
(guest agent ready over vsock + `codex --version` via guest exec) passes.

For PORT-03 retained movement, `/opt/agent/portable-session-control` is the
only guest command the host route invokes. It accepts the fixed
stage/activate/abort/quiesce/checkpoint/reclaim/wipeCapability vocabulary, verifies the
materialized Git post-image before stage, drives only baked
`oa-workroomd lifecycle` commands for the exact graph agents, and journals
public-safe operation results. It is not a command tunnel. A production image
is not PORT-03-ready until its bake receipt includes
`portableSessionControlSha256` and a boot smoke proves the binary is present.

Capability installation uses a separate authenticated
`POST /v1/portable-agent-computers/capabilities/install` octet-stream route.
Only public refs travel in `X-OA-*` headers; the material body is mutable,
passes to the fixed guest controller through a raw vsock stdin frame, and is
zeroized on both sides. The host derives the retained resource from
`targetRef + sessionRef`, requires the exact staged owner/attachment/generation,
and verifies that the lease was planned by the stage operation. A marker with
only `leaseRef` and `evidenceRef` is committed after successful installation;
`wipeCapability` removes both material and marker.

Checkpoint materialization uses the authenticated octet-stream
`POST /v1/portable-agent-computers/checkpoints/materialize` route after a
nonaccepting prepare. The archive is digest-bound to the exact checkpoint and
contains only a Git bundle, manifest, and sorted post-image. The guest rejects
traversal, devices, hard links, unknown entries, and escaping links; bounded
relative symbolic links are recreated from manifest-declared link-target
bytes. It checks every size/mode/digest, reconstructs the pinned revision,
recomputes repository/diff/graph digests, and only then records stage. Any
resolver, upload, digest, or verification failure invokes replay-safe
`abortPrepared`; teardown is journaled as pending before its effect so a lost
ack or missing VM reconciles to completed cleanup rather than orphaning a VM.

Real work acceptance is separate from lifecycle activation. The authenticated
`POST /v1/portable-agent-computers/continuations` route accepts one bounded
private task for exactly the canonical root and every child, binds each to a
unique turn ref and the installed planned provider lease, and sends the body
to the guest over raw stdin. The guest executes one real `oa-workroomd codex
session` turn per agent against the materialized workspace. Only accepted
agent/turn refs, monotonic cursors, evidence refs, and `material: excluded` are
journaled. Same-operation replay returns the stored result without a second
turn; changed bytes conflict.

The 2026-07-13 capability image bake and live Firecracker boot smoke are green.
The materializer/continuation additions require a new nested-virt rebake and a
full stage/install/activate/continue/quiesce/checkpoint/reclaim live receipt
before the image manifest or #8748 can claim the complete movement rung.

## In-repo `oa-workroomd` guest binary (#8591)

Agent Computer guest images include the **in-repo** workroom sidecar, not a
binary from the historical private `OpenAgentsInc/cloud` checkout.

| Item | Path |
| --- | --- |
| Source crate | `crates/oa-workroomd` |
| Dockerfile | `docker/cloud/oa-workroomd.Dockerfile` |
| Staging script | `apps/pylon/deploy/agent-computer/build-workroomd-for-image.sh` |
| Guest install path | `/usr/local/bin/oa-workroomd` |
| Manifest pointer | `agent-computer-image.manifest.json` → `runtime.workroomd` |

```sh
# Host-triple local stage (dev smoke)
apps/pylon/deploy/agent-computer/build-workroomd-for-image.sh

# Linux guest binary via Docker (nested-virt bake hosts)
apps/pylon/deploy/agent-computer/build-workroomd-for-image.sh --docker
```

The script writes the binary + a public-safe staging receipt under
`var/agent-computer/staging/` (gitignored). Bake steps copy the binary into the
rootfs at `/usr/local/bin/oa-workroomd` before sealing rootfs digests. Secrets
are never accepted as script arguments.

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
2. ~~**Control-plane guest transport**~~ — DONE. The live vsock guest protocol,
   `guest_exec`/`guest_copy_out`, and real readiness poll are in
   `crates/oa-codex-control/src/cloud_vm.rs`.
3. ~~**placement -> firecracker -> executor integration**~~ — DONE in source.
   A `cloud-gcp` placement carrying a bounded `work_context_b64` runs the baked
   turn-runner inside the live Cloud-VM provisioner and publishes lifecycle,
   compute, token, and reclaim evidence through the normal run event stream.
4. ~~**Brokered model-token receipt path**~~ — DONE in source and fixtures. The
   turn-runner redeems the owner-scoped provider grant into scratch-only
   `CODEX_HOME`, runs the baked Codex binary, and refuses a missing/inexact or
   unexpectedly metered receipt.
5. **Arm + dispatch** — expose the validated nested-virt control daemon through
   the approved private/public control boundary, bind the production Worker to
   it, and run one real physical-mobile owner turn. This remains the literal
   #8547 exit; a direct host smoke is readiness evidence, not mobile acceptance.

## Build and live-host readiness smoke (2026-07-12, #8547)

The production control image is reproducibly built from the repository root:

```sh
IMAGE="us-central1-docker.pkg.dev/openagentsgemini/oa-cloud/oa-codex-control:openagents-main-$(git rev-parse --short=10 HEAD)"
CLOUDSDK_CONFIG=/path/to/isolated-automation-config \
  gcloud builds submit . \
    --project openagentsgemini \
    --region us-central1 \
    --config docker/cloud/cloudbuild-oa-codex-control.yaml \
    --substitutions="_IMAGE=${IMAGE},_REVISION=$(git rev-parse HEAD)"
```

The runtime image includes `iproute2` and `iptables`, which the live
Firecracker network setup executes directly. The daemon must run on the
nested-virtualization host with `/dev/kvm`, `/dev/net/tun`, the pinned kernel
and rootfs, the Firecracker binary, and its runtime directory available. Keep
the control bearer in a mode-0600 host env file or Secret Manager; never place
it on the command line or in a receipt.

At commit `b1af7b55ec`, an authenticated loopback request to the current-image
daemon on `agent-computer-gce-1` completed the exact live
`POST /v1/cloud-vm/sessions` lifecycle with `provisionerKind=live`, guest exec
code `0`, and `cleanupReceipt.tornDown=true`. The post-run host audit reported
zero Firecracker processes, zero zombies, zero TAP devices, and zero jail
directories. The image digest was
`sha256:98957ec230d20a02371fef6d5a2fe274427228719711706eb72bcc1bfef2d642`;
the provision and cleanup receipt digests were respectively
`sha256:60643d1150e4d2dbce36faf68003ff6e9859678962e352eab9363078f3509d20`
and
`sha256:34246f548bdad2083a7c7131bd7b0db52168c1d7294c25c37a261a8c971327a0`.
The same change retains an async waiter for each Firecracker child and ships
the process-control utility used by teardown, so a long-lived control host does
not retain a live or defunct process after scratch and networking are reclaimed.

This smoke deliberately used loopback and did not repoint production. Before a
physical-phone turn, the operator must expose the daemon only through the
approved control boundary, configure the Worker control URL/token, and verify
that the owner has an active GitHub connection plus an owner-scoped Codex grant.
The current OpenAgents mobile execution-target catalog does not yet advertise a
`managed_cloud`/Agent Computer option; it exposes hosted, named Codex, and named
Claude targets. The mobile start leg is therefore not owner-executable until
that server-authorized target is projected into the catalog and composer. Do
not ask the owner to look for a selector that the shipped source cannot render.

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
