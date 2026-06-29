# Blueprint Signature Lookup And Apple FM Tool Use Audit

Date: 2026-06-07

Status: architecture audit and implementation roadmap for combining the current
Probe Apple FM tool-use lane with the OpenAgents product surface Blueprint Program Signature system.

## Executive Summary

Probe now has a working local Apple Foundation Models tool-use stream. The
bridge can create an Apple FM session with projected tools, call a Probe-owned
loopback callback server, receive tool output, stream replacement snapshots, and
return a final answer with redacted receipts.

That proves the local runtime path, but it is only the first slice. Apple FM has
one important architectural constraint: tool definitions must be known when the
Foundation Models session is created. A model cannot discover a broad Blueprint
registry mid-stream and then dynamically add newly discovered tools to the same
Apple FM session. Probe therefore needs a preflight capability step before an
Apple FM turn:

1. Resolve the work objective, context pack, backend, actor, risk ceiling, and
   allowed surfaces.
2. Look up the relevant Blueprint Program Signatures, Module Versions, tool
   scopes, release gates, and receipt requirements.
3. Select a bounded tool menu for the backend.
4. Project that menu into the backend-specific tool format.
5. Only then create the Apple FM session and stream the turn.

The same lookup system should not be Apple-specific. Codex-style hosted
execution, API-key chat models, local Qwen/Psionic routes, swarm routes, and
future Pylon-hosted Probes all need the same signature registry, tool-scope
policy, and Program Run receipt model. The difference is projection timing.
Apple FM needs the selected tool definitions up front. Other backends may
support dynamic tool lists between messages or richer MCP-style discovery, but
they still should draw from the same Blueprint signature registry.

The correct shape is:

```text
OpenAgents product surface Blueprint = source of Program Signature, Module Version, release gate,
context, action-submission, evidence, receipt, and export contracts.

Probe = coding-agent runtime that consumes those contracts, selects and projects
tools for a backend, executes or refuses tools under Probe-local policy, records
tool evidence, and emits Program Run or action-submission refs back to OpenAgents product surface.
```

Do not create a separate Probe-only signature registry. Start with a
Probe-local mirror of OpenAgents product surface's current typed seeds because OpenAgents product surface's Blueprint
registry and contract export are not live HTTP routes yet. Then replace that
fixture with authenticated `GET /api/blueprint/program-registry` and
`GET /api/blueprint/contracts` once OpenAgents product surface wires them.

## Source Material Reviewed

OpenAgents transcripts:

- `openagents/docs/transcripts/206.md`
- `openagents/docs/transcripts/194.md`

Current OpenAgents product surface Blueprint kernel:

- `openagents/docs/2026-06-05-autopilot-sites-agent-ready-master-roadmap.md`
- `openagents/docs/blueprint/2026-06-05-legacy-blueprint-primitives-openagents-inventory.md`
- `openagents/docs/blueprint/2026-06-05-program-type-and-signature-schemas-v1.md`
- `openagents/docs/blueprint/2026-06-05-program-registry-projection-v1.md`
- `openagents/docs/blueprint/2026-06-05-contract-export-seed-v1.md`
- `openagents/docs/blueprint/2026-06-06-signature-contribution-draft-v1.md`
- `openagents/docs/blueprint/2026-06-06-developer-package-contribution-v1.md`
- `openagents/workers/api/src/blueprint/README.md`
- `openagents/workers/api/src/blueprint/schemas/program.ts`
- `openagents/workers/api/src/blueprint/schemas/program-run.ts`
- `openagents/workers/api/src/blueprint/schemas/program-registry.ts`
- `openagents/workers/api/src/blueprint/schemas/continuation-decision.ts`
- `openagents/workers/api/src/blueprint/services/continuation-decision.ts`
- `openagents/workers/api/src/blueprint/services/program-run-authority.ts`
- `openagents/workers/api/src/blueprint/fixtures/autopilot-continuation-signatures.ts`
- `openagents/workers/api/src/blueprint/fixtures/program-registry.ts`
- `openagents/workers/api/src/blueprint/exports/contract-export.ts`

Deprecated Blueprint source material:

- `autopilot4-deprecated/blueprint/README.md`
- `autopilot4-deprecated/blueprint/docs/programs-optimization-and-rlm.md`
- `autopilot4-deprecated/blueprint/docs/integrations/probe.md`
- `autopilot4-deprecated/blueprint/docs/api-sdk-contract.md`
- `autopilot4-deprecated/blueprint/docs/receipt-and-evidence-contract.md`

Probe and Apple FM implementation:

