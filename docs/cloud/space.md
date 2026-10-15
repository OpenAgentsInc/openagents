# SPACE computer driver

Issue [#11728](https://github.com/OpenAgentsInc/openagents/issues/11728).
The [assessment](../research/perplexity/2026-10-15-space-assessment.md)
defines the measured spike. SPACE runs as a separate service, pinned to
[`de87b436`](https://github.com/perplexityai/SPACE/commit/de87b436c35dac056564bdd5b2a113c9046e189b).
Production continues to use Boat.

## Driver

Build `oa-computers` with `--features space`, set
`OA_COMPUTERS_DRIVER=space`, and provide these variables:

| Variable | Value |
| --- | --- |
| `SPACE_API_URL` | Management API origin, without `/v1` |
| `SPACE_DATA_PLANE_URL` | Command stream origin |
| `SPACE_API_TOKEN` | Management token; never log it |
| `SPACE_TEMPLATE` | Template reference, preferably pinned by digest |

HTTPS is required except on loopback. Redirects and inherited proxies are
disabled. Commands attach to the configured data-plane origin with their
own session token. Per-turn secrets go through process input because SPACE
saves the command environment in its session database.

The driver declares Core and Suspend. Suspend uses a memory snapshot;
resume uses `:wake`. Snapshot and Fork return unsupported: this version's
public API has no create-from-snapshot or fork operation. Files are bounded
to 16 MiB, and command output to 16 MiB. HTTP calls and command streams have
deadlines. Interrupted requests return Unknown; callers observe the
computer before repeating an effect. The existing reconciler polls lifecycle
readiness; accepting an operation does not mean the computer is ready.

SPACE template references differ from GCE image names. A create that names
an unavailable template fails at SPACE. The lifecycle service enforces its
computer lifetime; the driver does not translate a hard lifetime into
SPACE's access-idle timeout. The public API exposes no billable-seconds
counter, so `Meter.seconds` is absent. The `small` size leaves the shape to
SPACE so snapshot boots remain eligible; `medium` and `large`
request explicit shapes and therefore cold-boot in this SPACE version.

## Disposable GCE setup

Use the configured gcloud login. The scripts create only resources named
`oa-space-spike-11728` in `openagentsgemini`, with a dedicated network,
Cloud NAT, IAP SSH, no VM external address, and no service account.
The `n2-standard-8` VM has nested virtualization and a 375 GiB btrfs local
SSD. It automatically deletes after six hours; run teardown to remove the
network and NAT too.

```sh
deploy/space-host/host.sh create
deploy/space-host/host.sh install
deploy/space-host/host.sh status
```

Wait for `SPACE installation and first sandbox passed.` before preparing
the runtime template:

```sh
deploy/space-host/host.sh runtime
deploy/space-host/host.sh template
```

Installation and template publication run as bounded systemd jobs. On the
host, `/var/lib/space-spike/home/install.log` and `template.log` show their
progress. The runtime step extracts only Coder and Claude binaries from
the `oa-coder-main-20261015` image in our project (override with
`SPACE_RUNTIME_IMAGE`); its temporary disk is read-only and deleted after
extraction. The template keeps SPACE's guest daemon and kernel.

Build the check programs for Linux with `--features space`: the
`computer-conformance` binary and the `computer-driver` example
`space-spike`. On Nix, use the `x86_64-unknown-linux-musl` target with a musl
C compiler so the programs run on Ubuntu. Set `SPACE_CONFORMANCE_BIN` and
`SPACE_SPIKE_BIN` to their paths for the first upload:

```sh
deploy/space-host/host.sh check conformance
# After conformance finishes, run each arm separately.
deploy/space-host/host.sh check prepare-cold
deploy/space-host/host.sh check cold
deploy/space-host/host.sh check prepare-snapshot
# Wait for snapshots to become available before starting the second arm.
deploy/space-host/host.sh check inspect
deploy/space-host/host.sh check snapshot
```

The check jobs write results under `/var/lib/space-spike/home/evidence/`.
Each arm records twenty creates and their Firecracker launch arguments:
cold boots supply `--config-file`, while restores start idle and load a
snapshot through the Firecracker API. A mismatched boot mode fails the run.
The timer ends after the first successful command, before collecting these
observations. The runner also checks runtime versions, a separate 4 GiB
guest's 1 GiB memory process before and after suspend, and network access.
Preparation allows only `api.github.com`; the network check requires that
request to succeed and a request to `example.com` to fail. Keep the two
arms and conformance sequential so they do not contend for the same host.

For an existing deployment, `computer-conformance --space` and
`space-spike EVIDENCE.json cold|snapshot` use the environment variables above.
Use a fresh evidence path for each run. Without `SPACE_BOOT_OBSERVER`, the
measurement runner marks the actual boot mode as unverified.

```sh
deploy/space-host/host.sh destroy
```

Teardown deletes the VM, its disks, router, firewall, subnet, and network,
then lists any remaining spike instances or disks. Retain evidence before
deleting the host's local SSD.
