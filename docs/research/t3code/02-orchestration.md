# Orchestration, persistence and conversation ownership

## The identities are deliberately different

T3 separates an application thread from a provider's native thread and from a
live process. Treating them as one chat ID would break provider switching,
restart recovery and subagent routing. The following are T3 concepts, not new
OpenAgents wire definitions. [V2 contracts][contracts], [adapter contract][adapter].

| Concept | Meaning |
| --- | --- |
| Environment | Execution host and durable-state boundary |
| Provider driver kind | Integration implementation, such as `codex` or `claudeAgent` |
| Provider instance | Configured account/binary/home/environment for that driver |
| Application thread | User-visible conversation owned by T3 |
| Provider session | Runtime attachment to an instance; can cover one or several provider threads depending on capability |
| Provider thread | Native conversation binding and metadata for a T3 thread |
| Run / attempt | T3 execution and an attempt to perform it |
| Provider turn | An agent-native unit of execution associated with a run |
| Node / turn item | Execution structure and transcript content such as text, tools, plans or requests |
| Runtime request | A request for approval/input, including how it can be answered |

Driver factories receive an `instanceId`, settings and resolved environment.
They return an orchestration adapter, status snapshot, text-generation service
and optional auth/usage services. Mutable state belongs to the instance. The
adapter registry looks up an instance, not merely a model/provider string, and
guards session opening while its credential binding is changing.
[Driver interface][driver], [adapter registry][registry].

## Mobile sends have their own durable outbox

The phone does not rely on a socket write as proof a message was sent. Mobile
stores queued messages in per-message files through atomic writes, including
stable message/command identity. It tracks in-flight persistence and exposes
a flush before an app-update restart. The delivery worker prepares/uploads
attachments, checks permissions and whether the queued payload revision is
still current, then calls the shared command operations with those identities.
It tracks retries and acknowledgment/removal separately. A queued new-thread
launch has a separate path from a send into an existing thread.
[Outbox storage][outbox-storage], [delivery worker][outbox-delivery].

The shared command layer builds application commands such as `message.dispatch`,
including a `commandId`, rather than constructing native Claude/Codex frames
on the phone. Server-side queued runs and the phone's not-yet-delivered outbox
are different queues. This is the operation-owned retry path mentioned in the
connection chapter; it does not make the low-level RPC transport replay every
mutation. [Command operations][commands], [V2 contracts][contracts].

## A complete message round trip

```mermaid
sequenceDiagram
    participant App
    participant RPC as T3 RPC server
    participant O as Orchestrator / intake
    participant DB as SQLite
    participant EW as Effect worker / session manager
    participant P as Provider adapter / agent
    App->>RPC: Dispatch command (stable command identity)
    RPC->>RPC: Authenticate session and authorize method
    RPC->>O: Validate and decide command
    O->>DB: Commit events + projections + receipt + effects
    DB-->>O: Transaction committed
    O-->>App: Command receipt
    DB-->>EW: Claim durable effect
    EW->>P: Ensure/resume native thread; start turn
    P-->>EW: Text/tool/request/lifecycle events
    EW->>DB: Normalize and persist ordered updates
    DB-->>App: Publish committed subscription updates
    App->>RPC: Respond to request / interrupt / next message
```

The server's dispatch handler routes message intake and orchestration commands.
The orchestrator serializes decisions; it does not run provider or filesystem
effects inside the decision itself. `EventSink` commits events, projection
changes, the accepted-command receipt and effect-outbox entries transactionally.
Live subscribers receive committed data after commit. Consequently, a successful
send acknowledgment means T3 durably accepted intent, not that the coding agent
has finished, or even that native turn startup has succeeded.
[RPC dispatch][ws], [orchestrator][orchestrator], [event sink][sink],
[architecture overview][overview].