- `probe/docs/2026-06-07-apple-fm-first-backend-audit.md`
- `probe/docs/probe-apple-fm-backend.md`
- `probe/docs/apple-fm-admitted-mac-acceptance.md`
- `probe/packages/runtime/src/backends/apple-fm/client.ts`
- `probe/packages/runtime/src/backends/apple-fm/tools.ts`
- `probe/packages/runtime/src/cli.ts`
- `openagents/swift/foundation-bridge/README.md`
- `openagents/swift/foundation-bridge/Sources/foundation-bridge/Types.swift`
- `openagents/swift/foundation-bridge/Sources/foundation-bridge/ChatHandler.swift`
- `openagents/swift/foundation-bridge/Sources/foundation-bridge/Server.swift`

## Transcript Prior Art

### Episode 206: Guidance Modules

Episode 206 introduced the primitive then called Guidance Module. The name has
since been superseded by the Blueprint Program system, but the core product
need remains intact.

The problem statement was clear:

- Coding agents were strong co-pilots but weak autopilots.
- A long-running agent loop needed intelligence between turns, not only a human
  typing "continue" or a dumb checklist iterator.
- The between-turn decision engine needed to see the last turn, goal,
  constraints, environment, remaining time, budget, rate-limit/account state,
  tool/plugin discoveries, and open blockers.
- Soft guidance should recommend the next action.
- Hard guardrails should override guidance.
- The system should be measurable, evaluable, and improvable over time.

The DSPy-style vocabulary from that episode maps almost directly to current
OpenAgents product surface Blueprint:

| Episode 206 term | Current Blueprint term | Meaning for Probe |
| --- | --- | --- |
| Signature | Program Signature | Stable typed input/output contract for a decision or tool-planning step. |
| Module | Module Version | Implementation artifact: prompt, deterministic reducer, Effect module, runtime adapter, human-review module, optimizer candidate. |
| Guidance Module | Program Type plus selected Module Version and release gate | A governed behavior family, not a one-off prompt. |
| Between-turn decision | Continuation Program Signature and Program Run | Evidence-only decision such as continue, fix, test, stop, summarize, request context, retry account, escalate, prepare review. |
| Guardrails | Tool scopes, Context Pack scope, Source Authority, Release Gate, Action Submission | Runtime constraints that cannot be overridden by model confidence. |
| Marketplace packages | Signature contributions and developer package contributions | Reviewed package refs that can enter release gates but do not gain runtime authority automatically. |

The episode also made an economics point that matters for Probe: once
intelligence is expressed as small typed contracts, it becomes reusable,
searchable, comparable, optimizable, and monetizable. Probe should therefore
avoid hardcoding backend-specific prompt blobs for tool selection. The
selection of "which decision contract and tools apply to this coding turn" is
itself a Blueprint Program Run.

### Episode 194: Local Apple FM Tool Use

Episode 194 framed Apple Foundation Models as local inference that could reduce
cloud dependence for coding-agent workflows. The concrete prior art was:

- A local desktop model could be controlled from a mobile device.
- The Apple Foundation Models API was already good enough for agentic search
  through a codebase, including grep-style tool calls.
- The first use was not full cloud-agent replacement. It was local
  orchestration and bounded subwork that could shift some workload from cloud
  providers to Apple Silicon.
- The longer thesis was that the percentage of coding-agent workload running
  locally could rise over time as tool use, orchestration, MLX, and local model
  capability improve.

The current Probe Apple FM tool stream finally turns that thesis into a local
runtime proof. Apple FM can now invoke a Probe tool callback and stream a final
answer. The next step is to stop treating the tool menu as a demo-specific
list and make it a Blueprint-governed selection.

## Blueprint Prior Art

### Deprecated Blueprint Contract

The deprecated Blueprint source described a typed operating substrate under
Autopilot. It owned business objects, relationships, object sets, functions,
Program Types, Action Types, Source Authority, Context Packs, policies,
evidence, events, receipts, evals, and release gates.

The old `programs-optimization-and-rlm.md` document is especially relevant. It
states the working rule:

```text
Blueprint owns the program contract and governance.
Runtime systems execute under that contract.
```

It then gives the durable primitives:

- Program Type: versioned typed behavior contract.
- Program Signature: DSPy-style stable input/output contract inside a Program
  Type.
- Module Version: implementation artifact for a Program Type and Signature.
- Program Run: execution record and decision evidence.
- Optimizer Run: background compile/eval workflow that can produce candidate
  Module Versions but cannot promote them silently.
- RLM execution strategy: a governed strategy for complex recursive work, not
  the default graph shape.

