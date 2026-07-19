# Ascii Box, Optibox, and the OpenAgents Google Cloud sandbox seam

- Date: 2026-07-19
- Status: point-in-time teardown and architecture recommendation
- Dispatch: no; this document does not authorize implementation or deployment
- Upstream Box API snapshot: public OpenAPI bytes fetched 2026-07-19,
  SHA-256 `9ae1e0b7ded4a2d537bfa076f8e047baa2bdf7e3736de2cc397d349457c3cbac`
- Upstream TypeScript SDK inspected: `@asciidev/box-sdk@0.0.24`, MIT,
  npm integrity
  `sha512-w77vTWA+yrJ5O+FmchCkurjux1UZkQ5yeurnzX/FJTlQulEtj1xp0g/2cSh/GZWLXrgCV0exU99E+NyiilBeHA==`
- Optibox source snapshot:
  `ariana-dot-dev/optibox@88ba6e58cc99ed12602ca48495800ee8e96c74ab`
- OpenAgents source snapshot:
  `OpenAgentsInc/openagents@9f539ba23e`
- Live Google Cloud inventory observed read-only in project
  `openagentsgemini` on 2026-07-19

## Executive recommendation

**Do not replace the OpenAgents Google Cloud substrate with Ascii Box. Do use
the Box v1 API and SDK as a compact external compatibility target over the
OpenAgents workroom substrate.**

The fastest useful path is:

1. keep Google Cloud, OpenAgents workrooms, capability brokers, isolation,
   event truth, and receipts authoritative;
2. implement the small, high-value subset of the Box v1 HTTP contract behind
   an OpenAgents-owned base URL;
3. use the unmodified MIT TypeScript SDK as a black-box contract client by
   setting its supported `basePath` option to that URL;
4. map SDK calls into the existing `openagents.sandbox.v1` client seam and the
   Rust `oa-codex-control` / `oa-workroomd` substrate;
5. preserve native OpenAgents events and receipts beside the lossy
   Box-compatible projection; and
6. port Optibox's useful lifecycle ideas into the Effect service graph, but do
   not copy Optibox source unless its owner supplies a license.

This is materially faster than either building a new sandbox product from
scratch or adopting Ascii as the production control plane. The client contract
is small, generated, MIT-licensed, configurable, and already shaped around the
same operations OpenAgents exposes internally. The difficult part is not the
SDK. It is making lifecycle, tenant isolation, secrets, snapshots, events,
desktop URLs, cancellation, and cleanup truthful. OpenAgents already owns most
of those harder boundaries.

The first spike should be a **compatibility facade, not a vendor migration**.
It should admit only owner-local or explicitly managed-cloud test work until
its conformance and isolation gates pass.

## Evidence labels

- **`[source]`**: observed in upstream docs, OpenAPI, SDK package, or pinned
  source.
- **`[target]`**: observed in the pinned OpenAgents source or current repo
  contracts.
- **`[live]`**: observed from a read-only Google Cloud inventory query.
- **`[inferred]`**: an architectural conclusion from source and target
  evidence, not an upstream claim.
- **`[limitation]`**: a boundary on what this audit proves.

No Ascii account was created and no paid Box was provisioned. No live Box
latency, isolation, snapshot-integrity, or failure-recovery test was run.
Google Cloud resource existence does not prove workload readiness. [limitation]

## 1. What Ascii Box actually is

Box is a hosted Linux-computer control plane, not merely an `exec` API. Its
documented platform flow is: provision or resume a persistent machine, queue a
Codex or Claude Code prompt or run a custom harness, observe cursor-paginated
events, inspect files or a desktop, and stop/archive the machine into a
filesystem snapshot. [source]

The currently documented hosted machine is a Hetzner Cloud CX33 with 4 shared
vCPUs, 8 GB RAM, 75 GB local NVMe storage, and a curated development image.
Ascii's documentation explicitly treats that host choice as replaceable; its
snapshot abstraction is filesystem-oriented rather than a whole-VM image.
[source]

The public API has 27 method/path combinations across six concerns:

| Concern | Box v1 surface |
| --- | --- |
| Account | current user, limits, API-key metadata |
| Repository/setup | repository listing/selection, account secret setup |
| Lifecycle | list, create, inspect, update, stop, resume, fork, delete |
| Agent/runtime | prompt, prompt status, events, interrupt |
| Machine I/O | files, bounded commands, artifacts, desktop, SSH key |
| Persistence | list/latest snapshots, tree, file/folder, chunk download |

