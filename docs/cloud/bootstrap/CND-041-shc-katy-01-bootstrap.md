# CND-041 SHC Katy Bootstrap Smoke

> **Historical bootstrap note (#8591).** Kept for archaeology and ops memory.
> Active Cloud implementation is in the public monorepo (`crates/*`,
> `docs/cloud/`). Deprecated authority names: **Vortex** → Worker/Khala Sync;
> **Treasury product** → Worker credits + MDK/Nexus payout bridge only;
> **Nexus-as-registry** → Worker/Khala Sync (CLI may still say `nexus`).
> Do not treat this note as current product-authority ownership.

Status: measured bootstrap smoke passed on 2026-06-01

Host: `oa-shc-katy-01` at `23.182.128.195`

## Host Inventory

```text
OS: Ubuntu 24.04.4 LTS
Kernel: 6.8.0-124-generic
User: ubuntu with passwordless sudo
CPU: 16 logical CPUs, Intel Xeon Processor (Skylake, IBRS, no TSX)
Memory: 62 GiB usable
Disk: 247 GiB root disk, about 245 GiB free before bootstrap
KVM: /dev/kvm present, kvm-ok reports acceleration can be used
Virtualization flags: 32 vmx/svm matches
```

## Setup Performed

Installed the same baseline tooling used for the GCP fallback VM lane:

```bash
sudo apt-get update
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y \
  ca-certificates curl git jq build-essential pkg-config libssl-dev \
  bubblewrap cpu-checker qemu-kvm iproute2 iptables nftables socat nodejs npm rsync
```

Rust was installed with rustup stable:

```text
rustc 1.96.0 (ac68faa20 2026-05-25)
cargo 1.96.0 (30a34c682 2026-05-25)
node v18.19.1
npm 9.2.0
```

Codex CLI was installed from the official `openai/codex` GitHub static release
tarball, not through Cargo:

```bash
case "$(uname -m)" in
  x86_64) A=x86_64-unknown-linux-musl ;;
  aarch64|arm64) A=aarch64-unknown-linux-musl ;;
esac
V=$(curl -fsSL https://api.github.com/repos/openai/codex/releases/latest | jq -r .tag_name)
curl -fsSL "https://github.com/openai/codex/releases/download/$V/codex-$A.tar.gz" \
  | sudo tar -xzC /usr/local/bin
sudo mv "/usr/local/bin/codex-$A" /usr/local/bin/codex
sudo chmod +x /usr/local/bin/codex
```

Result:

```text
codex-cli 0.135.0
```

Attempting `cargo install codex-cli --version 0.135.0 --locked` failed because
that crate/version was not available from crates.io on this host. Do not use
Cargo as the Codex CLI bootstrap path for this lane.

## Cloud Bootstrap Proof

The Cloud repo was synced to `~/openagents-cloud` without `.git` or `target`.
The baseline bootstrap proof passed:

```bash
scripts/verify-bootstrap.sh
```

Result:

```text
cloud bootstrap verification passed
```

That proof ran `cargo check`, contract tests, `oa-node --help`,
`oa-workroomd --help`, `oa-node status --json`, and
`oa-workroomd status --json`.

## Codex Workroom Runner Smoke

The narrow fake-Codex runner test passed on the SHC VM:

```bash
cargo test -p oa-workroomd --test codex_run -- --nocapture
```

Result:

```text
test codex_run_executes_captures_artifacts_scrubs_auth_and_redacts_events ... ok
```

This proves the VM can build and execute the local Codex workroom runner path:
assignment validation, session auth-state handling, event redaction, artifact
capture, closeout, workspace cleanup, and auth cleanup. It does not prove a
real ChatGPT/Codex account-backed run.

On 2026-06-01, a real ChatGPT/Codex auth grant for
`provider-account_dc23d0ab_mputrw3b` was materialized on SHC and
`codex login status` returned `codex_login_status_ok`. Codex
`workspace-write` then failed in this nested VPS because the Linux sandbox
could not bring loopback up through bubblewrap (`loopback: Failed RTM_NEWADDR`)
and legacy Landlock is incompatible with this permission profile. The first
real SHC account-backed Codex run therefore uses the explicit
`danger_full_access` assignment profile inside the no-wallet VM/workroom
boundary.

## Managed Node Lifecycle Smoke

Using a temporary state directory, `oa-node` initialized the SHC node identity,
modeled the intended `systemd` service manager path, restarted the service
state, entered quarantine, emitted a quarantine receipt, exited quarantine, and
projected a valid final node status.

Key outputs:

```text
node_id: oa-shc-katy-01
operator_identity: org.openagents
service_manager: systemd
service_status after restart: running
quarantine enter result: new_work_blocked
quarantine exit result: released
final observed_status: offline
final sandbox_policy: disabled_until_profiled
final settlement_policy: no_wallet
```

This is a scaffold lifecycle smoke, not a real host-level `systemd` unit
installation.

## Firecracker Smoke

The SHC host now supports the manual Firecracker smoke:

```text
Firecracker: v1.15.1 installed at /usr/local/bin/firecracker
Jailer: v1.15.1 installed at /usr/local/bin/jailer
Guest kernel: vmlinux-6.1.155
Guest rootfs: Ubuntu 24.04.3 LTS
Guest proof: SSH to 172.16.0.2 returned OA_FIRECRACKER_GUEST_OK
Cleanup: Firecracker process stopped, TAP device deleted, NAT rules removed,
         ip_forward restored to 0, smoke workdir removed
```

Cleanup sanity check:

```text
tap-oa-fc0: absent
ip_forward: 0
kvm-ok: KVM acceleration can be used
```

This proves the SHC Katy VPS can manually boot a nested Firecracker microVM.
It does not prove the production `sandbox.firecracker.exec` profile. Promotion
still requires `oa-workroomd` integration, jailer/cgroup policy, guest
kernel/rootfs digests, TAP/firewall receipts, artifact closeout, idempotent
cleanup, and Psionic sandbox evidence.

## Next Step

Run a real account-backed Codex workroom through the product surface:

```text
Autopilot/Worker/Khala Sync provider-account grant
  -> Cloud control API
  -> oa-shc-katy-01 or GCP fallback runner
  -> per-session CODEX_HOME
  -> codex login status
  -> codex exec
  -> artifact and closeout receipts
```

Do not reuse a global VM Codex login for workrooms.