The old Probe integration contract is also still correct:

- Blueprint can give a Probe mission a bounded Context Pack and allowed tools.
- Probe owns the coding-agent session loop, sandbox, file edits, command
  lifecycle, session transcript, and Probe-local approvals.
- Blueprint owns business graph authority, Context Pack scope, Access
  Explanation, Action Submission requirements, Source Authority, events, and
  receipt schema.
- Probe emits session, tool, file-diff, and command evidence.
- Blueprint receipts may cite Probe transcript spans, command refs, diffs, and
  verification output.

That is the boundary this audit preserves.

### Current OpenAgents product surface Blueprint Kernel

OpenAgents product surface has rebuilt the live Blueprint kernel as Effect-first TypeScript, not as
a restored Rust service. The roadmap is explicit:

```text
OpenAgents product surface owns the first live Blueprint kernel as Effect-first TypeScript services,
schemas, migrations, APIs, projections, and tests.

Rust pylons, Probe, Psionic, Nexus, and Treasury consume or emit typed
contracts and receipts through narrow bridges.
```

The important implemented pieces are:

- `BlueprintProgramType`: behavior family, purpose, instructions, strategy
  refs, evidence requirements, receipt requirements, release gates, risk class,
  tool scopes, status, and direct-mutation policy.
- `BlueprintProgramSignature`: input schema ref, output schema ref, decode
  policy, evidence and receipt requirements, tool scopes, version ref, and
  supported families.
- `BlueprintModuleVersion`: implementation artifact, module kind, provenance,
  release state, scorecards, release decision, rollback and deprecation refs.
- `BlueprintProgramRunRecord`: evidence-only execution record with typed output,
  confidence, route ref, cost ref, latency, evidence refs, receipt refs, and
  direct-mutation-disabled flags.
- `BlueprintProgramRegistryProjection`: operator-safe projection of Program
  Types, Program Signatures, Module Versions, Release Gates, recent runs,
  promotion state, evidence refs, receipt refs, and failure refs.
- `BLUEPRINT_CONTRACT_EXPORT_SEED`: seed export map for JSON Schema refs,
  OpenAPI operations, event catalog, and receipt catalog. It explicitly covers
  Probe, Pylon, Psionic, Nexus, Treasury, `oa-node`, `oa-workroomd`, and AI
  agents.
- `AUTOPILOT_CONTINUATION_PROGRAM_SIGNATURES`: draft continuation signatures
  for continue, test, fix, summarize, request context, retry account, stop,
  escalate, prepare review, route selection, research policy, email
  decisioning, and proof projection.
- `decideBlueprintContinuation`: evidence-only continuation decision service
  that selects a next action from typed turn facts and links it to a Program
  Signature and Module Version.

Two constraints matter most for Probe:

1. Program Runs are evidence, not write authority.
2. Action Submissions are the write-side boundary for external writes, deploys,
   PR creation, email sends, public claim upgrades, payment actions, and
   legal-sensitive commitments.

Probe can edit files inside its own controlled coding sandbox and produce
patches under Probe-local policy. But once the work crosses into business
authority, repository writeback, deployment, payment, public claim, or customer
communication, it must become a Blueprint Action Submission and receipt path.

### Current API Reality

OpenAgents product surface currently has typed seeds for the future registry and contract routes:

- `GET /api/blueprint/program-registry`
- `GET /api/blueprint/contracts`

Those are not live HTTP routes yet. The docs state that the registry and export
are typed seeds, not authenticated routes. Probe should therefore implement the
consumer shape in a way that supports three sources:

- inline assignment payload from OpenAgents product surface;
- checked-in/generated local fixture copied from the OpenAgents product surface seed;
- authenticated OpenAgents product surface HTTP route when it exists.

The runtime should not block on the live route before it can use the concept.
It should also not invent a parallel contract that diverges from OpenAgents product surface's
schemas.

## Current Probe Apple FM Tool-Use State

Probe now implements:

- `apple_fm_bridge` backend profile;
- live `GET /health` readiness;
- plain-text completion;
- replacement snapshot streaming;
- session-backed Apple FM tool callbacks;
- callback token validation;
- approval-pending and refused tool states;
- callback transcript and redacted receipts;
- live local `tool-stream-demo`.

The Apple FM bridge path is:

1. Probe creates an Apple FM tool-callback session with concrete tools.
2. Probe starts a loopback callback server.
3. Probe calls `POST /v1/sessions` on the Swift bridge with:
   - `instructions`;
   - `model`;
   - `tools`;
   - `tool_callback`.
