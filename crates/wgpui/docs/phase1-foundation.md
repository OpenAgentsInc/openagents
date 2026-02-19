# Phase 1 foundation system

This document is the comprehensive reference for the Phase 1 foundation system
added to WGPUI for directive d-025. It covers the entity system, element
lifecycle, window abstraction, styled DSL, and async task support, plus how they
fit together.

## Scope and source files

Phase 1 spans these modules:

- `crates/wgpui/src/app/mod.rs`
- `crates/wgpui/src/app/entity_map.rs`
- `crates/wgpui/src/app/app_context.rs`
- `crates/wgpui/src/app/subscription.rs`
- `crates/wgpui/src/element/mod.rs`
- `crates/wgpui/src/element/element.rs`
- `crates/wgpui/src/element/into_element.rs`
- `crates/wgpui/src/element/render.rs`
- `crates/wgpui/src/element/any_element.rs`
- `crates/wgpui/src/element/drawable.rs`
- `crates/wgpui/src/window/mod.rs`
- `crates/wgpui/src/window/window.rs`
- `crates/wgpui/src/window/window_handle.rs`
- `crates/wgpui/src/window/invalidator.rs`
- `crates/wgpui/src/window/dispatch.rs`
- `crates/wgpui/src/styled/mod.rs`
- `crates/wgpui/src/styled/style.rs`
- `crates/wgpui/src/styled/refinement.rs`
- `crates/wgpui/src/styled/styled.rs`
- `crates/wgpui/src/styled/helpers.rs`
- `crates/wgpui/src/async/mod.rs`
- `crates/wgpui/src/async/task.rs`
- `crates/wgpui/src/async/executor.rs`

WGPUI core modules (layout, scene, text, input, etc.) are pre-existing and
integrated by Phase 1, but are not redefined here.

## High-level architecture

Phase 1 introduces a reactive state system (App + Entity), a GPUI-style element
lifecycle, and a window abstraction that ties layout, prepaint, and paint to a
single render pass.

```
App (entities, observers, tasks)
   |
   | Render / RenderOnce
   v
Element tree (request_layout -> prepaint -> paint)
   |
   v
Window (layout + dispatch + scene + text)
   |
   v
Scene -> Renderer (outside Phase 1)
```

Key separation of concerns:

- `App` owns state and reacts to changes via `notify`.
- `Element` produces layout, hit-test data, and draw commands.
- `Window` orchestrates the pipeline and collects render output.

## Entity system

### Goals

- Stable typed handles to state (`Entity<T>`) that can be cloned.
- Centralized storage with ref counting and deterministic cleanup.
- Reactive invalidation via `notify` and observers.

### Core types

- `EntityId`: slotmap key for entities, convertible to `u64`/`NonZeroU64`.
- `Entity<T>`: typed handle, internally wraps `AnyEntity`.
- `WeakEntity<T>`: weak handle, upgrades to `Entity<T>` if still alive.
- `AnyEntity`/`AnyWeakEntity`: type-erased handles with runtime `TypeId`.

Type-erased helpers:

- `AnyEntity::downcast<T>` returns a typed `Entity<T>` or the original `AnyEntity`.
- `AnyWeakEntity::upgrade` returns `None` if the entity has been released.
- `AnyWeakEntity::is_upgradable` is a fast check for availability.

### Storage and ref counts

`EntityMap` stores entities in a `SecondaryMap<EntityId, Box<dyn Any>>`, and
tracks ref counts in `EntityRefCounts` (a `SlotMap` of `AtomicUsize`).

Key behaviors:

- `reserve::<T>()` creates a new `EntityId` and returns a `Slot<T>`.
- `insert(slot, value)` stores the entity and returns the typed handle.
- `read(&Entity<T>)` performs a type-safe read from storage.

`EntityMap::assert_valid_context` (debug-only) ensures that an entity handle is
used with the `App` instance it originated from.

### Leasing for mutation

Mutations go through a lease to enforce exclusive access:

- `EntityMap::lease(&Entity<T>)` removes the entity from storage and returns a
  `Lease<T>`.
- `EntityMap::end_lease(lease)` reinserts the entity.
- Dropping a `Lease<T>` without calling `end_lease` panics.