The API uses bearer authentication, HTTP-status authority, and `{ ok, type }`
success/error envelopes. It exposes explicit prompt-run state rather than
requiring clients to infer completion from machine state. [source]

### 1.1 The useful lifecycle

The strongest Box product idea is its deliberately boring lifecycle:

```text
create/fork -> provisioning -> ready/idle -> running
       ^                                  |
       |                                  v
     resume <- archived <- archiving <- stop
```

`stop` takes a final snapshot and pauses billing; `resume` restores it; `fork`
creates another machine from the latest completed snapshot. Enabled systemd
services return after restore while hand-run processes do not. [source]

Snapshots cover `/home/user`, Docker named volumes, and user changes under
system directories. They exclude memory, running processes, open ports,
network identity, and the base image. Ascii documents incremental snapshots
every minute plus a final snapshot on stop; a failed final snapshot aborts the
stop rather than discarding the running machine. [source]

That separation is worth adapting: durable filesystem state, runtime process
state, ingress state, and agent-session state are different resources. A single
`running` boolean cannot honestly represent all four. [inferred]

### 1.2 The platform pattern

Ascii's recommended platform pattern is one private, `noEnv` box per user or
project, resumed on demand and stopped after an idle window. It recommends
forking from a prepared template instead of reinstalling the stack for every
user. A custom harness can run behind a private in-box daemon, or callers can
use command/file endpoints directly. [source]

This is close to OpenAgents' intended managed-workroom shape, but Ascii's API
combines infrastructure convenience with product choices that OpenAgents must
not inherit blindly:

- machine TTL counts from creation and may stop mid-work;
- machine `idle`/`running` reflects Box prompt work, not arbitrary custom
  harness processes;
- desktop URLs may themselves contain bearer material;
- account-level secrets are copied into normal boxes;
- per-box environment values are raw values, not scoped broker leases; and
- a public VNC option can deliberately return an ungated URL. [source]

Those are valid Box features. They are not sufficient OpenAgents authority or
receipt semantics. [inferred]

## 2. The TypeScript SDK is a practical compatibility target

The SDK is an OpenAPI-generated TypeScript client with source included in the
npm tarball. Version `0.0.24` is MIT-licensed and ships ESM and CommonJS output,
model types, generated API methods, and handwritten wait/stream helpers.
[source]

Most importantly, the constructor officially supports a caller-selected base
URL:

```ts
const box = new BoxApi(new Configuration({
  basePath: process.env.BOX_BASE_URL ?? "https://ascii.dev/api/box/v1",
  accessToken,
}))
```

That means an application can point the unchanged SDK at
`https://openagents.com/api/box/v1` or a dedicated Google Cloud service without
forking the SDK. [source]

The helper layer is intentionally simple:

- readiness and idle waiters poll `GET /boxes/{id}`;
- prompt waiters poll first-class prompt status;
- `streamEvents` long-polls ascending cursor pages;
- `streamPrompt` queues one prompt and then polls prompt/response/tool events;
- file and command helpers wrap the corresponding deterministic endpoints.

No SSE or WebSocket implementation is required for SDK compatibility. That
reduces the facade's first milestone to ordinary authenticated HTTP plus a
durable cursor. [inferred]

### 2.1 What "point it at our API" does and does not buy

It buys:

- a ready-made, typed third-party client;
- stable method names and request/response models;
- a compact external developer experience;
- an executable compatibility corpus from generated SDK calls; and
- a way for examples such as Optibox to use OpenAgents infrastructure with a
  base-URL change.

It does not buy:

- server implementation;
- tenant authorization;
- idempotent provisioning;
- lease fencing;
- snapshot correctness;
- process supervision;
- network or filesystem containment;
- secret brokering;
- durable event truth;
- cleanup guarantees; or
- OpenAgents authority and receipt semantics.

The facade must reproduce the **observable contract**, not imitate names while
returning semantically incompatible state. [inferred]

### 2.2 Installation decision

Do not add `@asciidev/box-sdk` to the production runtime yet. Add exact
`0.0.24` only as a development dependency in an isolated compatibility-test
package once an implementation packet is admitted. Pin both the npm integrity
and OpenAPI digest recorded above. Use it to test our server from the outside;
do not make OpenAgents product code depend on vendor model types. [inferred]

