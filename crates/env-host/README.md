# Environment allocator

`env-host` implements the NIP-ENV allocator, materializer, participant, and
worker-gate roles in Rust. It signs leases, materializations, states,
attachment grants, cleanup receipts, accounting, and operation results. A lease
is not a wallet authorization, a commercial order, or a grant of workspace or
credential access.

For a registered external native operation, `with_runtime_execution` holds ENV
and COORD admission through the runtime callback. COORD saves the call charge
and unknown execution hold before exposing the exact inputs. The callback gets
a borrowed `RuntimeFence`, whose session-side checks compare the actual grant,
controller, worker, lease, materialization, engine, and deadline without releasing
either authority lock. A returned start acknowledgment does not settle the job.
A lost callback acknowledgment stays unknown across restart and cannot dispatch
again. The runtime still owns its exact capability and operation implementation.

```sh
env-host init --state DIR --key-file KEY --policy POLICY.json
env-host submit --state DIR --key-file KEY --provider-root RESOURCES --input BUNDLE
env-host materialize --state DIR --key-file KEY --provider-root RESOURCES --lease ID --input SPEC
env-host status|reconcile|cleanup --state DIR --key-file KEY --provider-root RESOURCES --lease ID
env-host expire|leases --state DIR --key-file KEY --provider-root RESOURCES
```

The key file must have mode `0600`. POLICY names the independently authorized
`owners`, `controllers`, and `readers`, the provider `account`, and maximum TTL
and lifetime. A request cannot add itself to those sets. BUNDLE carries one
signed private `3188` `event`, the exact `artifacts` it references (a digest to
byte-array map), and further signed `declarations` such as the caller's execute
declaration. The same `Allocator::submit` API serves local and remote calls.
The `openagents env` alias waits for the next Coder release; use this binary.

## What the allocator enforces

- **Admission before effects.** A request is admitted only from an authorized
  owner naming this allocator. The allocator reserves the request's
  `spend_microunits` ceiling against the parent reservation, net of every other
  lease's hold, before any provider call. It retains a RUN `dispatched` record
  first, so a crash leaves an allocation to reconcile, never a blind retry.
- **Unknown stays unknown.** A lost create response records `unknown`.
  `reconcile` looks up the exact idempotency key; it never creates again. Only
  an authoritative absence marks the allocation failed.
- **Exact readiness.** `materialize` places the definition's exact component
  and workspace bytes and compares the digests actually observed in the
  resource. A missing or substituted component, an unlisted recipient, a
  changed executor runtime or configuration, an opaque model under requirements
  that need model evidence, a required bound without a mechanism, or a plan
  that widens the request refuses readiness. Each readiness gets a fresh
  generation. Bootstrap supports the separately admitted fixed manifest
  initialization profile; other bootstrap operations refuse.
- **One slot, bound before dispatch.** An attachment needs the current revision
  and materialization, the primary executor as worker, a controller with a
  current generation, a caller-signed execute declaration whose plaintext
  requires `openagents.environment-binding.v1`, the exact input digest, retained
  current COORD claim, and a reservation that does not exceed the lease. Exact retries
  return the original grant; changed bytes under the same lease, caller, CJ
  request, and attempt conflict. A second execute cannot reserve the slot.
- **Participants admit what they can see.** The primary worker decrypts the
  signed CJ execute and checks byte identity with the declaration; others check
  their own projection. Activation needs every participant's admission.
- **Dispatch check.** `participant::intake` decodes only executes that carry
  the binding feature and then requires `Allocator::check_dispatch`: an active
  attachment, the same execute fingerprint, and unexpired lease and grant. The
  generic CJ decoder still refuses that feature.
- **Expiry and drain.** Effective expiry is the earliest of `expires_at`,
  `last_activity_at + idle_seconds`, and `not_after`. Renewal needs owner or
  controller authority, the current revision, and an unexpired lease; an
  expired lease is never resurrected. `expire` moves expired leases to
  draining. Only fencing receipts from every participant free the slot; an
  unreachable dispatcher keeps the hold and cleanup pending.
