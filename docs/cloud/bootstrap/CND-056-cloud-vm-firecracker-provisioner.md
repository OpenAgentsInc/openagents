# CND-056 Cloud-VM firecracker provisioner (qa-runner CloudVm seam)

Status: code-complete in `cloud`; live boot is the deploy step on a Linux KVM
host.

Tracks: OpenAgentsInc/openagents issue #6200 (follow-up from #6186).

## What this is

The production cross-OS Cloud-VM provisioner behind the qa-runner's typed
`CloudVmProvisionerV2` / `CloudVmHandle` seam. The qa-runner (openagents repo,
`apps/qa-runner/src/backend.ts`) defines a typed
`provision -> exec -> copyOut -> teardown` lifecycle on a requested OS tier
(`linux` | `macos` | `windows`) and ships an owner-gated INERT stub plus a real
local analogue (`apps/qa-runner/src/container-backend.ts`, Docker). This work is
the **production** implementation, which lives owner-gated here in `cloud`,
backed by **firecracker** microVMs (reference `projects/repos/firecracker` +
`sek8s` for KVM/TDX/jailer patterns), exposed over HTTP by `oa-codex-control`.

## Where it lives

- `crates/oa-codex-control/src/cloud_vm.rs` — the provisioner:
  - `CloudVmProvisioner` trait: `provision` / `exec` / `copy_out` / `teardown`.
  - `FakeProvisioner` — deterministic, no KVM, materializes a public-safe
    artifact set. Used by unit tests and any no-KVM host.
  - `LiveFirecrackerProvisioner` — gated behind Linux + `/dev/kvm` + configured
    binaries/images; boots firecracker under the jailer (seccomp/cgroup/chroot
    isolation). Refuses honestly (never falls back to a local browser, never
    fakes a green) when KVM is unavailable or the OS tier has no host pool.
  - `run_cloud_vm_session()` — the one-shot
    `provision -> exec -> copy_out -> teardown` driver with guaranteed teardown.
- `crates/oa-codex-control/src/main.rs` — the HTTP surface:
  - `POST /v1/cloud-vm/sessions` (alias `/start`) — bearer-gated like every
    `/v1/*` route. Request body (camelCase) mirrors the seam:
    `{ runId, os, targetName, ownerRef, sessionCommand }`. Response is the
    refs-only `CloudVmSessionOutcome`
    (`vmId`, `os`, `provisionerKind`, `exec{code,output}`, `extractedTo`,
    `provisionReceipt`, `cleanupReceipt`).
- `crates/oa-codex-control/tests/cloud_vm_contract.rs` — the contract test that
  boots the real daemon and proves the route fulfils the
  `CloudVmProvisionerV2` provision/exec/copyOut/teardown lifecycle in the exact
  wire shape the TypeScript seam sends.
- `docs/contracts/openagents.cloud_vm_provisioner.v1.md` — the wire contract.

## qa-runner ↔ cloud wire mapping

| qa-runner seam (`backend.ts`)        | cloud HTTP                                   |
| ------------------------------------ | -------------------------------------------- |
| `CloudVmProvisionerV2.provisionVm`   | `POST /v1/cloud-vm/sessions` (provision stage) |
| `CloudVmHandle.exec(cmd, args)`      | response `exec: { code, output }`            |
| artifact extraction (`copyOut`)      | response `extractedTo` + dereferenceable `result.json` |
| `CloudVmHandle.teardown()`           | response `cleanupReceipt.tornDown`           |
| `CloudVmHandle.id` / `.os`           | response `vmId` / `os`                        |

The qa-runner injects an HTTP-backed provisioner that POSTs to this route; the
route runs the full one-shot session and returns the outcome. Because the host
artifact dir is owned by the daemon (under its state root) and returned as
`extractedTo`, the caller never supplies a host path.

## Owner-gating / arming (default OFF)

Mirrors the container backend's posture and `gce_capacity::LiveGceProvisioner`:

- The Cloud-VM lane defaults to the **fake** provisioner. Set
  `OA_CLOUD_VM_PROVISIONER=live` to opt into the firecracker lane.
- The live lane additionally requires **all** of:
  - a Linux host with a reachable `/dev/kvm`,
  - `OA_CLOUD_VM_KERNEL_IMAGE` (guest vmlinux) present on the host,
  - `OA_CLOUD_VM_ROOTFS_IMAGE` (ext4 rootfs bundling a headless browser + the qa
    session entrypoint) present on the host.
- Absent any of those, `provisioner_for(Live)` falls back to **fake** so no-KVM
  hosts (this macOS dev box, CI) never attempt a real boot.
- When armed but KVM is genuinely unavailable, `provision` refuses with
  `KvmUnavailable` (HTTP 500) — it does **not** fall back to a local browser and
  does **not** fake a result.
- A non-linux OS tier refuses with `OsTierUnavailable` (HTTP 400) until a
  macOS/Windows microVM host pool comes online.