OpenAgents' canonical schemas should remain Effect Schema. The compatibility
package should decode SDK JSON into those schemas at the HTTP boundary and
encode Box-shaped projections on return. This keeps the SDK removable and
prevents a generated camelCase model from becoming infrastructure authority.
[target] [inferred]

## 3. What OpenAgents already has

OpenAgents' target is more fragmented than Box's polished public API, but its
important internals are already stronger. [target]

### 3.1 Current source architecture

| OpenAgents component | Existing responsibility |
| --- | --- |
| `packages/ai-sdk-sandbox-openagents` | Thin AI SDK `HarnessV1SandboxProvider` adapter over an `openagents.sandbox.v1` client |
| `crates/oa-codex-control` | placement, GCE capacity, Codex runs, opt-in Firecracker Cloud-VM provisioning |
| `crates/oa-node` | managed-node identity, readiness, capability inventory, update/quarantine, assignment receipts |
| `crates/oa-workroomd` | workroom lifecycle, brokered capabilities, Codex auth materialization, runner events, artifacts, closeout |
| Cloud Run Worker/API | admission, owner/product state, event ingest, public-safe projections |
| Khala Sync / Cloud SQL | durable thread/runtime projections and cursors |
| GCS | private artifacts and raw-event archives |

The AI SDK adapter already defines the exact substrate interface a Box facade
needs: create/resume/stop/destroy session, read/write files, run/spawn process,
expose ports, and set network policy. It also binds session creation to snapshot
identity inputs including base image, repo, toolchain, agent setup, sandbox
profile, lockfiles, and bridge recipe. [target]

The adapter is not itself a server or provisioner. A production
`OpenAgentsSandboxV1Client` still has to connect those methods to the authoritative
control plane. [target]

### 3.2 Current Google Cloud truth

Read-only inventory on 2026-07-19 showed: [live]

- 26 Cloud Run services in `us-central1`; 25 reported their first condition as
  ready, while `openagents-runtime` did not;
- `openagents-monolith`, `khala-live-hub`, `khala-capture`, `oa-queue-worker`,
  `oa-updates`, and `oa-cloud-run-bridge` present as ready services;
- 43 GCE instances total, 26 running and 17 terminated;
- `agent-computer-gce-1` and `oa-codex-control-1` running;
- seven `pylon-gcp-*` machines running across several GCE machine classes;
- `khala-sync-pg` running on Cloud SQL PostgreSQL 17; and
- owned GCS buckets for artifacts, updates, build logs, training, and retained
  archives.

This proves that Google Cloud is not a paper destination. It does **not** prove
that a generic per-user sandbox API is ready, that Firecracker is live on every
host, or that `openagents-runtime` is healthy. The repo deliberately defaults
GCE and Cloud-VM adapters to fake modes, and live Firecracker remains explicitly
gated on Linux, KVM, and configured images. [live] [target] [limitation]

### 3.3 Existing sandbox and Cloud plans

OpenAgents currently has three distinct execution shapes:

| Shape | Status | Best use | Important boundary |
| --- | --- | --- | --- |
| Owner-local AI SDK sandbox | implemented fixture | local harness tests and owner-trusted work | explicitly not multi-tenant or kernel/network containment |
| Full ephemeral GCE session VM | contracted and deployed control-plane components | near-term isolated managed sessions | slower/costlier than warm microVM density; readiness must be observed |
| Firecracker workroom/Cloud-VM | implementation plus opt-in live lane | dense isolated guests and portable workrooms | live only with KVM/images; otherwise honest refusal/fake tests |
| Cloud Batch benchmark attempt | designed execution lane | bounded Terminal-Bench/SWE-style jobs | batch work, not an interactive persistent workroom |

The earlier compute-isolation plan intentionally launches with full ephemeral
GCE VMs, then moves toward Firecracker/gVisor/TDX only when measured gates are
met. The current invariant ledger also forbids silently substituting fake or
local execution for a requested managed isolated target. [target]

## 4. Comparison

