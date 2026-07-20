# SBX-05 managed-sandbox guest I/O

- Date: 2026-07-19
- Issue: [#9026](https://github.com/OpenAgentsInc/openagents/issues/9026)
- Status: default-off component accepted
- Live GCP proof owner: SBX-09
- Private schema: `openagents.managed_sandbox_guest_io.v1`

## Outcome

SBX-05 connects the admitted Box file, command, and artifact calls to one
private control route. The route targets one exact sandbox generation and one
active capability.

The Worker checks lease, lifecycle, guest, filesystem, capability, and quota
facts before each private call. It sends no cloud credential or provider
topology to the client.

## Closed actions

The private action set has four members:

- `read_file`
- `write_file`
- `execute_command`
- `read_artifact`

Each request binds owner, tenant, work unit, sandbox, generation, operation,
retry identity, capability, time, and limits. Extra action fields cause a
refusal.

## File policy

Paths must equal `workspace` or start with `workspace/`. Absolute paths, dot
segments, empty segments, backslashes, NUL bytes, and paths above the root are
invalid.

The guest driver must use a no-follow beneath-root file operation. A success
receipt must state `resolved_beneath_workspace_root` and
`symlinkTraversal=false`.

Text and base64 content have a one MiB operation cap. The private guard checks
the exact digest and byte count. Known private-key and bearer patterns cause a
refusal.

## Command policy

The command runs only in the exact guest. The control host does not invoke a
shell. The request sets duration, CPU, output, process, and network limits.

The compatibility command has zero network bytes and the deny-all network
policy. A success receipt requires a closed process tree, zero descendants,
clean scratch, closed ingress, and denied egress.

The control guard places its driver in a process group. It kills that group if
the driver exceeds the declared command deadline plus a short control margin.

## Artifact policy

Artifact bytes use base64 only on the private control channel. The Worker
returns raw bytes with no-store, digest, artifact, generation, retention, and
receipt headers.

The artifact ref is `artifact.sha256.<hex>`. Its receipt binds the exact
content digest, byte count, source generation, source path digest, content
type, retention time, and evidence refs.

## Configuration

The Worker requires its current private control URL and bearer. The control
service requires an absolute `OA_MANAGED_SANDBOX_IO_DRIVER` path.

The deployment helper accepts `--managed-sandbox-io-driver`. It refuses a
relative path or use without `--enable-managed-sandbox`.

Absent configuration returns typed `503`. There is no local, fake, alternate
provider, broad host, wallet, payment, or cloud-admin fallback.

SBX-09 supplies the live driver at
`scripts/cloud/managed-sandbox-io-driver.mjs` and installs the guest executor
from `scripts/cloud/managed-sandbox-guest-io.py` in the immutable image. The
control image includes the OpenSSH client (`ssh`, `scp`, and `ssh-keygen`), and
the control driver uses those tools through `gcloud compute ssh/scp` over the
guest's internal IP only. The guest executor resolves file and artifact paths
with Linux `openat2` beneath `/workspace`, rejects symlinks and magic links,
and runs commands in a Bubblewrap network namespace with no network
interface. It applies CPU, process, duration, and combined output limits,
kills the process group on timeout or output excess, scans returned bytes for
secret-shaped material, and removes operation scratch before it emits a
success receipt.

Both guest drivers use `--ssh-key-expire-after=10m`, the relative-duration
gcloud flag. They must not pass a duration to `--ssh-key-expiration`, which is
an absolute-timestamp flag and fails before guest-key installation.

## Fault response

The component refuses these classes before it returns success:

- path escape or an unproven symlink boundary
- expired or revoked capability
- stale generation or scope drift
- invalid base64, digest, byte count, or retention
- private-key or bearer material
- file, artifact, output, CPU, duration, process, or network excess
- open process descendants, scratch residue, ingress, or egress
- guest driver failure, crash, malformed output, or deadline excess

Provider lifecycle delete remains the cleanup owner for compute, firewall,
disk, scratch, ingress, and grants. An incomplete delete remains
`recovery_required` under the SBX-02 contract.

## Verification

```bash
pnpm --dir packages/managed-sandbox-contract run typecheck
pnpm --dir packages/managed-sandbox-contract run test
pnpm --dir apps/openagents.com/workers/api run typecheck
pnpm --dir apps/openagents.com/workers/api exec vp test --run src/managed-sandbox-box-v1-routes.test.ts
cargo test -p oa-codex-control managed_sandbox_guest_io
bash -n scripts/cloud/gcp-codex-control-deploy.sh
node --check scripts/cloud/managed-sandbox-io-driver.mjs
python3 -m py_compile scripts/cloud/managed-sandbox-guest-io.py
pnpm run check:fast
```

The contract suite checks the closed request and receipt shapes. The Worker
suite checks the unmodified SDK, raw receipts, artifact headers, path faults,
size faults, capability revoke, and private adapter digest check.

The Rust suite checks every action. It also checks path, secret, quota,
symlink, egress, process, digest, and deadline faults.

## Proof boundary

The receipt is
`docs/sol/evidence/2026-07-19-sbx05-managed-sandbox-guest-io.json`.

This packet proves the default-off component and fault boundary. It does not
claim a live guest driver, live GCP I/O, public facade, or release approval.
SBX-09 owns those independent live claims.
