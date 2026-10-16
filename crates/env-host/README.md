# Environment allocator

`env-host` implements the NIP-ENV allocator, materializer, participant, and
worker-gate roles in Rust. It signs leases, materializations, states,
attachment grants, cleanup receipts, accounting, and operation results. A lease
is not a wallet authorization, a commercial order, or a grant of workspace or
credential access.

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
  generation. Bootstrap operations are not supported and refuse.
- **One slot, bound before dispatch.** An attachment needs the current revision
  and materialization, the primary executor as worker, a controller with a
  current generation, a caller-signed execute declaration whose plaintext
  requires `openagents.environment-binding.v1`, the exact input digest, retained
  claim bytes, and a reservation that does not exceed the lease. Exact retries
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
  stops only the recorded process; detach leaves the host running. The launcher
  has no side-effect-free probe, so SSH adoption and lost-create lookup refuse
  or stay unknown. Client exit and tunnel loss remain observational.

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

Cloud-VM placement, bootstrap operations, and COORD claim validation beyond
exact retained bytes remain in #11736. All fixtures use throwaway keys, local directories, and fake allowance
units; nothing here moves money.

The store retains at most 64 leases, plus the shared artifact limits from
`workspace-host`. Limits stop new admissions; they never discard history.
