# Private SPACE service

This directory owns the separate maintained deployment for #11814. It does not
reuse a disposable spike host. The native service, recovery, and capacity checks
must pass before this is ready for customer computers.

The key broker uses a dedicated Cloud Run identity and KMS key. Its ingress is
internal, its workload bearer and policy come from numeric Secret Manager
versions, and at most one instance serves eight requests at once. Only the key
broker identity can use its KMS key. Workers receive no broker identity.

Run `python3 deploy/space-service/broker.py foundation` to create the owned private
network, two subnets, NAT, IAP-only operator access, identity, and rotating KMS
key. The helper refuses unrelated resources with matching names. No public SSH
or management listener is opened.

Run `broker.py configure TENANTS.json` with a JSON list of allowed local SPACE
organization UUIDs. Retrying preserves the existing bearer and identical policy.
A different policy requires a reviewed new secret version: removing an
organization or key route can make its saved computers inaccessible. Retain old
KMS versions and routes while private data references them.

Build `key-broker-cloudbuild.yaml` with the full `_COMMIT`, then run
`broker.py deploy COMMIT sha256:DIGEST --policy-version N --token-version N`.
Deployment verifies source ancestry and the registry digest. Pin all numeric
secret versions; do not use `latest`.

`broker.py probe-host` creates an isolated, private, credential-free test VM that
deletes itself after two hours. Run the Rust `space-key-broker` example `probe`
there with the endpoint, mounted bearer file, namespace, and two distinct allowed
organization IDs. It never prints plaintext keys or credentials. Delete the
probe VM as soon as the check ends; its boot disk deletes with it.

`control.py images`, `control.py database`, and `control.py nodes` create the
dedicated image repository, private PostgreSQL instance, and two private nodes.
The database starts at 20 GiB, grows to at most 40 GiB, and keeps seven daily
backups plus seven days of point-in-time recovery. The control node has four
vCPUs and an 80 GiB data disk; the nested-KVM worker has eight vCPUs, 32 GiB RAM,
and a 300 GiB data disk. Data disks survive VM deletion. Neither node has a
public address; the worker has no Google identity. Run `initialize.py control`
and `initialize.py worker` from committed main to install the pinned k3s binary.

`storage.py` provisions separate templates and snapshots buckets. Access is
limited to the owned VPC, with public access prevention and bucket-scoped HMAC
credentials in Secret Manager. Administration uses a private SSH TCP forward:
Google authentication and TLS stay on the operator's machine. Deleted objects
remain recoverable for seven days; active private data has no age-only expiry.
`storage-probe.py` exercises the S3-compatible XML protocol with synthetic bytes
and removes its active objects and unfinished uploads. Native checkpoint
recovery still requires the worker-replacement qualification. Native downloads
use one complete response because [Cloud Storage returns whole-object checksums
for ranged responses](https://docs.cloud.google.com/storage/docs/xml-api/get-object-download).
This keeps SDK checksum validation enabled. Each worker runs at most two uploads,
two downloads, and one compression pass at once. Snapshot streaming and deferred
writes each have a 64 MiB budget within the transfer service's 1 GiB memory limit.


## Native service qualification

Render the pinned production charts with `render.sh`. `image-pins.json` records
all deployed native images, the TLS startup fix for the migration CLI, and the
private streaming frontend. The maintained seed recipe selects the public
Firecracker CI kernel by its verified digest; it does not claim the upstream
custom BPF profile whose configuration is absent from the public source.

Install `host-prerequisites.sh` before starting worker services. It uses the
upstream service identities and bounds the guest cgroup to four CPU cores,
8 GiB of memory, 4,096 processes, and 64 MiB/s and 2,000 operations/s in each disk
I/O direction. Control services have one replica each. The worker and builder
run the same pinned Firecracker, Jailer, kernel, and containerd shim. The shim's
authenticated proxy needs TCP 9800 to the worker's local bridge, `10.220.1.1`.
The sandbox network policy permits only that exact hop. Verify the worker's
bridge address before admitting a replacement with a different Kubernetes pod
CIDR, and exercise the guest denial check after changing this allowance.
Node-bound RPCs use the control node's private TLS listener directly; sending
them through the pod network changes their source address and correctly fails
the native node-identity check. The single control listener restarts in place.

Run the migration Job in grants-only mode, activate its five existing logins
with `database_roles.py --activate` on the control node, and run
`database.py --set-role-passwords` on the operator machine. Cloud SQL prevents
the bootstrap account from disabling statement logging, so passwords go through
the SQL Admin API instead of SQL statements. Run `database_roles.py` on the
control node to verify each saved password, TLS, and the superuser fence.

`control.py builder --builder-zone us-central1-a` creates one private temporary
builder with a four-hour deletion limit and no Google identity. Initialize it
with `initialize.py builder`, install the host prerequisites, runtime and shim,
and start `builder-containerd.sh` before applying its rendered services. Delete
the builder and its two disks as soon as publication ends; the timeout is a
fallback. No customer checkpoint belongs on this temporary node.

The API and data frontends use internal Cloud Run ingress, a dedicated identity,
one instance each, and verified TLS to the control node. Their proxies stream
HTTP and WebSockets without buffering or logging request paths and headers.
`qualification-fixture.py`, `qualification_fixture.py`, and
`qualification_native.py` use a separate synthetic account and a four-hour
admin session. Remove its computers, credentials, session, and owner rows when
qualification ends. The production Boat deployment remains separate.

The deployment is still under qualification. A healthy process and successful
database migration do not prove checkpoint recovery, capacity admission,
upgrades, or customer readiness. Retain that distinction in published evidence.


The separate `oa-space-service-computers` candidate uses Cloud SQL Auth Proxy
2.26.0, pinned by digest, on loopback beside the Rust service. The proxy verifies
Cloud SQL TLS over the private IP; its service identity has Cloud SQL Client
access only to the native instance through an IAM condition. It accepts at most
32 database connections. The application uses the limited `space_computers`
login in a separate database; the two application roles cannot connect to each
other's database. See Google's [proxy documentation](https://docs.cloud.google.com/sql/docs/postgres/connect-auth-proxy)
and [instance-scoped IAM conditions](https://docs.cloud.google.com/sql/docs/postgres/iam-conditions).

Before creating qualification computers, run `database_roles.py --candidate-limits`
on the control node after the empty candidate has applied its migrations. It
bounds both the service and its synthetic account to two computers, four running
vCPUs, 8 GiB RAM, 60-second commands, and one hour of stopped retention. Later
limit changes use `oa-computers-usage` so committed allocations are checked.
The sandbox namespace separately limits guest pods. Firecracker resources are
controlled by the host cgroup and Computers allocation limits; the native pause
container declares no Kubernetes CPU or memory request.
Object transfers use two 8 MiB upload parts, two 8 MiB download readers, and a
256 MiB process-wide download-buffer budget.

`database-recovery.py` creates and restores only the separate
`oa-space-service-restore-11814` copy. Export its public connection settings after
the restore operation completes, verify them with `database_restore.py` on the
control node, and delete the copy immediately afterward. It never restores a
backup over the maintained instance.
