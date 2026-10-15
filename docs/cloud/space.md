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
counter, so `Meter.seconds` is absent. The `small` size uses the template's
default shape so snapshot boots remain eligible; `medium` and `large`
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
deploy/space-host/host.sh runtime
deploy/space-host/host.sh template
```

Installation and template publication run as bounded systemd jobs. On the
host, `/var/lib/space-spike/home/install.log` and `template.log` show their
progress. The runtime step extracts only Coder and Claude binaries from
the `oa-coder-main-20261015` image in our project (override with
`SPACE_RUNTIME_IMAGE`); its temporary disk is read-only and deleted after
extraction. The template keeps SPACE's guest daemon and kernel.

Run conformance with `cargo run -p computer-conformance --features space
-- --space`. For measurements, run `cargo run -p computer-driver --features
space --example space-spike -- EVIDENCE.json cold` and repeat with a fresh
evidence path and the `snapshot` arm after enabling the template's base
snapshot policy. Retain backend boot-mode observations: a requested arm
alone does not prove a snapshot was used. The runner records twenty
creates, versions, and a 1 GiB memory process before and after suspend.
Set `SPACE_EXPECT_GITHUB_ONLY=1` only with a template whose network policy
allows GitHub alone, to add the egress check.

```sh
deploy/space-host/host.sh destroy
```

Teardown deletes the VM, its disks, router, firewall, subnet, and network,
then lists any remaining spike instances or disks. Retain evidence before
deleting the host's local SSD.