- **Cleanup obligations.** Cleanup first copies the resource's outputs into the
  allocator's store. If retention fails, nothing is deleted. Created resources
  are destroyed by exact identity and generation; adopted resources are only
  detached. Unknown or failed teardown stays `unknown` or `failed`; only a
  confirmed teardown releases the lease. An unknown charge keeps the
  reservation hold even after release. Cleanup intent and copied outputs persist
  before deletion; recovery probes the original teardown without deleting again.
  Resource lookup supplies no cost receipt and cannot turn an unknown charge
  into zero.

Accounting keeps measured, declared, estimated, and unknown cost entries
separate and does not count an identical receipt twice.

## Providers

- `local::LocalDirectories` is a private directory per lease, owned by an exact
  marker record (`openagents.environment-ownership.v1`). A path or name never
  proves ownership; adoption requires the exact marker bytes and generation.
- `ssh::SshEnvironments` maps the existing `coder-ssh` launcher. The remote
  account's records decide managed versus external: a managed host maps to a
  created resource; an external host is never relabeled as created. Destroy
  requires an atomic stop of the exact ownership generation. The legacy
  launcher cannot enforce that condition, so this adapter refuses teardown
  without calling its unguarded remove. Detach leaves the host running. SSH
  adoption and lost-create lookup also refuse or stay unknown until a provider
  supplies read-only observation. Client exit and tunnel loss remain
  observational.

`ssh_native::NativeSsh` is the Linux process provider. Install the standalone
`env-host` helper on the SSH account and supply a private `RemoteConfig` with
absolute `root`, `executable`, and `runtime` paths, the SSH `account`, fixed
`arguments`, and the installed executable's `executable_digest`. The arguments
must run the host in the foreground and select the configured runtime record.
The helper starts that installed executable; it does not download a release.

Run `env-host ssh-profile --config PRIVATE_CONFIG` on the account to get its
configuration digest. The client's `ClientConfig` contains `destination`,
absolute `helper` and `configuration` paths, and that `configuration_digest`.
Use `--ssh-config CLIENT_CONFIG` in place of `--provider-root` for allocator
commands. `ssh-observe --ssh-config CLIENT_CONFIG` observes an existing host
without creating a lease or starting a process. Its exact ownership bytes can
be supplied in an adopt request; adopted hosts are only detached.

The remote book saves the create key before launch and the child's boot/start
identity before exec. Lost-create lookup is read-only and never launches.
Destroy holds the book lock, validates the full ownership generation, and
signals a Linux process descriptor, so PID reuse cannot stop a replacement.
An uncertain stop is only observed afterward. The private book keeps its
terminal entries; it never removes another launcher's runtime record. Kernel
support for process descriptors is required before create. Client exit and
tunnel loss leave the process running.

`ssh-profile` and `ssh-observe` return the native executable component and
its ArtifactRef. Supply those exact descriptor bytes as the component, rather
than uploading a large installed binary. Materialization checks the actual
running executable against that descriptor's binary and configuration pins;
arbitrary components refuse.
Retained output files belong in the provider root's private `outputs`
directory: at most 64 regular files and 128 KiB total. Unknown costs keep their
holds. This profile does not enable arbitrary bootstrap or grant host access.

`transport` publishes the allocator's exact native CAP, lock, context, and
requirements. `invoke` signs a management CJ request using the caller's key;
`serve` handles it through the existing owner/participant checks. The endpoint
uses one durable CJ identity book for all leases and retains the complete RUN
chain. Exact retries return the original signed response, including after
restart. `recover` uses the saved domain result or returns an explicit unknown
and fences a late original handler. Recovery never starts another provider
operation. A completed management call can report unknown resource effects;
it does not establish that allocation or cleanup succeeded. The endpoint
supports a 1 MiB response bound; unsupported time bounds and CJ cancellation
refuse. Lease release and participant fencing remain separate admitted controls.

