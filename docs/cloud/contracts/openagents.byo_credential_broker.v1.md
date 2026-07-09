# `openagents.byo_credential_broker.v1`

Status: design contract for managed Cloud BYO model credentials

This contract describes how a customer-owned model API key reaches a
customer-scoped isolated session VM without the OpenAgents Cloud control plane
ever custodying that key. The control plane brokers compute, isolation, routing
metadata, and receipts only. The key travels point-to-point from the Pylon
client to the isolated VM, is encrypted in transit, is injected only into the
target exec environment, and is wiped when the session or capability is
released.

## Purpose

BYO credentials let a customer run a Cloud workroom against their own model
provider account while preserving the private Cloud boundary:

- Pylon remains the customer-side origin for the raw model API key.
- The isolated session VM is the only Cloud-managed endpoint that receives the
  raw key.
- The control plane allocates and verifies compute but never stores, logs,
  derives, fingerprints, or replays the key.
- Receipts and projections retain refs, decisions, reasons, policy digests, and
  redacted evidence only.

## Invariant Binding

This contract is a concrete application of `INVARIANTS.md` capability and
secret handling rules:

- Workrooms consume capabilities through brokers or local gateways, not raw
  provider secrets on disk.
- Every BYO credential attachment is scoped, revocable, auditable, and tied to
  a workroom, run, org, and session policy.
- Every credential access emits redacted evidence that can be audited without
  leaking secret material.
- The key enters a bounded workroom only as a short-lived, run-scoped process
  environment variable. It is not persisted to artifacts, callbacks, traces,
  tracked files, D1, normal logs, receipts, state snapshots, or closeout
  manifests.
- Durable event persistence must run the same forbidden-secret marker checks
  used by Codex and gateway paths before any non-secret event is retained.

The `danger_full_access` invariant still applies: BYO credentials are allowed
only for an externally isolated VM or container workroom profile with no wallet
authority, no broad host/cloud credentials, session-scoped provider auth, and
cleanup receipts.

## Actors

| Actor | Role |
| --- | --- |
| Pylon client | Customer-side key origin and policy confirmer. Holds the raw key before transfer. |
| Cloud control plane | Allocates compute, records refs, enforces policy, and receives redacted receipts. Never receives the raw key. |
| Managed node | Starts the isolated session VM and reports non-secret readiness evidence. |
| Isolated session VM | Terminates the credential channel, holds the key in volatile memory, injects it at exec time, and wipes it on release. |
| `oa-workroomd` | VM-local sidecar that enforces local gateway policy, exec injection, receipt emission, and cleanup. |
| Model provider | Receives normal API requests from the workroom process using the customer's key. |

## Session Fields

`openagents.byo_credential_session.v1` records compute and policy metadata. It
is refs-only and must never contain raw credential material, encrypted
credential material, credential digests, or provider secret-store refs.

| Field | Purpose |
| --- | --- |
| `workroom_id` | Private workroom receiving the BYO credential capability. |
| `run_ref` | Bounded run, task, or session scope. |
| `organization_ref` / `project_ref` / `user_ref` | Non-secret customer scope selected by the product authority. |
| `credential_session_ref` | Random non-secret rendezvous ref for this BYO transfer. Not derived from the key. |
| `customer_credential_ref` | Optional customer-visible label or ref. Must be non-secret and non-derived. |
| `provider_kind` | Provider family such as `openai`, `anthropic`, `google`, `openrouter`, or `custom_http`. |
| `allowed_env` | Exact environment variable names that may receive the key, such as `OPENAI_API_KEY`. Names only, never values. |
| `isolation_profile_digest` | Digest of the VM/container workroom profile that permits BYO credential injection. |
| `gateway_policy_ref` | Local gateway policy ref that authorizes model capability access. |
| `vm_endpoint_ref` | Non-secret direct-channel endpoint ref for the isolated VM credential receiver. |
| `vm_attestation_ref` | Optional attestation or boot evidence ref for the session VM. |
| `vm_credential_pubkey_ref` | Digest/ref for the VM's ephemeral credential-channel public key. |
| `receipt_sink_ref` | Ref where redacted credential receipts are delivered. |
| `issued_at_ms` / `expires_at_ms` | Session TTL. Must not outlive the workroom run or VM lease. |

