# Repository environment onboarding

Status: proposed specification, October 8, 2026. No implementation or production
availability is claimed by this document. The source inventory was checked
against `a28e2c899203d9e4962967e522a8f879aef5e8a8`; issue states were read
on October 8. The [Cursor workflow analysis](cursor-environment-onboarding-analysis.md)
and [ordered tool ledger](cursor-environment-onboarding-tool-sequence.md)
supply the observed reference process.

Implement a guided setup conversation beside a reviewable environment panel.
The setup agent discovers the repository, creates and repairs an install/start
recipe, builds an immutable candidate, verifies it in a separate fresh machine,
and presents **Save environment**. Saving selects the exact verified version
for later project tasks. Native records own every stage; the conversation and
clients explain and project those records.

The first release targets an admitted operator project on a dedicated Boat
sandbox, Linux x86-64, and the currently qualified headless Coder/Codex adapter.
Keep the provider, executor, source, privileges, credentials, output capacity,
deadline, and spend bounds explicit. Other architectures, engines, providers,
and customer-funded setup require their own qualification.

## Relationship to issue #10964

[#10964](https://github.com/OpenAgentsInc/openagents/issues/10964) is the remaining
Coder Cloud web implementation roadmap. Its linked children provide useful
building blocks, but it has no repository environment recipe, immutable version,
build/verifier lifecycle, or environment Save owner. This proposal adds that
native scope and then uses the web work already completed under the roadmap.
It does not mark an existing open web issue implemented.

| Issue | State at review | Use in this process |
| --- | --- | --- |
| [#10943](https://github.com/OpenAgentsInc/openagents/issues/10943) | Closed | Shared Rust Native components/catalog for chat, status, editors, review, and evidence. |
| [#10948](https://github.com/OpenAgentsInc/openagents/issues/10948) | Closed | Honest proposed/unavailable/qualified availability on the Cloud entry. |
| [#10949](https://github.com/OpenAgentsInc/openagents/issues/10949) | Closed | Authenticated workspace, native membership, private view cleanup. |
| [#10950](https://github.com/OpenAgentsInc/openagents/issues/10950) | Closed | Canonical task/child/evidence observation and original bytes. |
| [#10951](https://github.com/OpenAgentsInc/openagents/issues/10951) | Closed | Reviewed original native requests, revision fences, and lost-reply recovery. |
| [#10952](https://github.com/OpenAgentsInc/openagents/issues/10952) | Closed | Project snapshots and operator Cloud jobs; the initial execution/observation boundary. |
| [#10953](https://github.com/OpenAgentsInc/openagents/issues/10953) | Closed | Granted terminals and canonical workbench navigation, if setup-terminal access is separately admitted. |
| [#10956](https://github.com/OpenAgentsInc/openagents/issues/10956) | Open | Retail browser delegation/custody; relevant only to a later customer contract. |
| [#10970](https://github.com/OpenAgentsInc/openagents/issues/10970) | Open | Authenticated retail purchase/recovery; not an environment registry or saved-machine contract. |
| [#10963](https://github.com/OpenAgentsInc/openagents/issues/10963) | Open | Browser packaging, reconnect/accessibility acceptance, and activation record. |

Native Cloud issues #10912–#10917 are closed: durable remote jobs, Boat and GCE
adapters, source transfer, terminal/CLI projection, and runtime images exist.
The current resident operator bridge still qualifies only `mode=coder`,
`executor=codex`, with explicitly selected `OPENAI_API_KEY` or
`OA_CODEX_AUTH`; only `GH_TOKEN`/`GITHUB_TOKEN` are additional admitted
credential names. Generic Boat integrated-agent support does not qualify a
browser adapter for that mode. See
[the adapter admission](../../../crates/coder-cloud/src/operator_adapters.rs)
and [the current Cloud runbook](../README.md).

These closed issues establish code scope, not customer availability. Existing
owner qualification in NEEDS_OWNER.md remains applicable. This document creates
no issue claims and opens no new implementation issues; the slices below are
proposed work to register after review.

## Ownership and reuse

Add a Rust domain owner, provisionally `crates/coder-environment`, with portable
DTOs/state transitions and a host feature for storage and provider coordination.
Keep source materialization, execution, provider resources, task conversations,
project bindings, authority, and ATIF in their existing owners. The new crate
joins their identities and results; it does not implement another agent loop,
VM service, account store, permission system, or billing book.

| Capability | Implemented owner and source | Reuse or required addition |
| --- | --- | --- |
| Setup conversation and steering | [Canonical task](../../../crates/coder/src/task.rs), [task-owner contract](../../coder/runtime/task-owner.md) | Reuse task identity, immutable intent, runs, follow-ups, and controls. Add explicit environment-purpose/link DTOs in the environment owner. |
| Project/source admission | [Operator profile](../../../crates/coder-cloud/src/operator.rs) | Reuse project/workspace, source revision/digest, paths, executor, bounds, credential-file bindings, and current assignment checks. |
| Repository snapshot | [Workspace capture/restore](../../../crates/coder-cloud/src/workspace.rs) | Reuse archive/patch/include materialization. This snapshot transfers source; it is not a machine image. |
| Durable remote work | [Record and driver](../../../crates/coder-cloud/src/lib.rs) | Reuse the atomic per-job store and leases, provider task/resource IDs, cursors, reconciliation, usage, and cleanup. Add command/build operations rather than treating install as ordinary task success. |
| Isolated machine and snapshot | [Boat SDK](../../../crates/boat/README.md) | Reuse create/fork/stop/resume, idempotency keys, streaming/detached commands, hydration, named snapshots, and usage. |
| Trusted base/runtime | [Shared host installer](../../../scripts/cloud/coder-host-setup.sh), [runtime packaging](../../../scripts/cloud/build-coder-runtime.sh) | Reuse the commit-pinned portable runtime and no-login image preparation. The installer's apt, Node patch, and engine package selection also need retained resolved versions; they are not fully pinned package inputs. Repository recipes layer on an immutable base. |
| Daily reusable images | [Boat template builder](../../../crates/boat-template/src/main.rs), [template runbook](../../deployment/boat-template.md) | Extract/reuse build, stop, snapshot, readiness, and fresh probe behavior behind a durable native adapter. The daily global template is not the project environment registry. |
| GCE base image | [GCE bake runbook](../../deployment/coder-host-image.md), [pool](../../../crates/coder-cloud/src/pool.rs) | Reuse for trusted image updates. Repository setup requires a dedicated builder before this is a supported backend. |
| Build/resource control | [Leases](../../coder/runtime/leases.md), [capacity](../../coder/runtime/capacity.md) | Reuse build slots, target/disk bounds, receipts, provider capacity, and reset behavior. Add combined setup/build/verifier resource budgets. |
| Independent qualification | [Protected artifact checks](../../coder/guides/artifact-verification.md), [fresh Boat probe](../../deployment/boat-template.md) | Reuse frozen check identities and separate execution. Add an environment verifier adapter on a writable fresh image fork. |
| Tool evidence | [ATIF](../../../crates/atif), [traces](../../coder/runtime/traces.md) | Reuse schema and append-only lifecycle. Add full streamed payload storage and completeness accounting below existing capture limits. |
| Browser/application check | [Browser helper](../../coder/guides/browser.md) | Reuse fresh profile, selected port, process ownership, and screenshots. Add recipe service/readiness/cleanup contracts. |
| Original artifact reads | [Operator OriginalChunk](../../../crates/coder-cloud/src/operator.rs) | Reuse digest, length, offset, prefix, admission, and bounded paging. Extend the artifact allowlist/owner for environment evidence. |
| Web workspace and controls | [Operator views](../../../crates/openagents-web/src/cloud/operator.rs), [controls](../../../crates/openagents-web/src/cloud/controls.rs), [host bindings](../../../crates/openagents-web/src/cloud/hosts.rs) | Project native environment records and signed requests through current sessions/bindings; add no browser execution authority. |
| Save, history, and task selection | No environment lifecycle owner exists | Build immutable versions, verification/promotion records, project default selection, rollback, retention, and pinned job resolution. |

### Backend limits that affect the design

Boat is the first adapter because a candidate and verifier can each own a
machine. Its SDK already supports the lifecycle. Commands and prompts are
not automatically retried; `boat_direct_failed` can mean work is running.
The environment owner must reconcile the retained command identity before
trying another operation.

Current convenience command paths lose information:
[Boat::command](../../../crates/coder-cloud/src/boat_backend.rs) returns stdout
only for success/exit zero and a generic error otherwise; the
[GCE command path](../../../crates/coder-cloud/src/gce_backend.rs) discards stderr.
Use the lower-level streaming/follower facilities and extend the native
observation contract before using these paths for install evidence.

The GCE pool admits two runs per host. Its current job Spec uses the pool image
and shape and refuses per-job templates. Apt/root installation in that pool can
alter another run's machine. Keep GCE setup unavailable until dedicated
isolated builders, immutable output images, restore verification, and cleanup
are admitted. Do not add a setup escape hatch to a shared worker.

Boat template names can be replaced, and GCE families resolve moving images.
The saved version must retain the resolved provider identity and manifest
digest. A mutable alias may help selection; it must not define what a verifier
boots or what a recorded task runs.

Boat's [create request](../../../crates/boat/schema/boat-v1.yaml) accepts a named
snapshot in `from`; the API exposes `snapshotId` for artifact inspection but
does not establish direct deployment by that ID. Qualify immutable selection
in ENV-04. The initial adapter should own a unique, never-replaced name for
each candidate/version, retain its underlying snapshot ID, fence and check
the name-to-artifact mapping around allocation, and verify the restored
manifest. Refuse drift. Recording a snapshot ID alone cannot make an alias
restore immutable.

The [interactive runtime template](../../../crates/boat-template/src/main.rs)
excludes `.cache/`, `.cargo/`, `.rustup/`, and `openagents/` through
`~/.boxignore`. Inventory and reconcile inherited exclusions before capture;
declared repository outputs and dependencies must survive the snapshot.
Retain the exact capture policy and verify required paths on the fresh fork.
Snapshot storage preserves admitted filesystem state, not running processes
or memory; services need per-boot startup. See the
[snapshot scope](../2026-10-02-boat-sdk-plan.md).

## User process

1. **Choose project and source.** Open **Environments** from the authorized
   project. Select a repository, exact revision, admitted local changes if
   supported, and a qualified execution profile. Show an existing saved
   environment, or **No environment configured**.
2. **Set up environment.** Enter the objective and select the qualification
   profile, services/ports, required package registries, bounded credentials,
   resource/deadline limits, and permitted repository effects. The native owner
   retains admission before allocation. Existing authorization covers routine
   commands within those effects.
3. **Discover.** The setup task inspects manifests, instructions, current
   inventory, workspaces, caches, and application entry points. It produces a
   retained plan and proposed acceptance checks. Show progress beside the chat.
4. **Edit recipe.** Show install, per-boot start, working directory, environment
   variable names, readiness checks, and exclusions in an environment panel.
   Every saved edit increments the draft revision. Values for credentials
   remain private references, not recipe text.
5. **Install and repair.** Run within the dedicated setup machine. Preserve
   command outputs and failures. The agent may revise the draft and rerun the
   recipe without discarding the earlier attempts.
6. **Demonstrate.** Execute the frozen qualification plan. A service profile
   starts its declared services and exercises its application path. A library
   profile can have no service, but is labeled toolchain/library qualification
   rather than application end to end.
7. **Prepare candidate.** Retain recipe/inventory/evidence outside the machine.
   Stop admitted transient services and remove ephemeral credential mounts.
   An exploration snapshot is optional and labeled as such.
8. **Build.** Create a builder from the exact trusted base and recipe/source
   identities. Execute the recipe and seal its output image. A captured
   exploration snapshot is an optional acceleration input with explicit lineage,
   not a substitute for clean-base qualification.
9. **Verify fresh machine.** Fork a separate machine from the exact sealed
   output image. Check image identity/hydration, versions, locked/offline
   dependencies where required, representative behavior, startup/readiness,
   and declared application interactions. Retain a separate verifier task
   and machine record. No verifier repairs the candidate being qualified.
10. **Review result.** Show recipe diff, source identities, build/verifier
    results, inventories, full authorized output, omitted checks, evidence
    completeness, cost/usage, and cleanup. **Ready to save** requires the
    declared qualification; an agent summary cannot grant it.
11. **Save environment.** Submit a reviewed native promotion request binding
    the draft revision, recipe digest, image identity, check plan/result, and
    project default revision. Editing any bound input makes this request stale.
12. **Start future work.** Resolve the selected saved version before provisioning
    intent is recorded. Show the environment version and actual image in the
    new task. Run the version's per-boot start/readiness operations as needed.
13. **Revise or roll back.** Create a new draft from a saved version, retaining
    history. Promotion changes future selection. Existing tasks retain their
    pinned version. Rollback selects an earlier qualified version through the
    same reviewed native owner.

The conversation continues to support steering and follow-ups throughout.
Closing a page detaches observation and does not cancel the native operation.
Stop, provider acknowledgment, deletion, and final usage remain separate facts.

## Records and identities

All names and schemas in this section are proposed contracts. Use stable opaque
IDs, explicit schema versions, canonical digests, private stores, atomic writes,
and bounded DTO pages. Large evidence payloads live in separate blobs.

| Record | Required retained contents |
| --- | --- |
| Environment | Owner workspace/project, repository identities, authority policy reference, current selection revision, active immutable version, history and retirement state. |
| SetupSession | Environment/draft IDs, canonical setup task/run references, source/admission digest, executor/profile, machine/resource/credential policy, deadline and combined budget, all attempts and cleanup. |
| RecipeRevision | Exact install/start bytes, source/toolchain/lock inputs, working directory, base/runtime/architecture pins, network and secret-name policy, check plan, parent revision and digest. |
| BuildAttempt | Frozen input manifest, builder job/resource, stable provider operation IDs, command IDs/results, sanitization outcome, input/output image identity, status, usage, cleanup, and evidence references. |
| EnvironmentVersion | Immutable recipe/source/base/runtime/input digests, sealed provider image identity, inventory, qualification tier, accepted verifier results, evidence manifest, creation and retention policy. |
| VerificationAttempt | Sealed build/candidate manifest, independent verifier task/job/machine, exact frozen checks, observations, verdict, full outputs, service cleanup, provider cleanup, and usage. |
| Promotion | Request identity, actor/policy, expected environment/draft/project-selection revisions, exact version and verification identity, native outcome, and selected-pointer transition. |
| StartupAttempt | Work task/job and pinned environment version, restore/hydration evidence, actual source identity, each per-boot service/readiness result, terminal outcome, and cleanup responsibility. |
| EvidenceManifest | Parent/child linkage, calls started/completed/unresolved, events and byte ranges, payload digests/lengths, redaction/truncation/gaps, artifact index, finalization, and source provenance. |

A source snapshot and an environment image have different identities. An
environment may be reused with a later source commit, but it must record the
recipe's source baseline and dependency compatibility. A changed toolchain,
lock digest, OS/architecture requirement, or accepted service contract must
trigger explicit requalification or an honest incompatible/unchecked state.
Do not mutate a saved image while preparing later work.

Verification targets a sealed `BuildAttempt` candidate and image manifest.
Create the final immutable `EnvironmentVersion` only after accepting the
required verifier evidence. Further checks create separate attempts or a new
version; they do not rewrite accepted evidence inside a saved version.

The current [source restore](../../../crates/coder-cloud/src/workspace.rs)
reinitializes Git and commits a synthetic `Cloud workspace input` revision.
The retained source manifest establishes provenance; remote `HEAD` equality
does not prove the original commit. Recipes requiring original Git history,
remotes, or submodules need a separately admitted clone/materialization adapter.

A proposed recipe envelope includes:

```json
{
  "schema": "openagents.environment.recipe.v1",
  "repository": {"url": "<authorized repository>", "revision": "<exact commit>"},
  "base": {"provider": "boat", "image_id": "<resolved immutable identity>", "digest": "<manifest digest>"},
  "runtime": {"revision": "<Coder runtime revision>", "digest": "<artifact digest>"},
  "platform": {"os": "linux", "architecture": "x86_64"},
  "install": {"cwd": "<admitted source path>", "artifact": "<script blob>", "digest": "<script digest>"},
  "start": {"services": [], "readiness": []},
  "inputs": {"toolchain": "<digest>", "locks": ["<path and digest>"]},
  "credential_names": [],
  "qualification": {"profile": "rust-library", "plan_digest": "<frozen checks>"},
  "limits": {"deadline_seconds": 3600, "concurrent_machines": 2, "total_machine_allocations": 4, "output_bytes": 268435456}
}
```

The limits above illustrate fields, not default grants or quoted prices.
Provider capabilities, local policy, and capacity determine admitted values.
Count setup, builder, verifier, optional idempotence forks, and replacements
against the total allocation budget as well as the concurrent cap.
A recipe is inert data until the native owner accepts a matching execution
request. A repository file can supply defaults, but cannot grant privileges,
credentials, network recipients, publication, or spend.

## State and operation contract

Keep agent execution, recipe, build, verification, promotion, and cleanup as
separate state machines. A compact UI stage can summarize them, but the DTO
must preserve the underlying facts.

| Operation | States and terminal observations |
| --- | --- |
| Setup | Draft, admitted, provisioning, discovering, installing, repairing, awaiting input, ended/failed/cancelled. Ending setup does not verify or save. |
| Build | Requested, provisioning, installing, preparing image, snapshot pending, ready, failed/cancelled/needs reconciliation. Image-ready is distinct from install exit zero. |
| Verification | Requested, restoring, checking, passed/failed/incomplete/cancelled/needs reconciliation. Required checks and evidence completeness determine passed. |
| Promotion | Requested, admitted, selected/rejected/needs reconciliation. It binds a qualified immutable version and expected current selection. |
| Cleanup | Not started, requested, acknowledged, complete/failed/unknown. Cleanup may lag a task or build result. |
| Evidence | Recording, finalized complete, finalized with redactions, incomplete/gapped, unavailable. Execution completion cannot imply complete evidence. |

Persist intent and stable request identity before each provider side effect.
Store the original response before emitting it to clients. Repeated requests
return the retained operation. On host restart or lost reply, query/reconcile
the original builder, snapshot operation, verifier fork, or promotion.
Never retry an ambiguous install command by running it again without knowing
whether the first invocation is still active. Bound every wait and preserve a
resumable checkpoint.

Use per-record leases plus optimistic revision fences to serialize draft edits
and publication. Two saves against the same expected project selection cannot
both win. Repeated save of the same request is idempotent. The provider snapshot
may exist before promotion; a failed or lost save does not recreate the image.

### Proposed agent and native operations

Expose domain tools over the same native owner used by CLI and clients.
These names describe proposed behavior, not existing MCP or HTTP endpoints.

| Operation | Inputs and result |
| --- | --- |
| environment.inspect | Current draft/version, machine inventory, source/runtime identity, capabilities, accepted policy, and existing build/verifier references. |
| environment.recipe.update | Expected draft revision, script/check artifact digests; returns a new immutable recipe revision and invalidated eligibility. |
| environment.command.start/status/output/stop | Exact command/cwd/permit/deadline, stable command ID, separate stdout/stderr byte reads, direct process outcome, acknowledgment, and uncertainty. |
| environment.capture.start/status | Setup-machine identity, expected recipe revision, sanitization/checkpoint policy; returns an optional exploration image with scope and readiness. |
| environment.build.start/status/logs | Frozen input manifest and budget; returns retained build identity and paged original result payloads. |
| environment.verify.start/status | Exact sealed build/candidate manifest and frozen plan; returns separate task/machine/check identities and typed verdict. |
| environment.propose | Exact version plus evidence; returns a review object. The setup agent can propose but does not invent or bypass publication authority. |
| environment.promote/reconcile | Original reviewed request, expected project/environment revisions, qualified version; returns the retained selection outcome. |
| environment.retire | Reviewed identity/reference policy; stops future selection and schedules provider deletion only when permitted by live references/retention. |

The first environment tools can use the operator's admitted Coder/Codex path.
Do not enable every engine merely because the Boat SDK can dispatch it.
The setup/build adapter must admit system-install effects only on the dedicated
builder, with ephemeral credentials, appropriate network policy, and its own
resource limits. Source-candidate checking retains its existing narrower
authority.

## Recipe build, startup, and qualification

Separate trusted base image preparation, repository installation, and per-boot
startup. The shared cloud installer already knows OpenAgents toolchains,
runtime packaging, warm targets, and image manifests. Do not fork that logic
into browser handlers or a new repository-specific base installer.

The setup conversation can create arbitrary admitted repository shell recipes;
product orchestration, identities, permissions, storage, and transitions stay
in Rust. Shell and retained Python tooling remain infrastructure exceptions.

Initial qualification requires these facts:

1. **Identity:** the builder/verifier agree on the exact provider image,
   manifest, architecture, runtime, recipe, source, and frozen check plan.
   Provider readiness must include actual restored-file hydration. The current
   boat-fork helper can print `fork_ready_restored=false` with exit zero;
   use the typed readiness result rather than its process exit alone.
2. **Clean-base install:** the recipe succeeds from the admitted trusted base.
   Reusing a repaired exploration disk is labeled accelerated and does not
   alone prove this property.
3. **Fresh restore:** a different machine restores the output image and obtains
   the required tools/dependencies without help from the setup machine.
4. **Idempotence:** after checking the untouched restored candidate, rerun the
   exact recipe on a disposable fork and check declared inventory/inputs.
   Reinstallation cannot repair missing dependencies before the baseline
   fresh-image checks or mutate the sealed candidate. Retain both invocations.
5. **Behavior:** representative declared commands exercise the intended work.
   An empty test filter or exit zero without assertions is not a check pass.
6. **Startup/application:** required services start under tracked process
   ownership, readiness succeeds, and the declared CLI/browser flow works.
   For a library-only profile, this is explicitly not applicable.
7. **Evidence:** required command results and artifacts are retained, paired,
   finalized, and available through current authority. Omitted checks and
   redactions are explicit.
8. **Cleanup:** verifier and builder cleanup obligations and usage are retained.
   Unknown cleanup is visible and blocks further allocations according to
   policy; it is never labeled deleted because the conversation ended.

Freeze required checks before build/qualification. Pin executable check
artifacts and capability manifests, and protect them outside setup/build write
grants, as the [task-owner contract](../../coder/runtime/task-owner.md) requires.
A plan-text digest alone does not protect its implementation. A setup agent may
propose changes after discovering an unsuitable plan, but the owner retains a new plan
revision and requires a new corresponding verification. The verifier executes
the plan and reports faults; it cannot install a missing dependency and then
declare the original image passed.

For this repository, a first approved profile can target the root Rust
workspace and representative atif/coder-lease checks. A service profile must
also choose a real service configuration and application interaction using
isolated state and test credentials. Gateway, relay/PostgreSQL, Verse/graphics,
and mobile hosts have different prerequisites; no one profile should silently
claim them all. The phone workspace's lock problem in the Cursor example is a
historical limitation to recheck, not a permanent exclusion.

Run heavy Cargo work through `openagents lease build --keep-target-dir`,
keep a long-lived target slot outside the worktree, and honor capacity/disk
admission. Use `openagents browser run` for a fresh profile/selected port.
Declare service dependencies, process groups, timeouts, port/readiness policy,
screenshots, browser assertions, and stop cleanup in the check plan. Do not
leave verification tasks in ordinary user lists after isolated qualification.

Startup executes after each relevant restore/provision, not only during the
image build. Retain readiness results in the work job. A saved version whose
startup fails cannot make that job appear ready. Desktop/Chrome/GPU packages
are selected by profile; a headless build environment need not carry a VNC
desktop.

## Complete tool and build evidence

This is a prerequisite for the requested onboarding experience, not an optional
analysis export. The Cursor exercise needed several overlapping sources to
recover returned responses, and one call still lacks a result. OpenAgents must
record complete authorized output as the work occurs.

Existing retention is useful but insufficient for that promise:

- Coder shell capture is 16 KiB upstream of ATIF; the model sees an even smaller
  prefix. ATIF records the bytes the shell held, not bytes already discarded.
- The local task owner grants bounded per-stream capture.
- Cloud records cap individual events at 1 MiB and the record near 32 MiB;
  the driver stops work on overflow.
- Runtime ATIF collection accepts files below 8 MiB. Its normalized-event
  fallback is a derived presentation and cannot recover missing original bytes.
- Current operator artifact reads allow four standard job artifacts, not an
  arbitrary full command/output archive.

Add a private append-only recorder and content-addressed payload store.
Scrub output before disk spooling. Preserve sufficient rolling overlap for
each stream to detect selected credential values split across chunks; apply
the same policy to structured arguments/results. Spool stdout and stderr
incrementally, acknowledge only persisted byte ranges,
and link payloads from ATIF/evidence DTOs. Keep model-visible excerpts and full
retained output separate. Page-size/UI limits must never silently cap archives.
Admission declares the output/retention budget; exhaustion produces an explicit
incomplete result and promotion refusal, rather than silently claiming completeness.

Every call record includes its stable ID, parent/task/run identity, tool and
arguments, source request, start/end timestamps, selected backend operation,
stdout/stderr/result payload references, byte lengths/digests, direct exit or
signal/deadline result, and redaction/truncation/gap/capture-error status.
Every started call ends with a real result or an explicit unresolved/unknown
entry. Do not transform a still-running event into successful completion or
cancellation merely to close a trace.

Credential values must not enter source, logs, traces, fixtures, or public
artifacts. Inject credentials after restore through existing private runtime
custody. Record the redaction fact without retaining the secret. Blob lengths,
digests, and read cursors describe the retained post-redaction bytes. Promise
complete authorized results after
declared redaction, and distinguish them from byte-identical originals.
Do not snapshot engine login homes, provider keys, Git tokens, or transient
session files. A credential-bearing setup session must remove private mounts
and pass the snapshot exclusion policy before capture; trusted builders can
run recipe commands without a model login in their reusable filesystem.

Retain explicit links to the child verifier and its complete evidence.
A child ID or prose link is not transcript completeness. Use original-byte
read contracts with immutable digest, total length, byte cursor, prefix digest,
and current observe authority. A damaged or unclosed ATIF log remains faulty,
as its existing reader requires. Export a single bundle/manifest containing
recipe, inventory, source patch, command results, build logs, image identity,
verifier checks/outputs, promotion, usage, and cleanup. Archive first; SSE or
other client streams are delivery projections with resumable cursors.

## Web and client projection

Add an environment route under the authenticated project; its exact URL and
transport contract are implementation details to freeze with the native DTOs.
The page composes the existing task conversation with an environment panel:

- Install/start/check editors and revision/digest state.
- Inventory and supported/unavailable capabilities.
- Current stage, build operations, full output links, and explicit failures.
- Fresh verifier task/machine/check result, with evidence completeness.
- Candidate version, saved version, history, and reviewed Save/Rollback.
- Optional separately granted terminal, artifact browser, source diff, and
  associated workbench links.
- Actual usage, deadline/capacity waits, cancellation, cleanup, and uncertainty.

Use current account membership, host binding, observe/operate grants, and
admitted project/environment policy. Account sign-in alone cannot create host
or builder authority. If existing closed host rights do not express a required
operation, extend them through their owning protocol/qualification rather than
inventing a web-only permission. Submit original signed native requests through
the current reviewed-control mechanism, with source/recipe/version fences and
stable recovery IDs.

Refresh/reconnect loads authoritative snapshots and cursors. Changed
membership, host generation, source profile, credentials, or policy retires
private data and disables stale controls. Repeated Save or lost response
reconciles the original promotion. A browser close detaches; it does not stop
the builder. Essential setup works without Verse or a 3D renderer.

## Future task selection and retention

Add a native environment-version reference to Cloud job admission/Spec rather
than overloading `template` with an unreviewed name. Resolve the project's
saved version once before provisioning and retain exact provider/runtime/
recipe/qualification identities in the job. A later promotion does not alter
that job or a retained continuation.

Keep the daily runtime template as the base/fallback when no project environment
is selected and that fallback is admitted. Show the absence explicitly; do not
pretend the daily template proves repository qualification. Restored source
may be materialized at a newer commit, subject to explicit compatibility and
startup checks.

Reference-count versions used by tasks, verifier/build history, and project
selection. Retire selection separately from provider deletion. Garbage
collection uses retained IDs, retention policy, and provider deletion results;
the daily-template pruner must not delete project images it does not own.
Record provider storage/compute usage or unknown amounts. Quoted cost,
reservation, metered charge, and operator expense keep their existing owners.

## Deliberate scope choices

The initial process uses the operator lane. The frozen
[retail v1 contract](../retail-contract.md) deletes each sandbox, excludes saved
reuse, customer shell/SSH, fan-out, and publication, and runs one Codex session.
Cursor-style onboarding changes those product terms. Customer environments
require a separately versioned contract defining image persistence, setup and
verifier cost, credential custody, disclosures, bounds, terminal rights,
retention, and allowed publication. #10956/#10970 remain useful customer
dependencies but cannot implicitly authorize this new class.

Reuse Boat's VM/snapshot storage and the existing GCE image infrastructure.
Do not build a hypervisor, image distribution service, Cursor API clone,
parallel agent engine, or another credit ledger for the first slice.
Do not reproduce Cursor's hidden exec-daemon/FUSE/VNC system assets unless an
admitted application profile needs equivalent capability.

Repository publication remains an optional independent effect. Environment
Save need not wait for an AGENTS.md PR, and does not automatically land it.
No broad mobile/desktop/GPU profile, private-repository retail expansion,
environment marketplace, scheduled fleet warming, or cross-provider migration
is required to qualify the first operator Boat workflow.

## Implementation sequence and acceptance

These are proposed slices, not registered issue numbers. Register them with
the project board and explicit blockers before implementation. Keep native
contracts ahead of browser projections, and complete each slice with targeted
checks for its owner. Documentation review does not require the Rust release gate.

| Slice | Deliverable and reuse | Required acceptance |
| --- | --- | --- |
| ENV-01 | Rust environment records/store/state machine and project/source links; depend on native tasks/Cloud and #10952. | Draft revision fences; immutable versions; parent/build/verifier linkage; restart; unknown outcomes remain visible; no side effects from reads. |
| ENV-02 | Full command/result and retained evidence storage over Boat streaming/follower + ATIF. | Large stdout/stderr beyond current caps; split UTF-8 bytes; selected credential split across chunks; lost connection; duplicate replay; result pairing; nested child archive; redaction before spool; quota exhaustion/incomplete evidence; no false complete export. |
| ENV-03 | Dedicated Boat setup session and recipe revision tools over admitted Coder/Codex profile. | Pinned source/base/runtime; isolated privileges; explicit credentials; user steering; install failure/repair/rerun; idempotent request; deadline/cancellation; reconciled ambiguous command. |
| ENV-04 | Durable clean recipe builder, optional explored capture, sanitized immutable output image. | Recipe edit stales build; no login files in output; inherited exclusions and required captured paths; stop/snapshot readiness; provider restore hydration; qualified immutable selection or owned never-replaced names; crash during snapshot; usage and cleanup uncertainty; no alias drift. |
| ENV-05 | Frozen independent verifier and startup/application profiles on exact output image. | Different machine; untouched baseline before disposable idempotence fork; protected check artifacts; no repair of candidate; locked/offline dependencies; meaningful selected behavior; service/browser readiness; failed/missing/empty checks; changed plan; complete child result; cleanup. |
| ENV-06 | Reviewed promotion/rollback, saved history, pinned future job selection. | Concurrent saves; stale draft/version/grant; lost reply; repeated original request; existing jobs retain old version; new jobs use exact selected version; startup failure remains failure. |
| ENV-07 | Project environment panel/chat/evidence/export through #10949–#10953. | Empty/unavailable/denied/stale/failed/cancelled/reconciling; refresh/tab suspension; original-byte paging; native request recovery; optional terminal admission; keyboard/narrow/accessibility checks. |
| ENV-08 | Operator packaging and qualification using relevant #10963 criteria. | Code acceptance: isolated simulated setup→build→fresh verify→save→new task run, recovery, and browser acceptance. Availability: separately authorized real Boat/deployed-origin run, acknowledged teardown, measured restore/build time and costs, deployment/source/custody record. |
| ENV-09 | Optional dedicated GCE image adapter. | One isolated builder; pinned image identity; new boot verifier; no shared-pool installation; delete/storage/usage reconciliation; equivalent evidence. |
| ENV-10 | Optional customer environment contract and retail integration. | Reviewed new contract/price/retention/credential/authority class; #10956/#10970 transport; fake-funding/recovery checks; owner-funded lane qualification before availability. |

ENV-03 lives in [`crates/coder-environment-setup`](../../../crates/coder-environment-setup/src/lib.rs).
A setup session runs on its own computer
(`coder_working_computer::Purpose::EnvironmentSetup`) through the CMP-01
driver and provider, plus that provider's identified, at-most-once
`Commands` (Boat: a `mkdir`-claimed wrapper that keeps its own output, pid,
spec digest, and exit). Its tools are inspect, recipe update (through the
ENV-01 draft fence), command start/poll/reconcile/stop, install run (bound to
one recipe revision and digest), steer, await input, end, cancel, and
cleanup. Git auth is per-process `GIT_CONFIG_*` naming the credential
variable; no token reaches `.git/config`. Every tool call goes through the
ENV-02a recorder. Before any install, the session materializes the exact
pinned commit with the shared source step
([`source`](../../../crates/coder-environment-setup/src/source.rs)): it
fetches the pinned revision by URL (no remote recorded) with ephemeral Git
auth, proves `HEAD` equals the pin and the tree is clean, checks that the
checkout's Git configuration holds no credential, and prints a typed report
into the evidence. Qualification against real Boat is part of ENV-08.

ENV-04 lives in [`crates/coder-environment-build`](../../../crates/coder-environment-build/src/lib.rs).
A `BuildJob` rebuilds the pinned recipe revision on a fresh builder computer
(`coder_working_computer::Purpose::EnvironmentBuild`), never the setup or a
chat computer and never restored from a checkpoint. It first runs the same
source step (its typed report is retained on the job and in the image
manifest), then runs the exact install script once by identity, then a sanitization command with no credentials that
removes sign-ins (`~/.claude/.credentials.json`, `~/.codex/auth.json`, `gh`,
npm, Cargo, Git, Docker, SSH), private mounts, declared exclusions, and
explored state the recipe does not keep (`Recipe.capture.keep_explored`);
strips credentials from Git configuration; and verifies every required path.
Capture is gated on that typed report, then on a stopped builder, then on the
provider's typed image readiness with an immutable snapshot ID (Boat: a named
snapshot under an owned `oaenv-<build>-<digest>` name that a capture reads
first and never replaces). The name, snapshot, and image-manifest digest are
recorded on the `BuildAttempt`. A recipe edit stales earlier builds
(`Environment::is_stale`): a stale build cannot be verified or saved, and one
that goes stale mid-build is cancelled before capture. A crash or lost reply
leaves the attempt needing reconciliation; the next visit reads the command
or image by identity before acting. Usage and cleanup are separate retained
facts, and unknown cleanup blocks new builders for the environment.
Restore hydration of the output image is checked by the ENV-05 verifier.

ENV-05 lives in [`crates/coder-environment-verify`](../../../crates/coder-environment-verify/src/lib.rs).
A `VerifyJob` targets one ready, current build. Before any allocation the
builder's retained `ImageManifest` must reproduce the `BuildAttempt`'s
manifest digest and the provider must still hold that name as the same
`Ready` immutable snapshot. The frozen `CheckPlan` is a protected JSON
artifact addressed by `Recipe.qualification.plan_digest`, and every check
script it names is a blob pinned by digest. The image boots on a fresh
computer (`coder_working_computer::Purpose::EnvironmentVerify`, Boat:
`create` with `from` = the named snapshot) with no credentials. That
computer is never the setup or builder computer and is never restored from
a checkpoint. Restore readiness is the provider's typed hydration fact
(`Images::hydration`, Boat `hydrated`).

On the untouched baseline, the verifier proves the checkout is the pinned
commit with the shared source step in verify mode, fetching only if the
plan says `materialize`. It checks lock files against the recipe's frozen
digests and starts declared services under provider process ownership with
their health rules. It then runs readiness/browser and behavior checks.
With `offline`, package managers are offline and other clients get a dead
proxy.

Only after the baseline passes does a second fresh boot of the same image
fingerprint the declared inventory, rerun the build's exact install script
and startup, and fingerprint again. Any difference fails the run. A
non-zero exit, a timeout, a lost process, or a missing or altered artifact
fails the run. So does a plan without a behavior check or a check with no
assertion result (`OA-CHECK passed=` markers or `cargo test` summaries). A
recipe revision or an altered plan artifact cancels the run as invalid.

Each machine's commands are a child evidence record archived into the
run's ENV-02a record. The verdict reaches the `VerificationAttempt` with
its evidence digest and `evidence_status`; Save requires `Passed` with
complete evidence. Both machines are deleted and their usage retained
before the verdict is recorded. Unknown cleanup holds the verdict and
blocks new verifiers for the environment. An owner restart that loses the
live evidence ends the run incomplete, and its machines are still cleaned
up. Qualification on real Boat is part of ENV-08.

ENV-06 lives in [`coder-environment`](../../../crates/coder-environment/src/promotion.rs)
and the operator job path in [`coder-cloud`](../../../crates/coder-cloud/src/operator.rs).
`Environment::propose` returns the exact `Candidate` a Save would record
(recipe revision and digest, source pin, base and runtime pins, image
identity with snapshot and manifest digest, build and verifier run links,
plan and evidence digests) without side effects. A reviewer grants a
bounded `Review` naming that displayed candidate. `SaveVersion` and
`Promote` recompute the candidate and refuse with `StaleReview(field)` if
anything changed, `StaleDraft` after a recipe edit, `ReviewExpired`, or
`ReviewUsed` when the grant already saved a version. Save creates the
immutable version with its review stamp; `Promote` also moves the
selection under `expected_selection_revision`, retaining both or neither,
so concurrent promotions have one winner and the loser leaves no version.
Every pointer move is a retained `SelectionChange` (`promoted`, `selected`,
`rolled_back`); rollback is a `Select` of an earlier version and never
rewrites one. `Environment::history` lists saved versions newest first with
their selection marks. Request IDs replay the original result after a lost
reply and conflict when reused for a different operation.

The operator reads `<state>/environments` once when it admits a new job
(`Store::selected`; two live environments selecting for one project are
refused as ambiguous) and retains the `VersionPin` on the job record.
Continuations, retries, and queued jobs keep that pin; selection changes
never reach them. With a pin, Boat starts the job from exactly the saved
named image after checking that it still holds the sealed snapshot. A
missing or replaced image fails the job without provisioning; it never
falls back to the profile or daily runtime template. Pins apply only to a
job the operator policy already admits, in Boat Coder mode; other profiles
are refused while a version is selected. A selection is not customer
availability.

ENV-07 was the project environment panel in `openagents-web`, removed with
the old Cloud pages on 2026-10-08 ([the Cloud reset](../../web/cloud-reset.md));
`/environments` replaces it. It lived
([`cloud/environment.rs`](../../../crates/openagents-web/src/cloud/environment.rs))
at `/cloud/app/hosts/{binding}/cloud/{project}/environment`, linked from
the project's operator jobs page. Its native contract is
[`coder_access::environment`](../../../crates/coder-access/src/environment.rs):
`environment.read` and `environment.evidence` (Observe, reads only) and
`environment.promote`, `environment.select`, and `environment.steer`
(Operate, replies retained by request ID). The coder-cloud operator answers
them ([`operator_environment.rs`](../../../crates/coder-cloud/src/operator_environment.rs))
under the same per-device project assignment as Cloud jobs, from
`<state>/environments` and the verifier evidence under
`<state>/environment-verify/evidence/<verify job>`; reads open records
read-only and create nothing. The panel shows draft and recipe revisions,
setup sessions with their steering and command states, build and
verification progress with explicit reconciling, failed, cancelled, and
stale states, the exact candidate behind a reviewed Save and select, saved
history with Select and rollback, and selection changes. Promote recomputes
the candidate and refuses unless its digest is the one displayed; a
repeated original request returns its first result. Evidence pages serve
verified original bytes by cursor with gaps disclosed, an exact-byte
download, and a JSON export of coverage and gaps. Effects go through the
shared request book (WEB-08/09), so a refresh or lost reply recovers the
original request instead of sending another. The job view shows the
`VersionPin` a job started with. Setup steering is answered only when a
setup owner is composed with `Operator::with_setup`
([`coder_environment_setup::panel::Panel`](../../../crates/coder-environment-setup/src/panel.rs)):
it retains steering and its evidence before answering and hands wake-ups to
the owner's loop, which calls `Setup::resume`; without one the panel shows
setup as unavailable. A terminal is the separately granted native
workbench.

ENV-08 packages the owners in
[`coder-environment-operator`](../../../crates/coder-environment-operator/src/lib.rs).
`Owners::open` builds the setup, build, and verify owners over the
operator's private state directory (`<host root>/cloud-operator`), and
`attach` composes the setup owner with `Operator::with_setup` and starts
their loop on its own thread. The service handle lives inside the composed
operator, so the loop starts and stops with it; `Service::stop` also waits
for the current visit. With the owners packaged, the panel lists and steers
setup sessions instead of showing setup as unavailable. Every owner keeps its
state under the operator's state directory, created `0700`:

| Path | Owner | Contents |
| --- | --- | --- |
| `environments/` | ENV-01 | environment records; the operator admits jobs from them |
| `environment-setup/sessions/`, `evidence/`, `blobs/` | setup | sessions, tool evidence, install scripts |
| `environment-build/jobs/`, `evidence/` | build | build jobs and evidence |
| `environment-verify/jobs/`, `evidence/<verify job>` | verify | jobs and the run evidence the panel pages |
| `environment-verify/artifacts/<digest>` | verify | protected check plans, check scripts, and install scripts sealed by digest |
| `environment-computers/` | all | setup, builder, and verifier computer records |

The loop resumes a session the panel steered. On start and on every tick,
`Owners::recover` advances builds and verifications that are not finished or
whose machines are not confirmed deleted. It times out setup commands past
their deadline, wakes a session whose steering arrived while the owner was
down, and retries the cleanup of ended sessions. A restarted operator
therefore recovers the same sessions, jobs, and versions. An owner restart
that loses a live recorder is disclosed: setup and build open a new evidence
segment and mark the old one interrupted, and a verification ends
incomplete. Only the operator writes protected artifacts:
`Owners::seal_artifact` retains plans and check scripts, and
`Owners::verify` seals the build's exact install script before the
verifier starts.

`openagents host serve ... --cloud-operator POLICY --environment-owners
CONFIG` turns the package on (see the [Cloud README](../README.md#environment-owners)).
In this release a setup session, a build, or a verification is opened
through the package API (`Owners::setup.open`, `Owners::build`,
`Owners::verify`). The browser panel reads, steers, promotes, and selects;
it does not start them.

Code acceptance is one isolated, simulated end-to-end test,
[`environment_e2e_tests.rs`](../../../crates/openagents-web/src/cloud/environment_e2e_tests.rs),
over the in-memory provider, synthetic identities, the resident host, and
real HTTP through `openagents-web`. A setup session materializes the pinned
commit with the real source script (local `sh` and `git` against a scratch
origin) and runs a named-credential command that is redacted. Its install
fails, and the session pauses for input. The operator then restarts (host,
operator, and owners) and recovers the same records; the restart is disclosed
as an interrupted evidence segment. Steering through the panel's request book
wakes the session through the packaged loop. The recipe is repaired and the
install reruns, bound to the new revision. A clean build runs on a fresh
builder, and a fresh verification runs the untouched baseline, then the
idempotence fork. A reviewed Promote from the browser saves and selects the
exact candidate. A new task submitted from the browser pins `v1` with the
saved image, which the provider still holds `Ready`. The evidence export is
complete with no gaps and two machine records. No retained file holds the
credential value. All four machines are deleted with acknowledged cleanup, and
the panel pages pass keyboard and narrow-screen checks: a viewport that still
zooms, a skip link, labelled controls, and button-submitted forms.
The tier qualified is simulated; no real Boat machine, image, or deployed
origin ran. That run, with measured build and restore time and cost, and
the deployment, source, and custody record, is the owner-only ENV-08
qualification entry in the workspace `NEEDS_OWNER.md`. Until it passes,
environment onboarding is not available on a deployed origin.

Proposed blocker edges are ENV-02 → ENV-01; ENV-03 → ENV-01/02;
ENV-04 → ENV-01/02/03; ENV-05 → ENV-04; ENV-06 → ENV-05;
ENV-07 → ENV-01/02/06 and the completed web foundations; and
ENV-08 → ENV-03–07. Here `A → B` means A depends on B. Browser design can
proceed earlier against the native contracts; qualification requires integration.

The first usable operator release ends at ENV-08. Close code-complete issues
after their own checks and packaging pass. Record owner-only credentials,
funding, deployed-origin checks, or provider activation in `NEEDS_OWNER.md`;
they keep availability gated rather than implementation issues open. Use the
relevant browser/reconnect/accessibility criteria from #10963 without making
all unrelated WEB slices blockers of this feature. ENV-09 and ENV-10 can proceed
independently after the shared native contracts stabilize. Implementation
should not wait for unrelated sales, marketplace, or every web-roadmap slice.

The first demonstration must retain one source/recipe/base identity and show:
setup repair if needed, clean install, exact image build, independent fresh
verification, reviewed Save, a new task using the saved version, complete
authorized tool results, and acknowledged machine cleanup. Report the actual
qualification tier and any omitted applications. That is the concrete
OpenAgents equivalent of the useful parts of the Cursor onboarding experience.
