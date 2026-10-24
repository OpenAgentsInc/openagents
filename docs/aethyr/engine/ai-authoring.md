# AI authoring and bounded actions

Status: proposed design, 2026-10-22. The system described here requires implementation and domain evaluation before any claim of reliable automatic editing.

## One editing system

Natural language is another input to the same typed command system used by buttons, hotkeys, inspectors, and authorized automation. It cannot execute an unvalidated private edit path. Models interpret meaning and suggest changes; code owns authority, IDs, units, counts, geometry, transactions, and activation. The command envelope and validation result belong to [Contracts](contracts.md).

There are two distinct creation modes:

| Mode | Use | Result |
| --- | --- | --- |
| Instant action | Supported placement, transforms, materials, room edits, and behavior presets | One bounded command with visible result and undo |
| Authoring job | New scripts, shaders, assets, large layouts, or changes beyond the instant-action limits | An isolated branch with a reviewable diff, diagnostics, and a preview |

An instant action never falls back silently to an open-ended agent. The creator explicitly starts an authoring job when new capabilities or source generation are needed. Neither mode requires a model provider for manual editing or playing an existing game.

## Captured context and target resolution

Submitting a request captures the branch and document revision, selected stable IDs, aimed surface, camera pose, current tool, relevant asset catalog, and a capability reference. The context distinguishes observations from inferred descriptions. It contains only the relevant neighborhood and permitted project records. The instant profile is text-only. A separate captured-image authoring job may use a screenshot as evidence; it cannot authorize a change or replace an entity ID. Such a job requires an explicitly authorized capture object and digest, registered byte and dimension limits, privacy review, and normal resource accounting. It cannot fetch an arbitrary image URL or inherit an exemption from the context budget.

The resolver uses this order for singular targets: explicit ID, explicit unique qualified name, matching current selection, then an aimed compatible surface for deictic phrases such as “this wall.” It never resolves “that ghost” to a selected barrel. A named type with multiple compatible candidates requires an explicit scope or a compact chooser. Plural phrases such as “the ghosts” mean all matching ghosts in the captured editing scope, which the UI displays; they never mean every ghost in an unbounded project.

“Left,” “right,” “ahead,” and “behind” use the submitted camera orientation projected onto the horizontal plane. “North” uses world axes. Explicit coordinates require a visible coordinate space; the default is world meters. The request captures its reference pose once, so latency does not turn “to my left” into a different location as the player looks around. If the view is vertical, use the captured camera rig's valid horizontal heading; if none exists, request a ground anchor rather than normalizing a zero-length vector. Placement remains provisional until geometry and occupancy checks pass.

Exact fields use deterministic parsing and catalog lookup where possible. “Add five huge pumpkins to my left” supplies count `5`, the installed pumpkin template, a named size preset, and a captured relative anchor. The model need not invent the number or coordinates. “Huge” resolves to the active template's published preset; the receipt displays its actual dimensions or scale. The placement planner finds five valid non-overlapping positions within the shown footprint or rejects the entire action with a useful remedy. It does not reduce the count silently.

## Semantic decision adapter

Jev is the preferred initial semantic routing adapter; the interface remains provider-independent. TypeSafe's Choice primitive selects from named options and returns the selected option with its distribution and confidence. It does not establish that the selected action is permitted or correct. [TypeSafe Choice](https://docs.typesafe.ai/primitives/choice).