| Dimension | Ascii Box | OpenAgents on Google Cloud | Recommendation |
| --- | --- | --- | --- |
| Developer API | coherent public REST plus TS/Python SDKs | multiple internal contracts and adapters | copy the coherence through a compatibility facade |
| Compute | current hosted Hetzner Linux VM | GCE full VMs; opt-in Firecracker; Cloud Run control services | retain GCP authority and provider abstraction |
| Persistence | incremental filesystem snapshots; stop/resume/fork | workroom artifacts, GCS, VM/image and portable-session contracts; no equivalent polished generic snapshot API | expose only proven snapshot operations; do not fake fork |
| Agent execution | built-in Codex/Claude prompt plus custom daemon/command path | native runtime adapters, Full Auto, Pylon, Codex workroom runner | map prompt to exact runtime selection and preserve native identity |
| Events | cursor-polled prompt/response/tool/lifecycle projection | richer native events, Sync cursors, raw/private archives, receipts | serve a Box projection beside native lossless events |
| Secrets | account/per-box env and files; `noEnv` scrubbing | scoped broker grants, session homes, Secret Manager, refs-only receipts | translate `noEnv`; reject raw account-secret replication |
| Network | hosted URL/desktop primitives; public VNC possible | named lane policy, deny-first public egress, receipt-bearing ingress | never map public VNC implicitly; require typed exposure authority |
| Cost control | per-second VM billing, TTL, stop/archive | GCP instance/service spend plus leases and usage receipts | adapt idle stop, but fence it on actual run settlement |
| Authority | account bearer controls Box operations | owner/work/unit/capability/target authority intersection | OpenAgents checks remain authoritative; bearer is authentication only |
| Portability | client can select `basePath`; snapshots intended host-independent | internal substrate intended to span owner-local and managed targets | make OpenAgents API provider-neutral, Google-backed now |

Box is ahead on API ergonomics and filesystem lifecycle. OpenAgents is ahead on
typed authority, capability brokering, native event retention, provenance, and
receipt design. The useful product is their intersection, not a wholesale
choice of one over the other. [inferred]

## 5. Optibox findings

Optibox is a 176-commit, private-package TypeScript prototype around Box. The
pinned repo contains a PostgreSQL-backed engine, a small direct Box HTTP client,
seven harness examples, lifecycle/event UI code, and detailed incident notes.
[source]

Its central interaction is **shared-first prewarm**:

1. a user sends a message;
2. private box start/resume begins immediately;
3. a restricted shared harness can answer or bridge while the box warms;
4. a warm box receives direct turns;
5. private harness work streams text and tool events;
6. an idle timer stops/archives the box after the harness actually settles.

### 5.1 Ideas worth adapting

- Persist one active private runtime per user and native harness-session IDs per
  conversation/harness.
- Use database advisory locks for user lifecycle and conversation turn order.
- Eagerly begin readiness work, but keep routing and machine state explicit.
- Treat "no visible output" as **not** equivalent to an idle agent loop.
- Arm idle stop only after a structural harness completion signal.
- Preserve an ordered UI event journal separately from cleaned model context.
- Stream lifecycle, tool, billing, and completion events with a turn ID.
- Keep shared/no-tool and private/tool-enabled execution structurally distinct.
- Make interrupt and native-session resume first-class across harnesses.
- Restart enabled services after resume rather than pretending processes were
  snapshotted.

These ideas match failures OpenAgents has already encountered in Full Auto and
mobile: hidden work, ambiguous waiting, duplicated turns, stale runtime state,
and lifecycle actions inferred from UI silence. [source] [target] [inferred]

### 5.2 Ideas to reject or narrow

- **Do not start private compute for every greeting by default.** Admission and
  cost policy should decide whether prewarm is justified.
- **Do not let a shared model make an unaudited hidden routing decision.** Use a
  typed semantic selector and show effective placement truth.
- **Do not inject provider keys as ordinary per-box environment values.** Use
  scoped, expiring broker redemption into a per-run process environment.
- **Do not keep an in-memory session store in any production path.** Optibox's
  current engine has moved to PostgreSQL for this reason.
- **Do not equate a custom Box HTTP client's state strings with process truth.**
  Optibox itself had to probe executable readiness because state lagged.
- **Do not adopt its raw UI event body as canonical evidence.** Keep a
  versioned native plane and a redacted UI projection.

Most importantly, the pinned Optibox repository contains no license file or
declared package license. Public readability is not permission to copy,
modify, or redistribute the source. Study its behavior and independently
implement the patterns unless the owner supplies a compatible license.
[source] [limitation]

## 6. Proposed Box-compatible OpenAgents API

Use a dedicated compatibility service or route group, for example:

```text
https://openagents.com/api/box/v1
```