Attachments require an operator-mounted live `ClaimAuthority`; the default
host refuses them. Mount the actual coordinator with `--coordinator-state` and
`--coordinator-key-file`. `coordination-host prepare-environment` signs the
exact ENV-bound execute for its named worker. The claim must be current, its
controller authorized, and its frame, resources, worker, declaration, and full
execute bytes unchanged. The execute deadline must fit within the current
lease, so waiting for intake cannot extend its authority. The native
reservation is a zero-spend child of the
actual claim reservation in the same currency.

A session can use a lease created before its own invocation claim. The resource
and invocation reservations may have independent parents when both hold exactly
zero USD. The allocator checks the resource hold, and the actual COORD authority
checks and charges the invocation's own allowance. Nonzero holds still require
the existing lease ancestry. This supports pinning an existing lease and
materialization in the session input before constructing its CJ request; it
does not change provider cost holds or grant spending authority.

`env-host run` uses that execute and the explicit worker state/key. The worker's
ENV decoder runs only on this path; generic worker intake refuses the binding.
The allocator keeps its active attachment fence through actual COORD input
disclosure, and COORD checks current authority again at that boundary. Drain,
revocation, expiry, or token replacement prevents new intake. Exact saved worker
replies remain readable without another execution. Combined AUTO/ENV targets
remain unsupported; they require a separately wired adapter.

`bootstrap-binding` takes the actual worker claim, the owner's signed COORD
auxiliary admission, and the bootstrap request ID. It returns pins for
`environment/bootstrap-manifest`, a fixed Rust initializer that verifies the
placed components and writes an initialization manifest. Supply the owner's
separate signed bootstrap CJ event in `MaterializeSpec.bootstrap`. The plan
requires one call, at most 30 seconds, and 1 MiB each for input and output,
using `native-ledger`, `native-response-deadline`, and `native-bytes` enforcement.
The actual COORD container pays the call before initialization, under the same
current-claim fence. Readiness requires the successful terminal RUN result.
Recovery reads the original provider receipt; it never repeats initialization,
component placement, or the parent charge. Other bootstrap targets refuse.

The request must supply or already own every referenced artifact. A cached
hash from another owner grants no access. Attachments also require the exact
frame of the live COORD claim.

`gce::GceEnvironments` implements sponsored, finite VM placement over the
public `oa-boat` Compute owner. Its operator configuration pins the project,
zone, machine, exact image ID, private subnet, disk size, lifetime (30–3,600
seconds), funding ledger/workspace, and fixed operating hold. The workspace
must already exist and contain funds. The provider creates no credit. Every
allocation reserves that real ledger allowance before its one downstream
insert; all children and hosts using that funding authority share its holds.
The caller's service fee is zero. Unknown provider costs retain the operating
hold after cleanup; this is not a hard GCP price guarantee.

Use `--gce-config CONFIG` instead of `--provider-root`. Credentials default to
the VM metadata service; `--gce-credentials gcloud` explicitly selects the
operator's gcloud login. `gce-profile` returns the supported image descriptor,
`gce-observe --instance NAME` observes an explicitly allowed external VM, and
`gce-funding` reads the provider's funding statement. These inspection commands
need the configuration, without an allocator key or state argument.

Lost-create lookup uses the original request UUID and exact VM incarnation.
The instance's actual boot disk image ID and deletion policy must match before
materialization. The original absolute deletion time bounds resource lifetime.
Cleanup retains `placement.json`, then waits for that exact incarnation's
successful automatic deletion operation; it never deletes a reusable name.
Missing VM or operation history remains unknown. Adoption only detaches.
This image profile exposes no guest command, file transfer, or arbitrary
bootstrap. Those roles require a separately supported adapter.

Worker intake rechecks the provider's actual current resource generation
before input disclosure. A changed local marker, stopped SSH process, replaced
VM, or expired native VM lifetime cannot reuse a saved materialization.

All fixtures
use throwaway keys, local directories, and fake allowance units; nothing here
moves money.

The store retains at most 64 leases, plus the shared artifact limits from
`workspace-host`. Limits stop new admissions; they never discard history.
