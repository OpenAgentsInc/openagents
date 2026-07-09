# `openagents.cloud_vm_provisioner.v1`

Status: code-complete in `cloud` (provisioner + HTTP route + fake lane +
contract test); the live firecracker boot is the deploy step on a Linux KVM
host. Tracks OpenAgentsInc/openagents issue #6200 (follow-up from #6186).

The production cross-OS Cloud-VM provisioner behind the qa-runner's typed
`CloudVmProvisionerV2` / `CloudVmHandle` seam (openagents
`apps/qa-runner/src/backend.ts`). It implements the
`provision -> exec -> copyOut -> teardown` lifecycle on a requested OS tier over
firecracker microVMs and exposes it over HTTP from `oa-codex-control` so the
qa-runner can call it.

Implementation: `crates/oa-codex-control/src/cloud_vm.rs`, wired into the HTTP
surface in `crates/oa-codex-control/src/main.rs`
(`POST /v1/cloud-vm/sessions`). Contract test:
`crates/oa-codex-control/tests/cloud_vm_contract.rs`. Runbook:
`docs/bootstrap/CND-056-cloud-vm-firecracker-provisioner.md`.

## OS tiers

`os` is one of `linux` | `macos` | `windows` (mirrors the qa-runner `CloudVmOs`).
Linux is the first production tier. macOS/Windows tiers refuse honestly
(`OsTierUnavailable`) until a microVM host pool for that OS comes online.

## Provisioner lane

The live firecracker path is gated behind `OA_CLOUD_VM_PROVISIONER`:

- `fake` (default) — deterministic provisioner that never touches KVM. It
  returns a stable opaque VM ref, records the exec command, and materializes a
  public-safe artifact set (`result.json` shaped as the qa-runner
  `openagents.qa_runner.result.v1` with `backend = "cloud-vm"`, plus a snapshot
  marker). Used by unit tests and any no-KVM host.
- `live` — gated behind a Linux host with a reachable `/dev/kvm` **and** a
  configured guest kernel + rootfs image. The live provisioner writes a per-run
  firecracker config (kernel + rootfs + a vsock control channel) and launches
  firecracker under the **jailer** for seccomp/cgroup/chroot isolation
  (firecracker production guidance + sek8s hardened-guest patterns), waits for
  the guest to report ready, execs over the guest control channel, copies the
  in-VM artifact dir (`/qa/artifacts`) back out to a host dir, and tears the
  jail down. If KVM is unavailable, the images are missing, or the OS tier has
  no host pool, it refuses (never falls back to a local browser, never fakes a
  green); `provisioner_for(Live)` falls back to `fake` so no-KVM hosts never
  attempt a real boot.

Live env (kernel + rootfs are required to arm the live lane):

```bash
export OA_CLOUD_VM_PROVISIONER=live
export OA_CLOUD_VM_KERNEL_IMAGE=/srv/openagents/cloud-vm/vmlinux
export OA_CLOUD_VM_ROOTFS_IMAGE=/srv/openagents/cloud-vm/rootfs.ext4
# optional overrides:
export OA_CLOUD_VM_FIRECRACKER_BIN=firecracker
export OA_CLOUD_VM_JAILER_BIN=jailer
export OA_CLOUD_VM_RUNTIME_DIR=/srv/openagents/cloud-vm
```

## HTTP surface

`POST /v1/cloud-vm/sessions` (alias `/v1/cloud-vm/sessions/start`).
Bearer-gated by the daemon `OA_CODEX_CONTROL_TOKEN`, like every `/v1/*` route.

Request body (camelCase):

```json
{
  "runId": "run_contract_demo",
  "os": "linux",
  "targetName": "openagents.com-staging",
  "ownerRef": "owner://sha256/...",
  "sessionCommand": ["sh", "-c", "qa-session --emit /qa/artifacts"]
}
```

- `sessionCommand` is the command run INSIDE the VM to produce the session +
  artifacts; it must write outputs under `/qa/artifacts`. Mirrors the container
  backend's `sessionCommand`. It is passed verbatim, never shell-interpolated by
  the daemon.
- The host extraction dir is owned by the daemon (under its state root) and
  returned as `extractedTo`; the caller never supplies a host path.

Response body (`CloudVmSessionOutcome`, refs-only):

```json
{
  "contractVersion": "openagents.cloud_vm_provisioner.v1",
  "vmId": "cloud-vm-ref://sha256/...",
  "os": "linux",
  "provisionerKind": "fake",
  "exec": { "code": 0, "output": "..." },
  "extractedTo": "/var/lib/openagents/codex-control/cloud-vm-artifacts/<digest>",
  "provisionReceipt": {
    "contractVersion": "openagents.cloud_vm_provisioner.v1",
    "runRef": "cloud-vm-run://cloud/session/<digest>",
    "vmRef": "cloud-vm-ref://sha256/...",
    "os": "linux",
    "provisionerKind": "fake",
    "healthy": true,
    "receiptDigest": "sha256:...",
    "emittedAtMs": 0
  },
  "cleanupReceipt": {
    "contractVersion": "openagents.cloud_vm_provisioner.v1",
    "runRef": "cloud-vm-run://cloud/session/<digest>",
    "vmRef": "cloud-vm-ref://sha256/...",
    "tornDown": true,
    "artifactsExtracted": true,
    "receiptDigest": "sha256:...",
    "emittedAtMs": 0
  }
}
```

## Seam mapping (qa-runner `CloudVmProvisionerV2` ↔ this contract)

| seam member                        | this contract                                  |
| ---------------------------------- | ---------------------------------------------- |
| `provisionVm({ target, os, ... })` | `POST /v1/cloud-vm/sessions` (provision stage) |
| `CloudVmHandle.id`                 | `vmId`                                          |
| `CloudVmHandle.os`                 | `os`                                            |
| `CloudVmHandle.exec(cmd, args)`    | `exec: { code, output }`                        |
| artifact extraction (`copyOut`)    | `extractedTo` + dereferenceable `result.json`  |
| `CloudVmHandle.teardown()`         | `cleanupReceipt.tornDown`                       |

## Errors (honest, never a fake green)

| condition                       | typed error          | HTTP |
| ------------------------------- | -------------------- | ---- |
| empty/forbidden request field   | `InvalidRequest`     | 400  |
| unknown / unavailable OS tier   | `OsTierUnavailable`  | 400  |
| armed live but KVM unavailable  | `KvmUnavailable`     | 500  |
| provision/exec/teardown failure | `Runtime`            | 500  |

## Invariants

- Refs-and-limits only: the handle and both receipts reject raw KVM socket paths,
  tap devices, guest IPs, SSH keys, kernel/rootfs absolute paths, credentials,
  wallet material, bearer tokens, and private topology markers
  (`contains_forbidden_material`, asserted over the full serialized outcome).
- No wallet authority crosses the VM/workroom boundary.
- Degrade-or-refuse: failed acquire / unhealthy boot tears down any partial jail
  before refusing; teardown is idempotent and always runs (even on
  exec/copy_out failure) so a VM is never leaked.