Optional live env (with defaults):

- `OA_CLOUD_VM_FIRECRACKER_BIN` (default `firecracker`)
- `OA_CLOUD_VM_JAILER_BIN` (default `jailer`)
- `OA_CLOUD_VM_RUNTIME_DIR` (default `/srv/openagents/cloud-vm`)

## Invariants honored

- Refs-only handle + receipts: no raw KVM socket paths, tap devices, guest IPs,
  SSH keys, kernel/rootfs absolute paths, credentials, wallet material, bearer
  tokens, or private topology markers. `contains_forbidden_material` rejects
  these and is asserted over the full serialized outcome in tests.
- No wallet authority: the VM/workroom boundary carries no wallet seeds, node
  entropy, preimages, or raw accounting credentials.
- Degrade-or-refuse: a failed acquire / unhealthy boot tears down any partial
  jail before refusing; teardown is idempotent and always runs (even on
  exec/copy_out failure) so a VM is never leaked.

## Verify locally (no KVM, deterministic)

```bash
cd /Users/christopherdavid/work/cloud
cargo test -p oa-codex-control cloud_vm                 # fake-runtime unit tests
cargo test -p oa-codex-control --test cloud_vm_contract # provisioner-V2 contract test
cargo build                                             # workspace build
```

The live firecracker proof is `#[ignore]` by default (no `/dev/kvm` on the dev
box).

## Deploy step: live boot on a Linux KVM host

The live-on-hardware run is the deploy step (like other cloud infra). On a Linux
KVM host:

1. Build/obtain firecracker + jailer and a guest kernel (vmlinux) + an ext4
   rootfs image that bundles a headless browser and the qa session entrypoint
   writing artifacts under `/qa/artifacts`. See
   `projects/repos/firecracker/docs/getting-started.md` and `sek8s` for the
   hardened-guest / jailer patterns.
2. Place the images on the host and configure:
   ```bash
   export OA_CLOUD_VM_PROVISIONER=live
   export OA_CLOUD_VM_KERNEL_IMAGE=/srv/openagents/cloud-vm/vmlinux
   export OA_CLOUD_VM_ROOTFS_IMAGE=/srv/openagents/cloud-vm/rootfs.ext4
   ```
3. Run the ignored live proof to exercise the exact production path end to end:
   ```bash
   cargo test -p oa-codex-control \
     live_cloud_vm_session_extracts_and_tears_down -- --ignored --nocapture
   ```
   It must extract a `result.json` and report `tornDown = true`.
4. Run the daemon and point the qa-runner's HTTP-backed provisioner at
   `POST /v1/cloud-vm/sessions` with the daemon bearer token.

## Code-complete vs requires-a-KVM-host

- Code-complete now (no KVM): the provisioner trait + fake lane + the
  `provision -> exec -> copy_out -> teardown` driver + the HTTP route + the
  contract test + owner-gating/refusal posture + refs-only receipts.
- Requires a Linux KVM host (deploy step): the actual firecracker boot, the
  guest control-channel transport (vsock/ssh) for `exec`/`copy_out`, and the
  built kernel/rootfs images. The live module is structurally complete and
  gated; the guest-agent transport binary is wired per host image at deploy
  time, and the `#[ignore]` live proof + this runbook drive the on-hardware run.
- macOS/Windows OS tiers: refuse honestly until a host pool exists; tracked as
  they come online.

## Live host substrate proof (2026-07-07, openagents#8503)

The Firecracker substrate is proven on the real GCE host (public host
`agent-computer-gce-1`, project `openagentsgemini`, `us-central1-a`,
`n2-standard-4`, nested-virt on, `/dev/kvm` present, host kernel
`6.17.0-1020-gcp`). This is the substrate the Agent Computer flow needs; it does
not yet exercise `cloud_vm.rs`'s guest transport (still the deploy step below).

Installed + staged on the host:

- `firecracker` + `jailer` `v1.16.1` at `/usr/local/bin`.
- Baseline kernel + rootfs at `/srv/openagents/cloud-vm/` (firecracker-ci v1.10
  `vmlinux-5.10.223`, `ubuntu-22.04.ext4`). These match `OA_CLOUD_VM_KERNEL_IMAGE`
  / `OA_CLOUD_VM_ROOTFS_IMAGE`, so `require_ready()` would now pass on this host.

Proven lifecycle (tested working, tap + host NAT):

- boot: a real microVM came up in ~1s with its own guest kernel `5.10.223`
  (host runs `6.17`), 2 vCPUs / 1024 MiB — a true separate-kernel boundary.
- exec-in-guest: real commands ran inside the microVM over ssh-on-tap.
- egress: HTTPS `200` from inside the microVM through host NAT
  (`iptables -t nat MASQUERADE` on the default iface + `ip_forward=1`), fetched
  real bytes from a public repo. (Stock CI rootfs `curl` needs a CA bundle; the
  TLS handshake reached without it, so routing is confirmed.)