This prevents reads while a mutable borrow is active and catches
"double lease" errors (panic from `double_lease_panic`).

### Access tracking

`EntityMap` maintains `accessed_entities` for external invalidation tracking.

- `extend_accessed` and `clear_accessed` are provided for batching.
- `read` and `lease` both mark an entity as accessed.

### Drop and release flow

When the last strong handle drops:

- `AnyEntity::drop` decrements the ref count.
- If the count hits zero, the entity id is queued in `dropped_entity_ids`.
- `EntityMap::take_dropped` drains those ids and returns the stored entities.

This is integrated into `App::flush_effects`, which runs release listeners for
entities that were dropped during the current flush cycle.

### Observers and notifications

`App` provides a reactive notification pipeline:

- `App::notify(entity_id)` queues the entity and triggers `flush_effects`.
- `App::observe(&Entity<T>, callback)` registers an observer keyed by entity id.
- `Context::observe` does the same but automatically removes itself when the
  observing entity is dropped.

Observers run inside `flush_effects` until all pending notifications and
callbacks are drained. Re-entrant notifications are supported; the flush loop
continues until both `pending_notifications` and `deferred` are empty.

### Release listeners

`Context::on_release` registers callbacks that run when a specific entity is
released (dropped). This is where you should clean up associated state.

### Subscription management

Subscriptions are RAII handles defined in `app/subscription.rs`:

- `Subscription::new` stores an unsubscribe callback.
- Dropping a `Subscription` unsubscribes.
- `detach` keeps the subscription alive until the emitter is dropped.
- `join` combines two subscriptions into one handle.

`SubscriberSet` is used internally by `App`:

- `insert` returns `(Subscription, activate_fn)` so subscriptions can be created
  before activating.
- `retain` runs callbacks and removes them if the callback returns `false`.

### Threading model

`App` and `SubscriberSet` use `Rc` and `RefCell`, so the entity system is
single-threaded by design. Use the async executors for background work, then
update state on the main thread.

## App and Context API surface

### App

- `App::new` creates a fresh app with empty entity storage and executors.
- `new_entity` allocates a slot, builds the state, and inserts it.
- `update_entity` provides an exclusive mutation lease and ends it for you.
- `read_entity` provides shared read access.
- `notify` queues notifications for observers.
- `observe` registers a callback for an entity id.
- `defer` schedules a callback to run after the current flush cycle.
- `background_executor` and `foreground_executor` expose executors.

### Context<T>

- `entity_id`, `entity`, and `weak_entity` access the current entity handle.
- `notify` triggers observers for the current entity id.
- `observe` subscribes to another entity and auto-unsubscribes if this entity drops.
- `on_release` registers a callback that runs when this entity is dropped.
- `spawn` runs a background task via `BackgroundExecutor`.
- `Context` derefs to `App` for convenience (`Deref` and `DerefMut` are implemented).

## Element system

### Goals

- Separate layout, hit-testing, and drawing into explicit phases.
- Allow elements to hold per-phase state.
- Provide type erasure to store heterogeneous trees.

### Element trait

```
trait Element {
    type RequestLayoutState;
    type PrepaintState;

    fn request_layout(&mut self, cx: &mut LayoutContext) -> (LayoutId, Self::RequestLayoutState);
    fn prepaint(&mut self, bounds: Bounds, request_layout: &mut Self::RequestLayoutState, cx: &mut PrepaintContext) -> Self::PrepaintState;
    fn paint(&mut self, bounds: Bounds, request_layout: &mut Self::RequestLayoutState, prepaint: &mut Self::PrepaintState, cx: &mut ElementPaintContext);
    fn id(&self) -> Option<ElementId> { None }
}
```

Lifecycle ordering is enforced by `Drawable` and should always be:

1. `request_layout`
2. `prepaint`
3. `paint`

### Context types

- `LayoutContext`: wraps `LayoutEngine` and provides `request_layout`,
  `request_leaf`, and `request_measured` helpers. `request_layout` takes a style
  plus child layout ids. `request_measured` accepts a Taffy measure function for
  custom sizing (e.g., text or dynamic content).
