# Standing objectives for Foreman

The [objective catalog](objectives.json) defines work Foreman can consult
when no eligible GitHub issue is available. The owner requested the first
five objectives on 2026-10-22. Three additional objectives are proposed;
the picker leaves them out until the owner activates them.

The catalog is an authoring source, checked by
[its schema](objectives.schema.json). The server imports the catalog shipped
in its build, saves immutable revisions, and reserves each exact app assignment
before dispatch. The Grid host requests the server-selected objective when
its complete GitHub read reports no eligible issues. This implementation
uses the existing app assignment path. When native Kiln is configured,
`kiln-host` imports the same objectives into native WORK history and publishes
their signed declarations through encrypted Nostr envelopes. The app chat
consults those records and submits one bounded task to the persistent host.
Native AUTO scheduling remains a separate integration.

## Initial objectives

| ID | Objective | Desired outcome |
| --- | --- | --- |
| `improve-psionic` | Improve Psionic | Faster supported models, more model families, and more device types, with correctness checks and matched measurements. |
| `improve-kiln` | Improve Kiln | Reliable coding and checked patch delivery, persistent unattended execution, and broader qualified runner support. |
| `improve-pylon` | Improve Pylon | More reliable model discovery, routing, and recovery; lower end-to-end latency; better use of existing devices. |
| `improve-coder` | Improve Coder | More real coding tasks completed correctly, reliable tool use and recovery, and clearer evidence for review. |
| `improve-microcoder` | Improve Microcoder | More independently verified tasks completed with less time, inference, and cost, measured on held-out tasks. |
| `improve-foreman` | Improve Foreman — proposed | Better work selection, stall recovery, objective follow-through, and timely delivery without duplicate work. |
| `improve-gym` | Improve Gym — proposed | More reproducible evaluations, better regression coverage, and trustworthy comparisons of agent and model changes. |
| `improve-openagents` | Improve OpenAgents — proposed | More reliable chat and coding flows across desktop, phone, and web, using the shared Rust domain code. |

The five requested objectives begin at equal priority. Follow up active
work and explicit requests first. Among equally useful, feasible objective
proposals, prefer the objective least recently explored. Candidate examples
are prompts for investigation, not assertions that a defect exists.

Each exploration reads fresh source and measurements, searches claims,
issues, PRs, and prior attempts, and selects one bounded task. It records
why the task matters, its checks, resources, and baseline before coding.
An objective can remain active after many completed tasks. A run that finds
no worthwhile candidate records that finding and backs off.

## Representation in Nostr

The existing OpenAgents contracts cover this design. No new NIP or event
kind is needed for the initial profile.

| Concern | Existing contract | Mapping |
| --- | --- | --- |
| Ongoing objective | [NIP-WORK](../../nips/openagents/NIP-WORK.md) | An `openagents.work-item.v1` with `kind: initiative`, an accountable owner, and a body describing the desired outcomes. It has no execution frame or task acceptance policy. |
| Concrete improvement | NIP-WORK and [NIP-CTX](../../nips/openagents/NIP-CTX.md) | A child `kind: work` item with a `parent` relation to the initiative. Pin its exact task frame, source revision, scope, and acceptance policy before execution. |
| When to explore or continue | [NIP-AUTO](../../nips/openagents/NIP-AUTO.md) | An owner-admitted finite plan for a bounded exploration or task campaign, with deadlines, aggregate limits, and current grants. |
| Assignment and resource ownership | [NIP-COORD](../../nips/openagents/NIP-COORD.md) | Admit the task and its resource claim before dispatch. The working chat claims the linked issue. |
| Attempts and recovery | [NIP-CJ](../../nips/openagents/NIP-CJ.md), [NIP-RUN](../../nips/openagents/NIP-RUN.md) | Retain the admitted request, attempt, results, unresolved effects, and evidence links. |
| Source issue, patch, and PR | NIP-WORK source/evidence links and [NIP-34](../../nips/official/34.md) | Link the exact GitHub observation and Kiln patch/check/PR. A source status or green test does not itself accept or merge work. |

The shared [private artifact envelope](../../nips/openagents/contracts.md#private-artifact-envelope)
uses kind `3188`: signed, NIP-44 encrypted, with an exact recipient and
random mailbox. It carries WORK and AUTO artifacts when a separately
signed declaration is needed. The objective title and description stay
inside that envelope. Receiving an event does not start a worker.
The existing public Grid status can show an explicitly approved summary;
it does not become a second writable objective store.

NIP-75's kind `9041` describes a fundraising goal. The historical SA goal
kind `39203` was replaced by WORK objectives in
[NIP-SOV](../../nips/openagents/NIP-SOV.md#provenance-and-migration); do not revive it.

### Import and revision rules

1. Validate the local catalog, resolve the real owner and host authority,
   and admit the scope and policy through the existing owner control path.
2. Persist a mapping from `(owner, repository, catalog ID)` to one random
   WORK item ID and its `WorkRef`. Catalog slugs are local aliases, not
   pubkeys, random protocol IDs, event IDs, or `WorkRef`s.
3. Store each objective body as an artifact. Create its initiative through
   `openagents.work-command.v1`; retain the accepted
   `openagents.work-revision.v1` and original authenticated declarations.
   Reimport with the same command ID returns its prior result. Changed
   definitions use a conditional replacement against the exact revision.
4. Keep priority, allowed computers, exclusions, and resource limits in the
   host's admitted selection/execution policy. They are neither inert
   `meta` fields with hidden behavior nor new fields in the closed WORK
   schema. The catalog's desired state requests activation; it grants none.
5. Link each child task to the initiative and pin the exact objective
   revision/body used to brief it. Later objective edits do not rewrite a
   running task's inputs or acceptance checks. Conflicting authority
   revisions require resolution; choosing the latest relay timestamp is
   insufficient.
6. Keep the ongoing initiative separate from finite execution plans.
   NIP-AUTO v1 requires finite expiry and occurrence/aggregate bounds.
   Exhausting a campaign pauses dispatch without claiming that the
   initiative succeeded. A replacement plan needs current owner admission;
   the agent cannot renew its own authority indefinitely.

`work-host` already implements native WORK history and conditional
commands; `automation-host` implements bounded schedules and checked
continuation. Their current execution profiles do not run arbitrary coding
agents or Kiln. The app-assignment adapter and catalog importer run through the
Grid command path. [`kiln-host`](../../crates/kiln-host/README.md) supplies
native WORK publication and a separately admitted Kiln executor; it does not
extend the generic WORK or AUTO executor profiles. See
[work-host](../../crates/work-host/README.md) and
[automation-host](../../crates/automation-host/README.md) for their actual
supported roles.

## Kiln and delivery

Foreman selects the objective and task. A coding chat on `coderos-4080`
or a supported persistent Kiln executor performs it. The same work item
owns the checks, benchmark evidence, signed patch, and draft PR links.
Use one delivery route per attempt: a direct draft PR or the Kiln bridge.
The bridge must not open a second PR for a change already delivered.

`kiln-host` persists the shared agent loop, files, and pending commands.
The runner supports configured root packages and the fixed nested Psionic
workspace, with separate build state. Native intake checks the exact signed
objective revision; changing code invalidates its passing test. The host
publishes the checked patch, obtains the runner's check of that exact event,
and opens a draft through the existing bridge. GPU profiles still need device
qualification. Merging,
deploying, expanding resource budgets, and stopping production model
processes remain outside this objective catalog's authority.
