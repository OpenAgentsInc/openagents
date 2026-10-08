# Cloud computers and repository environments

Status: first web integration implemented and published to staging on October 8,
2026; see the [deployment record](../deployment/openagents-web.md#october-8-2026-chat-and-cloud-composer-on-staging).
The native integration uses existing admitted operator Cloud jobs. Prepared environment versions, service
previews, scheduled wake, and general persistent-machine custody remain designed.
[The glossary](../glossary.md) defines the terms and their implementation status.
[Orbs research](../research/orbs.md) supplies the external reference; the
[Cloud web specification](coder-cloud.md) and
[repository onboarding contract](example-cursor-cloud-agent-onboarding/environment-onboarding.md)
remain the native product requirements.

## Product model

A chat keeps the conversation and references to Coder work. A project identifies
the authorized repository and planning scope. A computer executes work under a
current grant. An execution environment binds the runtime, source, credentials,
limits, and actual materialization. Choosing any of them grants no authority.
The account workspace supplies membership; it is distinct from a checkout and
the designed workspace resource namespace.

| External concept | OpenAgents term and owner |
| --- | --- |
| Amp thread | Thread or chat; ordered conversation and links to Coder tasks and ATIF. |
| Amp project | Project, repository source, and admitted checkout; existing project supervision owns planning. |
| Orb | Cloud computer with an execution environment; do not introduce another product name. |
| Runner | Resident host on an enrolled computer, with explicit workspace and device rights. |
| Prepared project snapshot | Verified environment version, separate from the source snapshot and mutable working checkout. |
| Portal | Application preview with current access, service readiness, and bounded disclosure. |
| Pause | Provider-specific stop or suspension. Boat restores filesystem state; it does not preserve running processes or RAM. |

Model inference, executor choice, and computer size remain separate. Hosted model
fallback supplies inference, not a computer or a retail entitlement. A browser
may supervise work started elsewhere; closing it detaches observation.

## Initial implementation

Connect the repository, branch/revision, and Environment controls beside the
homepage and chat composer to real choices and retained selections.

1. Public repository selection reads public metadata and resolves a branch to
   an exact commit. It supplies context for questions and grants no checkout,
   private repository access, model credential, or computer.
2. An authenticated operator selects only native projects and profiles admitted
   through the current resident binding. Optional repository and branch display
   metadata explain the profile; the existing source revision and digest remain
   authoritative. A profile is a base runtime, not a verified saved environment.
3. Retain the selected source/runtime pins with the chat and freeze them on each
   accepted request. Re-read native admission at review and confirmation. Changed
   source, profile, workspace, membership, or grant refuses the old selection.
4. Repository execution stages the existing exact native Cloud request. It uses
   the retained review and confirmation flow, then observes the canonical job
   and retained original output through authorized bounded reads using the
   chat's work reference. The public
   knowledge worker keeps its existing policy.
5. Follow-ups use native continuation and recovery on the same retained job.
   Current Boat continuation resumes its sandbox; completion stops it and checks
   that its meter stopped. This first slice does not keep a development server
   awake between turns. GCE continues to use its admitted shared pool.
6. Unconfigured native access opens a useful explanation and access route. A
   selected public repository never produces a simulated execution result.

Use Axum, Maud, HTTP commands, and SSE observations from the chat refactor.
Keep immediate input, drafts, caret, and scroll in the small Rust browser adapter.
Selection updates preserve the textarea. Stable request IDs, CSRF, native grants,
private DOM retirement, and original request recovery remain enforced.

## Prepared environments and managed machine lifecycle

The Cursor onboarding contract supplies the reusable environment owner:
inspect → recipe revisions → install/repair → clean builder → fresh verifier →
reviewed Save → immutable version → later task. Reuse its ENV implementation
slices; add no second setup or promotion system. An immutable version binds the
recipe, trusted base, source/dependency compatibility, inventory, provider image,
verification evidence, and original command output.

Boat is the first isolated provider. Reuse `crates/boat`, runtime packaging,
source transfer, and the Coder execution loop. Own never-replaced snapshot names,
retain resolved provider identities, verify restored manifests, and refuse drift.
Source transfer is a repository snapshot, not a machine image. Credentials must
be applied after restore and excluded from reusable images.

Persistent computer custody needs a native owner joining chat, project,
environment version, provider resource, grants, services, activity, and usage.
Its durable state distinguishes preparing, awake, stopping, stopped, resuming,
failed, and unknown. A child Coder turn finishing must not implicitly destroy
that computer. Record provider operation identities before further work; reconcile
lost responses without replacement dispatch. Snapshot completion, resource stop,
meter stop, deletion, and billing settlement remain separate facts.

Setup installs dependencies. Per-boot start/resume restores current credentials
and declared services. Reuse trusted worktree hooks where their existing scope
fits; do not silently reinterpret them as machine setup or resume hooks. Boat
filesystem restoration requires process restart and readiness checks. Choose idle
bounds from the admitted policy rather than copying Amp's timers.

GCE's two-slot pool is suitable for qualified jobs. Root installation, persistent
project services, and fresh environment verification require a dedicated isolated
builder/computer adapter before GCE can provide this lifecycle.

## Services, previews, and later capabilities

A native service owner binds a committed command, working directory, port,
environment names, health rule, logs, process ownership, and cleanup. Start it
per boot, retain readiness evidence, and restart it after filesystem restore.
A preview checks current membership, computer, service, port, and disclosure
before proxying. Keep provider access tokens server-side. Prefer one browser
origin for frontend and API services. A reachable port alone does not prove
application readiness.

Scheduled work uses durable automation occurrences outside sleeping computers.
Webhook acknowledgment means queued; handlers deduplicate and retain processing
outcomes. Graphical desktops need separate capture/input grants and a qualified
transport. These capabilities do not follow from selecting Environment.

Retail v1 is one ephemeral purchased task and forbids customer shells and reused
sandboxes. Persistent computers need a new frozen contract, price class,
reservations, metering, funding, and recovery before sale. Keep operator execution,
retail spending, and sponsored inference separate.

## Acceptance and publication

The first issue covers working selectors, frozen source/runtime identity,
existing native execution integration, targeted checks, and staging publication.
Test public metadata selection, invalid and stale choices, CSRF, chat refresh,
drafts, review fencing, original job recovery, and unconfigured availability.
Use an isolated host and provider resource for qualified execution; never use
an owner's personal checkout or ambient login. Record which paths are real,
synthetic, or unconfigured.

Public web chats use generation-fenced GCS records. Native reviewed requests
retain their existing file-backed journal and canonical resident jobs. Hosting
those controls requires a persistent custodian or a shared admitted journal;
ephemeral Cloud Run replicas do not supply it. Qualify restart and replica
recovery before enabling native execution on the hosted site.

Production promotion requires a later owner request. Follow the existing staged
Cloud Run process and preserve production traffic, sidecar, runtime, and game
assets. The native environment and persistent-computer slices retain their own
qualification; this interface does not mark them implemented.