Questions distinguish action, target scope, installed asset or template, spatial relation, size preset, behavior preset, and unsupported intent. Ask independent questions over the same captured context together and consume only the answers required by the selected route. A second request is justified only when the first retrieves genuinely new evidence or changes the candidate set. This follows the documented closed-set function-dispatch pattern. [TypeSafe function calling](https://docs.typesafe.ai/cookbooks/function_calling).

Catalog retrieval must preserve an explicit no-match outcome. If a request names an installed asset omitted from the shortlist, expand or correct retrieval before labeling it unsupported. A high-confidence wrong shortlist is still a product failure. The model's candidate IDs are checked against the exact supplied catalog revision.

There is no universal confidence threshold. Each route has a pinned policy evaluated against labeled requests, target-resolution cases, stale-state cases, and adversarial input. Before a route passes its evaluation, semantic output offers a concrete preview for application rather than automatic editing. The policy records false-application and clarification rates separately; confidence expresses model distribution concentration, not measured end-to-end correctness. [TypeSafe confidence](https://docs.typesafe.ai/confidence).

## Interactive limits and freshness

Initial instant-action admission limits are 1,200 Unicode scalar values of request text, 16 KiB of encoded model context, eight operations, 32 distinct explicit mutation targets across the whole command, counting new building/entity IDs and addressed existing IDs, and a 64 KiB encoded command envelope. A large world selection does not bypass the target cap through one group ID. Limits apply before inference and after expansion. Generated structural records remain bounded by the topology profile in [World model](world-model.md); they do not permit an unbounded implicit mutation. Structural IDs are persisted by authoring before compilation.

The initial response budget is a 2.5-second total deadline for semantic interpretation, including transport and any retry. Retry is allowed only within that deadline and the adapter's cost ceiling. Timeout returns a retryable UI state; a late result is discarded. This is an admission budget, not a measured model latency promise. Frame rendering, collision, and behavior ticks never call a language model.

The resolver emits the shared `schema`, `projectId`, `branchId`, `baseRevision`, `commandId`, `actorId`, `capabilityRef`, `operations`, `preconditions`, and `context` fields. An authorized retry of `commandId` with identical content returns its recorded result or compacted history pointer; reusing it with different content is an error. Revoked access cannot retrieve a private result through idempotency lookup. Model responses cannot supply trusted actor or grant values.

If a dependent target, selection, catalog, or structure changes before submission, the result is stale. The editor cancels on relevant local edits. The authority checks `baseRevision` and preconditions again; it cannot apply a stale candidate merely because a response arrived successfully. Reinterpretation against new context requires a new visible request. Cancellation does not erase a transaction that already committed; the UI obtains its receipt and offers undo.

## Unknown requests and failures

| Situation | Required behavior |
| --- | --- |
| Supported meaning with all fields resolved | Submit the validated command without an unnecessary confirmation loop |
| Genuine missing target or materially different interpretations | Highlight specific choices and ask one focused question |
| Installed content not found in current candidates | Repair retrieval or offer catalog search; do not substitute unrelated content |
| Requested feature absent | Say what is missing and offer an explicit authoring job or supported alternative |
| Invalid geometry, no room, or profile overflow | Explain the constraint, show the failed footprint, and leave the document unchanged |
| Model timeout, outage, or exhausted budget | Keep manual tools working and preserve typed request text for retry |
| Access denied | Explain the permission needed without trying another provider or identity |

Unsupported input is not an excuse for a vague “could mean more than one thing” error. Ambiguity requires actual competing candidates. New-feature requests must never be represented as successful placeholder operations.

## Kiln and agent authority

Kiln provides a project-workspace adapter, file operations, tool execution, and diagnostics. It is not an alternate authority over a running world. Editing semantic source through a workspace becomes an imported candidate checked by the same validators and revision rules as an editor command. Invalid partial files remain in a draft and cannot replace the active document.

An authoring job receives an immutable context snapshot, a scoped writable branch, a bounded filesystem view, allowed tools, cost and time ceilings, and explicit network policy. Project text, imported asset metadata, tool output, and comments are untrusted content. Instructions inside them cannot expand a grant, read another project, publish a release, or send messages. Provider keys and editor session tokens stay in trusted services; game code and generated bundles receive neither. See [Security](security.md) and [Integrations](integration.md).

Jobs propose source or transactions and compile a candidate preview. Generated gameplay uses the restricted TypeScript behavior dialect compiled to the owned fuel-metered VM described in [Scripting](scripting.md), not unrestricted browser JavaScript with implied safety. Generated shaders and assets pass bounded compiler admission and rights checks. A reviewer can inspect affected targets, changed files, resource estimates, and failures before merging. Publishing, purchases, external messages, and permission expansion require their own explicit authority.

## Evidence and acceptance

Record the route-policy version, provider and model identity, bounded candidate IDs, relevant judgments, timing, cost, command identity, validation outcome, and resulting revision. Raw prompts, screenshots, and source text follow the project's explicit retention policy; diagnostics must not leak credentials or unrelated project content. A creator can report a failure using a request ID without disclosing the entire project.

- **AI-01:** Five-pumpkin placement resolves count, preset, orientation, and exact scope; equivalent button actions yield the same operations.
- **AI-02:** Wrong-type selections, unseen catalog entries, plural scopes, stale selection, changing camera, timeout, cancellation after commit, and duplicate submission have dedicated tests.
- **AI-03:** Floating or chasing a ghost installs a runtime behavior once; animation runs locally without model traffic or project writes per frame.
- **AI-04:** “Add a cottage” selects a semantic building template; “add a room to this cottage” edits its structure. An opaque decorative house cannot pass as an editable building.
- **AI-05:** No model response bypasses capability, size, topology, target, or revision validation, including malicious project instructions and generated code.
- **AI-06:** Every enabled auto-apply route publishes its pinned evaluation set, false-application budget, measured error rate, and unsupported-language behavior. Provider or question changes rerun that gate.
