# DeepSeek Harness lessons for the new agent and TUI

Research snapshot: October 6, 2026. The requested repository was cloned
locally and inspected at
[`5badb15009ae1756c3afe0ae0cef1faafc290ccc`](https://github.com/deepseek-ai/deepseek-harness/tree/5badb15009ae1756c3afe0ae0cef1faafc290ccc),
an October 3 release merge for `dsh-0.2.1-alpha.1`. It is
[MIT licensed][license] and in developer preview. This survey reads its
architecture, framework, plugin management, loop, sessions, tools, context,
skills, subagents, terminal, hooks, and verification mechanisms. No dependencies
were installed, and no runtime or tests were executed.

The strongest lesson is to compose the product from explicit service
interfaces and lifecycle-owned plugins, while keeping task history and
projections independent of the screen. Pair that with OpenAgents' typed
workflows, Nostr distribution, and host-owned payments as described in
[the carry-forward proposal](plugin-architecture-carry-forward.md).

## What “everything is a plugin” means in this implementation

DeepSeek Harness uses Cordis plugins for the model adapter, tool registry,
prompt assembly, session store, agent loop, APIs, commands, guards, context
features, and UI. A profile selects ordered bundles and patches; composition
starts from an empty entry list. Base behavior is itself a bundle. The
[architecture][architecture] and [profile assembly][profiles] support the
README's claim with concrete implementation.

A service has three roles: definition, provider, and consumer. Consumers
depend on an interface instead of importing one concrete implementation.
Provider swaps can therefore affect an entire capability family: filesystem
and subprocess providers share an execution world, so changing that world
moves Bash, PTY, and LSP together.
See [the execution-world seam][execution-world].

There is still a framework substrate: Cordis' root context owns fibers,
service resolution, the registry, events, and logging. Ordinary host plugins
are imported into Node with host process authority. Dependency injection and
scoped contexts are lifecycle/composition mechanisms, not execution
containment. See [the root context][context], [Plugin Manager's host boundary][manager],
and [the cooperative dynamic runner][dynamic-runner].

For OpenAgents, adopt the service/composition idea in Rust. Keep host-owned
built-ins separate from NIP-EXT extensions with bounded Wasm. Make the trusted
host's authority and wallet boundary explicit even when its product features
are supplied by replaceable providers.

## Service graphs, profiles, and lifecycle

### Make dependencies and state observable

Cordis entries name identity, module, configuration, and required/provided
services. Required dependencies control activation. Fibers have pending,
loading, active, failed, unloading, and disposed phases; dependency changes
can unload and reload consumers. Registrations are scoped effects, and
asynchronous disposal is awaited. Sources:
[entry contract][entries], [registry activation][registry], and
[fiber lifecycle][fibers].

Carry forward these distinctions:

| State | Meaning for the new system |
| --- | --- |
| Installed | Verified bytes are available locally. |
| Enabled | The operator wants the component in a profile. |
| Admitted | Host policy allows its exact binding and scope. |
| Active | Dependencies exist and startup completed. |
| Failed/unavailable | A specific dependency, compatibility, policy, or startup failure prevents use. |
| Draining/disposed | New work is blocked; owned resources settle before removal completes. |

DeepSeek's [inventory][inventory] separates saved enablement from runtime
phase. That distinction would improve our TUI's plugin and provider views:
a checked toggle should not conceal an activation failure.

Resolve and record a pinned graph for a task. Give interactive, headless,
and background clients profiles over the same services. Explain layer
precedence and show the effective graph. Apply replacements at explicit
task/turn boundaries; runtime-wide hot reloading is not a prerequisite for
the first version.

### Own every registration and resource

Borrow Cordis' rule that handlers, schemas, prompt sections, adapters,
timers, and subscriptions belong to the plugin that created them. Missing
required services should block activation. Unloading must revoke future
eligibility and await resource cleanup.

DeepSeek's loop disposal cancels, waits for quiescence, disposes the agent
scope, closes persistence, and then detaches exact registry objects. Resume
requires persistence and exclusive write ownership. See
[disposal][dispose] and [resume][resume].

Use that ordering for a Rust lifecycle contract. Await work that started;
do not treat dropping a future as confirmed child shutdown. An external
command, transfer, or publication cannot be undone by removing its handler.
Keep unknown effects for reconciliation and preserve ownership/fencing when
the writer restarts.

## Durable facts, projections, and live streams

DeepSeek separates durable session events, live agent/capability events,
and transient assistant-stream frames. The admitted request is reconstructed
and frozen from the session log before it goes to the model. Projection
plugins synchronously fold committed events into typed JSON state and client
views, with schema/state versions and a shared sequence cut. Sources:
[turn and session flow][events] and [projection service][projections].

Carry this separation into the new TUI:

- **Durable facts:** task/turn/input/effect/result/approval/payment state and
  the exact model-visible inputs needed for reconstruction.
- **Pure projections:** transcript, rail, current plan, inbox, costs, file
  changes, and inspection views derived from those facts.
- **Live display:** incremental text and progress, labelled by their stream
  identity and sequence, with gaps and reconnect behavior defined.

A client can reconnect to a living owner without inventing another task
history. Replaying a projection must not rerun tools or payments. A new
renderer should consume the same state as headless and remote clients.

DeepSeek commits failed/cancelled attempts at settlement. Its Web stream
accumulator provides a process-local reconnect baseline with dense revisions
and indexed chunks. A hard process loss before settlement loses transient
attempt chunks. See [attempt settlement][attempts] and [live accumulator][streams].
For OpenAgents, specify that durability cut explicitly. Do not promise
token-by-token crash recovery unless the journal actually supplies it.

### Design durable plugin events before allowing indispensable plugin state

DeepSeek refuses unknown required event types; out-of-repository plugin
events must be marked `ignorable`. That makes replay independent of which
plugins happen to be loaded, but limits third-party ownership of essential
durable state. See [event compatibility][event-types].

Our contract needs schema identity, version/migration rules, maximum sizes,
and a declared omission policy. Missing support for an essential state
transition should refuse the corresponding resume, rather than silently
skip it. An ignorable presentation annotation may be omitted; a payment
attempt, pending effect, or execution-ownership record may not.

## A durable inbox with distinct controls

DeepSeek's inbox records edits and consumption as durable splices. Pending
input can be reconstructed without a live agent. Its control API distinguishes
four behaviors:

| Control | DeepSeek behavior | Carry-forward requirement |
| --- | --- | --- |
| Follow-up | Queue input for the next turn and wake it. | Durable ordered queue with visible edit/remove state. |
| Steer | Deliver at the next step and wake work. | Distinguish current-task steering from a new task. |
| Inject | Add next-step context without waking. | Context admission and provenance, without an accidental new run. |
| Cancel | Stop; clear pending input unless `keepInbox` is requested. | Name the cancellation scope and pending-input disposition explicitly. |

Sources: [control API][controls] and [inbox reconstruction][inbox].

This is a direct response to gaps in our
[current terminals](claude-code-replacement-history.md#current-terminal-survey):
one shell replaces a pending draft, another has a FIFO that is not restored
from its journal, and the chat TUI refuses ordinary submission while streaming.
Choose the intended semantics, then test resume and cancellation against them.
A shared control interface must still expose foreign engines' actual support;
it cannot simulate in-place steering by hiding cancellation/restart.

## Guarded operations and bounded concurrency

DeepSeek registers tools in scopes, validates outputs, supports replayable
presentation, and defaults concurrency to exclusive. Explicitly parallel-safe
calls can overlap in bounded groups; exclusive barriers and ordered policy/result
settlement preserve predictability. Cancellation drains started calls and
records skipped calls. See [tool registration][tools], [output and concurrency
contract][tool-contract], [value validation][tool-values], [scheduling][scheduler],
and [cancellation settlement][tool-cancel].

Its final guards are deny-only and run after the extensible approval
waterfall. A later listener cannot turn a guard denial into permission.
See [final admission guards][guards]. Carry that monotonic rule forward:
plugins may narrow an admitted operation, but cannot expand the host's grant
through listener order or a model score.

Keep policy, presentation, and execution interfaces distinct. A model-facing
operation is one consumer of an underlying service, not the only route to it.
OpenAgents workflows and typed decisions should use the same binding as
ordinary requests, with schemas, effects, cancellation, bounds, and receipts
checked at dispatch.

DeepSeek's crash repair adds results for incomplete calls and closes open
steps/turns. That repairs history structure; it does not prove the external
operation's outcome. See [repair][repair]. Our journals need confirmed,
failed, cancelled, and unknown effect outcomes beyond a syntactically complete
transcript.

## Context, skills, and subagents

Treat context construction and compaction as replaceable services. DeepSeek
has a compaction backend, token meter, optional result pruner, and command.
Compaction durably brackets the operation, rechecks the selected history,
and retains an orphan when closing fails. Overflow retry requires a confirmed
capacity error and durable progress. Sources:
[compaction limits][compaction], [transaction][compaction-transaction], and
[overflow retry][overflow].

Carry forward the transaction and provenance. Preserve user constraints,
pending effects, active grants, and payment identities outside summaries.
Measure auxiliary model cost and cache loss. Summarization cannot fix a
system/tool prefix or indivisible unit that already exceeds the context limit.
See [those limits][compaction-limits].

Filesystem skills are instruction/asset bundles, distinct from executable
Cordis plugins. Discovery scopes roots, validates metadata, skips invalid
entries with diagnostics, and reads the current body at load. Catalog inputs
are logged and scope restrictions remove guidance. See
[skill metadata][skills], [current-body loading][skill-loading], and
[logged catalog][skill-catalog], and [scoped loading][skill-tool]. Adapt that to
OpenAgents' signed guidance digests and precedence rules: optional relevance
selection cannot remove applicable user or host constraints.

DeepSeek's subagent service exposes one-shot and continuable providers,
checks actual support, persists nesting depth, reserves activation capacity,
and restricts messaging to exact live parent/child identities. Cold resume
preserves persisted depth and reserves a live activation slot. Native and
foreign providers keep different permission and execution semantics. Sources:
[subagent ownership][subagents], [persisted depth][depth], and
[activation reservation][activation].

Adopt explicit support and resource accounting, rather than copying its
particular default limits. Mount filesystem, shell, PTY, workspace, and
subagents against a consistent execution world. Record foreign engine
capabilities and current ownership honestly.

## Plugin management and the Nostr economy

DeepSeek offers profile-level toggles, bundle selection, package inspection,
install/remove, and compatibility exemptions. Removal unloads first; replacing
installed code requires restart because already imported modules and caches
remain. Its package-manager rollback restores manifest/lockfile state but
cannot undo build-script effects. Compatibility checks declared peer ranges;
missing declarations do not establish API conformance. Sources:
[manager behavior][manager], [removal][removal], [replacement][replacement], and
[peer compatibility][compatibility].

Borrow the distinction between desired state and applied state, explicit
restart requirements, owned removal, and diagnostics. For distribution,
reuse our signed NIP-EXT/Nostr registry and exact artifacts rather than
adopting npm as the product registry. For installed executable extensions,
retain our Wasm boundary and independently admitted native host bindings.

No purchase, entitlement, revenue-sharing, or paid marketplace mechanism was
found in the inspected Cordis, boot, plugin-manager, or dynamic-runner scope.
DeepSeek supplies composition lessons; our [registry and payment proposal](plugin-architecture-carry-forward.md#nostr-registry-with-built-in-payments)
supplies the commercial boundary. Installation, compatibility, payment,
authority, correctness, and default adoption need separate evidence.

## Terminal and validation lessons

The shipped profiles emphasize Web, headless, SDK, SDK-minimal, and ACP.
Terminal packages expose persistent model-facing PTYs across calls. Sessions
are process-local, output is bounded/paginated, and interruption targets a
verified process group. The tool explicitly lacks named-key/full-screen TUI
operation, resize, BEL, and automatic startup behavior. Sources:
[terminal ownership][terminals] and [terminal tool limits][terminal-tool].

Use the PTY service as execution reference material. The new TUI still needs
its own input, paste, selection, copy, layout, accessibility, and terminal
restoration contract. Closing that screen must have an explicit relationship
to task, writer, and shell lifetime.

DeepSeek's verification design includes lifecycle/disposal tests, keyless
actual-profile replay, workspace mutations, live-provider tests, scaling
checks, and browser snapshots. See [testing policy][testing]. These mechanisms
were inspected, not run here; coverage and snapshot checks do not establish
coding-task parity.

For our composition, add acceptance cases for missing/replaced dependencies,
failed activation, unload during work, two competing writers, inbox resume,
late completion after cancel, transient stream gaps, unavailable replay
schemas, changed plugin releases, and unknown paid effects. Then evaluate
representative complete work with and without optional plugins. This is how
the architecture can earn a default without repeating the historical gap
between a successful demo and a reliable daily workflow.

[license]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/LICENSE#L1-L21
[architecture]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/docs/architecture.md#L9-L29
[profiles]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/boot/app-boot/src/profile.ts#L800-L814
[context]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/vendor/cordis/src/context.ts#L70-L83
[manager]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/boot/plugin-manager/README.md#L14-L97
[dynamic-runner]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/extensions/cordis-host-runner/src/sandbox.ts#L1-L10
[entries]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/vendor/loader/src/config/entry.ts#L9-L23
[registry]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/vendor/cordis/src/registry.ts#L91-L145
[fibers]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/vendor/cordis/src/fiber.ts#L611-L695
[inventory]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/host/plugin-inventory/src/index.ts#L58-L113
[dispose]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/core/agent-loop/src/index.ts#L524-L567
[resume]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/core/agent-loop/src/index.ts#L807-L846
[events]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/docs/architecture.md#L84-L129
[projections]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/session/session-projection/src/index.ts#L40-L115
[attempts]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/core/agent-loop/src/agent.ts#L434-L475
[streams]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/api/session-controller/src/assistant-stream.ts#L23-L100
[event-types]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/core/session/src/known-event-types.ts#L9-L20
[controls]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/core/agent-loop/src/agent.ts#L154-L181
[inbox]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/core/agent-loop/src/inbox.ts#L26-L113
[tools]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/core/tools/src/index.ts#L1063-L1123
[scheduler]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/core/agent-loop/src/tool-calls.ts#L60-L101
[guards]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/core/tools/src/index.ts#L1493-L1535
[repair]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/core/session/src/repair.ts#L44-L97
[compaction]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/compaction/compaction-basic/README.md#L119-L129
[compaction-transaction]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/compaction/compaction-basic/src/region.ts#L158-L275
[overflow]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/compaction/compaction-basic/src/index.ts#L190-L233
[skills]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/skill/skill-filesystem/src/index.ts#L797-L854
[skill-tool]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/skill/tool-skill/src/index.ts#L71-L125
[subagents]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/subagent/subagent/README.md#L107-L121
[depth]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/subagent/subagent/src/depth.ts#L18-L35
[removal]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/boot/plugin-manager/src/index.ts#L617-L644
[replacement]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/boot/app-boot/src/profile-resolution/service.ts#L79-L105
[compatibility]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/boot/app-boot/src/plugin-compatibility.ts#L51-L87
[terminals]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/terminal/terminal/README.md#L12-L134
[terminal-tool]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/terminal/tool-terminal/README.md#L166-L175
[testing]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/docs/testing.md#L9-L55
[tool-contract]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/core/tools/src/index.ts#L260-L297
[execution-world]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/docs/architecture.md#L131-L135
[tool-values]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/core/tools/src/index.ts#L1831-L1861
[tool-cancel]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/core/agent-loop/src/tool-calls.ts#L199-L259
[compaction-limits]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/compaction/compaction-basic/README.md#L12
[skill-loading]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/skill/skill-filesystem/src/index.ts#L186-L224
[skill-catalog]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/skill/tool-skill/src/index.ts#L28-L41
[activation]: https://github.com/deepseek-ai/deepseek-harness/blob/5badb15009ae1756c3afe0ae0cef1faafc290ccc/packages/subagent/subagent/src/continuation-activation.ts#L40-L55