- `PrepaintContext`: read-only layout access plus `DispatchTree::register`.
- `ElementPaintContext`: access to `Scene`, `TextSystem`, scale factor, and
  scroll offset. Use `ElementPaintContext::with_scroll_offset` for scrolled
  subtrees.

`ElementPaintContext::component_context` is a bridge to the legacy `Component`
paint API.

### Type erasure

- `Drawable<E>` stores `E` plus phase state (`RequestLayoutState`,
  `PrepaintState`) and enforces phase ordering.
- `AnyElement` stores a `Box<dyn ElementObject>` (internally a `Drawable<E>`)
  and exposes `request_layout`, `prepaint`, and `paint` without generics.

### Render traits

- `Render` is for entities that render repeatedly via `Context<T>`.
- `RenderOnce` is for one-off rendering via `App`.

These traits return `impl IntoElement` but do not yet include a renderer.

### IntoElement

`IntoElement` is a conversion trait for element types. It provides
`into_element` and a convenience `into_any_element` for type erasure.

### Element ids and hit testing

Elements can supply an id via `Element::id`, which `Window::render_root` will
register in the dispatch tree at depth 0. Elements can also register additional
hit targets during `prepaint` with `PrepaintContext::register`.

## Window abstraction

### Goals

- Centralize layout, hit testing, text layout, and draw command collection.
- Provide invalidation signaling and a lightweight handle.

### Window structure

`Window` owns:

- `LayoutEngine` for layout.
- `Scene` for draw commands.
- `TextSystem` for text shaping and glyph caching.
- `DispatchTree` for hit testing.
- `Invalidator` for dirty flags.
- `size` and `scale_factor`.
- `focused` element id.

### Render flow

`Window::render_root` performs a full frame:

1. `begin_frame` clears scene, dispatch, and layout, and consumes invalidation
   flags.
2. `request_layout` on the root element.
3. `layout.compute_layout` with the window size.
4. `prepaint` and dispatch registration.
5. `paint` to populate the scene.

If the root element has an id (`Element::id`), it is registered at depth 0.

### Size and scale updates

- `Window::resize` updates the logical size and requests layout invalidation.
- `Window::set_scale_factor` updates the scale factor, reconfigures `TextSystem`,
  and requests layout invalidation.

### Dispatch tree

`DispatchTree` stores `DispatchNode` entries with bounds and depth. Hit testing:

- Filters nodes whose bounds contain the point.
- Sorts by depth descending.
- Returns an ordered `Hit` list.

Depth is an integer; higher depth means "on top".

### Invalidation

`Invalidator` holds atomic layout/prepaint/paint flags. `WindowHandle` clones
those flags so other systems can request invalidation without owning the window.
`Window::handle` returns a `WindowHandle`.

- `request_layout` sets layout + prepaint + paint dirty.
- `request_prepaint` sets prepaint + paint dirty.
- `request_paint` sets paint dirty.

`Invalidator::take` clears flags and returns `InvalidationFlags`.

### Focus

`Window` tracks a single focused element id with:

- `set_focus(id)`
- `clear_focus()`
- `focus()` getter

Focus handling beyond storage is expected in a later phase.

### Text system and fonts

`TextSystem` is part of `Window` and defaults to Square721 Std Roman, with Vera
Mono embedded for monospace use. Fonts are embedded from
`src/gui/assets/fonts/Square721StdRoman.ttf` and `src/gui/assets/fonts/VeraMono*.ttf`
in `text.rs`.

## Styled DSL

### Style and refinement

`Style` is the resolved style for a component:

- `layout: LayoutStyle`
- `background: Option<Hsla>`
- `border_color: Option<Hsla>`
- `border_width: f32`
- `corner_radius: f32`
- `text_color: Option<Hsla>`
- `font_size: Option<f32>`

`StyleRefinement` is a partial override used by the `Styled` trait. It can be
applied to a `Style` via `apply_to` or converted to a full `Style` via
`resolve`.

### Styled trait

The `Styled` trait exposes fluent helpers that mutate a `StyleRefinement`:

- Layout: `flex`, `flex_row`, `flex_col`, `items_*`, `justify_*`, `gap`,
  `w`, `h`, `w_full`, `h_full`