4. The bridge converts each tool definition into a Foundation Models `Tool`.
5. Probe calls `POST /v1/sessions/{id}/responses/stream`.
6. If Apple FM chooses a tool, the bridge posts to the Probe callback URL with
   `session_token`, `tool_name`, and generated `arguments`.
7. Probe executes or refuses the tool and returns `{ "output": "..." }`.
8. The bridge streams `snapshot` events and a `completed` event.
9. Probe emits runtime events and a transcript receipt.

The local live proof works. The limitation is that the CLI demo hardcodes one
`read_file` tool and a local path. That is enough for bridge proof, not for the
Blueprint-governed runtime.

## The Apple FM Constraint

Apple Foundation Models requires the tool set at session creation time. The
current bridge takes the tool definitions in `SessionCreateRequest.tools` and
builds `RemoteTool` instances before the response stream starts.

This has several consequences:

- The model cannot ask for an arbitrary new tool halfway through a session.
- Probe should not register every possible tool in every session. That would
  widen the visible tool surface, waste context, increase schema failures, and
  weaken policy.
- The tool menu must be selected before the Apple FM session starts.
- If the selected menu is wrong, the correct recovery is a new turn or new
  session with a revised tool menu, not silent dynamic mutation inside the
  active Apple FM stream.
- Tool schemas need an Apple-specific projection adapter. In local testing, the
  bridge accepted schemas only after the root object had `title`, `x-order`,
  and constrained string fields such as enum-backed path values. A generic JSON
  Schema cannot be assumed to be valid Foundation Models generation schema.

This is the main reason Probe needs a Blueprint signature lookup and tool-menu
preflight.

## Required Architecture

### Core Flow

The final flow should be:

```text
Probe assignment
  -> resolve runner identity and backend capability
  -> mount or reference Context Pack
  -> lookup Blueprint Program Signatures
  -> select Program Type, Program Signature, Module Version, and tool scopes
  -> derive backend tool menu
  -> project backend tool definitions
  -> run backend session
  -> execute/refuse tools under Probe policy
  -> record Probe tool evidence and backend transcript receipt
  -> record or emit Blueprint Program Run evidence
  -> propose Action Submission for write-side effects
```

The lookup step is not a string search. Workspace instructions prohibit ad hoc
keyword routing for user-facing intent routing, retrieval routing, and tool
selection. The lookup implementation must be one of:

- exact ref lookup from an assignment or Context Pack;
- typed semantic selector using embeddings and cosine similarity;
- structured query planner against registry fields;
- explicitly modeled parser for bounded enum/id fields after a typed route is
  selected.

### Signature Lookup Request

Probe needs a shared request shape. This can live under
`packages/runtime/src/blueprint/` as a narrowed consumer contract that mirrors
OpenAgents product surface refs.

```json
{
  "lookupId": "blueprint_signature_lookup.run_123.preflight_1",
  "actorRef": "probe_runner.local.chris_mac",
  "workRef": "openagents_workroom.thread_456",
  "objectiveRef": "objective.coding.fix_test_failure",
  "assignmentRef": "probe_assignment.run_123",
  "backend": {
    "kind": "apple_fm_bridge",
    "profile": "apple-fm-local"
  },
  "families": [
    "context",
    "continuation",
    "routing",
    "source_selection",
    "action_planning"
  ],
  "riskCeiling": "medium",
  "allowedSurfaces": [
    "agent_api",
    "omni_workroom"
  ],
  "contextPackRefs": [
    "context_pack.thread_456.repo_scope_v1"
  ],
  "sourceAuthorityRefs": [
    "source_authority.repo.openagents.probe.v1"
  ],
  "capabilityRefs": [
    "probe.backend.apple_fm_bridge",
    "probe.tool.read_file",
    "probe.tool.code_search"
  ],
  "promptSummaryRef": "summary.run_123.user_goal.v1",
  "maxToolCount": 8
}
```

Important points:

- `promptSummaryRef` is a ref, not raw prompt text in the durable registry
  query. Probe may use raw prompt text inside the local process, but persisted
  lookup records should stay ref-first and redacted.
- `capabilityRefs` come from runner/backend capability reporting.
- `riskCeiling` and `allowedSurfaces` narrow the result.
- `contextPackRefs` and `sourceAuthorityRefs` cannot widen access. They only
  make existing allowed context explicit.

### Signature Lookup Result

The result should give Probe enough information to build a backend tool menu
and record evidence.