## Transfer Contract

The raw model API key is transferred only over an endpoint-encrypted channel
whose cryptographic endpoints are the Pylon client and the isolated session VM.

1. The control plane allocates an isolated session VM and returns only refs:
   `credential_session_ref`, `vm_endpoint_ref`, `vm_attestation_ref`,
   `vm_credential_pubkey_ref`, `allowed_env`, TTL, and policy refs.
2. The VM generates the credential-channel private key inside the isolated
   session. The private key never leaves the VM.
3. Pylon verifies the returned refs and opens the credential channel directly
   to the VM receiver. TLS is sufficient only when it terminates inside the VM
   receiver. TLS terminated at a Cloud load balancer, proxy, or control-plane
   service does not satisfy this contract for credential bytes.
4. Pylon sends the key, provider kind, declared env target, and run scope
   through that encrypted channel.
5. If a relay is required for network traversal, it must be a non-durable byte
   relay that cannot decrypt the stream and does not persist ciphertext. The
   control plane must not terminate the credential channel.
6. The VM receiver stores the key only in a volatile credential slot referenced
   by `credential_slot_ref`, a random non-secret handle not derived from the
   key.

The control plane must reject any request or callback that attempts to include
the key, key ciphertext, key hash, key prefix/suffix, provider bearer token,
`Authorization` header value, `.env` content, or other secret material.

## VM-Local Handling

The isolated VM may retain the key only in volatile session memory. It must not
write the key, a reversible encrypted form, or a digest/fingerprint of the key
to disk.

VM-local state files may contain only refs and status:

```text
byo-credential-state.json
byo-credential-receipts.jsonl
gateway-access.jsonl
resource-usage-receipts.jsonl
```

Allowed state fields include `workroom_id`, `run_ref`,
`credential_session_ref`, `credential_slot_ref`, `provider_kind`,
`allowed_env`, receipt digests, policy refs, timestamps, decisions, and
redacted reasons. Disallowed state fields include raw keys, encrypted keys,
secret-store refs for this customer key, key digests, authorization headers,
HTTP request headers containing credentials, and process environment values.

## Exec-Time Injection

`oa-workroomd` injects the key only when starting the declared process for the
declared run.

- The key is placed into the exact env var name listed in `allowed_env`.
- The key is not written to a shell rc file, `.env` file, Codex auth cache,
  command-line argument, config file, artifact, trace, callback payload, or
  normal log.
- The env var is scoped to the child process tree for that run and is removed
  from the parent sidecar environment immediately after spawn.
- The runner must redact environment snapshots before logging and must refuse
  durable persistence for any event payload that matches forbidden secret
  markers.
- The model gateway may report non-secret capability availability, provider
  kind, and credential refs, but `/openagents/model` and other local gateways
  must never return the key or env value.

The intended capability name is:

```text
model.byo_credential.exec_env
```

It may be listed under the `model` link-local gateway allow-list. Gateway access
continues to emit the normal redacted `gateway-access.jsonl` event with gateway,
capability, decision, and reason only.

## Release And Wipe

The VM must release the credential slot and attempt best-effort memory wiping
on every terminal or revocation path:

- normal run completion;
- workroom closeout;
- user revocation from Pylon;
- capability revocation by policy;
- timeout;
- cancellation;
- process failure;
- VM quarantine;
- VM teardown.

Release kills or detaches credential-bearing child processes, clears the
volatile slot, removes the env var from any sidecar-owned process state, and
emits a redacted wipe receipt. A workroom that accepted a BYO credential cannot
claim successful credential closeout unless a wipe receipt exists or the VM
teardown receipt proves the whole isolated session was destroyed.

