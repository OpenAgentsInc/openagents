# Frontend UI and engine boundary

Status: selected design, 2026-10-22. No frontend implementation or dependencies
are added by this specification. [Creator experience](creator-experience.md)
owns the interaction design; [architecture](architecture.md) owns process and
trust boundaries; [decision 0005](decisions-history/0005-react-effect-frontend.md) records
this choice.

## Selected stack

| Layer | Choice | Owns |
| --- | --- | --- |
| Editor components | React DOM, TypeScript, CSS | Hotbar, contextual panels, asset search, command field, accessible outline, diagnostics |
| Reactive UI state | Effect Atom with `@effect/atom-react` | Selection, active tool, drafts, derived inspector values, service results, save/build status |
| Application services | Effect 4 | Typed command submission, project access, worker connections, imports, compilation jobs, cancellation, persistence, collaboration |
| World view | Owned direct WebGPU renderer in the isolated preview | Camera integration, frame preparation, GPU resources and submission |
| Simulation and compilation | Rust/Wasm kernel and compiler | Fixed-step state, physics, animation, navigation, semantic validation and derived artifacts |

Effect Atom provides reactive state and Effect-backed values; React DOM renders
the controls through the official [Atom React bindings](https://github.com/Effect-TS/effect/blob/main/packages/atom/react/README.md).
Effect 4 supplies the application service layer. Vite remains the initial editor
development/build tool. Exact compatible React, Effect 4, and Atom binding
versions are pinned together at M0. The [Atom API](https://effect.website/docs/v4/api/effect/reactivity/Atom)
is currently marked unstable, so its integration stays behind an owned
editor-state adapter. Portable engine contracts do not expose atom handles or
React component identities.

This choice applies to editor controls. Authored gameplay UI remains the bounded
declarative model in [game systems](game-systems.md); it cannot inject components
or acquire editor state through this frontend adapter.

## State ownership

Components subscribe to the smallest relevant values. Examples include selected
semantic IDs, available actions, one object's inspected properties, catalog
search results, a source draft, and the latest command/build result. Pure derived
atoms combine these values without copying a whole project into each control.
Short-lived presentation state can remain component-local.

The accepted project document remains under the command authority. Simulation
state remains in the kernel. Atoms expose read-only views of those authorities;
writing a UI value cannot mutate authored topology, physics, or a release channel.
A draft field is provisional until its validated command succeeds. Display the
accepted revision, candidate status, and active generation separately.

Use a project-scoped atom registry and Effect service scope, with explicit disposal
on project close. Scope private results and subscriptions by project, branch, and
session identity; switching projects cannot expose the previous project's values.
Revocation clears private views and stops privileged operations through the
existing [security broker](security.md). Persistence of source and drafts follows
[operations](operations.md), rather than depending on an atom cache surviving.

## Commands and input

The editing path is **control → Effect command service → validated transaction →
compiler/runtime → bounded result/status events → atoms → components**. The
command service supplies authenticated context and handles receipts, conflicts,
retries, and cancellation under [contracts](contracts.md). A component render,
derived atom read, or automatic subscription must not submit an edit or publish.

The host input adapter forwards movement/look input through the existing bounded
input bridge. It does not require a React state update for each pointer event or
camera frame. Focus, mode, pointer-lock, and visibility changes clear input under
the creator interaction rules. Low-latency visual handles and target outlines
belong to the preview renderer; DOM controls send typed intentions.

All editor-to-preview traffic crosses the authenticated bridge between isolated
origins. Atoms cannot inspect kernel memory, obtain a preview DOM handle, or bypass
capability checks. Selecting a target provides its stable ID and permitted summary,
not a mutable reference to an engine object.

## Independent frame loop

The baseline runtime worker owns fixed-step simulation and GPU encoding together.
Physics, camera integration, animation, interpolation, frame scheduling, and GPU
submission must not wait for React rendering, a React hook, an atom notification,
or an inspector subscription. Neither one atom per live entity nor a per-frame
full-world snapshot is an acceptable engine representation. Effect scopes own
session resources at useful granularity; fibers do not represent individual draws
or vector operations.

The preview advances without any mounted inspector or runtime-data subscriber.
The engine publishes requested summaries through bounded queues keyed by session,
generation, tick, and request identity. Continuous diagnostic/pose views default
to at most ten UI updates per second and coalesce superseded values. Selection
and current status use latest-value delivery; command outcomes remain recoverable
through the durable receipt/status lookup in [contracts](contracts.md), independent
of notification delivery. Faults and save/activation notifications use a separate
bounded stream with cursors and gap detection. After a gap, obtain retained
lifecycle history and current status; report an unavailable interval explicitly
when retention has expired. Bounded diagnostics expose truncation. A missing event
cannot imply save or activation success. Slow consumers never require an unbounded
reliable worker queue or backpressure a tick or GPU submission. Future higher-rate
views require an explicit resource profile.

This scheduling boundary does not promise immunity to browser suspension,
compositor delays, or shared CPU/GPU contention. Measure UI and engine costs
separately and together under [performance](performance.md). Explicit pause,
visibility, and shutdown policies still apply.

## Lifecycle and recovery

An explicit project/session service owns the preview worker and connections.
Mounting a panel subscribes to data; unmounting it releases that subscription.
The last inspector subscriber disappearing cannot stop or recreate the preview.
Changing panel keys cannot transfer the canvas again or reset player state.
Viewport attachments use explicit surface identities and idempotent attach/detach
operations, separately from the engine session's lifetime.

Application bootstrap keeps the isolated preview browsing context and its stable
surface identity outside remountable panels and UI-only hot-refresh subtrees.
Retaining an Effect service handle cannot keep a worker alive after its owning
browsing context is removed. A genuine preview-context or full-page reload uses
checkpoint/reconstruction recovery under [runtime](runtime.md); it cannot promise
that the same worker, GPU objects, or uncheckpointed state survives.

Keep component rendering pure. Development [React Strict Mode](https://react.dev/reference/react/StrictMode)
can rerun rendering and effect setup/cleanup; the service lifecycle must tolerate
that without duplicate workers, commands, GPU devices, or event handlers. Panel
errors have local recovery while the valid preview continues. A true preview
failure follows runtime recovery; a deliberate project close shuts down its
resources and resolves outstanding save/command outcomes explicitly.

## Acceptance requirements

- **FE-01:** React DOM renders the world-first accessible controls; Effect Atom
  connects their UI state to Effect 4 services through one owned adapter.
- **FE-02:** Direct controls, semantic commands, and Kiln use the same transaction
  service and produce equivalent validated outcomes.
- **FE-03:** Simulation/frame counters advance without UI-data subscriptions or
  React commits, subject to the declared browser lifecycle. Slow UI consumers
  cannot create a wait dependency or unbounded runtime-to-UI queue.
- **FE-04:** Continuous view updates meet their rate/size bounds; obsolete session
  or generation responses cannot overwrite a current selection or status.
- **FE-05:** Strict Mode, panel remounts, UI errors, and UI-only hot refresh retain
  the preview browsing context, one session, and compatible pose/state. Actual
  context reload follows checkpoint recovery; explicit close disposes workers,
  scopes, subscriptions, and input handlers.
- **FE-06:** Project switches and revoked grants remove private cached views;
  draft persistence, save labels, and recovery remain accurate.
- **FE-07:** Keyboard, touch, screen-reader, focus, and pointer-lock flows reach
  the same operations and keep movement input separate from text entry.
- **FE-08:** UI update storms, inspector churn, and slow result consumption are
  included in integrated latency, frame, and endurance measurements. Architecture
  compliance and measured performance both pass before qualification.