`EffectWorker` claims effects and performs side effects through services such as
`RunExecutionService` and `ProviderSessionManager`. Results feed back into the
domain. Command receipts make repeated commands deduplicate even after older
events have been pruned. This is not a guarantee of exactly-once external tool
execution: a provider can perform a side effect before a crash makes its outcome
uncertain. Recovery must reason about that boundary.
[Worker][worker], [receipts][receipts], [recovery][recovery].

## Native protocol normalization

`ProviderAdapterV2` defines session opening, capabilities, selection transitions,
ensure/resume thread, start/steer/interrupt turn, respond to requests, snapshots,
rollback and fork. Optional hooks cover compaction, background tasks, subagents,
model changes, feedback and MCP apps. Events carry normalized identities and
domain data. This interface is substantially larger than `send(prompt)`.
[Adapter contract][adapter].

The client renders messages, tool execution/output, reasoning, plans, questions,
approvals and run state from T3's common contracts. Raw provider frames remain a
server concern. Adapters preserve native request/option identities so a UI label
does not accidentally become an invalid native response. Capability metadata
also keeps an unsupported action from being offered as though all agents had
identical semantics. [Contracts][contracts], [provider guidance][providers],
[provider extension guide][adding].

Text streaming and lifecycle are separate. A text delta is not turn completion;
completion is not necessarily the end of background work. `RunFinalizationService`
handles follow-up checkpoint/diff work separately, so late finalization does not
inflate provider execution duration or leave the UI reporting live generation.
[Finalization][finalization], [overview][overview].

## Sessions, queued messages and background work

`ProviderSessionManager` owns runtime entries, scopes, event subscriptions,
attached thread sets, idle release/unload and pending background work. It
supports broadcasting a shared runtime's events to independent thread
subscriptions. A Codex instance can serve multiple native threads on one
app-server; Claude declares one provider thread per session. Those are different
resource/lifetime models behind the same client contract.
[Session manager][manager], [Claude capabilities][claude], [Codex capabilities][codex].

Message intake and execution policy decide whether a message starts work, queues
behind an active run or steers an existing one. The adapter must distinguish
true native steering from interrupt/restart behavior. Native acknowledgments can
arrive before a native turn has actually started, so interrupts cannot assume
that a returned turn ID already identifies running work. This is especially
visible in Codex's queued-turn handling. [Intake][intake], [execution][execution],
[Codex adapter][codex].

Subagents/background tools can outlive the foreground provider turn. The runtime
exposes pending-work probes and continuation mechanisms. Events must route to
the actual owning subagent/native thread, rather than whichever top-level run
is currently active. Session release may clear tasks only when the integration
can assert that they end with its process. A disconnected phone does not own
this process lifetime. [Adapter contract][adapter], [manager][manager],
[continuations][continuations], [adding providers][adding].

## Approvals and questions are not just chat text

There are two especially important response mechanisms:

| Mechanism | Response path | Lifetime |
| --- | --- | --- |
| Live native approval or blocking question | Resolve the provider's outstanding callback/RPC using preserved native IDs/options | Native process/request must still exist; persistence cannot resurrect a callback |
| Codex asynchronous question | Resolve persisted request and post a new application user message | Can survive completed turns, provider exit and server restart |

The second path uses `responseCapability: { type: "message" }`. The server
validates required answers against the stored question, resolves it and posts
the user message in one transaction. Repeating the command returns its receipt,
not a second answer message. The ordinary intake path then decides start,
resume, queue or steer. An unresolved asynchronous question need not block a run.
See [provider protocol traps][providers] and the [Codex adapter][codex].

Both Claude and Codex declare that their **live** pending requests do not
survive restart. That is compatible with the durable asynchronous path; it is
not a contradiction or permission to replay stale approval replies. UI state
must model response capability and request lifetime explicitly.
[Claude capabilities][claude], [Codex capabilities][codex].

## Durable state and reconnect