- Spacing: `p`, `px`, `py`, `m`, `mx`, `my`
- Paint: `bg`, `border`, `border_color`, `border_width`
- Text: `text_color`, `text_size`

### Helpers

`styled/helpers.rs` provides convenience constructors and layout helpers:

- `div()`, `text()`, `button()`
- `px(value)`, `pct(value)`

### Sharp-corner requirement

OpenAgents requires sharp corners. `Style` has `corner_radius` but defaults to
0.0 and should remain 0.0 for UI surfaces.

## Async task system

### Task

`Task<T>` wraps a oneshot channel. It can be:

- Created via `Task::ready(value)` for synchronous results.
- Created via `Task::from_receiver` (internal).
- Polled as a `Future` or via `try_take()`.
- Detached via `detach()`.

If a task is polled after completion, it panics with "task already completed".

### BackgroundExecutor

`BackgroundExecutor::spawn`:

- Uses a native thread on non-wasm targets.
- Uses `wasm_bindgen_futures::spawn_local` for wasm with `web` feature.
- Returns a `Task<R>` for completion.

### ForegroundExecutor

`ForegroundExecutor` wraps a `LocalPool` and runs tasks on the main thread.
You must call `run_until_stalled` to drive tasks to completion.

### App integration

`App` owns both executors:

- `App::background_executor()`
- `App::foreground_executor()`

`Context::spawn` delegates to `background_executor`, returning `Task<R>`.

Use pattern:

- Spawn async work.
- Poll or `await` the task on the main thread.
- Update entities via `App::update_entity` and call `notify`.

## Example workflows

### Entity + observer

```rust
use wgpui::{App, Entity};

struct Counter {
    value: i32,
}

let mut app = App::new();
let counter = app.new_entity(|_cx| Counter { value: 0 });

let _sub = app.observe(&counter, |counter, app| {
    let value = app.read_entity(&counter).value;
    println!("counter changed: {}", value);
});

app.update_entity(&counter, |state, cx| {
    state.value += 1;
    cx.notify();
});
```

### Element + window

```rust
use wgpui::{Bounds, LayoutContext, LayoutStyle, Window, Size, Scene, Element, ElementPaintContext, PrepaintContext, LayoutId, px};

struct Root;

impl Element for Root {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn request_layout(&mut self, cx: &mut LayoutContext) -> (LayoutId, Self::RequestLayoutState) {
        let style = LayoutStyle::new().width(px(200.0)).height(px(80.0));
        (cx.request_leaf(&style), ())
    }

    fn prepaint(&mut self, _bounds: Bounds, _request_layout: &mut Self::RequestLayoutState, _cx: &mut PrepaintContext) -> Self::PrepaintState {
        ()
    }

    fn paint(&mut self, _bounds: Bounds, _request_layout: &mut Self::RequestLayoutState, _prepaint: &mut Self::PrepaintState, _cx: &mut ElementPaintContext) {
    }
}

let mut window = Window::new(Size::new(800.0, 600.0), 1.0);
let mut root = Root;
window.render_root(&mut root);
let _scene: &Scene = window.scene();
```

### Styled usage

```rust
use wgpui::styled::{div, Styled};
use wgpui::theme;

let container = div()
    .flex()
    .flex_row()
    .items_center()
    .justify_between()
    .gap(8.0)
    .bg(theme::bg::SURFACE)
    .border(theme::border::DEFAULT, 1.0);
```

## Invariants and common pitfalls

- Always call `EntityMap::end_lease` (done automatically by `App::update_entity`).
- Do not mutate entities outside `App::update_entity`.
- `App` is single-threaded; do not access it from background threads.
- `Drawable` panics if `prepaint` or `paint` are called out of order.
- `ComponentElement` only uses `Component::size_hint` for layout; it does not
  read the styled layout fields.

## Phase 1 boundaries

Phase 1 provides the foundation but does not yet include:

- Event routing beyond basic hit testing.
- Focus chains or keyboard traversal (planned in later phases).
- A binding layer between `App` and `Window` beyond the `Render` traits.

That work is intentionally deferred to later phases.
