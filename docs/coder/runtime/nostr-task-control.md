# Scoped Nostr task control

`coder-control` connects a separately authenticated Nostr client to one existing
local task owner. It implements the bounded host bridge tracked in
[#9691](https://github.com/OpenAgentsInc/openagents/issues/9691). Pairing, task
commands, reads, and revocation use the four native CAP operation definitions
installed with the host. CJ carries each invocation; original private `3188`
artifacts retain the signed inputs and outputs.

The bridge cannot submit a new task, start an executor, grant tools, approve a
proposal, call a model, spend money, transfer ownership, or install a program.
The local [task owner](task-owner.md) retains execution authority. A CJ
`completed` result means the control operation answered. Check its CTRL result
to learn whether a correction or cancellation was accepted; neither result
means the coding task passed its checks.

## Implementation and trust

- `crates/nostr/src/control.rs` parses all twelve closed CTRL artifact shapes.
  Parsing establishes shape, not authority.
- `crates/coder-control` implements private durable host state, exact command
  retries, client envelope helpers, finite projections, and a small CLI.
- `crates/nostr-transport` provides bounded authenticated sockets and private
  artifact publication and retrieval. The existing Coder and labor artifact
  paths reuse it.
- Building `coder-control` with `default-features = false` includes the client
  and transport APIs without `coder`, its task owner, or its process execution
  implementation. The CLI requires the default `host` feature.

Local setup explicitly maps an opaque wire task ID to an existing local task ID
and its original intent digest. Setup is a trusted owner action, not a remotely
accepted declaration. Reopening compares the exact stored configuration and
refuses a different authority, scope, generation, task mapping, or operation
pins. Pairing never establishes ownership by itself.

The operation closure pins its native CAP definitions, role SchemaRefs,
single-component locks, context, and requirements. The shared CTRL validator
also checks the complete closed operation shape. A client cannot substitute an
operation, supply another context, or turn text into an execution grant.

`observe`, `steer`, and `cancel` are separate rights. Every request checks the
original client signature, exact declaring event, current grant, scope,
generation, policy, and host time. Cancel-only clients receive a narrow command
receipt without the task's new revision, instructions, result, or transcript.
Only the configured owner can revoke in this profile. A revocation is terminal
and survives restart. Expired or revoked grants cannot fetch retained pages.

## Exact retries and correction semantics

The host atomically retains the signed command, full canonical fingerprint,
admission time, and exact derived local command bytes before applying a local
effect. The local command ID is derived from the grant and command identities.
If the process fails after the local effect, replay uses those same bytes and
receives the original local receipt instead of applying a second correction or
cancellation. A conflicting body under the same identity cannot replace it.

An uncertain effect remains `unknown`. It does not become a fresh attempt or a
successful stop. Completed command results can be retrieved after the command's
short expiry while the grant remains current. An unresolved command does not
acquire fresh execution permission merely because it is retried after expiry.

A steering command records a replacement instruction through the existing
local correction action. The original instruction, signed correction, execution
grant, trace, costs, and results remain retained. Correcting a running task
requests cancellation of its now-superseded context; it does not continue that
execution under silently changed instructions. A later execution still needs
a separate local grant. Nonempty `replaces` lists are refused in this initial
bridge because it does not implement remote CTX objective/constraint editing.

## Private instruction delivery

The command pins exact bounded UTF-8 `text/plain` bytes. For relay delivery,
`client::text` creates two private signed artifacts:

1. An external-content envelope declares the exact text digest and length with
   `inline: null`.
2. A bounded JSON carrier contains that reference and the UTF-8 text. The text
   reference names the carrier as an exact event source hint.

The host checks command admission before resolving the text. It verifies both
original authors, recipients, event identities, the closed carrier shape, and
the raw text digest. It retains both original encrypted declarations before
dispatch. The source hint supplies bytes, never authority. This profile follows
no URL, arbitrary file path, or unnamed latest artifact. Local library callers
can instead supply already retained exact text bytes explicitly.

## Reads and disclosure

The installed disclosure policy explicitly permits task instructions and ATIF
content. Grant `observe` only to a principal intended to receive that content.
The state display omits local workspace paths and execution grants. ATIF may
itself contain paths and task content; this profile is not a general secret
redaction service.

Reads freeze a finite cut rather than following a growing transcript forever.
An opaque cursor is retained with its exact grant, scope, policy, view, cut, and
offset. Reconnecting or reopening the host cannot silently switch that cursor
to newer evidence. A fresh read can capture a newer cut.

Local task records and ATIF are not original signed RUN history. The bridge
returns signed current-authority projections, with exact content references and
evidence descriptors, and reports `coverage: partial`. Source bytes remain in
the private host store; the authorized page closure contains only the display,
projection, and provenance artifacts. It does not disclose the whole store or
claim complete original RUN evidence.

The initial profile limits a page to 128 items and 32 KiB, including conservative
encrypted artifact and CJ framing allowances. Requests outside supported bounds
are refused. A captured history includes at most 1,024 steps and 2 MiB of source
step bytes. Individual displays above 8 KiB become explicit bounded omissions
with source digests. Missing or damaged local evidence remains unavailable or
partial; it never becomes proof of task success. Costs without authoritative
data remain unknown.

The private store retains at most 128 invitations, 256 CJ jobs, 256 logical
commands, 32 finite snapshots, and 512 text-source events, within its 32 MiB
document bound. It does not silently prune retry identities. Reaching a bound
refuses new work. Operators must preserve the store; deleting it is not a
recovery procedure.

## CLI

Build with `cargo build -p coder-control`. Use the pinned repository toolchain.
Set `CODER_CONTROL_KEY_FILE` to a private, singly linked `0600` file containing
the current principal's hex secret key. Keys never appear in flags, receipts,
or logs. Input files must also be private regular files. Output creation is
exclusive: an existing file is never silently replaced.

These commands describe operator actions; they are not an unattended deployment
or a request to launch a task:

```text
coder-control init SETUP TASK_DIR TASK_ID OWNER_PUBKEY
coder-control client-config SETUP CLIENT_CONFIG
coder-control invite SETUP HOST_DIR CLIENT_PUBKEY observe,steer,cancel INVITATION
coder-control publish RELAY INVITATION
coder-control text CLIENT_CONFIG INSTRUCTION_FILE TEXT_DELIVERY
coder-control prepare CLIENT_CONFIG ROLE BODY_JSON PACKET [TEXT_DELIVERY]
coder-control call RELAY PACKET RESPONSE
coder-control serve-once SETUP HOST_DIR RELAY
```

`ROLE` is `pair`, `command`, `read`, or `revoke`; `BODY_JSON` is its closed
[CTRL artifact](../../../nips/openagents/NIP-CTRL.md). Share the separate client
configuration through an authenticated channel. It contains public authority
and operation pins, without host paths or execution grants. Verify that
authority before pairing.

`prepare` persists the exact signed packet before publication. Retrying `call`
uses that existing packet; do not prepare a new logical command to recover an
uncertain delivery. A lost result is unknown delivery until the retained result
is recovered. `serve-once` processes one authenticated invocation and exits;
it is not a daemon or a new task owner. It prints readiness only after its relay
subscription reaches EOSE. The client similarly subscribes before publishing
the ephemeral CJ request.

## Verification and remaining boundaries

The [verification record](../verification/2026-09-26-task-control/README.md)
retains the applicable logs, source hashes, and limits. The focused Rust tests
use synthetic tasks and an actual in-process WebSocket
test relay with NIP-42 authentication and per-principal private-event delivery.
They cover original signature and input substitution refusal, invitation
consumption, rights narrowing, expiry, generation mismatch, revocation after
restart, exact command replay, an injected failure after a local effect, bounded
read refusal, and finite history reopened in a separate child process. The
parent verifies that child's signed page against the original retained cut;
this check does not rely on the parent's host object. The socket fixture includes a
bounded synthetic subprocess, authenticated correction and cancellation, process
group clearance, and refusal of a late write. It makes no model calls.

These are code verification results. They do not establish a production relay
deployment, physical mobile compatibility, background wakeups, commercial usage,
or coding performance. No model run, benchmark campaign, or platform rehearsal
is required to use or review this change. The initial fixture proof written
before the operator's stop is retained separately from subsequent code-only
tests. It predates the final carrier and child-process additions and is not a
claim that the final source produced those earlier bytes.

This bridge is not the complete CTX, SESS, ENV, or RUN runtime. It does not
advertise complete `nip-ctrl-v1` host conformance, controller handoff, full signed
RUN replay, remote task creation, remote process start, or automatic approval.
Those limitations must remain visible in any client UI or deployment claim.