```json
{
  "lookupId": "blueprint_signature_lookup.run_123.preflight_1",
  "registryVersionRef": "blueprint_contract_export.seed.v1",
  "policyRef": "policy.blueprint.operator_safe_registry_projection.v1",
  "selectedProgramTypeIds": [
    "program_type.probe.tool_menu.select"
  ],
  "selectedProgramSignatureIds": [
    "program_signature.probe.tool_menu.select.v1"
  ],
  "selectedModuleVersionIds": [
    "module_version.probe.tool_menu.select.candidate_1"
  ],
  "toolMenu": [
    {
      "toolRef": "tool.probe.read_file",
      "toolName": "read_file",
      "access": "read",
      "requiresApproval": false,
      "inputSchemaRef": "schema.probe.tool.read_file.input.v1",
      "outputSchemaRef": "schema.probe.tool.read_file.output.v1",
      "allowedSurfaces": [
        "agent_api",
        "omni_workroom"
      ],
      "contextPackRefs": [
        "context_pack.thread_456.repo_scope_v1"
      ],
      "sourceAuthorityRefs": [
        "source_authority.repo.openagents.probe.v1"
      ]
    }
  ],
  "releaseGateRefs": [
    "release_gate.probe.tool_menu.select.v1"
  ],
  "evidenceRequirements": [
    "evidence.context_pack_required"
  ],
  "receiptRequirements": [
    "receipt.program_run"
  ],
  "directMutationAllowed": false,
  "actionSubmissionRequiredForDirectEffects": true,
  "safeProjection": true
}
```

This result is still evidence and planning. It does not execute a tool and does
not grant write authority.

### Tool Menu Projection

After lookup, Probe should build a backend-independent `ProbeToolMenu`:

```json
{
  "menuId": "probe_tool_menu.run_123.apple_fm.preflight_1",
  "backendKind": "apple_fm_bridge",
  "programSignatureIds": [
    "program_signature.probe.tool_menu.select.v1"
  ],
  "tools": [
    {
      "toolName": "read_file",
      "toolRef": "tool.probe.read_file",
      "policy": "allow",
      "inputSchema": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string"
          }
        },
        "required": [
          "path"
        ],
        "additionalProperties": false
      },
      "approvalPolicyRef": "policy.probe.tool.read_file.allow_v1",
      "contextPackRefs": [
        "context_pack.thread_456.repo_scope_v1"
      ]
    }
  ]
}
```

Then each backend adapter projects that menu.

For Apple FM:

- Convert each selected tool into `AppleFmToolDefinition`.
- Add Foundation Models-compatible root schema details such as `title` and
  `x-order`.
- Use enum fields where the allowed values are already bounded by context, for
  example workspace-relative file paths from a Context Pack.
- Start a callback server before session creation.
- Create the Apple FM session with only this selected menu.

For hosted API models:

- Use the same menu to build OpenAI-compatible tools or provider-specific tool
  definitions.
- The model may receive `lookup_blueprint_signatures` as a normal tool if the
  provider supports mid-conversation tool use, but the result must still be
  checked by the same registry client.

For Codex-style hosted execution:

- The initial assignment can include the selected signature refs and allowed
  tools.
- Codex can still run its own native tools, but Probe must record which
  Blueprint tool scopes and Program Signatures authorized the run.

For Pylon/swarm routes:

- The Pylon capability report should include backend capability refs, supported
  tool refs, registry version refs, and whether the host can project the
  selected signatures to its local backend.

### Stable Meta-Tools

There are two possible meanings of "Blueprint signature lookup tool":

1. A Probe-controller preflight function. This is required for Apple FM.
2. A model-visible tool that a backend can call during a session. This is
   useful for hosted/API backends and for future Apple FM replanning, but it
   cannot solve Apple FM's active-session dynamic-tool limitation.

Probe should implement both against the same service.

Recommended stable tool names:

- `lookup_blueprint_signatures`
- `read_context_pack`
- `request_tool_menu_update`
- `propose_action_submission`

For Apple FM, `lookup_blueprint_signatures` should usually run before the Apple
FM session, not inside it. `request_tool_menu_update` can be included as a
small safe tool when the selected menu may be incomplete. Its output should
tell the model to stop with a `needs_replan` style result. Probe then closes
the turn, performs another lookup, and starts a new Apple FM session with the
new menu.

Do not let `request_tool_menu_update` mutate the active Apple FM session. That
would create a hidden tool-authority path outside the bridge contract.

### Proposed First Probe Program Signatures

OpenAgents product surface's current continuation signatures are Autopilot-general. Probe should
ask OpenAgents product surface to add or export coding-runtime signatures like these:

