# Native task integration for the compiled iOS chat

The owner assigned the replacement iOS screen's wiring to #13645 on
2026-11-16. #13620 supplies the server, resident, shared client state and CLI.
Do not restore the removed JavaScriptCore/UIKit projection to obtain its task
screens. Its real staging receipts are historical qualification of that
carrier and the shared execution contract.

Reuse `apps/shared/durable-chat.coax` and `apps/shared/chat-task-app.coax`.
The prior Coax adapters now live in
`apps/openagents-desktop/home/durable.coax` and `native-task.coax`; they show
the response-to-reducer mapping. The compiled phone's `AppEffects` owns HTTP,
account signing, entropy, cancellation and wakes. Project those effects from
the shared reducers and paint their state through the GPU component kit.

The flow remains one saved conversation and turn:

1. Submit through the durable v2alpha1 API with an owned operation ID. Keep
   the exact body after an uncertain write; reconnect to that turn's event
   cursor rather than submitting again.
2. Read its native association with `GET /api/chat/turn-task/v1alpha1?id=TURN`.
   A 404 before routing finishes means no association yet. It does not
   authorize another Start. Retain the returned actual host, engine,
   provider account, model, effort and task IDs.
3. Use the existing account Task and Transcript endpoints from
   [the execution contract](../protocol/environment-execution-v1alpha1.md).
   Render correlated native calls/results, the current held permission and
   the actual native session. Imported transcript data never authorizes an
   approval or an executable action.
4. Allow/Deny must name the exact held interaction and observed revision.
   Keep their command ID and complete bytes for retry. Stop is a separate
   retained intent; only native terminal observation confirms cancellation.
   A completed task racing Stop remains completed. Unknown stays unknown.
5. On cold reopen, recover the saved association and pending permission.
   Clear private state on account change, and pause foreground polling under
   backgrounding and navigation. Use the existing bounded wait/backoff.

The same functionality is available through `openagents chat` and
`openagents tasks`: inspect the native association, task and verified complete
transcript; respond to permissions; Stop; and retry the original retained
command. Projects provide pinned instructions and knowledge, independently
of computer grants.

The installed phone qualification's source is retained as a text artifact at
`docs/verification/fixtures/13620-native-controls.swift.txt`. It uses the old
carrier and is not a current iOS target. The replacement screen needs its own
GPU controls, lifecycle and installed-app observations.