## Receipt Fields

`openagents.byo_credential_broker_receipt.v1` records redacted evidence for the
credential lifecycle.

| Field | Purpose |
| --- | --- |
| `receipt_kind` | `prepared`, `channel_established`, `credential_received`, `env_injected`, `access_checked`, `released`, `wiped`, or `refused`. |
| `workroom_id` / `run_ref` | Workroom and bounded run scope. |
| `credential_session_ref` | Non-secret session ref. |
| `credential_slot_ref` | Non-secret VM-local volatile slot ref. Omitted before receipt. |
| `customer_credential_ref` | Optional non-secret customer label or ref. |
| `provider_kind` | Provider family, not account secret. |
| `env_names` | Env var names that were allowed or used, never values. |
| `gateway_policy_ref` | Policy ref used for local access decisions. |
| `decision` | `accepted`, `denied`, `released`, `wiped`, or `failed`. |
| `reason` | Redacted bounded reason string. |
| `evidence_digest` | Digest over non-secret receipt evidence only. Not a digest of the key or ciphertext. |
| `emitted_at_ms` | Receipt emission time. |
| `receipt_digest` | Local `sha256:` digest over the redacted receipt material. |

Receipts must not contain:

- raw model API keys;
- encrypted credential payloads;
- key hashes, prefixes, suffixes, or fingerprints;
- authorization headers or bearer tokens;
- process environment values;
- model SDK config files containing the key;
- stderr/stdout snippets that include the key;
- provider account secrets or secret-store refs for this BYO key.

## Retained Projections

Any durable projection retained by Cloud, Forge, Nexus, Autopilot, Vortex,
Probe, or receipt sinks is refs-only.

Allowed retained projection fields:

```text
workroom_id
run_ref
credential_session_ref
customer_credential_ref
provider_kind
allowed_env names
gateway_policy_ref
isolation_profile_digest
receipt_digest refs
release_status
wipe_status
```

Forbidden retained projection fields:

```text
raw key
encrypted key
key digest or fingerprint
key prefix or suffix
Authorization header value
process environment value
.env content
provider SDK config containing the key
secret-store ref for this BYO key
```

The key must never appear in any artifact, log, receipt, trace, callback,
fixture, tracked file, closeout manifest, resource usage receipt, screenshot,
or durable event payload.

## Validation Rules

- `expires_at_ms` must be positive and no later than the run or VM lease TTL.
- `allowed_env` must be an explicit allow-list of env var names. Wildcards are
  invalid.
- `credential_session_ref`, `credential_slot_ref`, and
  `customer_credential_ref` must be non-secret, bounded strings and must not be
  derived from the key.
- The isolation profile must allow BYO credential injection and must forbid
  wallet authority, broad host credentials, and broad cloud credentials.
- The credential channel must be endpoint-encrypted between Pylon and the
  isolated VM receiver.
- The control plane must not terminate the credential channel, store
  ciphertext, store plaintext, or persist a key-derived digest.
- Every receipt, reason, callback, artifact candidate, event payload, and
  retained projection must pass the raw secret, token, wallet, private key, and
  private-topology marker filter before persistence.
- A release or VM-teardown receipt is required before successful credential
  closeout.

## Non-Goals

- No Cloud-managed secret manager custody for customer BYO model keys.
- No reusable stored provider credential.
- No encrypted-at-rest key escrow in the control plane.
- No model API key in public Pylon operator docs, fixtures, logs, receipts, or
  artifacts.
- No wallet authority or settlement credential sharing.
- No generic secret tunnel for arbitrary files or credentials.
- No replacement for provider-account grants such as
  `openagents.codex_auth_grant.v1`; this contract is only for customer-origin
  model API keys sent directly to an isolated session VM.
