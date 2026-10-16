# The OpenAgents SDK

2026-10-15. Status: delivery direction with tracked implementation work.
The first desktop SDK delivery is authentication and reading existing account
chats: sign in, list previous chats, and open their saved history. The public
Computers API remains a separate delivery track. Host
publication, external providers, additional language packages, and automatic
payments have separate activation conditions. This document does not claim
those surfaces are already implemented or published.

The [Home dashboard client](dashboard.md) supplies mobile and desktop widgets,
account layouts, and reusable layout packages (#11912).

Start with the [delivery plan](delivery-plan.md),
[saved chat reads](chat-history.md),
[Computers contract](computers.md), and umbrella
[#11800](https://github.com/OpenAgentsInc/openagents/issues/11800).

The [custom-agent plan](../custom-agents.md) proposes SDK-defined agents,
assignment through the existing session flow, a creation screen in the
OpenAgents mobile app at `apps/openagents/`, and a protocol mapping across
the existing NIPs. The [definition contract](agents.md) describes the first
implementation slice; assignment and mobile delivery remain tracked work.

[Coder Desktop](../coder-desktop/README.md) is the planned first external reference
client: a separately maintained Zeron fork using the Rust SDK first, with
TypeScript where needed in its own repository. Its plan defines the application
integration; the shared API/client delivery and web validation remain here.

## One family, three boundaries

OpenAgents SDK names a family of small components:

| Part | Purpose | First delivery |
| --- | --- | --- |
| Client | Call account-authorized models, decisions, chats, computers, projects, and payment resources | App authentication and existing chat history; Computers follows on its own track |
| Host | Run and control agent sessions with hooks; today's `coder-sdk` | Audit the public boundary and actual consumers; publication stays deferred |
| Provider | Supply a computer or other service through a qualified contract | Existing driver/conformance documentation; external admission stays separate |

A computer is a resource a client requests, a place a host session executes,
and something a provider serves. Keep those ownership and authorization
boundaries explicit. A driver that passes conformance does not thereby gain
permission to run a customer's private repository.

The public contract is HTTP and OpenAPI at
`https://api.openagents.com/v1`. Rust is the first SDK language. Existing
OpenAI-compatible model interfaces remain available through compatible model
clients; a new SDK does not need to duplicate them before computers are useful.

## What exists

| Area | Existing pieces | Remaining work |
| --- | --- | --- |
| Clients | `openagents-sdk` authentication, account/workspace discovery, saved-history readers, and the qualified Rust Computers client ([evidence](computers-client-evidence.json), #11805), and web adoption ([evidence](computers-web-evidence.json), #11806); decisions in `jev` | Broader schemas and supported SPACE operation |
| Host | `coder-sdk`, Claude/Codex/ACP transports, hooks, policy and session ownership | Explicit third-party boundary, supported capabilities, and publication evidence |
| Providers | `computer-driver`, Core/Suspend/Snapshot/Fork declarations, `computer-conformance`, Boat wire schema | Per-computer backend registry and separate external-provider qualification |
| Computers | `oa-computers` intent/reconciliation, web chat integration, deployed gated account-authenticated lifecycle ([evidence](computers-lifecycle-evidence.json), #11801), durable command sessions ([evidence](computers-commands-evidence.json), #11802), bounded workspace files ([evidence](computers-files-evidence.json), #11803), and a maintained Computers contract with generated Rust bindings ([evidence](computers-openapi-evidence.json), #11804) | Solver/Environment adoption and supported SPACE operation |
| SPACE | Completed opt-in Rust adapter and GCE spike in #11728 | Actual HTTP snapshot boots, full environment artifacts, credentials, operations, usage, and qualification |
| Discovery | API docs, `llms.txt`, docs MCP, agent cards | Incrementally complete verified OpenAPI and examples |

The [SPACE report](../cloud/space.md) and
[evidence](../cloud/evidence/2026-10-space-spike.json) replace the earlier
planned-spike description. HTTP cold-to-first-command p50 was 2,045.8 ms
on 2 vCPU/1 GiB. Native node snapshot-to-ready p50 was 319.0 ms on
2 vCPU/2 GiB. The pinned Kind HTTP path still cold-boots, and the two
measurements have different boundaries. Production remains on Boat.

## First desktop delivery: sign in and read existing chats

Owner priority, 2026-10-15: authenticate through the SDK, show previous chats
in Coder Desktop's sidebar, and open any saved chat. Track this milestone in
[#11824](https://github.com/OpenAgentsInc/openagents/issues/11824), including
work implemented in the separate desktop fork. It precedes the desktop's
computer workflow; it does not replace the independent Boat/web milestone.

Reuse [OpenAgents authentication](../auth/README.md): website GitHub sign-in
approves an app's device grant, which issues a revocable session on the same
account. The SDK uses `https://api.openagents.com/v1` for app API calls and
`https://openagents.com/device` for browser approval. Keep deployment pairs
explicit for staging/local use. No desktop GitHub secret, Zeron WorkOS account,
new auth provider, or proposed `/auth/app` flow is needed.

The implemented [saved history API](chat-history.md) uses `/v1/history*` for
bounded summary pages, complete revision-checked messages, saved files and
captures, and shared-project discovery. The typed Rust SDK and native sidebar
and reader are implemented. The older `/v1/threads/{id}` detail remains a preview
of the newest 200 messages; it does not replace the complete history reader.
The [thread API contract](threads.md) covers mutation, reply admission, sharing,
and computer sync. The [cloud chat client](chats.md) sends, streams, and stops
turns in that same cloud chat from the desktop and phone composers.
The [standalone example](../../examples/sdk-history/README.md) and
[delivery evidence](../coder-desktop/delivery-evidence.json) record the external
Git build, deployed API checks, native reads, cleanup, and remaining owner checks.

Include web chats and terminal/phone chats already synced to the account,
archived-chat access, and authorized shared-project history. Sign-in does not
upload unsynced local chats. Opening saved history does not start execution.
Verify account isolation, paging beyond 200 chats/messages, restart, expiry,
revocation, account switching, and unavailable services. SDK publication,
Computers, SPACE, host execution, and new message sending do not block this
milestone. Use a pinned external build and dedicated test accounts.

## Public Computers API and Rust client

Deliver one complete account-scoped workflow on Boat:

1. Authenticate as a dedicated test account through the supported public
   account/key boundary, without a lifecycle service token.
2. Create a computer and observe it becoming ready.
3. Upload a file, create a command, attach/read its bounded output, and
   observe the result.
4. Stop and wake the same computer, then read the unchanged file.
5. Delete the computer and verify resource cleanup.
6. Prove a separate account cannot access any resource, file, or command.
7. Exercise the same contract from a real web chat and retain scrubbed evidence.

Public lifecycle/authentication is [#11801](https://github.com/OpenAgentsInc/openagents/issues/11801); command sessions are
[#11802](https://github.com/OpenAgentsInc/openagents/issues/11802); files are [#11803](https://github.com/OpenAgentsInc/openagents/issues/11803); the first specification is
[#11804](https://github.com/OpenAgentsInc/openagents/issues/11804); the Rust client is [#11805](https://github.com/OpenAgentsInc/openagents/issues/11805); web adoption and the
milestone proof are [#11806](https://github.com/OpenAgentsInc/openagents/issues/11806),
qualified in the [web evidence record](computers-web-evidence.json).

Keep the current named-agent/admin gate while billing and broader admission
remain incomplete. SPACE, own-machine support, full all-route schema coverage,
and crates.io publication do not block this milestone.

## Rust packaging and release boundaries

The development client package is [`openagents-sdk`](../../crates/openagents-sdk/README.md),
exported as `openagents_sdk` or an application's `openagents` dependency alias.
Its auth facade reuses the CLI's portable device protocol, with typed sessions,
account/workspace discovery, caller-owned storage, bounded transport, and
cancellation. It never adopts CLI credentials or launches a browser. Public
handler fixtures check its types against implemented behavior; no crates.io
package is published.

Use generated public resource bindings with a small ergonomic layer for waits,
streaming, files, credentials, and retry identities as those contracts ship.
The future Computers module remains `openagents::computers`; #11805 can extend
this package without holding authentication on its prerequisites. Existing
binaries and workspace-wide publication policy keep their names and rules.

Client-only builds must avoid host harness/process stacks, provider services,
and server database dependencies. Add other client domains as their public
contracts and consumers are ready. A possible eventual family is:

```text
OpenAgents SDK
  client
    auth, chats
    computers: independent Boat delivery
    later: decisions, projects, opt-in pay helpers
  host
    sessions, harness adapters, hooks, placement
  provider
    computer-driver, conformance
    separately qualified: external providers and Pylon
```

The client can evolve with compatible server changes. The host layer carries
its own adapter/platform and release constraints. Publication to crates.io is
a separate owner decision after a usable client and real consumer pass; this
plan does not authorize publishing a package or changing release channels.

The [Coder 1.0 binary freeze](../coder/1.0-freeze.md) remains binding.
Server and web improvements can ship now. New CLI features, compiled launch
changes, and host-binary-dependent work wait in
[the next-release list](../coder/next-release.md), tracked by [#11823](https://github.com/OpenAgentsInc/openagents/issues/11823).
Existing Coder 1.0.0 routes, old addresses, fields, errors, and model IDs keep
working throughout the migration.

## Product consumption

Our products should exercise the same public account-scoped contract that
developers receive. Trusted server-to-service implementation calls remain
internal; they must not become developer credentials or bypass account checks.

| Consumer | Migration |
| --- | --- |
| Coder Desktop | First external reference client, in a separate Zeron fork; consume the exported Rust SDK without monorepo-only access; [desktop plan](../coder-desktop/README.md) |
| Web chat | Qualified public Rust client adoption in #11806, with saved requests/output readers, chat/task identities, per-turn authority, and confirmed cancellation |
| Solver | Implemented account-scoped allocation, saved commands, recovery, and original-job cleanup behind explicit account admission ([#11807](https://github.com/OpenAgentsInc/openagents/issues/11807), [inventory](solver-computers.md), [evidence](solver-computers-evidence.json)). The GitHub-connected staging job remains unqualified. |
| Environments | Shared setup, builder, and fresh-verifier implementation under verification, with explicit account enrollment ([#11808](https://github.com/OpenAgentsInc/openagents/issues/11808), [inventory and rollout](environment-computers.md)). |
| Existing OpenAgents desktop/mobile | Adopt supported public operations incrementally without pulling server/provider internals into clients |
| Frozen Coder/CLI | Keep compatible server behavior; compiled changes wait for [#11823](https://github.com/OpenAgentsInc/openagents/issues/11823) |
| External developers/agents | Use HTTP/OpenAPI and the Rust client with their own authorized account credentials |

The completed desktop session work #11709 contributes host-consumer evidence.
It does not establish a published stable host SDK. The separate host proposal
[#11819](https://github.com/OpenAgentsInc/openagents/issues/11819) requires an explicit boundary and two real consumers outside Coder.

## Contract and security rules

1. **Specify implemented behavior.** Maintain the Computers OpenAPI with its
   handlers and generated client. Expand the unified reference incrementally
   through [#11818](https://github.com/OpenAgentsInc/openagents/issues/11818); incomplete unrelated routes do not hold the first
   client. Proposed operations must not appear as implemented promises.
2. **Derive ownership from authentication.** Validate issuer, audience, expiry,
   scopes, account/workspace, and grants. A key prefix is not an authorization
   policy: `oa_sk_`, session/device, and `oak_` credentials require an explicit
   supported mapping. A decision-only key does not automatically gain compute.
3. **Keep long work observable.** Acceptance is not readiness or completion.
   Commands distinguish creation, execution, attachment, cancellation, and an
   uncertain result. Disconnecting a stream does not prove the process stopped.
4. **Retain retry identities.** Generate one idempotency key per logical request,
   reuse it across attempts, and expose a handle applications can retain across
   restart. Conflicting retries refuse; uncertain commands are observed rather
   than rerun under a fresh identity.
5. **Keep placement within authority.** Public callers express requirements and
   placement intent. Deterministic eligibility checks precede any typed Decisions
   API judgment. A model cannot override grants, capability, cost, or data policy.
6. **Describe credential limits honestly.** The SDK does not write keys to
   temporary files by default. Sending values through command stdin avoids one
   persistence path, but guest processes may retain them in RAM, snapshots,
   files, or logs. Credential-reference injection is separately qualified.
7. **Authorize references.** Public credential handles are account/grant scoped;
   arbitrary Secret Manager resource names are not trusted authority. Renew and
   revoke access across wake, permission changes, and deletion.
8. **Require payment consent.** Default clients return typed 402 responses without
   spending. Future x402 helpers require explicit opt-in, a defined offer,
   spending/fee/recipient limits, and retry-safe payment plus execution.
9. **Preserve stable errors and bounded transport.** Keep frozen client error
   meanings and use explicit deadlines, payload limits, `Retry-After`, safe
   origins, and secret-free diagnostics.

## Delivery tracks

The [issue directory](delivery-plan.md#issue-directory) records every scope,
prerequisite, and activation condition. The ordering is:

1. App authentication, existing chat lists and complete saved history in the
   external desktop client (#11824); first desktop priority.
2. Public Boat lifecycle, commands/files, Computers specification, Rust client,
   and web proof.
3. In parallel where independent, backend registry, supported SPACE snapshot
   path, compatible environment artifacts, lifecycle, credentials/egress,
   deployment, and usage; then repeat the real SDK/chat flow on SPACE.
4. Migrate Solver and Environments separately after the first client/web proof.
5. Add enrolled own machines with safe attach/detach semantics and explicit
   capabilities, without making them a cloud-delivery blocker.
6. Expand API coverage and documentation with actual consumers.
7. Activate deferred publication/provider/language/payment/CLI tracks only when
   their stated conditions are met.

Use affected-crate tests and formatting for implementation changes and link/path
checks for documentation. Run the scoped live scenarios when their capabilities
are ready. Add no GitHub workflows or GitHub-billed automation. Use dedicated
test accounts, never the owner's account or real computers, and clean all test
records and infrastructure.

## Deferred decisions

| Decision | Condition and issue |
| --- | --- |
| Rust package publication | Explicit owner decision after the client and real consumer pass; [#11805](https://github.com/OpenAgentsInc/openagents/issues/11805) |
| Public host SDK | Explicit API boundary, two non-Coder consumers, support policy, and publication decision; [#11819](https://github.com/OpenAgentsInc/openagents/issues/11819) |
| External computer providers/Pylon | Trust, account consent, qualification, custody, and payout policy; [#11820](https://github.com/OpenAgentsInc/openagents/issues/11820) |
| Other language clients | Concrete demand and owner-selected repository/package location; [#11821](https://github.com/OpenAgentsInc/openagents/issues/11821) |
| Automatic x402 payment | Defined paid offer, explicit opt-in/caps, and payment/execution recovery proof; [#11822](https://github.com/OpenAgentsInc/openagents/issues/11822) |
| New CLI/compiled host behavior | Owner lifts the Coder freeze and authorizes release work; [#11823](https://github.com/OpenAgentsInc/openagents/issues/11823) |

Rust remains the product language in this repository. Do not add TypeScript.
Additional generated product clients live only in an explicitly approved
location. The historical `openagents-ai` reference does not authorize reviving
that repository or publishing packages. These decisions do not prevent the
first Rust client and public API from being implemented and exercised.

The [public route inventory](public-routes.md) distinguishes verified schemas,
implemented gaps, gated/internal routes, and separately served API families.
Its manual drift checks keep the combined public reference tied to the front
without delaying the independent Computers delivery.

The public reference also composes the [System One slice](openapi.systemone.json)
from its owning decision contract. It does not publish standalone classification
or jobs routes. The app facade's auth/history fixture remains separate; `jev`
continues to own the existing decision caller. See the
[System One guide](https://openagents.com/docs/api/system-one) for request identity,
workspace access, responses, and refusals.