| Program Signature | Family | Purpose |
| --- | --- | --- |
| `program_signature.probe.signature_lookup.v1` | `source_selection` or `routing` | Given assignment refs, context refs, backend capability refs, and risk ceiling, return candidate Program Signatures and tool scopes. |
| `program_signature.probe.tool_menu.project.v1` | `routing` | Convert selected Program Signatures and tool scopes into a backend-independent Probe tool menu. |
| `program_signature.probe.apple_fm.tool_schema_project.v1` | `routing` | Convert a Probe tool menu into Apple FM generation-schema-compatible tools and reject unsupported shapes. |
| `program_signature.probe.coding_next_action.v1` | `continuation` | Decide continue, test, fix, summarize, request context, retry account, stop, escalate, or prepare review from Probe turn evidence. |
| `program_signature.probe.context_pack_mount.v1` | `context` | Decide which Context Pack refs a Probe run may mount and which source refs remain excluded. |
| `program_signature.probe.action_submission_draft.v1` | `action_planning` | Convert proposed write-side effects into Blueprint Action Submission drafts instead of direct mutation. |
| `program_signature.probe.tool_result_classify.v1` | `review` | Classify tool results as success, refusal, approval-pending, unsafe, malformed, stale, or needs retry. |
| `program_signature.probe.acceptance_evidence_project.v1` | `proof_projection` | Convert Probe transcript/tool/test evidence into public/customer/operator-safe proof refs. |

These should be Blueprint Program Signatures, not a Probe-only enum list. Probe
can carry a local fixture until OpenAgents product surface exports them.

## Apple FM Specific Design

### Session Preflight

Before `streamAppleFmSessionWithTools`, Probe should build an
`AppleFmSessionPreflight`:

```json
{
  "preflightId": "apple_fm_preflight.run_123.turn_1",
  "lookupRef": "blueprint_signature_lookup.run_123.preflight_1",
  "menuRef": "probe_tool_menu.run_123.apple_fm.preflight_1",
  "bridgeBaseUrl": "http://127.0.0.1:11435",
  "model": "apple-foundation-model",
  "projectedToolCount": 4,
  "projectedToolRefs": [
    "tool.probe.read_file",
    "tool.probe.code_search"
  ],
  "schemaProjectionWarnings": [],
  "requiresNewSessionForToolMenuChange": true,
  "maxModelRoundTrips": 8
}
```

The preflight should fail before inference when:

- no registry source is available;
- the selected Program Signature is not active or allowed for the actor;
- the release gate blocks the signature;
- the required Context Pack is absent;
- the selected tools exceed the backend or policy limit;
- a tool input schema cannot be projected into Apple FM generation schema;
- the backend health check is not ready.

### Tool Schema Projection

Apple FM should not receive raw Blueprint or JSON Schema payloads directly. It
needs an adapter:

```text
Blueprint schema refs
  -> resolved Probe tool input schema
  -> Probe policy narrowing from Context Pack
  -> Apple FM generation schema projection
  -> Swift bridge `arguments_schema`
```

The adapter should:

- require root object schemas;
- add stable `title` values;
- add `x-order`;
- reject unbounded arbitrary objects;
- prefer bounded enums for context-derived strings such as file paths, action
  kinds, and signature refs;
- preserve a mapping from backend tool call id to Blueprint tool ref;
- emit projection warnings as evidence refs, not hidden logs.

This is not only a compatibility detail. A narrower Apple schema improves
policy and reduces invalid generated arguments.

### Tool Callback Receipts

Probe already records `probe_backend_tool_callback` receipts for Apple FM. The
Blueprint-integrated shape should add refs:

```json
{
  "kind": "probe_backend_tool_callback",
  "backendKind": "apple_fm_bridge",
  "sessionId": "apple_fm_session_...",
  "toolCallId": "gen-...",
  "toolName": "read_file",
  "toolRef": "tool.probe.read_file",
  "programSignatureId": "program_signature.probe.tool_menu.project.v1",
  "programRunRef": "program_run.probe.run_123.turn_1",
  "contextPackRefs": [
    "context_pack.thread_456.repo_scope_v1"
  ],
  "sourceAuthorityRefs": [
    "source_authority.repo.openagents.probe.v1"
  ],
  "status": "success",
  "callbackUrl": "[redacted]",
  "callbackTokenRedacted": true,
  "contentRedacted": true
}
```

Do not include raw callback URLs, tokens, raw provider payloads, raw private
repo content, raw logs, or raw prompts in durable public projections.

### Tool Menu Replanning

Because Apple FM cannot add tools mid-session, Probe should support a visible
replan path:

1. Apple FM uses `request_tool_menu_update` or emits a final answer indicating
   `needs_replan`.