The environment stores T3 projections, events, command receipts and outbox state
in SQLite. Provider-native transcripts/session files are a separate store owned
by the agent. T3's client caches are a third copy with different authority and
lifetime. None is an interchangeable replacement for the others.
[Server paths][config], [overview][overview], [native state comparison](04-codex.md).

The event log is a bounded reconnect/retry history, roughly a day according to
the overview; it is **not an indefinitely retained event-sourcing log from which
the entire database can be rebuilt**. Projections remain authoritative.
Maintenance retains particular anchors/high-water marks and durable command
receipts while pruning older covered events. Thread subscriptions choose replay
or snapshot based on retained history and recheck prune boundaries around the
replay-to-live transition. [Maintenance][maintenance], [thread subscription][ws],
[overview][overview].

Restart recovery distinguishes durable intent from effects bound to a dead
process. It repairs interrupted runtime state and retires obsolete process-bound
effects instead of blindly starting everything again. Continuation is
provider/capability-dependent. A surviving native transcript does not imply
surviving in-flight tools or approval callbacks. [Recovery service][recovery],
[restart continuation][restart].

## Native continuity versus portable handoff

Same-provider native resume/fork/rollback can preserve the provider's own
conversation structure, within that provider's capabilities and compatible
continuation identity. Switching providers uses `ContextHandoffServiceV2`.
That handoff summarizes eligible user/assistant messages, commands, file
changes, checkpoints and prior handoffs. Text is whitespace-compacted and
limited to 240 characters per text-bearing item; file changes retain filenames,
checkpoints retain a file count. Other item types are omitted.
[Handoff notes][handoff-notes], [handoff service][handoff].

The service can use all eligible history or a delta since the destination
provider last participated; merge-back uses eligible child deltas. A handoff
does not transfer native context, full omitted text, tool state, live approvals
or caches. “Supports full-thread handoff” describes source coverage, not lossless
transfer. The separate legacy-v1 migration suffix has a 32,000-character budget;
that is not the portable-handoff algorithm. [Handoff notes][handoff-notes].

## Checkpoints and revert

T3 checkpoints filesystem state using hidden Git refs, rather than inserting
every checkpoint into the user's branch history. Revert coordinates filesystem
restoration with provider conversation rollback. A provider that cannot roll
back its native conversation must be rejected before filesystem mutation;
Antigravity is the documented example. Fork and provider switch likewise need
capability and continuation checks rather than assuming a common transcript is
enough. [Checkpoint store][checkpoints], [overview][overview], [providers][providers].

[contracts]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/contracts/src/orchestrationV2.ts
[adapter]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-core/src/server/ProviderAdapter.ts
[driver]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-core/src/server/driver.ts
[registry]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/ProviderAdapterRegistry.ts
[ws]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/ws.ts#L1949
[orchestrator]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Orchestrator.ts
[sink]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/EventSink.ts
[worker]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/EffectWorker.ts
[receipts]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/CommandReceiptStore.ts
[overview]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/overview.md
[providers]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/providers.md
[adding]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/adding-a-provider.md
[manager]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/ProviderSessionManager.ts
[claude]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/ClaudeAdapterV2.ts#L489
[codex]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/CodexAdapterV2.ts#L255
[intake]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/ThreadMessageIntake.ts
[execution]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/RunExecutionService.ts
[continuations]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/ProviderContinuationService.ts
[finalization]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/RunFinalizationService.ts
[config]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/config.ts
[maintenance]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/ProjectionMaintenance.ts
[recovery]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/ProviderRuntimeRecoveryService.ts
[restart]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/RestartContinuation.ts
[handoff-notes]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/context-handoffs.md
[handoff]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/ContextHandoffService.ts
[checkpoints]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/checkpointing/CheckpointStore.ts
[outbox-storage]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/mobile/src/state/thread-outbox-storage.ts
[outbox-delivery]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/mobile/src/state/threadOutboxDelivery.ts#L923
[commands]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/client-runtime/src/operations/commands.ts
