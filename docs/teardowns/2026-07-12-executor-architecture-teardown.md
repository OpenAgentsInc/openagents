# Executor Architecture Teardown — 2026-07-12

Read-only architecture audit of the open-source Executor repository, pinned to
commit
[0a50c796c2cc334cf3e9bf6d4be33c77dbfac93b](https://github.com/RhysSullivan/executor/tree/0a50c796c2cc334cf3e9bf6d4be33c77dbfac93b)
(`v1.5.33`, committed 2026-07-11 Pacific time).

This teardown asks a narrower question than the desktop and agent-runtime
teardowns: what did Executor's author mean by finding "primitives that unlock
your agent to do anything," which of those primitives exist in source today,
and should OpenAgents adapt them or consume Executor directly?

Evidence labels:

- **[source]** — directly observed in the pinned source tree
- **[schema]** — encoded in a public type, JSON Schema, HTTP API, storage
  descriptor, or package export
- **[test]** — exercised by a checked-in unit, integration, e2e, release, or
  smoke test
- **[history]** — supported by the pinned Git history
- **[vision]** — stated in `vision.md`, but not necessarily implemented
- **[inferred]** — concluded from several visible mechanisms
- **[limitation]** — a boundary on what this audit establishes

No Executor or OpenAgents runtime, user state, credential, connection, or
hosted account was modified or exercised. The analysis used source, tests,
package metadata, and Git history. Executor's MIT license permits reuse, but a
license is not an architectural compatibility guarantee.

## TL;DR

Executor's important new idea is not another MCP proxy. It is a closed
capability-production loop:

```text
imported APIs and MCP servers
        │
        ▼
normalized, authenticated tool catalog
        │
        ▼
user- or agent-authored typed TypeScript function
        │
        ▼
sandbox with only declared connection-backed tool handles
        │
        ▼
published custom tool re-enters the same catalog
        │
        └──────── usable from MCP, CLI, HTTP, SDK, and UI
```

The July 8 commit sequence makes the first vertical slice real [history]:

1. `953d4ec9` added the Apps engine, custom-tool authoring contract, bundle and
   publish pipeline, content-addressed store, and virtual-tool plugin.
2. `f595771a` added Git and local-directory sources, a hand-written smart-HTTP
   Git client, local `workerd` subprocess execution, Cloudflare Dynamic Worker
   execution, and host wiring.
3. `28fdd318` added source-management UI and e2e coverage across cloud,
   self-host, packed CLI, local directory, and desktop sidecar forms.

An authored tool declares input/output schemas, one or more required
integrations, and a handler. At invocation, the caller chooses concrete
connections. Executor removes those connection addresses from ordinary input,
creates proxy clients for only the declared roles, runs the handler in an
isolate, and routes nested calls back through the normal Executor invocation
path. Credentials remain in trusted provider space. The resulting tool is
projected into the same catalog as imported OpenAPI, GraphQL, Google, Microsoft,
and MCP operations. [source/schema/test]

That is the primitive the author is excited about: **the agent can manufacture
new governed verbs by composing existing governed verbs**. Loops, branching,
data transformation, and several remote calls collapse into one reusable,
typed operation without giving generated code raw credentials.

The implementation is promising and unusually aligned with OpenAgents, but
the full vision is not shipped. Workflows, UI, and skills are explicitly
recorded as `not supported yet`; runtime authoring over MCP, promotion back to
Git, reactive stores, rich output handles, the unified Run record, remote
cores, and the transitive `scope()` capability membrane remain vision. More
importantly, current source contradicts parts of that vision: the v2 owner
policy explicitly has no scope stack, and current toolkits store policy rules
rather than being pure curation. [source/vision/limitation]

The OpenAgents decision is therefore:

> **Adapt Executor's capability-artifact model and sandboxed composition loop;
> do not adopt Executor as Khala's core authority runtime. Support an Executor
> instance as an optional external integration, and consider direct use of a
> narrow kernel package only behind an OpenAgents-owned compatibility adapter
> after a pinned conformance spike.**

OpenAgents already has the stronger authority half: Effect Schema tool and
runtime contracts, allow/ask/deny toolsets, typed MCP grants, approval
lifecycle, redaction classes, event and receipt vocabulary, local and managed
sandbox interfaces, Blueprint/Fleet governance, and cross-device durable
projection. What it lacks is Executor's clean authored-tool loop: source →
bundle → validate → content address → isolate → connection-parametric bridge →
active descriptor and catalog projection.

## 1. Snapshot identity and confidence

| Field | Value | Evidence |
| --- | --- | --- |
| Repository | `RhysSullivan/executor` (package metadata also names `UsefulSoftwareCo/executor`) | [source] |
| Commit | `0a50c796c2cc334cf3e9bf6d4be33c77dbfac93b` | [source] |
| Release | `v1.5.33` | [history] |
| License | MIT, copyright Rhys Sullivan | [source] |
| Tracked files | 1,925 | [source] |
| Primary language/runtime | TypeScript, Bun 1.3.11, Effect 4 beta | [source] |
| Effect version | `4.0.0-beta.59` | [schema] root `package.json` |
| Main distribution forms | in-process SDK, CLI daemon, desktop, cloud, Docker self-host, Cloudflare self-host | [source] |
| Integration kinds | OpenAPI, GraphQL, MCP, Google/Microsoft OpenAPI, extensible plugins | [source] |
| Apps package | `@executor-js/plugin-apps@0.1.4` | [schema] |
| Apps local isolate | private `@executor-js/runtime-workerd-subprocess@0.0.5` | [schema] |
| Apps cloud isolate | Cloudflare Dynamic Worker | [source] |
| Test-bearing package files | more than 200 `*.test.ts[x]` files | [source] |

Confidence is high for the local architecture because the repository contains
the implementation, tests, e2e recordings, release packaging, migrations, and
host composition. Confidence is lower for hosted operational properties: this
audit did not inspect a production deployment, cloud configuration, billing,
or live credential behavior. [limitation]

The repository moves quickly. The Apps engine crossed more than 12,000 added
lines in its first three main commits, then received packaging and worker fixes
through `v1.5.33`. Treat its current package and descriptor versions as active
development surfaces, not frozen standards. [history/inferred]

## 2. Product model

Executor describes itself as an integration layer for AI agents, but its core
model is general [source/vision]:

- a **tool** is a typed unit of capability;
- an **integration** produces tools from an OpenAPI document, GraphQL endpoint,
  MCP server, or another plugin-defined source;
- a **connection** is a named authenticated account for an integration;
- a **credential provider** stores or resolves secrets outside tool I/O;
- a **policy** allows, approval-gates, or blocks invocation;
- a **host** projects the same runtime through SDK, MCP, HTTP, CLI, or web; and
- a **plugin** owns integration-specific discovery and invocation.

Current callable addresses use:

```text
tools.<integration>.<owner>.<connection>.<tool...>
```

The tool suffix may itself be dotted. The explicit owner and connection are
load-bearing: `work`, `personal`, `prod`, or another account is data in the
invocation address rather than ambient process state. [source]

The public `Executor` type exposes integrations, connections, OAuth, tools,
credential providers, policies, one `execute(address, args)` operation, and
close. Plugins add storage, routes, handlers, tool resolution, argument
validation, and invocation around that stable center. [schema]

This gives Executor a useful separation:

```text
integration-specific edge                   substrate-generic center
─────────────────────────                   ────────────────────────
parse OpenAPI / talk MCP                     catalog
discover operations                         connection identity
apply provider auth                         credential resolution
invoke upstream                             allow / approval / block
normalize result                            one execute operation
                                             storage and host projection
```

OpenAgents should preserve that seam. Integration adapters should not own
Blueprint authority, approval semantics, receipt shape, or client delivery;
the shared invocation processor should not know how every upstream API works.

## 3. The existing code-mode substrate

The Apps engine builds on an older Executor primitive called code mode. The
published `@executor-js/codemode-core` package defines a `CodeExecutor` and
`SandboxToolInvoker`; QuickJS, Deno subprocess, dynamic Worker, and now local
`workerd` implementations supply execution. [source/schema]

Code mode gives a model a small surface rather than injecting every tool schema
into context:

- search the catalog;
- enumerate an exact integration namespace;
- describe one tool;
- execute sandboxed TypeScript/JavaScript against a proxy; and
- pause and resume when a nested call requires OAuth, input, or approval.

This is valuable for three separate reasons:

1. **Context efficiency.** A large catalog is discovered lazily instead of
   copied into every prompt.
2. **Computational composition.** Iteration, filtering, branching, joins, and
   intermediate values stay inside code rather than consuming one model turn
   per primitive call.
3. **Authority mediation.** The sandbox sees callable object capabilities, not
   secret material or a general host API.

The current default search implementation is the wrong part for OpenAgents to
copy. It tokenizes strings, applies hand-written field weights, uses exact,
prefix, and substring matches, and ranks by a custom score. [source] That is an
ad hoc keyword router and conflicts with the workspace invariant requiring a
central typed semantic selector, embedding similarity, structured planner, or
modeled parser for user-facing retrieval and tool selection. OpenAgents should
adapt lazy discovery and exact namespace enumeration, while implementing
semantic selection through its owned selector contract.

## 4. The Apps/custom-tools slice

### 4.1 Source acquisition

An Apps source is either:

- an HTTPS Git repository, optionally at a named ref and with an explicitly
  stored token; or
- a local directory, enabled only by hosts that can safely expose one.

The Git client reads refs and packfiles directly over smart HTTP. Source state
records the configured source, resolved commit, current publication status,
tool list, errors, description, and update time. A cheap refs check skips pack
download and republish when the source commit is unchanged. [source/test]

Synchronization is operator-driven in the implemented slice: source creation
performs an initial sync and the console exposes another sync action, but no
repository webhook, file watcher, scheduled poller, or deploy/pull promotion
loop was found. The compiler is real; continuous source-to-production delivery
is not yet part of the proven lifecycle. [source/limitation]

The URL boundary has several good details:

- HTTPS is required by default;
- embedded username/password and token-like query parameters are rejected;
- diagnostic URLs remove credentials, query, and fragments;
- localhost, link-local, private IPv4, IPv6 private ranges, mapped IPv4, and
  alternate numeric IPv4 spellings are denied unless a host explicitly opts
  into private Git hosts; and
- redirects are revalidated against the same policy. [source/test]

Literal-host parsing is not the whole SSRF boundary. The visible check does not
resolve and pin DNS before fetch, so a public-looking name resolving to a
private address is not excluded by this helper alone. OpenAgents' fetch path
must enforce resolved-address and redirect policy at the actual network dial.
[source/limitation]

The configured Git token is written through Executor's provider seam under a
source-specific item. Source fetch intentionally does not borrow an unrelated
stored GitHub connection. That avoids ambient credential sharing. [source]

Publication accepts at most 256 relevant files, one MiB per file, and four MiB
total. Those limits bound the admitted network/source input and relevant-file
set, but they do not visibly bound compressed-pack expansion or decompression
work. OpenAgents would still need decompression, dependency, CPU, memory, and
generated-output budgets in its own artifact contract. [source/inferred]

### 4.2 Canonical layout and authoring API

The implemented file convention is deliberately small:

```text
executor.json                 # optional description
tools/<slug>.ts               # implemented
tools/<slug>.tsx              # implemented
workflows/*                   # recognized, deferred
ui/*                          # recognized, deferred
skills/*                      # recognized, deferred
package.json                  # dependency input to the bundler
```

Tool filenames are lowercase slug identities. A module default-exports one
`defineTool(...)`, a factory returning one, or a record of named tools. The
authoring API accepts Standard Schema implementations such as Zod or raw JSON
Schema. [source/schema/test]

The current source filter is stricter than the layout sketch suggests. It
fetches only the direct tool files plus root `executor.json`, `package.json`,
and supported lockfiles. Nested `src/`, `lib/`, and helper modules are ignored.
This keeps the first slice bounded but means ordinary multi-file application
organization is not yet an implemented authored-app model. [source/limitation]

A definition contains:

```ts
defineTool({
  description,
  integrations: {
    github: integration("github"),
    inboxes: integration("gmail").array(),
  },
  input,
  output,
  annotations: {
    readOnly,
    destructive,
    requiresApproval,
  },
  handler(input, { github, inboxes }) {
    // composition code
  },
})
```

An integration declaration is not a credential. It is a role saying which
kind of connection the eventual caller must supply. `.array()` asks for
several connections; `.describe()` gives the account-selection field a human
description. [schema]

This **connection-parametric** design is one of Executor's strongest ideas.
The same authored function can run against a personal or work inbox, a staging
or production API, or several accounts without changing source. Account choice
becomes validated invocation input, while the handler receives a role-typed
callable proxy whose operation results remain untyped.

The integration kind and role are typed at declaration time, but calls through
the current operation client resolve as `Promise<unknown>`. Executor therefore
proves typed capability selection and schema-checked outer inputs/outputs, not
end-to-end generated types for every nested integration operation.
[schema/limitation]

OpenAgents should adapt the role concept but use an OpenAgents-owned schema:
`CapabilityRequirement { role, integrationKind, cardinality, authorityCeiling,
dataClasses, connectionSelection }`. A plain integration slug is not enough
for payments, deployment, owner-local process authority, or cross-device
delegation.

### 4.3 Publish pipeline with optimistic replacement

The publish path is:

1. enforce file/count/byte limits;
2. discover canonical tool files and mark reserved future artifacts deferred;
3. bundle every entry;
4. load it in the execution substrate and collect its tool declarations;
5. run collection twice and reject nondeterministic declarations;
6. convert Standard Schema input/output to JSON Schema;
7. reject integration-role names that collide with ordinary input fields;
8. hash source and bundle material;
9. write content-addressed bundle blobs;
10. build a versioned descriptor containing source ref, source hash, toolchain,
    schemas, annotations, and bundle keys; and
11. publish the projected rows and active descriptor if the expected prior
    source ref still matches, tombstoning removed tools.

That is more than a file watcher. It is a small artifact compiler with
optimistic concurrency, deterministic discovery, immutable payload identities,
and an active descriptor/tool-row projection. [source/test]

Dependency handling has useful supply-chain guardrails. The publish worker
allows configured npm registry origins, rejects install/prepare lifecycle
scripts, non-registry dependency specs, and native-addon artifacts, and bounds
the emitted bundle. It does not yet establish publisher signatures, a trusted-
dependency policy, SBOM, commit-signature verification, lockfile fidelity, or
a reproducible-build receipt. [source/limitation]

The generation switch also deserves stronger atomicity. `putMany` projects
tool rows and a later write replaces the active descriptor. The expected prior
source ref detects a source change at that check, but the visible sequence is
not one transaction across both writes and does not prove every race is
rejected. OpenAgents should require an atomic active-generation pointer so
storage failure cannot expose a partially projected generation.
[source/inferred]

The pattern maps directly onto OpenAgents' existing component ledger,
extension lifecycle, behavior contracts, and receipts. OpenAgents should make
the stages first-class and receipted:

```text
declared → fetched → verified → bundled → collected → policy-reviewed
         → staged generation → activated → superseded / rolled back / revoked
```

Executor currently records pending, published, up-to-date, and failed source
states, but does not yet expose OpenAgents-grade publisher identity,
signature/transparency proof, license policy, authority diff, staged human
review, compatibility window, activation receipt, rollback receipt, or public-
safe execution receipt. Those must remain OpenAgents requirements.

### 4.4 Invocation and the capability bridge

At call time, Executor resolves each selected connection address and verifies
it belongs to the declared integration. It strips connection fields from the
handler's ordinary data input and creates one proxy root per declared role.
Proxy calls are serialized as:

```text
<role>[#index].<tool path>
```

The trusted bridge rejects undeclared roots, absent bindings, invalid indexes,
and empty nested paths. Valid calls reconstruct the full connection-backed
tool address and re-enter `ctx.execute(...)`, carrying invocation options.
Consequently, nested operations still pass through catalog lookup, policy,
approval/elicitation, credential resolution, plugin dispatch, and result
normalization. [source/test]

That is the key security property of the implemented Apps slice. The code does
not receive a raw token, `SecretRef`, provider SDK client, unrestricted
Executor object, or general fetch capability. It receives only declared proxy
roots, and those roots can call only through the normal tool dispatcher.

There is an important distinction, however:

- **implemented:** an invocation is confined to the integration roles and
  concrete connections supplied for that call;
- **not yet demonstrated:** the authored artifact captures the author's full
  transitive capability scope and can never be invoked under wider authority.

The second property is claimed by the vision's `scope()` membrane, but current
v2 owner policy explicitly says there is no scope stack, and the public
`Executor` type has no `scope()` operation. Current Apps declarations name
integration kinds, while the caller selects connections. OpenAgents must not
describe that as object-capability non-amplification until a formal
intersection law and adversarial tests prove it.

### 4.5 Local and cloud isolation

For local/self-hosted execution, Executor starts a separate `workerd` process.
It writes modules and config into a mode-0700 temporary directory, writes files
mode 0600, binds the isolate only to a random loopback port, and creates a
random-token-protected loopback host bridge. `globalOutbound` defaults to a
blocked Worker; unsafe eval defaults off. The host kills the process on
disposal, removes temporary material, bounds startup, invocation, and
unresponsive-host time, and fails pending requests if the process exits.
[source/test]

For Executor Cloud, equivalent bundles run in Cloudflare Dynamic Workers. The
Apps runtime keys warm isolates by tenant, content-addressed bundle, and driver
version so identical bytes from two organizations do not share module-level
state. [source/test]

This local/hosted substrate substitution is exactly the kind of parity
OpenAgents needs:

| Property | Executor local | Executor cloud | OpenAgents target |
| --- | --- | --- | --- |
| Code container | `workerd` subprocess | Dynamic Worker isolate | owned isolate/guest adapter selected by execution profile |
| Network | default blocked | binding-controlled | deny-by-default compiled egress with receipt |
| Tool access | tokened loopback bridge | Worker binding | capability broker entering the canonical Khala dispatcher |
| Credential access | trusted host only | trusted host only | broker-owned refs only, never artifact input/output |
| Identity | tenant + bundle + driver | tenant + bundle + driver | owner + WorkContext + artifact generation + authority manifest |
| Completion evidence | typed result/error | typed result/error | durable Run/Work Unit events plus execution and delivery receipts |

OpenAgents' current `ai-sdk-sandbox-local` creates bounded workspace paths but
defaults its declared network policy to `allow-all` and launches `/bin/bash`
directly on the host. A JSON environment variable describing policy is not
containment. Executor's local `workerd` runner is therefore a concrete and
timely reference for the missing authored-code lane. It is not a replacement
for OS/VM isolation used by coding agents that need shell and filesystem
authority.

Apps packaging is not fully uniform across Executor's advertised host forms.
The slice is wired into local CLI/desktop, Executor Cloud, and Docker self-host.
The separate Cloudflare self-host plugin set does not include Apps at this
snapshot. “Same functionality, different packaging” is therefore a product
direction rather than a true Apps parity statement today. [source/limitation]

The in-process Apps executor is explicitly test-only. Production local and
cloud hosts take the isolate paths; OpenAgents should preserve that boundary
rather than allowing a convenient in-process fallback to acquire production
authority. [source/limitation]

## 5. Catalog, policy, approvals, and secrets

### 5.1 Catalog projection

After publication, the Apps plugin registers a normal no-auth integration and
a `published` connection, then resolves active authored tools through the same
plugin seam as imported integrations. Input schemas are projected with enum
values for currently available connection addresses. One-role fields choose
one account; many-role fields choose arrays. [source/schema]

The descriptor records source ref, source path and hash, bundle key, toolchain,
input/output schemas, integration requirements, and annotations. Removed tools
are tombstoned rather than silently left callable. An expected-ref mismatch is
reported as a publication conflict instead of deliberately applying that stale
publish; the nontransactional projection caveat above still applies. [source]

OpenAgents should add two missing identities:

- an immutable **artifact version** independent of the friendly tool name;
- a captured **catalog generation** and authority-manifest digest attached to
  every model advertisement and invocation.

A model must execute the generation it saw. A source update cannot silently
replace semantics between description and call.

### 5.2 Policy and approval

Executor's core supports `approve`, `require_approval`, and `block`; MCP hosts
can pause, return an execution ID, use native elicitation or a browser approval
URL, and resume within a bounded approval window. Nested Apps calls re-enter
that machinery. [source/test]

The authored-tool annotation projection is narrower than it first appears.
`requiresApproval` maps to the catalog default, and `readOnly: true` forces the
default to no approval. The `destructive` annotation is stored in the Apps
descriptor but is not mapped by the Apps plugin into a separate enforcement
class. User/toolkit policy may still require approval, but OpenAgents should
not treat author-declared safety annotations as authority.

OpenAgents already has a stronger vocabulary: per-tool authority classes,
allow/approval-required/deny, explicit permission requests, once/session/
project scopes, compiled agent-definition allow/ask/deny toolsets, network and
secret policy, typed approval events, and durable fleet/Blueprint outcomes.
The adaptation is to run authored tools through that existing dispatcher, not
replace it with Executor's policy database.

Apps source handlers also expose create, sync, and delete without an Apps-
specific role check visible in those handlers, while records and publications
are written at organization ownership. Ambient host authentication may narrow
the endpoint, but an OpenAgents org-wide publish transition must be an explicit
privileged command with its own authority, approval, and receipt.
[source/limitation]

### 5.3 Secret custody

Executor separates a connection's auth template from its resolved credential.
Provider plugins include encrypted storage, file secrets, OS keychain,
1Password, and WorkOS Vault. Apps receive callable proxies, not credential
values. Source Git tokens use their own provider item and diagnostic paths are
redacted. [source]

This agrees with OpenAgents' broker-only credential invariants. If OpenAgents
interoperates with an external Executor, the custody boundary must remain
truthful: OpenAgents can prove it sent a bounded request to a named Executor
connection and record the returned result, but it cannot claim to have
established Executor's internal credential, upstream, or containment facts
unless Executor returns a verifiable receipt for them.

## 6. Vision versus implementation

Executor's `vision.md` is unusually useful because it names the intended
algebra rather than only listing features. It proposes:

- one typed invocation primitive;
- strict scope intersection and typed meta-capabilities;
- authored tools, workflows, UI, skills, and stores;
- file/deploy and runtime/MCP authoring paths;
- Git-backed versioned artifacts and reactive agent state;
- a Run unifying audit, approvals, workflow state, resume, and debugging;
- rich-output delivery negotiated as inline value, handle, embedded resource,
  or deep link;
- remote cores that execute where credentials live; and
- the same capability membrane in local and cloud isolates. [vision]

The repository itself warns that this is destination, not current status. The
following table is the honest boundary at the pinned commit:

| Capability | Pinned status | Evidence |
| --- | --- | --- |
| Imported tool catalog, named connections, provider-resolved secrets | Implemented | [source/test] |
| OpenAPI, GraphQL, MCP integration plugins | Implemented | [source/test] |
| Code mode with lazy search/describe/invoke | Implemented | [source/test] |
| Custom typed tools from Git/local source | Implemented | [source/test] |
| Content-addressed bundle + descriptor generation | Implemented | [source/test] |
| Local `workerd` and cloud Dynamic Worker Apps execution | Implemented | [source/test] |
| Declared-role, caller-selected connection bridge | Implemented | [source/test] |
| Nested policy/approval/credential dispatch | Implemented | [source/test] |
| Workflows, authored UI, skills in Apps descriptor | Recognized but explicitly deferred | [schema] |
| Runtime `author_tool`, `create_workflow`, `skills.create` over MCP | Vision, not found as the Apps authoring path | [vision/limitation] |
| `executor deploy` / `pull` promotion loop for all artifacts | Vision | [vision/limitation] |
| Pure-curation toolkits separate from policy | Not current: toolkit records contain policies and connection patterns | [source/vision] |
| Scope stack and strictly intersecting `scope()` executor | Not current: v2 owner policy says no scope stack | [source/vision] |
| Typed meta-capabilities for author/deploy/UI/storage/egress | Vision | [vision/limitation] |
| Unified durable Run record across every surface | Partial execution pause/resume exists; full stated record not established here | [source/vision/limitation] |
| Reactive KV/SQLite/filesystem stores and large-result handles | Vision | [vision/limitation] |
| Remote cores and execute-where-credential-lives federation | Vision | [vision/limitation] |

This gap does not make the implementation unimportant. It makes the adaptation
decision more precise: take the compiled artifact and object-capability bridge
patterns that exist; do not import unimplemented security claims as product
requirements already satisfied.

## 7. What Executor changes in the OpenAgents analysis

The earlier teardowns converged on a signed, isolated typed catalog. Executor
adds the missing operational center of that phrase.

### 7.1 A catalog should be generative, not only aggregative

OpenAgents currently normalizes built-in and MCP tools into typed Khala tool
definitions and dispatches them through validation, authority, permission,
execution, bounded output, events, and redaction. Desktop also projects MCP,
plugin, and skill lifecycle into one audit view. Those are strong enforcement
and UX foundations.

The current Khala registry still needs generation discipline: registration
uses a mutable name map rather than immutable artifact identity, and its
deferred external-tool search includes lowercase substring matching. Before a
large authored catalog lands, duplicate/collision behavior must fail closed or
be generation-qualified, and free-form selection must move to the workspace's
central semantic selector. Executor reveals the missing artifact loop; it does
not excuse existing OpenAgents catalog shortcuts.

What is missing is a canonical way to turn a repository artifact into a new
tool that re-enters that dispatcher. Executor demonstrates the smallest useful
version. The OpenAgents extension program should therefore add **authored
capability artifacts**, not another privileged in-process plugin API.

### 7.2 Connections are invocation parameters

OpenAgents already isolates provider accounts and models owner-scoped
credential refs. Executor supplies a clean composition rule: artifact source
declares roles and integration kinds; invocation chooses concrete connections;
the trusted broker resolves them; sandbox code sees handles.

This should become a shared OpenAgents contract across local, Pylon, managed
cloud, and future marketplace execution. It also improves review: an artifact
can be audited without containing a production account or secret reference.

### 7.3 Local isolates can be a distinct execution profile

Not every generated operation needs a coding-agent VM. A pure TypeScript
composition over brokered tools can run cheaply in a deny-network `workerd`
isolate. Shell/file/process work still needs the stronger workspace-bounded or
guest profiles already planned. Executor adds a useful profile between
"projection only" and "workspace bounded":

- **brokered function isolate** — no filesystem, process, raw secret, or
  ambient network; only captured tool capabilities, bounded CPU/time/memory,
  typed input/output, and durable events.

### 7.4 Artifact activation is a runtime generation change

Executor's source ref, descriptor, bundle hash, optimistic replacement, and
tombstones are the beginning of the generation model the OpenCode V2 teardown
recommended. OpenAgents should connect this directly to its extension
lifecycle and component ledger. Declare, validate, grant, activate, revoke,
update, and rollback must name exact artifact and catalog generations.

### 7.5 One invocation path is the real composition primitive

Executor correctly routes authored nested calls through the normal dispatcher.
OpenAgents must do the same. A custom function is not permitted to invoke
adapter clients directly. It receives a broker whose only operation is the
canonical Khala tool invocation, with the current WorkContext, captured
catalog generation, authority manifest, approval state, accounting, and
receipt lineage.

## 8. Adapt or consume directly?

### Decision matrix

| Option | Value | Cost/risk | Decision |
| --- | --- | --- | --- |
| Replace Khala/Pylon tool authority with Executor | Fast access to broad integrations and Apps | Splits identity, policy, receipts, WorkContext, Sync, Blueprint, provider support, and runtime lifecycle | **Reject** |
| Embed `@executor-js/sdk` and its plugins as the OpenAgents core | Mature catalog/auth/integration substrate | Parallel database and owner model; exact Effect beta mismatch; another plugin/service graph; OpenAgents contracts become adapters | **Do not do** |
| Embed `@executor-js/plugin-apps` | Reuses authored-tool implementation | Package is pre-1.0, depends on private workspace runtime, and assumes Executor storage/plugin/connection internals | **Not a viable stable dependency today** |
| Run Executor as optional local/remote sidecar and connect through MCP/HTTP | Immediate ecosystem bridge with clear ownership boundary | Double policy, health/version/capability negotiation, partial receipts, separate connection UI | **Support as interoperability** |
| Directly evaluate `@executor-js/codemode-core` or QuickJS runtime | Small MIT kernel, published package, reusable conformance vocabulary | pre-1.0 behavior, exact Effect peer, different schemas/errors, potential duplicate parser/runtime | **Bounded spike only** |
| Port the Apps architecture into OpenAgents-owned contracts | Fits Khala authority, receipts, Sync, Blueprint, Effect Schema, and sandbox profiles | More implementation work and responsibility | **Primary recommendation** |

### Why the whole runtime should not be embedded

Executor and OpenAgents overlap at the most load-bearing seams:

- both own tool schemas and registries;
- both own MCP ingestion and serving;
- both own policy and approval;
- both own account/credential references;
- both own Effect runtime composition;
- both own local and cloud host forms; and
- both aspire to workflows, skills, stores, runs, UI, and remote execution.

Embedding one inside the other would not remove a system. It would create two
authorities and an adapter between them. Every invocation would need answers
to: which catalog generation, which policy won, which account owner model,
which approval ID, which runtime interrupted, which event log is durable, and
which receipt can be shown on mobile. That is precisely the sidecar and
authority ambiguity the adaptation analysis rejects.

There is also concrete version and packaging friction. OpenAgents pins Effect
`4.0.0-beta.70`; Executor pins `4.0.0-beta.59`. `@executor-js/plugin-apps` is
`0.1.4`, is omitted from Executor's explicit public-package release allowlist,
and its local and cloud runtime packages are private. The Apps plugin's source
exports are useful for study, but they are not currently a stable, standalone
runtime boundary. [schema]

### How direct interoperability should work

Executor should be accepted as one **external capability provider** through
two distinct adapter modes:

```text
HTTP leaf mode
  authenticated Executor HTTP catalog
    → OpenAgents-owned immutable catalog snapshot
    → OpenAgents semantic selection / grant / policy
    → versioned Executor HTTP invocation
    → normalized result + external-provider evidence

MCP meta-tool mode
  OpenAgents policy
    → named, scoped Executor `execute` / `skills` / `resume` meta-tools
    → Executor performs its own lazy leaf selection and execution
    → normalized result + external-provider evidence
```

Executor's compact MCP server does not enumerate every leaf tool; it exposes
the `execute`, `skills`, and `resume` meta-tool surface. Leaf-tool import and
invocation instead require the authenticated HTTP catalog/API. OpenAgents must
not claim a remote Executor catalog generation exists in either mode: for HTTP
import it creates and captures its own normalized snapshot; for MCP it records
the admitted meta-tools, toolkit/connection scope, and whatever provider
evidence Executor returns. [source/limitation]

Required adapter behavior:

- pin and report Executor endpoint and protocol/version;
- in HTTP mode, import bounded leaf descriptors into one captured OpenAgents
  catalog generation;
- in MCP mode, advertise only the compact meta-tools and do not synthesize
  unobserved leaf descriptors;
- map tools to explicit OpenAgents authority classes, never trust a remote
  `readOnly` hint as policy;
- select an explicit Executor toolkit or connection boundary;
- prevent arbitrary endpoint/namespace substitution after admission;
- preserve OpenAgents cancellation and timeout semantics;
- represent Executor approval pauses as typed foreign approval dependencies;
- bound and redact remote schemas, results, errors, and logs;
- health-check and expose `needs_auth`, stale, incompatible, and offline states;
- record that containment and credential custody are provider-asserted unless
  backed by a verifiable Executor receipt; and
- never let the optional bridge become necessary for local Khala core tools.

This route gives users immediate access to Executor's integration catalog while
keeping OpenAgents' public promises honest.

It does not yet provide an agent-facing authoring control plane. At this
snapshot, Apps source creation, synchronization, and deletion are HTTP/console
operations; the proposed MCP `author_tool` and deploy/pull loop exist only in
the vision. The first adapter can consume tools already configured and
published by an Executor deployment, but OpenAgents must not market it as an
immediate natural-language-to-published-tool path. [source/vision/limitation]

The adapter should land after the existing OpenAgents MCP split is reconciled.
`@openagentsinc/mcp-contract` still labels itself phase-0 with no exposed
runtime transport, while `khala-tools` and Pylon contain working but separate
MCP paths. Executor must become one source behind the winning shared contract,
not a third registry or transport authority.

Credential cleanup is part of that gate. Desktop's public MCP projection is
correctly secret-free, but its current local host persists configured MCP
environment/header/argument values in a private mode-0600 JSON file. File mode
is not encrypted custody. Broad Executor/MCP catalog support should move those
values behind encrypted or brokered secret refs before it multiplies the
number of configured upstreams.

### Prior OpenAgents position

OpenAgents had already reached a narrower “interop over rivalry” conclusion in
the harness-agnostic Agent Definition audit: use Executor as a toolset source
and OpenAgents dispatch as the target, while keeping the external reference
read-only. The July Apps work confirms that boundary and expands what is worth
adapting. Executor is now also a compiler and runtime reference for durable
authored tools, not only an imported-tool catalog.

The earlier autonomous-QA feature request remains useful for Executor's
`Target`, capability, artifact, and e2e machinery. Its shorthand description
of Executor as an MCP catalog is now incomplete, but its decision not to move
OpenAgents QA authority into Executor remains sound.

## 9. Concrete OpenAgents adaptation

### A. Add an Authored Capability Artifact contract

Create a browser-safe Effect Schema contract, owned near
`@openagentsinc/agent-runtime-schema` and `@openagentsinc/khala-tools`, with at
least:

- artifact ID, version, kind, friendly name, and description;
- publisher/owner identity, source URL/ref/path, license, signature, and
  content hashes;
- toolchain identity and reproducibility inputs;
- typed input and output;
- declared capability roles with one/many cardinality;
- authority ceiling, data classes, egress class, and side-effect annotations;
- compatible runtime profiles and host/protocol ranges;
- source, bundle, descriptor, and catalog-generation digests;
- lifecycle state and prior/superseding generation refs; and
- private and public-safe evidence/receipt refs.

Keep source artifact identity separate from the friendly tool address. A rename
or republish must not erase lineage.

### B. Add a Capability Broker service

The broker should resolve declared roles to concrete connection handles only
after WorkContext, agent-definition toolset, owner, policy, approval, budget,
and execution profile are fixed. Handles strictly intersect authority:

```text
effective child capability
  = parent captured grant
  ∩ artifact declared requirement
  ∩ invocation-selected connection grant
  ∩ current organization policy
  ∩ execution-profile ceiling
```

No inner artifact can widen any term. Model this bounded state space and add
counterexample-derived regression tests for:

- undeclared role calls;
- wrong-integration connection substitution;
- user/org and owner/tenant crossing;
- parent-to-child authority widening;
- source update between advertisement and call;
- policy or grant revocation during a paused nested call;
- retry after side effect;
- approval reuse across artifact generation or connection; and
- external Executor endpoint/toolkit substitution.

### C. Add a brokered-function isolate profile

Implement an OpenAgents-owned runtime adapter with:

- default-deny network and no raw sockets;
- no host filesystem, shell, process, environment, or credential access;
- only the tokened/bound capability bridge;
- bounded source, dependencies, bundle size, CPU, wall time, memory, nested
  calls, output, log, and emitted-artifact budgets;
- cancellation that interrupts isolate work and nested broker calls;
- tenant/owner/WorkContext/artifact-generation isolate keys;
- deterministic collection and schema conversion;
- full cleanup and crash tests; and
- an execution receipt naming actual runtime binary/version, profile digest,
  admitted bindings, policy version, bundle digest, timings, and termination.

`workerd` is a strong first adapter because it already exists in the
OpenAgents dependency graph through Cloudflare tooling and Executor provides a
well-tested reference. QuickJS remains useful for tiny embedded/offline paths,
but no fallback may silently weaken a requested profile.

### D. Build the Git/local source compiler

Adapt Executor's staged pipeline, adding OpenAgents requirements:

1. fetch and verify a pinned source revision;
2. enforce URL/redirect/private-network and archive limits;
3. identify license, publisher, signatures, and dependency lock;
4. discover canonical artifacts;
5. bundle in an isolated build worker with no ambient credentials;
6. collect schemas twice and reject nondeterminism;
7. statically and dynamically validate requested capabilities;
8. produce a content-addressed descriptor;
9. calculate a human-readable authority and behavior diff;
10. stage without activating;
11. obtain policy/owner approval where required;
12. atomically activate a new catalog generation; and
13. prove update, revoke, rollback, and deleted-tool behavior.

Local-directory sources stay owner-local and clearly labeled. Git sources do
not become organization-wide merely because a repository is reachable.

### E. Make authored tools first-class Khala tools

Published artifact tools should use the existing `KhalaToolDefinition`,
dispatcher, permission service, events, bounded output, private-data refs,
redaction, and agent-definition compiled tool policy. They must not create an
Apps-only dispatcher.

The current Khala registry already carries authority, availability, execution
mode, input/output schemas, permission mode, and renderer metadata. Extend it
with artifact/generation/provenance refs and capability requirements rather
than replacing it.

### F. Replace string search with semantic catalog selection

Keep Executor's exact namespace enumeration and lazy describe pattern. For
free-form user/model intent:

- use one typed semantic selector over normalized tool metadata;
- use cosine-similarity embedding search or a structured planner;
- return typed confidence, rationale/provenance refs, catalog generation, and
  bounded alternatives;
- apply grant/policy filtering before results become callable; and
- allow deterministic parsing only after route selection for bounded IDs,
  account refs, enums, dates, and amounts.

No copied token weights, substring scores, or integration-name keyword router
belongs in OpenAgents.

### G. Unify extension lifecycle and catalog generations

Desktop's current extension lifecycle is a pure projection over separate MCP
and plugin hosts, app-scoped, with current provider support explicitly Claude-
only for these extensions. That honesty is good, but authored capabilities
need an owning lifecycle rather than another derived list.

Define one generation-owned state machine shared by Desktop, Pylon, managed
cloud, and Sync:

```text
declared → validated → staged → granted → active
              │          │         │        │
              └ invalid  └ rejected└ revoked└ superseded / rolled_back
```

Activation and revocation publish durable events. Clients receive public-safe
projections. Runtime support remains per-provider and must say unsupported
rather than silently emulate.

### H. Integrate runs, approvals, and receipts

Executor's proposed unified Run is directionally right. OpenAgents should not
introduce a second run table for authored tools. Attach every outer and nested
tool call to the existing Thread/Turn/Item/Work Unit and Blueprint/Fleet
lineage, with:

- parent/child invocation refs;
- artifact and catalog generation;
- input hash and bounded output/artifact refs;
- selected connection refs without secrets;
- policy and approval refs;
- actual containment receipt;
- nested tool events and external-provider evidence;
- retry/idempotency class;
- cancellation/interruption result; and
- delivery/acceptance outcome separate from execution completion.

An authored multi-step tool is not automatically a durable workflow. If the
process dies between side effects, exactly-once behavior has not appeared.
Durable workflows need explicit step admission, idempotency, retry,
compensation, checkpoint, and replay semantics.

## 10. Ordered implementation consequences

| Order | Change | Owning surfaces | Proof |
| ---: | --- | --- | --- |
| 1 | Freeze `AuthoredCapabilityArtifact`, requirement, binding, generation, and lifecycle schemas | `agent-runtime-schema`, `khala-tools`, Desktop extension contracts | Schema identity tests; incompatible/unknown fields fail closed; no secret-bearing fields |
| 2 | Model strict capability intersection and nested-call lineage | Khala dispatcher, policy, Blueprint/receipts | Bounded model/checker plus adversarial tests prove no widening, cross-owner binding, stale-generation call, or approval replay |
| 3 | Add brokered-function isolate conformance contract | local sandbox, Pylon, Cloud | Same fixtures pass local `workerd` and managed isolate; network/filesystem/process/secret escape tests fail; receipts name effective profile |
| 4 | Implement staged local-directory source and one fixture tool | Desktop/Pylon | Source → bundle → double collect → stage → approve → activate → invoke → revoke/rollback passes without restarting clients |
| 5 | Add pinned HTTPS Git source with provenance and update diff | Desktop/Pylon/component ledger | redirect/SSRF/token-redaction, unchanged-ref no-op, concurrent update conflict, deleted-tool tombstone, rollback tests |
| 6 | Project authored tools through canonical Khala dispatch | Khala tools, Runtime Gateway, Desktop/mobile | built-in, MCP, and authored tools share validation, policy, events, redaction, accounting, cancellation, and receipts |
| 7 | Add semantic lazy catalog selection | selector service, composer/model loop | semantic fixtures, grant-filtering, catalog-generation capture, no keyword-routing invariant test |
| 8 | Add optional Executor provider adapter | MCP/HTTP connector, Desktop/Pylon settings | version/health/auth/approval/offline fixtures; bounded tool import; outer policy; external custody/containment truth labeled honestly |
| 9 | Promote authored tools across devices and organization scope | Khala Sync, component ledger, web/mobile | owner-local default; signed promotion; revoke/update convergence; no source or secret body in public projections |
| 10 | Add durable workflows and rich outputs only after tool loop proof | Blueprint/workflows, stores, UI/artifacts | crash/retry/compensation and handle/delivery contracts, not a renamed long-running function |

This fits the existing adaptation order. It belongs under the isolated
extension/catalog work in consequence 6, while the optional Executor adapter
can land independently after the core MCP transport and grant contracts are no
longer phase-0 groundwork.

## 11. What not to copy

1. **Do not copy keyword-weighted tool routing.** It violates the workspace
   semantic-routing invariant and will become brittle as the catalog grows.
2. **Do not copy author safety annotations into policy.** `readOnly` and
   `destructive` are review hints, not trusted enforcement facts.
3. **Do not make an app/plugin integration its own authority universe.** Every
   nested call enters the one Khala dispatcher and receipt lineage.
4. **Do not claim the vision's `scope()` membrane before it exists.** Specify,
   model, test, and enforce intersection at the actual broker boundary.
5. **Do not use a host subprocess with a policy-looking environment variable
   as an authored-code sandbox.** Containment must be mechanically effective
   and receipted.
6. **Do not auto-activate Git head.** Pin, stage, show the authority/behavior
   diff, approve where required, and preserve rollback.
7. **Do not advertise every schema to every model.** Lazy discovery is right;
   captured generation and semantic selection must make it safe.
8. **Do not confuse a custom tool with a durable workflow.** Process death,
   retries, idempotency, compensation, and partial side effects remain separate
   problems.
9. **Do not merge source, artifact, friendly tool, connection, invocation, and
   run identities.** Each changes on a different lifecycle.
10. **Do not make Executor a required cloud dependency.** Local OpenAgents
    remains useful without an account, hosted Executor, or remote catalog.

## 12. Final recommendation

Executor is the best focused reference in the teardown set for the question:
"How does an agent safely create a new reusable capability from existing
capabilities?" Its answer is compelling:

- normalize every upstream operation into one catalog;
- make accounts explicit connection parameters;
- let authored code declare capability roles rather than hold credentials;
- compile and content-address the artifact;
- run it in a default-deny isolate;
- expose only a broker back into the normal tool dispatcher; and
- publish the result into the same catalog.

OpenAgents should build that loop. It should combine Executor's artifact
compiler and capability bridge with OpenAgents' stronger existing contracts:
Effect Schema identity, semantic selection, WorkContext, allow/ask/deny
toolsets, approval scopes, broker-only credentials, Blueprint/Fleet governance,
cross-device durable state, redaction, exact accounting, and separate
authority, containment, execution, delivery, and acceptance receipts.

Executor itself should remain an excellent optional neighbor. Connect to it
over MCP or HTTP when a user already has integrations there. Preserve its
connection and credential boundary, apply OpenAgents' outer policy, and label
foreign evidence honestly. Do not bury its SDK and storage model inside Khala.

The shortest accurate product statement is:

> Executor found the capability-production primitive. OpenAgents should adapt
> it into the receipted multi-agent operating system, not outsource the
> operating system to it.

## 13. Primary source map

All paths below are relative to the pinned Executor repository unless noted.

| Concern | Primary evidence |
| --- | --- |
| Product and destination | `README.md`; `vision.md` |
| Public Executor contract | `packages/core/sdk/src/executor.ts`; `packages/core/sdk/src/plugin.ts` |
| Owner partitioning and absence of scope stack | `packages/core/sdk/src/owner-policy.ts` |
| Policy and approvals | `packages/core/sdk/src/policies.ts`; `packages/core/execution/src/engine.ts`; `packages/hosts/mcp/src/tool-server.ts` |
| Code-mode kernel and discovery | `packages/kernel/core/README.md`; `packages/core/execution/src/tool-invoker.ts` |
| Authored-tool API | `packages/plugins/apps/src/authoring.ts` |
| Discovery and descriptor | `packages/plugins/apps/src/pipeline/discover.ts`; `packages/plugins/apps/src/pipeline/descriptor.ts` |
| Bundle/dependency boundary | `packages/plugins/apps/src/pipeline/bundler-driver.ts`; `packages/plugins/apps/src/pipeline/publish.ts` |
| Git/local source security | `packages/plugins/apps/src/source/git-source.ts`; `packages/plugins/apps/src/git-client/url-security.ts`; `packages/plugins/apps/src/source/local-directory-source.ts` |
| Connection-parametric bridge | `packages/plugins/apps/src/plugin/bindings.ts`; `packages/plugins/apps/src/plugin/resolver.ts` |
| Catalog projection and source handlers | `packages/plugins/apps/src/plugin/apps-plugin.ts`; `packages/plugins/apps/src/plugin/handlers.ts`; `packages/plugins/apps/src/plugin/store.ts` |
| Local isolate | `packages/kernel/runtime-workerd-subprocess/src/index.ts`; `packages/plugins/apps/src/executor/workerd-app-tool-executor.ts` |
| Cloud isolate | `packages/plugins/apps/src/executor/dynamic-worker-app-tool-executor.ts`; `apps/cloud/executor.config.ts` |
| Console/e2e | `packages/plugins/apps/src/react/`; `e2e/scenarios/custom-tools.test.ts`; `e2e/cli/custom-tools-packed.test.ts`; `e2e/desktop-vm/custom-tools-sidecar.test.ts` |
| Package-consumption boundary | `scripts/publish-packages.ts`; package manifests under `packages/core`, `packages/kernel`, and `packages/plugins/apps` |
| OpenAgents comparison | `packages/khala-tools`; `packages/agent-runtime-schema`; `packages/mcp-contract`; `apps/openagents-desktop/src/extension-lifecycle-contract.ts`; `packages/ai-sdk-sandbox-local` |