The route group is an adapter over OpenAgents services, not a new authority.
Every request must resolve authenticated principal, tenant, work unit, target,
sandbox generation, capability lease, and current authority before touching
compute. [inferred]

### 6.1 Resource mapping

| Box concept | OpenAgents canonical concept |
| --- | --- |
| `box.id` | sandbox/workroom session ref plus generation |
| Box lifecycle state | public-safe projection of workroom, lease, guest, and process states |
| `ttlSeconds` | bounded lease deadline; never the only active-run stop rule |
| `noEnv` | immutable secretless capability policy |
| `env` | rejected by default or mapped to approved non-secret config; secrets use broker refs |
| repository selection | exact repository/ref grant and materialization receipt |
| prompt run | runtime turn/work unit with native harness identity |
| event cursor | durable projected event sequence; native cursor retained separately |
| command/file/artifact | sandbox capability operations with path and quota enforcement |
| desktop URL | short-lived receipted ingress capability, redacted at rest |
| snapshot | exact artifact/checkpoint identity with source generation and digest |
| fork | new workroom generation from an admitted checkpoint |

### 6.2 Phase 1: useful subset

Implement only:

- `GET /me`
- `GET /limits`
- `GET|POST /boxes`
- `GET|PATCH|DELETE /boxes/{boxId}`
- `POST /boxes/{boxId}/stop`
- `POST /boxes/{boxId}/resume`
- `POST /boxes/{boxId}/prompt`
- `GET /boxes/{boxId}/prompts/{promptId}`
- `GET /boxes/{boxId}/events`
- `POST /boxes/{boxId}/interrupt`
- `GET|PUT /boxes/{boxId}/files`
- `POST /boxes/{boxId}/commands`
- `GET /boxes/{boxId}/artifacts`

This is enough to prove SDK configuration, lifecycle, a real Codex/Claude turn,
incremental visible events, deterministic files/commands, cancellation, and
cleanup. It avoids claiming snapshots, fork, desktop, SSH, GitHub setup, or
account-secret parity before those semantics are proven. [inferred]

For unsupported endpoints, return a structured stable error such as
`501 capability_not_implemented`, never a fake success or empty list.

### 6.3 Phase 2: lifecycle parity

Add only after direct proofs:

- snapshot latest/list/tree/file/download;
- fork from an exact completed checkpoint;
- private desktop/preview URLs;
- bounded SSH key attachment;
- repository discovery/selection; and
- API-key metadata.

Account-wide raw secrets should remain intentionally incompatible. The facade
can support metadata and approved secret-reference attachment without exposing
or replacing whole secret sets. A difference from Box is acceptable when it is
typed, documented, and safer. [inferred]

### 6.4 Conformance suite

The compatibility package should:

1. install exact `@asciidev/box-sdk@0.0.24` as a dev dependency;
2. point `basePath` at a local fake, staging service, and owner-gated live GCP
   service in separate tests;
3. replay every admitted method, status, envelope, query, cursor, and error;
4. verify idempotent retries and conflicting-byte refusal;
5. fault create, resume, prompt, cursor, interrupt, stop, and teardown;
6. prove tenant and generation isolation;
7. prove secrets and desktop URLs never enter logs or receipts;
8. assert unsupported methods fail explicitly; and
9. pin the SDK, OpenAPI, translator, target contract, image, and provisioner
   identity in every receipt.

## 7. Effect architecture

The HTTP facade should be a thin Effect program:

```text
BoxV1Routes
  -> BoxV1Codec / BoxV1ErrorCodec
  -> BoxCompatibilityService
       -> WorkroomLease
       -> SandboxSession
       -> RuntimeTurn
       -> CapabilityBroker
       -> EventProjection
       -> CheckpointStore
       -> IngressCapability
  -> Google Cloud Layers today
```

Each service owns a typed error channel and a conformance test. Google Cloud is
the selected Layer, not a hard-coded fact in route logic. The existing
`OpenAgentsSandboxV1Client` can either become the first client implementation
behind `SandboxSession`, or be replaced by a narrower Effect Schema HTTP
contract while the AI SDK provider remains an adapter. [target] [inferred]

The facade must preserve two event planes:

- **native plane:** exact runtime/workroom events, authority decisions,
  receipts, and private evidence under existing retention policy;
- **Box-compatible plane:** text/tool/lifecycle projection with stable cursor
  and explicit translator version.

