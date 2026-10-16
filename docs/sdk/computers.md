# Computers: one public API over supported places work can run

2026-10-16. Status: implemented public lifecycle
([#11801](https://github.com/OpenAgentsInc/openagents/issues/11801)), command
sessions ([#11802](https://github.com/OpenAgentsInc/openagents/issues/11802)),
and bounded workspace files ([#11803](https://github.com/OpenAgentsInc/openagents/issues/11803)).
Placement and other capabilities are tracked separately. The implemented
slice below is distinct from the proposed resource and later operations.
Deployment and live qualification evidence belongs to the implementation issue.

Part of the [OpenAgents SDK](README.md). See the
[delivery plan](delivery-plan.md), [current lifecycle service](../cloud/computers.md),
and [SPACE production roadmap](../cloud/space-integration-roadmap.md).

## Current state

| Kind | Implemented path | Evidence and limits |
| --- | --- | --- |
| Our cloud, whole GCE VM | Boat driver through `oa-computers`; web chats use internal service calls | Earlier live check: about 77 s to first turn and 52 s resume; disk-only stop |
| Enrolled own machine | Existing Coder host/grants and product-specific access | No unified allocation adapter yet; online state and current rights determine availability |
| SPACE microVM | Optional Rust driver and completed GCE spike #11728 | HTTP cold p50 2,045.8 ms to first command; node snapshot p50 319.0 ms to readiness at a different shape; production still Boat |

The SPACE adapter declares Core and Suspend. Public Snapshot/Fork creation and
billable seconds remain unsupported on the pinned provider API. Same-host memory
restore preserved a full 1 GiB process buffer and took 9.293 seconds to answer;
cross-host recovery and a production chat flow remain unproved. See the
[measurement report](../cloud/space.md) for shapes, timing boundaries, p90,
source pin, and cleanup evidence.

A person's own computer is not an isolated allocated sandbox. Its availability
is not guaranteed instant, and a zero OpenAgents compute charge does not mean
hardware, electricity, or any other service cost is zero.

## Implemented public lifecycle

Deployed on 2026-10-16 for accounts with cloud access. The
[verification record](computers-lifecycle-evidence.json) pins both running
images, test results, and the dedicated-account staging workflows. Command
sessions have a [separate verification record](computers-commands-evidence.json).
Workspace files have their own [verification record](computers-files-evidence.json).
Other backends remain separate work.

The account-authenticated lifecycle uses `api.openagents.com/v1`. It accepts
live `sess_` sessions, `oa_sk_` sign-in keys, and account-bound `oak_` keys whose
native permissions admit account actions (`accounts`, including the public
`workspace` scope). Decision-only or unbound keys cannot use this surface.
The server checks native authority on every request; cookies and caller-supplied
internal owner headers do not establish identity. Cloud operations remain gated
to site admins and named agent accounts.

| Call | Implemented result |
| --- | --- |
| `POST /computers` | `202` new computer, `200` existing request; send `Idempotency-Key` or body `idempotency_key` |
| `GET /computers?kind=cloud&limit=20&after=...` | Cloud-only page and `next_cursor`; limit 1–200 |
| `GET /computers` | Existing enrolled-machine rows plus admitted cloud computers, preserving old callers |
| `GET /computers/{id}` | Account-owned cloud computer and desired/observed state |
| `POST /computers/{id}/stop`, `/wake` | `202` intent; poll for the observed result |
| `DELETE /computers/{id}?kind=cloud` | `202` cloud deletion intent, even if an enrolled machine has the same name |
| `DELETE /computers/{name}` | Existing enrolled names detach with `204`; otherwise a cloud ID records deletion intent |
| `GET /computers/capabilities` | Declared profiles, sizes, and the current account computer limit |

For an empty `POST` request, send `Content-Length: 0`. Cloud Run refuses a
lengthless `POST` before it reaches the API. The server's internal transport
sets this header for stop and wake; public SDK transports must do the same.

Create accepts optional `chat`, `size`, `environment`, and `version`. A chat
must belong to the account. Environment and version must be supplied together;
the server resolves the saved version's image only after checking ownership.
A public caller cannot supply a raw provider image or owner. Without an
environment, the service uses its configured default runtime. Organization
workspace selection and placement requirements remain later contract work.
Commands and files use the explicit account and computer checks described below.

Use a key of 1–128 ASCII letters, digits, `_`, `-`, or `.`, not starting with
`.`. When header and body keys are both sent, they must match. A changed
immutable create request returns `409 idempotency_conflict`. Keys remain bound
after deletion, including when the chat gets a replacement computer. A keyed
create retry does not undo a later stop; use wake explicitly. The internal
legacy chat ensure without a key keeps its previous wake behavior.

Admission serializes per account in PostgreSQL and counts computers still
awaiting deletion. The initial limit is 20 unsettled computers per account;
exhaustion returns `429 quota_exhausted`. Creates also have a bounded concurrent
admission limit and a 16 KiB public request limit. These controls do not replace
the later durable usage/billing work. Cloud-only reads report service outages;
the mixed legacy list keeps its previous best-effort cloud append behavior.

The maintained contract and its regeneration checks are described below.

## Published contract and generated Rust bindings

The [verification record](computers-openapi-evidence.json) pins the serving image,
contract hash, generated outputs, checks, and independent production workflow.
The public [OpenAPI reference](https://api.openagents.com/v1/openapi.json)
contains the implemented Computers operations and their public authentication.
The checked-in [Computers subset](openapi.computers.json) contains 18 operations:
14 Computers operations, including the existing named-computer detach
compatibility route, and four device/account operations. It includes only the
components those operations reference. The [operation index](computers-operations.json)
records methods, paths, authority, successful status codes, binary bodies,
response bounds, and retry rules.

The owning Rust handlers and their schema functions are the maintained source.
`app_openapi::computers_document` extracts this subset from the same document
composed into the public reference. Do not edit the JSON exports or generated
Rust by hand. Runtime tests validate actual handler responses against the schema;
example checks cover JSON and binary representations. A deliberate required-field
change must fail validation. These checks supplement request and authorization
tests; a generated schema does not establish permission to perform an operation.

The Rust [wire bindings](../../crates/openagents-sdk/src/generated/computers.rs)
provide typed values and request builders under
`openagents_sdk::generated::computers`. Request plans keep unencoded path
components, query values, headers, and binary bytes separate. The shared client
owns deployment validation, credentials, HTTP, cancellation, and bounded retries.
Its experimental `Client::computers` facade executes these plans and provides
account-bound lifecycle, command output, waits, and binary file helpers; see the
[Rust client guide](../../crates/openagents-sdk/README.md). The runnable
`computers` example qualifies the public Boat workflow with dedicated test
credentials and saves original request identities for recovery and cleanup.
Publication and host/provider SDKs remain separate work.
Generated bindings have no implicit network effects. Their debug output omits
values, and response objects preserve unknown fields. Enum values remain strings
so future values decode; an unknown state never means ready, successful, or safe
to replay. Missing file capability means unsupported.

`202` records accepted work. Poll the same resource for completion. A cancelled
local wait or lost connection does not cancel a remote command. Command output
uses authenticated byte-offset polling; this slice offers no SSE or WebSocket
command stream. File requests and responses use binary bodies. An upload needs
its exact `Content-Length`, including zero, and lowercase `X-Content-SHA256`.
A successful read includes the verified checksum, ETag, and `Cache-Control`.

Keep one request key for each intended create or command execution, including
across application restarts. Changed immutable settings conflict. A file write
with an uncertain result needs a read/hash comparison before a retry. Stop, wake,
delete, and cancel are explicit intents; do not automatically replay an old
intent after another caller has changed the resource.

Regenerate with the pinned Rust toolchain and Python 3.11 or later. The generator
uses only Python's standard library and the pinned `rustfmt`; its version and
input SHA256 appear in the generated artifacts. Keep the existing Cargo target
directory outside the worktree.

```sh
sdk_scratch="$(openagents scratch)"
cargo run -p openagents-web --example app_openapi > "$sdk_scratch/openapi.app.json"
cargo run -p openagents-web --example computers_openapi > "$sdk_scratch/openapi.computers.json"
cp "$sdk_scratch/openapi.app.json" docs/sdk/openapi.app.json
cp "$sdk_scratch/openapi.computers.json" docs/sdk/openapi.computers.json
python3 scripts/api/computers_codegen.py
python3 scripts/api/computers_codegen.py --check
python3 -m unittest discover -s scripts/api -p test_computers_contract.py
cargo test -p openagents-sdk
cargo test -p openagents-web --lib computers_api::
cargo test -p openagents-web --lib app_openapi::
```

The [public check](../../scripts/api/check_computers_public.py) compares the
reachable reference with the subset before loading a credential. Its default
mode reads only the schema. `--live --evidence PATH` runs create, binary upload,
a command-side checksum, stop/wake, identical readback, and deletion using paths
resolved from published operation IDs. It requires the approved dedicated test
account, uses `OPENAGENTS_TEST_TOKEN` or the private test-account file, and records
cleanup without credentials or content. It never revokes a supplied credential.
The older API addresses, enrolled-machine sync, and Coder 1.0.0 keep their
existing behavior. SDK publication and other language clients remain separate.

## Command sessions

Deployed on 2026-10-16 under the same account access gate. The
[verification record](computers-commands-evidence.json) pins the backend and
website images, targeted tests, and real Boat checks.

Commands use the existing turn queue and computer lifecycle. Separate internal
queue states prevent older workers from executing public commands as legacy
foreground turns during a rolling deployment. Create configures
output capture before execution; attaching only reads captured output. You can
attach after a short command exits without losing its initial output.

| Call | Result |
| --- | --- |
| `POST /computers/{id}/commands` | `202` queued command; `200` for the same request key and settings |
| `GET /computers/{id}/commands/{command}` | State, observed exit code, timing, output sizes, bounds, and expiry |
| `GET /computers/{id}/commands/{command}/output` | Bounded byte chunks, cursors, gaps, and terminal status |
| `POST /computers/{id}/commands/{command}/cancel` | `202` cancellation intent or the already observed terminal result |

Send a fresh `Idempotency-Key` for each intended execution. Retain that key across
network failures and application restarts. A changed command, working directory,
timeout, or environment with the same key returns `409 idempotency_conflict`.
The key remains bound after completion and computer deletion. Existing internal
turn reads and stops also recognize these IDs, preserving task references and
the legacy text result shape. Use the public output endpoint for exact bytes. Retrying creation
does not start another execution or undo cancellation. There are at most 16
unfinished turns per computer, including legacy chat turns; excess requests get
`429 quota_exhausted`.

```http
POST /v1/computers/cmp_0123456789abcdef0123/commands
Authorization: Bearer YOUR_ACCOUNT_CREDENTIAL
Idempotency-Key: build-2026-10-16-001
Content-Type: application/json

{"command":"printf 'hello'; printf 'notice' >&2","cwd":".","timeout_seconds":600}
```

The complete JSON request is at most 73,728 bytes.
Command text is at most 65,536 UTF-8 bytes. `cwd` defaults to `.`, relative to
the computer workspace, and must name an existing directory within it. On Boat,
the service uses `workspace` under the guest user's home; it does not require
permission to create a directory under `/`.
Absolute paths, `..` components, and control characters return `400`.
A missing directory or symlink escape fails with exit code 126 before the shell
command runs.
The wrapper creates the workspace root when needed. Timeouts are 1–3,600 seconds
(default 600); invalid values refuse rather than being silently changed.

`env` contains at most 16 **non-secret** values, each at most 4,096 UTF-8 bytes.
Allowed names are `LANG`, `LC_ALL`, `TZ`, `CI`, `TERM`, and `OA_PARAM_*`.
Environment values, command text, working directory, timeout, and request digest
are durable request metadata. Do not put credentials in these fields. Credential
values and credential-reference fields are refused by this initial public
operation; scoped references arrive in #11813. Existing internal chat turns keep
their in-memory secret delivery.

Output attachment uses authenticated cursor polling, including optional long
polling. Send `stdout` and `stderr` byte offsets (initially zero), `max_bytes`
(default and maximum 65,536 **per stream**), and `wait_ms` (0–10,000). Values of
zero for `max_bytes` select the default. Decode each base64 stream, append it,
and use the returned `next_cursor` without adjusting it for UTF-8 characters.
Continue until `terminal` is true **and** `has_more` is false. A terminal command
can still have several unread chunks. Disconnecting or abandoning a poll never
cancels the command. Repeating a cursor intentionally repeats the same bytes.
A cursor ahead of captured output returns `400` rather than silently skipping.

Each stream keeps its first 256 KiB. Both guest capture and PostgreSQL storage
are bounded even when a client never reads. Extra guest output is drained and
discarded; `gaps.stdout` or `gaps.stderr` and the corresponding command truncation
flag report that loss. These flags do not mean the returned prefix is missing
bytes. Output expires seven days after creation: reads then return `410 expired`,
and a periodic cleanup removes payloads within about a minute while the service
is running. Request identity and terminal metadata remain for safe retries. Expiry removes
the service copy; it does not erase files inside the computer. Those follow the
computer's storage and deletion policy.
An `unknown` command has no proven final output or exit, even if its truncation
flags are false.

States are `queued`, `dispatching`, `running`, `succeeded`, `failed`, `cancelled`,
`timed_out`, and `unknown`. Dispatch ownership is committed before contacting the
provider. After an uncertain reply or service restart, the worker observes the
same command ID and digest; it never runs the shell again. A start that cannot be
proved becomes `unknown`, including a crash after recording dispatch but before
sending it. This can leave an unexecuted command unknown; it avoids unsafe replay.
The worker then checkpoints the computer through the existing lifecycle.

Cancellation records intent first, signals the command's Linux process group,
and waits for its retained exit record. A signal acknowledgement alone cannot
produce `cancelled`. A queued cancellation prevents dispatch. If an exit is
recorded before cancellation, that result stays final; if cancellation arrives
before final observation, the observed exit can be classified as `cancelled`,
including exit code zero. A command timeout terminates the group with a short
kill grace. An explicit shell exit code 124 is still an ordinary failed command,
not proof of timeout. Stopping or deleting a computer also cancels its queued
commands and asks its active command to terminate before lifecycle teardown.

Every create, read, reconnect, and cancellation uses current account authority.
There are no attach tokens or provider URLs in this contract. Do not put a bearer
in a query string; unknown query fields refuse. An `Origin` header, if present,
must match this deployment's website origin. Provider references, service tokens,
raw provider errors, and internal owner headers are not part of public responses.
Before reverting the lifecycle service to an image from before #11802, stop
accepting new commands and drain active sessions. Older images skip public
command rows; they cannot provide this execution or observation contract.

These commands are non-interactive: no stdin, terminal, preview, daemon lifetime,
or permissions beyond the current cloud-account gate are implied.

## Workspace file transfers

The protected file API uses the same account authority as commands. It operates
under the guest user's `workspace` directory, which is also the command working
directory. It does not expose a provider URL, the worker's filesystem, or shared
account artifact storage. Older `/v1/files` and driver file calls keep their
existing behavior.

| Call | Result |
| --- | --- |
| `PUT /computers/{id}/files?path=input.bin` | Check and atomically publish a binary upload; return `size` and lowercase `sha256` |
| `GET /computers/{id}/files?path=input.bin` | Return complete binary bytes with `X-Content-SHA256` and an ETag containing the same hash |
| `GET /computers/capabilities` | `files` gives the transfer bounds, or is `null` when the driver does not offer protected transfers |

For an upload, send `Content-Type: application/octet-stream`, `Content-Length`,
and `X-Content-SHA256` with the SHA-256 of the complete body. An empty file is a
valid upload. The service checks the length and hash before writing; a partial
or mismatched body returns `422` and leaves the previous file intact. New and replaced
files use mode `0600`. An identical create-only retry keeps the existing file
and its permissions.
Parents must already exist; create directories through a command. The first
upload can create the fixed workspace directory itself.

By default, uploads are create-only. Repeating the same path and bytes succeeds;
a different existing file returns `412 file_exists`. Add `overwrite=true` to
atomically replace a regular file. Concurrent overwrites use the last completed
publication. There is no content compare-and-swap or file idempotency key.
Commands can change a file after a transfer completes. A lost upload response
can mean the write completed: read and compare the hash before retrying. A
deadline or uncertain backend write returns `503 write_uncertain`, never a
successful partial file.

The public upper bound is 4 MiB per file, with two transfers per service instance.
Each path is at most 1,024 UTF-8 bytes, with components at most 255 bytes. The
upload body and driver execution each have a 60-second bound. Transfer slots are
acquired before body buffering; excess work returns `429` with `Retry-After: 1`.
An oversized upload returns `413`. Its slot stays occupied while the remaining
body is discarded in chunks, for at most 60 seconds; discarded bytes never reach
the computer. Downloads are fully checked and bounded before a success response
starts. Both directions use `Cache-Control: no-store`. These limits are independent of the
older Boat and SPACE driver file limits.

Paths are relative, with `/` separators. Absolute paths, empty or dot components,
parent traversal, controls, backslashes, colons, percent signs, invalid UTF-8,
and the Unicode replacement character are refused. Decode query values once;
double-encoded traversal remains invalid. Symlinks, mount crossings, sockets,
FIFOs, directories, and devices are refused. Unicode names and spaces are allowed.

The Rust guest helper uses `openat2` beneath an open workspace directory and
[Landlock filesystem restrictions](https://docs.kernel.org/userspace-api/landlock.html).
The kernel restriction also denies writes through a directory handle after a
command moves that directory outside the workspace. The helper opens a file for
I/O only after confirming that its pinned inode is regular. It takes a Linux
file read lease and checks metadata before and after copying. Files already open
for writing, a broken lease, or changed metadata return `409 file_changed`.
The helper never truncates an existing inode; uploads always create a new inode
and replace a directory entry. This permits the same path restrictions on
Landlock ABI 2; ABI 3 also restricts truncation in the kernel. Kernels without
Landlock ABI 2, filesystems without the required lease support,
and files for which the guest user cannot obtain a lease return `501` rather
than using an unrestricted fallback. Reads use the guest user's permissions.

Transfers require desired state `running` and observed state `awake` or `turn`.
Stopped, starting, or checkpointing computers return `409 computer_not_ready`;
you must wake explicitly and wait. Transfers never allocate or wake a computer.
A missing or deleted computer returns `404`. Deletion or stop during a transfer
can make its result unavailable; it does not authorize a replacement computer.
Each request checks current account access, including after reconnect or
revocation. An `Origin` header must match the deployment. Unknown query fields,
including URL tokens, are refused.

## Public contract and provider contract

```text
developers, our apps, agents
        |
        | account authentication and current grants
        v
api.openagents.com/v1/computers
        |
        v
oa-computers: ownership, intent, operations, Postgres, reconcile
        |
        v
registry: stored provider/deployment binding per computer
        |
        +-- Boat adapter -> Boat HTTP -> GCE
        +-- SPACE adapter -> native SPACE HTTP -> Firecracker
        +-- later: own-machine adapter -> enrolled host authority/transport
```

Public callers ask for requirements, environment, and placement intent. The
provider contract is `computer-driver` with declared capabilities and
`computer-conformance`. Boat-compatible HTTP is the existing out-of-process
provider option; SPACE's native HTTP adapter does not make SPACE a Boat server.

Internal service tokens and `X-OpenAgents-Account` are a trusted server protocol,
not developer inputs. The public boundary derives the owner from authenticated
account/workspace/grant data and ignores or strips spoofed internal headers.
The supported key issuers/scopes must be audited; prefix matching is insufficient.
Keep the named-agent/admin gate until broader admission is explicitly ready.

## Proposed resource and placement

The exact additive schema is settled in [#11801](https://github.com/OpenAgentsInc/openagents/issues/11801) and [#11804](https://github.com/OpenAgentsInc/openagents/issues/11804).
The intended fields are:

| Field | Meaning |
| --- | --- |
| `id` | Stable logical computer identity, independent of provider resource IDs |
| owner/project | Authorized account/workspace and optional project binding |
| environment/version | Immutable verified environment and compatible backend artifact |
| size | Supported named class, with actual vCPU/memory available for inspection |
| needs | Explicit capability requirements, such as memory-preserving suspension or fork |
| placement | `auto`, our cloud, fast sandbox, or an authorized enrolled host; exact wire names remain proposed |
| desired/observed state | Accepted intent and actual lifecycle progress, including failure and uncertainty |
| capabilities | What the bound backend/artifact actually supports, including suspension semantics and limits |
| lifecycle | Idle policy, absolute lifetime, and private-state retention |
| egress | Named policy pinned to an immutable version at creation |
| usage/cost | Attributable observed usage and stated price policy; unknown amounts remain explicit |

Driver and deployment identity belong on the internal record and authorized
diagnostics. They are not public create requirements or a vendor picker. An
internal operator may select an exact backend for qualification while the public
request still uses capability and placement intent.

Placement rules in [#11809](https://github.com/OpenAgentsInc/openagents/issues/11809):

1. Check account policy, grant/enrollment, artifact, capabilities, shape, quota,
   capacity, and data boundaries deterministically.
2. An explicit placement either satisfies the request or refuses before effects.
   Never silently redirect an own-machine or fast-sandbox request to whole-VM
   cloud execution or a contributor's host.
3. If an automatic choice needs judgment among eligible targets, use the typed
   Decisions API. Never use chat keyword matching or let a model override rights.
4. Persist selected driver/deployment/artifact/policy and operation identities.
   Changing defaults cannot move an existing computer or change its trust boundary.
5. Missing or disabled deployment configuration means unresolved availability;
   it does not prove deletion or permit recreation under the old identity.

## First public operations

| Operation | Proposed public behavior | Issue |
| --- | --- | --- |
| Create/list/detail | Account-scoped resource, intent accepted promptly, later readiness observed | [#11801](https://github.com/OpenAgentsInc/openagents/issues/11801) |
| Stop/wake/delete | Change intent; operation completion and observed effect stay distinct | [#11801](https://github.com/OpenAgentsInc/openagents/issues/11801) |
| Commands | Create identified session, attach/read bounded output, observe result, explicitly cancel | [#11802](https://github.com/OpenAgentsInc/openagents/issues/11802) |
| Files | Bounded upload/download in the authorized workspace | [#11803](https://github.com/OpenAgentsInc/openagents/issues/11803) |
| Capabilities | Additive discovery without breaking existing list/response shapes | [#11809](https://github.com/OpenAgentsInc/openagents/issues/11809) |
| Snapshot/Fork | Refuse unless the exact provider operation and ownership semantics are implemented and qualified | Later capability work |

The intended command family is `/v1/computers/{id}/commands`; existing turn
routes and chat task references remain compatible. Attaching must either start
execution or read already retained output. The specification must state which
behavior is implemented and prevent lost early output. Scoped attach authority
never grants lifecycle-management access.

File path and transfer rules cover traversal, absolute paths, symlink escape,
partial writes, overwrite policy, size/deadline bounds, and stopped-state effects.
Do not expose arbitrary host paths or implicitly wake/allocate a computer during
an ordinary file read unless that policy is explicitly part of the contract.

## Retry, cancellation, and state

- Generate one key for a logical create/mutation, retain it across attempts, and
  expose it to applications for recovery after restart. The API scopes it to the
  account and request; conflicting immutable settings refuse.
- Create acceptance is not readiness. A request deadline expiring is not proof
  the accepted operation failed. Inspect the same resource/operation.
- A command has a stable identity before execution. Unknown dispatch is observed
  or attached to, never replayed as a fresh command to obtain a clean answer.
- Stream disconnect/detach is distinct from command cancellation. Explicit stop
  confirms process/children termination or reports uncertainty.
- Retained output has cursors, bounds, expiry, and explicit gaps. Reconnect checks
  current account/grant authority, including expired attach tokens.
- Deletion is final only after the relevant provider and private storage cleanup
  are observed. Detaching an own-machine record has different physical effects.

## Suspension and recovery

Keep disk-only stop and memory-preserving suspension distinct. Boat's existing
Suspend profile remains valid; add an explicit memory-preservation capability
rather than redefining it incompatibly.

Choose suspend-after-turn or a bounded idle window from measured behavior.
Queued turns wait for checkpoint/teardown completion; a provider state alone
cannot prove the prior operation finished. Wake verifies readiness and renews
current grants/credentials before dispatch. Restored RAM may contain stale
sockets, clocks, and services that need recovery.

API-process recovery preserves identities and leases. Worker-loss recovery needs
a verified durable checkpoint and exclusive restore ownership; same-host memory
survival does not prove that. Document recovery time/data loss and refuse claims
that have not been exercised. Lifecycle implementation is [#11812](https://github.com/OpenAgentsInc/openagents/issues/11812).

## Credentials and egress

The current per-turn approach avoids deliberately saving values in our command
metadata; SPACE stdin also avoids its saved `env` field. The process can still
read a supplied key, and applications, logs, files, or memory snapshots may hold
it. Do not describe this as a universal no-secrets-on-disk guarantee. Keep private
checkpoints account-bound and separate from clean reusable base snapshots.

The target public surface uses scoped credential handles, for example:

```json
{
  "credentials": [
    {"name": "github", "reference": "credential-handle-example"}
  ]
}
```

This is a proposal, not an implemented request field. A handle identifies an
account-authorized source/grant and allowed computer/turn/destination/expiry.
It is not permission to resolve an arbitrary Google Secret Manager path.
The broker performs authorization before cloud-secret resolution and at use.

Qualify Google Secret Manager or our broker with SPACE's egress gateway; upstream
AWS support is not proof of GCloud support. Verify real Git/model clients, TLS,
streaming, redirects, subdomains, expiry, and revocation. If a client requires a
real guest key, record its restricted exception and snapshot handling honestly.

Pin named default-deny egress profiles at create. Test DNS bypass, literal IPs,
redirects, private networks, metadata, and IPv6 where enabled, with a reachable
host-side denial control. Credential provider failure must fail closed. This
work is [#11813](https://github.com/OpenAgentsInc/openagents/issues/11813) and precedes general customer access.

## SDK and internal consumers

The first Rust client [#11805](https://github.com/OpenAgentsInc/openagents/issues/11805) supplies typed lifecycle methods, bounded
operation waits, command streaming/reconnect/cancel, file transfer, and stable
retry handles. It does not require the host harness stack or a provider database.
A proposed facade name is a packaging goal, not publication permission.

The web proves the client contract first in [#11806](https://github.com/OpenAgentsInc/openagents/issues/11806). Solver
[#11807](https://github.com/OpenAgentsInc/openagents/issues/11807) and Environment builders/verifiers [#11808](https://github.com/OpenAgentsInc/openagents/issues/11808) migrate
separately, preserving their existing task/build/review owners and legacy cleanup.
Do not tie those migrations to unfinished SPACE support.

Own-machine support [#11817](https://github.com/OpenAgentsInc/openagents/issues/11817) uses current enrolled-host grants for each
operation. Create attaches authorized logical access; delete detaches it without
deleting files, shutting down the machine, uninstalling the host, or revoking
unrelated grants. Unsupported suspend/fork/snapshot needs refuse. Core-only host
support may require API admission changes instead of pretending Suspend works.

New CLI tools and compiled remote launch changes are tracked in [#11823](https://github.com/OpenAgentsInc/openagents/issues/11823) and
[the next-release list](../coder/next-release.md). Compatible servers can improve
frozen Coder 1.0.0 now; new binary behavior cannot.

## Usage, rollout, and deferred capabilities

Durable usage/quota work [#11815](https://github.com/OpenAgentsInc/openagents/issues/11815) separates running guest capacity, suspended
private storage, shared pools/templates, worker idle cost, and network/storage
operations. Missing provider seconds is not zero cost. Preserve uncertain
reservations through restart and settle observed usage once. Account/service
limits precede unbounded allocation; retail pricing/entitlement is separately
approved and uses existing Bitcoin rails.

Deploy SPACE privately with durable state, compatible snapshots, bounded capacity,
monitoring, and rollback in [#11814](https://github.com/OpenAgentsInc/openagents/issues/11814); qualify the actual SDK/web path
in [#11816](https://github.com/OpenAgentsInc/openagents/issues/11816). Keep Boat serving existing/default computers until the named
rollout criteria pass. Disabling new SPACE placement must leave old computers
observable, wakeable, and deletable.

Snapshot/Fork, interactive terminals, previews, scheduled wake, external providers,
and automatic payment each require their own qualified capability and authority.
They are not implied by generic computer creation. Provider admission
[#11820](https://github.com/OpenAgentsInc/openagents/issues/11820) requires trust/consent/payout decisions; SDK auto-payment
[#11822](https://github.com/OpenAgentsInc/openagents/issues/11822) remains opt-in and deferred. These do not gate the first Boat flow.

See [delivery milestones and all implementation issues](delivery-plan.md).
