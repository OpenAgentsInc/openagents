# SDK publication and integration policy

The [1.0.0 release brief](../audits/2026-10-16-coder-1.0-big-bang.md)
supersedes the binary freeze. This document completes the preparation scopes
of #11819–#11822. It specifies publication and activation requirements; it does
not announce a host package, outside provider program, another language SDK,
or automatic payments. The remaining owner decisions are in
[`NEEDS_OWNER.md`](../../NEEDS_OWNER.md#sdk-publication-and-commercial-activation).

## Host boundary (#11819)

Keep `coder-sdk` internal until a separately reviewed host facade has two real
non-Coder consumers. A public Rust item is not automatically part of that
facade. Account clients use `openagents-sdk`; provider implementations use
`computer-driver` and `computer-conformance`. Neither requires a harness or
access to another process's credentials.

| Concern | Intended host facade | Implementation detail or separate authority |
| --- | --- | --- |
| Session | Typed start, observe, continue, stop, and capability reads; stable session and native-session identities | Adapter transport, child handles, sockets, native wire messages, process environment |
| Hooks and tools | Register typed hooks, receive approvals, inspect supported enforcement tiers | `coder_sdk::hooks`, command-hook runner, MCP adapters, floor implementation; plugins cannot weaken a floor |
| Workspace | An explicitly granted checkout and bounded filesystem rights | Checkout creation, Git locks, protected paths, provider resources; a prompt supplies no grant |
| Cancellation | Request cancellation and observe confirmed termination or uncertainty | Supervisor and process groups; stream loss does not prove cancellation |
| Resume and transfer | Explicit native resume or linked continuation with capabilities and lineage | Engine session files, import/export encoding, context construction; unsupported native resume must refuse |
| Trace | Bounded normalized events and explicit missing output | ATIF recording, hook receipts, native logs, secret removal; traces confer no execution rights |
| Credentials | Caller-owned account session or an explicitly scoped host grant | Provider logins stay with their owner; account sign-in does not expose a local engine login |

This inventory is based on the [host design](../coder/design/coder-sdk.md),
the [task owner](../coder/runtime/task-owner.md), and the current
`coder_sdk::{session,hooks,admission,checkout_guard,floors}` modules. Their
existing APIs remain internal. The single harness implementation selected by
#11894 must own the engine transport; a facade must call it rather than add
another driver. Live chat state uses `coder-sync` rooms and NIP-HOST grants.

The desktop work in #11709 is one candidate consumer record, not evidence of
two independently qualified products. The dependency audit also found
`briefed-agent`, `session-host`, and internal Coder programs. A dependency or
fixture alone does not qualify a second external consumer. Select a real
second product, then record its useful scenario and its exact source/build
against the same facade. Re-audit desktop calls after harness unification:
typed owner/control calls qualify; direct private adapter access does not.

The portable contract must report support per adapter and operation, including
hook tier (`native`, `command`, `emulated`, or unsupported), resume, steering,
questions, and process cancellation. An emulated observation cannot deny a tool
that already ran. Missing required enforcement, unsupported resume, or uncertain
native acknowledgment must refuse or return an explicit unknown result; never
silently start a fresh session. Test those outcomes for each advertised adapter.

Before publication, record the proposed package name, version, Rust/toolchain
and runtime requirements, supported OS/adapter matrix, credential ownership,
security reporting contact, examples, compatibility policy, and named
maintainer. Keep host/process dependencies behind a separate package or an
explicit opt-in feature; audit `cargo tree` for client-only and provider-only
builds. Both application workspaces must compile their shared domain crates.
Use a prerelease host API until its consumers and support matrix are qualified.
Publication and update-channel changes belong to the designated release owner.

## External provider admission (#11820)

The launch policy admits only explicitly configured first-party deployments.
An unknown, withdrawn, or self-declared contributor is ineligible for `auto`.
Passing technical conformance does not enroll an operator or approve data
custody. Outside providers and Pylon require a separate owner-approved offer
and account opt-in before any customer work reaches them.

The [Computers contract](computers.md) and
[SPACE roadmap](../cloud/space-integration-roadmap.md) remain the technical
references. Core, Suspend, Snapshot, and Fork are independent declarations;
memory-preserving suspend requires its own evidence. The Boat-compatible
out-of-process protocol and SPACE's native HTTP adapter are different
implementations of the Rust driver contract.

An admission record must identify the operator and authenticated control
endpoint, deployment namespace, protocol version, locations, platforms,
qualified capability/artifact revisions, permitted accounts and data classes,
capacity/health expiry, incident contact, and revocation procedure. Reject
redirects, mismatched identities/capabilities, stale grants, and expired health.
Namespace resources and operations by deployment. Account consent names the
offer, operator/location, price ceiling, data/credential access, checkpoint
custody, deletion period, and recovery/transfer rules. Placement models may
rank only the deterministically eligible set.

Revocation stops new placement. Existing computers keep their original owner,
deployment, operation identities, and bounded observe/delete/recovery path.
Never move private state to another operator without fresh consent. Missing
deletion evidence stays unknown and does not release storage/cost holds.
Credentials use account-scoped references, immutable egress profiles, and
renewal/revocation checks; an arbitrary secret name is not authorization.

Before commercial admission, define the sold unit, quoted price/expiry, durable
reservation, usage evidence, duplicate/fraud handling, settlement, refund and
withdrawal rules, attribution, and Bitcoin payout destination/fees. No price or
payout is inferred from elapsed wall time or a model answer. Use existing
[payment rails](../payments/README.md), without introducing another rail.

For each approved operator, open a scoped implementation issue and qualify a
synthetic account/provider: normal lifecycle, account separation, capability
lies, endpoint/redirect substitution, stale or replayed authority, response
loss, service failure, withdrawal, uncertain usage, and uploaded-object
deletion. Record the actual evidence before admitting a customer. This policy
does not activate or claim that provider integration.

## Additional language clients (#11821)

Rust and the verified HTTP/OpenAPI reference are the launch interfaces. There
is no approved additional language, package, or repository at this point. Add
no TypeScript product code here and do not revive `openagents-ai` by inference.
An owner-selected language, real use case/API domains, repository, package
name, maintainer, and publication authority are required before generation.

Pin the generator version, dependencies, input schema commit/hash, and output
format. Keep generated wire types separate from ergonomic waits, streaming,
binary files, account storage, cancellation, and retry policy. A clean repeat
generation must match. Generated defaults cannot enable redirects, automatic
payment, implicit credential files, or retries under new command identities.

The consumer qualification reuses the Rust client's deployed first-Computers
scenario and shared wire fixtures: typed errors and unknown enums, bounded
files/pages, `Retry-After`, exact mutation retry keys, stream interruptions,
late responses after account changes, cancel versus detach, and origin-bound
credentials. Record installation, canonical API base, supported features,
version policy, independent consumer evidence, and verified cleanup. No package
publication or external repository mutation is part of this preparation.

## Optional payment integration (#11822)

The SDK's implemented baseline is non-spending:
`openagents_sdk::PaymentRequired` preserves one bounded, untrusted
`PAYMENT-REQUIRED` header, returns a typed 402, omits its contents from debug
output, and neither follows its URLs nor pays. The existing five synthetic
SDK payment tests qualify that baseline. They do not qualify wallet automation.

Reuse the [existing x402 and wallet contracts](../payments/agent-payments.md)
and [payment recovery](../payments/README.md#paid-plugin-outcomes-and-recovery).
Do not build another settlement or replay store. A Computers payment adapter
must bind the approved sold unit to its original account, request, resource,
offer, amount, recipient, network, invoice, and expiry. A paid allocation does
not authorize unlimited later commands.

An opt-in policy must name a payer/wallet, per-call and durable per-day caps,
fee cap, permitted recipients/networks/resources, quote expiry, and cancellation
policy. Missing policy returns the original 402 with no wallet activity.
Changed terms require renewed authorization. Persist the original logical
request, payment lookup handle, reservation, and execution identity before
effects; use existing settlement/replay rules across response loss and restart.
Expose unpaid, pending, paid/settled, refunded, and execution-unknown outcomes
as supported by the payment owner. A paid-but-unknown result cannot trigger a
second invoice, replacement command, or released quota hold.

Qualification requires synthetic wallet/ledger failures at each payment and
execution boundary: restart, lost response, replayed/stale challenge, substituted
invoice, cross-account proof, redirected origin, changed price, cap exhaustion,
cancel, partial allocation, refund, and unknown execution. Prove no duplicate
payment/reservation/command and no secret in errors or logs. Couple settlement
to #11815's authoritative usage, not guessed duration. Funded checks remain an
owner step after a specific offer and implementation pass. Automatic payment
is unavailable in this release unless that separate approval and evidence
exist; this specification does not claim otherwise.