A Box SDK consumer should work normally without becoming the canonical record
of what happened. [inferred]

## 8. Fastest honest implementation sequence

### Packet A — contract harness (one to two days)

- Create an isolated compatibility-test package.
- Pin SDK `0.0.24` and the OpenAPI digest.
- Generate fixtures for admitted success/error envelopes and event cursors.
- Implement a fake server and prove the unmodified SDK against it.

Exit: client compatibility is executable, not prose.

### Packet B — GCP lifecycle adapter (two to four days)

- Implement create/get/stop/resume/delete over the current sandbox/workroom
  client and GCE target.
- Bind every box ID to owner, work unit, target, generation, and lease.
- Prove stop and teardown idempotency under disconnect and duplicate request.

Exit: a real owner-gated GCP sandbox can be created, stopped, resumed, and
destroyed with receipts.

### Packet C — real harness turn (two to four days)

- Map prompt/status/events/interrupt to the current runtime turn contract.
- Run one real Codex and one real Claude turn.
- Stream actual tool and text events through the SDK cursor helper.
- Preserve provider/model/runtime truth and native events.

Exit: `streamPrompt` shows what the runtime is doing as it happens and terminal
state reconciles exactly.

### Packet D — machine I/O and hardening (two to four days)

- Add bounded file, command, and artifact operations.
- Enforce path, process, output, time, egress, and quota limits below the SDK.
- Fault broker outage, guest crash, stale generation, expired lease, and
  teardown failure.

Exit: the facade is useful for harness frameworks without weakening sandbox
or authority policy.

### Packet E — snapshot/desktop only after proof

- Add exact checkpoint identity and fork.
- Add private desktop/preview ingress with short TTL and revocation.
- Keep public/ungated VNC unsupported unless separately authorized.

Exit: no endpoint claims parity beyond its observed implementation.

## 9. Decision

Ascii Box demonstrates the API OpenAgents should make easy: one durable
sandbox resource, one lifecycle, one event cursor, deterministic machine I/O,
and one SDK configurable to any base URL. Optibox demonstrates why the runtime
behind that API needs durable locks, native session IDs, real completion
signals, and visible lifecycle events.

OpenAgents should therefore build **Box API compatibility over its existing
Google Cloud and workroom substrate**, not rebuild its substrate around Box and
not fork the SDK prematurely. Install the MIT SDK only in an isolated contract
test package, retain Effect Schema as canonical, and independently port the
unlicensed Optibox ideas that survive OpenAgents' stronger authority,
isolation, and receipt requirements.

That gives us a familiar external SDK quickly while continuing to own the part
that matters: where work runs, what it can access, what actually happened, and
who had authority to make it happen.

## Sources

Primary external sources:

- [Ascii Box quickstart](https://docs.ascii.dev/box/quickstart)
- [Build a Platform on Box](https://docs.ascii.dev/box/platform-guide)
- [Box Public API v1](https://docs.ascii.dev/box/api/v1)
- [TypeScript and JavaScript SDK](https://docs.ascii.dev/box/sdks/typescript)
- [Machine capabilities](https://docs.ascii.dev/box/machines)
- [Template boxes](https://docs.ascii.dev/box/templates)
- [Snapshots](https://docs.ascii.dev/box/snapshots)
- [Secrets and setup](https://docs.ascii.dev/box/secrets)
- [Long-running tasks](https://docs.ascii.dev/box/long-running-tasks)
- [Box v1 OpenAPI](https://docs.ascii.dev/openapi/box-v1.yaml)
- [Optibox source](https://github.com/ariana-dot-dev/optibox)

OpenAgents evidence:

- [`../cloud/README.md`](../cloud/README.md)
- [`../cloud/ARCHITECTURE.md`](../cloud/ARCHITECTURE.md)
- [`../cloud/INVARIANTS.md`](../cloud/INVARIANTS.md)
- [`../cloud/benchmarks/2026-06-13-compute-isolation-benchmark-plan.md`](../cloud/benchmarks/2026-06-13-compute-isolation-benchmark-plan.md)
- [`2026-07-17-ai-sdk-v7-harnesses-teardown.md`](./2026-07-17-ai-sdk-v7-harnesses-teardown.md)
- `packages/ai-sdk-sandbox-openagents`
- `crates/oa-codex-control`
- `crates/oa-node`
- `crates/oa-workroomd`