- copy-out: `result.json` copied microVM -> host.
- reclaim: microVM killed, per-run scratch rootfs removed, tap deleted.

Tested tap + boot recipe (the concrete reference for wiring `guest_exec` /
`guest_copy_out` and `wait_guest_ready`; no secrets, standard demo IPs only):

```bash
TAP=fc0; TAP_IP=172.16.0.1; GUEST_IP=172.16.0.2
HOST_IFACE=$(ip -o -4 route show to default | awk '{print $5}' | head -1)
ip tuntap add dev "$TAP" mode tap
ip addr add "${TAP_IP}/30" dev "$TAP"; ip link set "$TAP" up
sysctl -w net.ipv4.ip_forward=1
iptables -t nat -A POSTROUTING -o "$HOST_IFACE" -j MASQUERADE
iptables -A FORWARD -i "$TAP" -o "$HOST_IFACE" -j ACCEPT
iptables -A FORWARD -m state --state RELATED,ESTABLISHED -j ACCEPT
# boot_args add: ip=${GUEST_IP}::${TAP_IP}:255.255.255.252::eth0:off
# network-interfaces: [{iface_id:eth0, host_dev_name:$TAP, guest_mac:06:00:AC:10:00:02}]
```

Substrate gaps this proof surfaced (the remaining `cloud_vm.rs` deploy step and
the Agent Computer image):

1. The baseline rootfs is stock Ubuntu 22.04 CI — no `git`, `bun`, `node`,
   Pylon runtime, or guest agent. The Agent Computer rootfs must bake these in.
2. `guest_exec` / `guest_copy_out` still return
   `guest ... transport not wired on this host (deploy step)`; wire them to the
   proven ssh-on-tap (or vsock) bridge, and make `wait_guest_ready` poll a real
   guest readiness signal instead of a `v.sock` file check.
3. For the Khala Code Agent Computer turn (openagents#8503 DoD), the
   `POST /v1/placement` path must additionally boot a microVM from the baked
   image and run the #8473 executor for the admitted work context — today
   placement binds to a Codex runner lane, not a Firecracker microVM.

## Baked agent-computer image + vsock guest agent (2026-07-07, openagents#8503, WIP)

Progress toward a real in-microVM coding turn (increments 1-2):

- **Baked rootfs built** on `agent-computer-gce-1` at
  `/srv/openagents/cloud-vm/agent-computer-rootfs.ext4` (577M, sha256
  `3e612f6fd4e9f98cacf167cdb5582eaf2399a57e86705e4b165797367a63cf0b`). Built via
  `debootstrap` jammy (the firecracker-CI ubuntu rootfs is stripped — no
  `/var/lib/dpkg/status`, so `apt-get install` is impossible in it). Baked in:
  git 2.34.1, bun 1.3.14, python3, ca-certificates, openssh-client, a **vsock
  guest agent** at `/opt/agent/guest-agent.py` (AF_VSOCK port 1024, systemd unit
  `agent-guest.service` enabled), and `/opt/agent/turn-runner` (a compiled
  linux-x64 bun binary of the openagents repo
  `apps/pylon/deploy/agent-computer/turn-runner.ts`, which drives the real #8475
  workspace-materializer for a public-repo checkout + coding step).
- **vsock control channel proven**: the microVM boots from the baked image, the
  guest agent reports ready over vsock in ~4s, and `turn-runner` runs INSIDE the
  microVM (turn.started + workspace.checkout runtime events observed host-side
  over the firecracker vsock UDS). Working host-side vsock client + boot recipe:
  scratchpad `vsock-turn-proof.py` on this Mac (connect the firecracker
  `uds_path` UNIX socket, send `CONNECT 1024\n`, then length-prefixed JSON
  `{op:ping|exec|copyout}`; the guest agent is the reference protocol).
- **BLOCKER**: in-guest git egress fails (`git fetch` -> `fetch_failed`). The
  debootstrap image enables `systemd-networkd` with no `.network` config, which
  likely clears the kernel `ip=` boot-arg address on `eth0`. Fix: drop the
  `systemctl enable systemd-networkd` and rely on the kernel `ip=` config (proven
  working with the stock rootfs), OR ship a proper `/etc/systemd/network/
  10-eth0.network` static config + ensure `/etc/resolv.conf` (8.8.8.8) survives.
  Re-run scratchpad `vsock-turn-proof.py` after the fix.

**Increment 2 (guest transport in `cloud_vm.rs`) is NOT yet ported to Rust.** The
proven Python vsock protocol above is the reference for wiring `guest_exec`/
`guest_copy_out` (connect the vsock UDS, `CONNECT 1024`, length-prefixed JSON)
and `wait_guest_ready` (poll `{op:ping}` for `"ready"`). Then run the ignored
`live_cloud_vm_session_extracts_and_tears_down` test.