2. Probe records the turn as incomplete but not failed.
3. Probe performs another signature lookup with the new evidence refs.
4. Probe creates a new Apple FM session with the updated selected tool menu.
5. The new session receives a concise redacted transcript summary and evidence
   refs, not raw callback logs.

This keeps Apple FM honest while preserving the Guidance Module idea of
intelligent decisions between turns.

## Shared Backend Design

The registry and tool menu must serve every backend:

| Backend family | Lookup timing | Projection |
| --- | --- | --- |
| Apple FM | Required before session creation | Foundation Models-compatible generation schemas and loopback callback. |
| OpenAI-compatible API | Before request and optionally between messages | Provider tool schema; can include model-visible lookup tool. |
| Codex-style hosted execution | Before assignment and between turns | Assignment instructions, allowed tools, MCP/tool config, evidence refs. |
| Psionic/Qwen local or remote | Before request and between turns | OpenAI-compatible or Psionic-native tool schema; may use local/swarm tool callback. |
| Pylon/SHC hosted Probe | Before dispatch and before local backend session | Capability-matched tool menu plus host policy and wallet/compute telemetry refs. |
| Swarm inference | Before dispatch and at coordinator replans | Sharded tool menu, context refs, and receipt requirements per worker. |

The shared service should be named around product intent, not implementation
history:

- `BlueprintSignatureRegistryClient`
- `BlueprintSignatureLookupService`
- `ProbeToolMenuPlanner`
- `ProbeBackendToolProjector`
- `ProbeProgramRunRecorder`

Avoid names tied to Apple FM, Bun, or the old Guidance Module label.

## Roadmap For Probe

### Phase 1: Local Blueprint Consumer Contracts

Add `packages/runtime/src/blueprint/` with a narrowed Effect Schema mirror of
the OpenAgents product surface export surface Probe actually needs:

- contract export seed;
- program registry projection;
- program type;
- program signature;
- module version;
- release gate refs;
- tool scope;
- program run evidence ref;
- signature lookup request/result;
- tool menu plan.

Tests:

- decode the current OpenAgents product surface seed shape;
- reject raw secret-shaped fields;
- reject direct-mutation-enabled Program Runs;
- reject registry entries that are not safe projections.

### Phase 2: Registry Sources

Implement a registry client with three sources:

- `staticFixture`: checked-in Probe fixture generated from OpenAgents product surface's seed;
- `assignmentInline`: OpenAgents product surface can attach a safe registry slice to a Probe
  assignment;
- `openagentsHttp`: future authenticated `GET /api/blueprint/program-registry` and
  `GET /api/blueprint/contracts`.

The first implementation can use `staticFixture`. The API shape must already
support the other two so Probe does not need a refactor when OpenAgents product surface routes go
live.

### Phase 3: Signature Lookup Service

Implement `lookupBlueprintSignatures(request)` as an Effect service.

Initial selector:

- exact `programSignatureId` or `programTypeId` refs from assignment if present;
- otherwise structured filtering by family, risk ceiling, allowed surfaces,
  status, release gate state, capability refs, and backend support;
- no ad hoc keyword matching.

Later selector:

- typed semantic selector with embeddings over registry entries, purpose refs,
  capability summaries, and fixture labels;
- retained failure fixtures for wrong tool menus;
- confidence and fallback to `request_context` or `escalate`.

### Phase 4: Tool Menu Planner

Add `planProbeToolMenu(lookupResult, assignment, backendCapability)`.

The planner should:

- convert Blueprint tool scopes into Probe tool definitions;
- preserve `toolRef`, `programSignatureId`, `contextPackRefs`, and
  `sourceAuthorityRefs`;
- mark policies as `allow`, `approval_required`, or `deny`;
- cap tool count;
- reject tools that exceed backend capability;
- produce evidence refs and projection warnings.

### Phase 5: Apple FM Projection Adapter

Add `projectAppleFmToolMenu(menu)`.

The adapter should:

- map `ProbeToolDefinition` to `AppleFmToolDefinition`;
- normalize root object schemas;
- add `title` and `x-order`;
- bound dynamic string fields by context-derived enum values where possible;
- reject unsupported schemas before session creation;
- add tests with Swift-bridge-compatible payloads.

The existing `tool-stream-demo` can then use:

```text
lookup -> tool menu -> Apple FM projection -> streamSessionWithTools
```

instead of constructing `readFileTool` directly in the CLI.

### Phase 6: Program Run Evidence

After every backend turn, record or emit a Blueprint-compatible Program Run
evidence object:

- actor ref;
- assignment/work ref;
- program type id;
- program signature id;
- module version id;
- input snapshot hash;
- typed output summary;
- route ref;
- cost/usage ref;
- latency;
- backend kind/profile/model;
- evidence refs;
- receipt refs;
- direct mutation disabled;
- no deploy/email/source mutation/spend flags.

Default persistence can be local JSON/event records until OpenAgents product surface's repository
route is live. The shape should match OpenAgents product surface's `BlueprintProgramRunRecord` and
safe projection discipline.

### Phase 7: Assignment Integration

Extend Probe assignments to accept:

- `blueprint.registryVersionRef`;
- `blueprint.programSignatureRefs`;
- `blueprint.contextPackRefs`;
- `blueprint.sourceAuthorityRefs`;
- `blueprint.toolScopeRefs`;
- `blueprint.releaseGateRefs`;
- `blueprint.actionSubmissionPolicyRef`.

This lets OpenAgents product surface dispatch a bounded Probe without requiring Probe to query the
full registry on every run.

### Phase 8: Pylon And SHC Capability Reporting

Extend capability reports with:

- supported Blueprint registry version refs;
- supported Program families;
- supported Probe tool refs;
- Apple FM schema projection support;
- max projected Apple FM tool count;
- local/swarm/API backend availability;
- whether the runner can record Program Run evidence locally when offline.

Pylons should advertise "I can run this signature/tool menu" rather than only
"I have Apple FM".

### Phase 9: Action Submission Boundary

Wire write-side actions into Blueprint Action Submission shape:

- creating a PR;
- deploying;
- sending email;
- posting public claims;
- spending money;
- legal-sensitive commitments.

Probe-local file edits can remain Probe tool actions under sandbox policy, but
external writeback must produce an Action Submission proposal and wait for the
appropriate approval/receipt path.

### Phase 10: Release Gates And Marketplace Contributions

Use OpenAgents product surface's Signature Contribution and Developer Package Contribution models
for new Probe tool packs and signatures.

Rules:

- a contribution draft cannot execute;
- review and release gate are required;
- fixture refs and retained failures are required for promotion;
- optimizer-produced Module Versions cannot self-promote;
- payments/attribution can attach to promoted package refs later, not to raw
  copied prompts.

## Immediate Implementation Targets

The next issues for Probe should be:

1. Add Blueprint consumer schemas and a static registry fixture.
2. Add signature lookup service with exact/ref and structured filtering.
3. Add backend-independent Probe tool menu planner.
4. Add Apple FM tool projection adapter and move `tool-stream-demo` through it.
5. Add Program Run evidence records for Apple FM tool-stream runs.
6. Extend backend capability reports with Blueprint signature/tool-menu support.
7. Add assignment fields for registry/signature/context/action-submission refs.
8. Add a fake OpenAgents product surface registry HTTP client test for the future live route.

The first live user-visible proof should be:

```sh
bun run --cwd packages/runtime probe apple-fm tool-stream-demo \
  --path README.md \
  --prompt "Use the Blueprint-selected read_file tool to inspect README.md and report the first heading."
```

The output should include:

- registry version ref;
- selected Program Signature refs;
- selected tool refs;
- Apple FM session id;
- tool callback transcript;
- final assistant text;
- redacted backend transcript receipt;
- Program Run evidence receipt.

## Non-Goals

- Do not restore the deprecated standalone Blueprint service.
- Do not copy OpenAgents product surface's full Blueprint kernel into Probe.
- Do not make Probe the source of Program Signature truth.
- Do not use ad hoc keyword matching to select tools or signatures.
- Do not expose every Probe tool to Apple FM by default.
- Do not let Program Runs directly create PRs, deploy, send email, spend money,
  mutate source-backed business facts, or upgrade public claims.
- Do not store raw prompts, raw callback URLs, callback tokens, provider
  payloads, private repo content, wallet material, or private customer data in
  public-safe registry projections.

## Final Recommendation

Treat Apple FM tool use as the first backend-specific proof of a general
Blueprint-governed Probe runtime.

The correct architecture is not "Apple FM gets a special local tool registry."
It is "Probe asks Blueprint what behavior contract and tool scopes apply, then
projects that bounded menu into Apple FM's up-front session tool format."

That preserves the original Guidance Module goal from Episode 206, uses the
local Apple FM tool-calling thesis from Episode 194, respects OpenAgents product surface's current
Blueprint authority, and gives every backend a common signature lookup path.
Apple FM simply forces Probe to build the lookup and tool-menu planning step
sooner because its tools must be defined before streaming starts.
