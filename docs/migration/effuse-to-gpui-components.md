# Effuse to GPUI Component Migration Guide

> **Created:** 2025-12-09
> **Purpose:** Detailed component-by-component migration guide from Effuse (TypeScript) to GPUI (Rust)
> **Prerequisites:** Read [gpui-complete-guide.md](./gpui-complete-guide.md) and [rust-migration-plan.md](./rust-migration-plan.md)

---

## Table of Contents

1. [Overview](#overview)
2. [Component Inventory](#component-inventory)
3. [Architecture Comparison](#architecture-comparison)
4. [Migration Patterns](#migration-patterns)
5. [Component-by-Component Analysis](#component-by-component-analysis)
6. [Example Implementations](#example-implementations)

---

## Overview

This document provides a comprehensive guide for migrating all Effuse components to GPUI. Each component is analyzed with:

- **Current Implementation:** TypeScript/Effuse structure
- **GPUI Equivalent:** Rust/GPUI structure
- **State Management:** Entity mapping
- **Event Handling:** Action system vs event delegation
- **Styling:** Tailwind classes → GPUI style methods
- **Dependencies:** Service requirements

---

## Component Inventory

### Core UI Components (15 total)

| Component | File | State Complexity | UI Complexity | Priority |
|-----------|------|------------------|---------------|----------|
| **APM Widget** | `apm-widget.ts` | Low | Medium | P1 |
| **MC Tasks** | `mc-tasks.ts` | Medium | High | P0 |
| **TB Controls** | `tb-controls.ts` | Medium | Medium | P0 |
| **TB Output** | `tb-output.ts` | Low | Medium | P0 |
| **TB Results** | `tb-results.ts` | Medium | High | P0 |
| **TB Learning** | `tb-learning.ts` | Medium | Medium | P1 |
| **Category Tree** | `category-tree.ts` | Medium | High | P2 |
| **Trajectory Pane** | `trajectory-pane.ts` | Low | Medium | P2 |
| **Container Panes** | `container-panes.ts` | Medium | Medium | P2 |
| **HF Trajectory List** | `hf-trajectory-list.ts` | Medium | High | P2 |
| **HF Trajectory Detail** | `hf-trajectory-detail.ts` | Low | Medium | P2 |
| **ATIF Details** | `atif-details.ts` | Low | Medium | P3 |
| **ATIF Thread** | `atif-thread.ts` | Medium | Medium | P3 |
| **Intro Card** | `intro-card.ts` | Low | Low | P3 |
| **Three Background** | `three-background.ts` | Low | High | P4 |

### Complex Components (Sub-systems)

| Component | Directory | Files | Complexity | Notes |
|-----------|-----------|-------|------------|-------|
| **TB Command Center** | `tb-command-center/` | 7 files | Very High | Multi-view dashboard system |
| **Agent Graph** | `agent-graph/` | 6 files | High | D3.js force simulation |
| **TestGen Graph** | `testgen-graph/` | 5 files | High | Canvas-based rendering |
| **Commander** | `commander/` | 2 files | Medium | Main app shell |
| **New Shell** | `new-shell/` | 2 files | Medium | Alternative shell |

### Widgets (2 total)

| Widget | File | Purpose |
|--------|------|---------|
| **Intro Card** | `widgets/intro-card.ts` | Welcome screen |
| **Three Background** | `widgets/three-background.ts` | 3D animated background |

---

## Architecture Comparison

### Effuse Component Structure

```typescript
interface Component<S, E, R> {
  id: string
  initialState: () => S
  render: (ctx: ComponentContext<S, E, R>) => Effect<Template>
  setupEvents: (ctx: ComponentContext<S, E, R>) => Effect<void>
  handleEvent: (event: E, ctx: ComponentContext<S, E, R>) => Effect<void>
  subscriptions?: (ctx: ComponentContext<S, E, R>) => Stream<Effect<void>>[]
}
```

**Key Characteristics:**
- Effect-based async operations
- Template strings with `html` tagged template
- Event delegation via `ctx.dom.delegate()`
- StateCell for reactive state
- Stream-based subscriptions (WebSocket, etc.)
- Service injection via Effect Context

### GPUI View Structure

```rust
struct MyView {
    state: Entity<MyState>,
    focus_handle: FocusHandle,
}

impl Render for MyView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .child(/* ... */)
            .on_click(cx.listener(|this, event, window, cx| {
                // Handle event
            }))
    }
}
```

**Key Characteristics:**
- Entity system for state management
- Declarative element builders (`.flex()`, `.child()`, etc.)
- Direct event handlers via `cx.listener()`
- Async tasks via `cx.spawn()`
- Observation via `cx.observe()`
- Focus management via `FocusHandle`

---

## Migration Patterns

### Pattern 1: Component → View

**Effuse:**
```typescript
export const APMComponent: Component<APMState, APMEvent> = {
  id: "apm-component",
  initialState: () => ({ count: 0 }),
  render: (ctx) => Effect.gen(function* () {
    const state = yield* ctx.state.get
    return html`<div>${state.count}</div>`
  }),
  // ...
}
```

**GPUI:**
```rust
pub struct APMView {
    state: Entity<APMState>,
}

impl Render for APMView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        div().text(format!("{}", state.count))
    }
}
```

### Pattern 2: StateCell → Entity

**Effuse:**
```typescript
yield* ctx.state.update(s => ({ ...s, count: s.count + 1 }))
const state = yield* ctx.state.get
```

**GPUI:**
```rust
self.state.update(cx, |state, cx| {
    state.count += 1;
    cx.notify();
});
let state = self.state.read(cx);
```

### Pattern 3: Event Delegation → Direct Handlers

**Effuse:**
```typescript
setupEvents: (ctx) => Effect.gen(function* () {
  yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (e, target) => {
    const action = (target as HTMLElement).dataset.action
    Effect.runFork(ctx.emit({ type: action }))
  })
})
```

**GPUI:**
```rust
div()
    .on_click(cx.listener(|this, event, window, cx| {
        this.handle_action("increment", cx);
    }))
```

### Pattern 4: Subscriptions → Observations

**Effuse:**
```typescript
subscriptions: (ctx) => {
  const socket = Effect.map(SocketServiceTag, s => s)
  return [
    pipe(
      Stream.unwrap(Effect.map(socket, s => s.getMessages())),
      Stream.filter(msg => msg.type === "apm_update"),
      Stream.map(msg => ctx.state.update(s => ({ ...s, ...msg.data })))
    )
  ]
}
```

**GPUI:**
```rust
// In initialization
cx.spawn(|this, mut cx| async move {
    while let Some(msg) = socket.recv().await {
        if msg.msg_type == "apm_update" {
            this.update(&mut cx, |this, cx| {
                this.state.update(cx, |state, cx| {
                    state.session_apm = msg.session_apm;
                    cx.notify();
                });
            }).ok();
        }
    }
}).detach();
```

### Pattern 5: HTML Templates → Element Builders

**Effuse:**
```typescript
html`
  <div class="flex items-center gap-4">
    <h1 class="text-2xl font-bold">${title}</h1>
    <button data-action="click">Click</button>
  </div>
`
```

**GPUI:**
```rust
div()
    .flex()
    .items_center()
    .gap_4()
    .child(
        div()
            .text_size(px(24.0))
            .font_weight(FontWeight::Bold)
            .text(title)
    )
    .child(
        div()
            .text("Click")
            .on_click(cx.listener(|this, event, window, cx| {
                // Handle click
            }))
    )
```

### Pattern 6: Conditional Rendering

**Effuse:**
```typescript
${state.expanded
  ? html`<div>Expanded content</div>`
  : html`<div>Collapsed</div>`}
```

**GPUI:**
```rust
div()
    .when(state.expanded, |div| {
        div.child(div().text("Expanded content"))
    })
    .when(!state.expanded, |div| {
        div.child(div().text("Collapsed"))
    })
```

### Pattern 7: Lists and Iteration

**Effuse:**
```typescript
const items = state.tasks.map(task => html`
  <div class="task">${task.title}</div>
`)
return html`<div>${joinTemplates(items)}</div>`
```

**GPUI:**
```rust
let mut container = div().flex().flex_col();
for task in &state.tasks {
    container = container.child(
        div().class("task").text(&task.title)
    );
}
container
```

Or use `list()` for virtualized rendering:

```rust
list(cx.view().clone())
    .items(tasks.clone())
    .render_item(|cx, task| {
        div().text(&task.title)
    })
```

---

## Component-by-Component Analysis

### 1. APM Widget (P1)

**Current (Effuse):**
- State: APM metrics, historical data, expanded/collapsed
- Events: `toggleExpand`, `refresh`
- Subscriptions: WebSocket messages for APM updates
- UI: Compact floating widget, expands to show details

**GPUI Migration:**
```rust
pub struct APMState {
    session_apm: f64,
    recent_apm: f64,
    total_actions: u32,
    duration_minutes: f64,
    apm_1h: f64,
    apm_6h: f64,
    apm_1d: f64,
    apm_lifetime: f64,
    claude_code_apm: f64,
    mechacoder_apm: f64,
    efficiency_ratio: f64,
    expanded: bool,
}

pub struct APMView {
    state: Entity<APMState>,
}

impl Render for APMView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let color_class = get_apm_color(state.session_apm);

        if !state.expanded {
            // Compact view
            div()
                .fixed()
                .bottom_4()
                .right_4()
                .rounded_xl()
                .border_1()
                .px_4()
                .py_3()
                .shadow_lg()
                .bg(rgb(0x1a1a1a).opacity(0.8))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.state.update(cx, |state, cx| {
                        state.expanded = !state.expanded;
                        cx.notify();
                    });
                }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .text_size(px(24.0))
                                .font_weight(FontWeight::Bold)
                                .text_color(color_class)
                                .text(format!("{:.1}", state.session_apm))
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(rgb(0x888888))
                                .text("APM")
                        )
                )
        } else {
            // Expanded view
            // ... (full implementation in examples)
        }
    }
}
```

**Migration Complexity:** Medium
- State mapping: Direct 1:1
- Event handling: Simple toggle
- Subscription: Need WebSocket integration
- Styling: Tailwind → GPUI methods

### 2. MC Tasks (P0)

**Current (Effuse):**
- State: Task list, loading, error, collapsed, assigning
- Events: `load`, `toggleCollapse`, `assign`
- UI: Table with priority badges, type labels, assign buttons
- Service: SocketServiceTag for loading tasks and assigning

**GPUI Migration:**
```rust
pub struct MCTask {
    id: String,
    title: String,
    description: String,
    status: String,
    priority: u8,
    task_type: String,
    labels: Vec<String>,
    created_at: String,
    updated_at: String,
}

pub struct MCTasksState {
    tasks: Vec<MCTask>,
    loading: bool,
    error: Option<String>,
    collapsed: bool,
    max_display: usize,
    assigning_id: Option<String>,
}

pub struct MCTasksView {
    state: Entity<MCTasksState>,
    socket: Arc<dyn SocketService>,
}

impl Render for MCTasksView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(rgb(0x333333))
            .child(
                div()
                    .text_color(rgb(0xffffff))
                    .font_weight(FontWeight::Bold)
                    .text(format!("Ready Tasks ({})", state.tasks.len()))
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .text(if state.loading { "Loading..." } else { "Refresh" })
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.load_tasks(cx);
                            }))
                    )
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.state.update(cx, |state, cx| {
                    state.collapsed = !state.collapsed;
                    cx.notify();
                });
            }));

        if state.collapsed {
            return div()
                .rounded_2xl()
                .border_1()
                .child(header);
        }

        // Task table
        let mut table = div()
            .rounded_2xl()
            .border_1()
            .child(header);

        for task in state.tasks.iter().take(state.max_display) {
            table = table.child(render_task_row(task, &state.assigning_id, cx));
        }

        table
    }
}

impl MCTasksView {
    fn load_tasks(&mut self, cx: &mut Context<Self>) {
        let socket = self.socket.clone();
        self.state.update(cx, |state, cx| {
            state.loading = true;
            state.error = None;
            cx.notify();
        });

        cx.spawn(|this, mut cx| async move {
            match socket.load_ready_tasks(50).await {
                Ok(tasks) => {
                    this.update(&mut cx, |this, cx| {
                        this.state.update(cx, |state, cx| {
                            state.tasks = tasks;
                            state.loading = false;
                            cx.notify();
                        });
                    }).ok();
                }
                Err(e) => {
                    this.update(&mut cx, |this, cx| {
                        this.state.update(cx, |state, cx| {
                            state.error = Some(e.to_string());
                            state.loading = false;
                            cx.notify();
                        });
                    }).ok();
                }
            }
        }).detach();
    }
}
```

**Migration Complexity:** High
- State mapping: Direct 1:1
- Event handling: Multiple async operations
- Service dependency: Need SocketService trait
- Styling: Complex table layout

### 3. TB Controls (P0)

**Current (Effuse):**
- State: Test parameters, docker mode, model selection
- Events: `run`, `stop`, `updateParam`, `toggleDocker`
- UI: Form with inputs, dropdowns, buttons

**GPUI Migration:**
- Input fields need custom components or use GPUI's text input
- Dropdown menus via GPUI's context menu or custom
- Form validation logic
- State updates on input change

**Migration Complexity:** Medium
- State: Simple form state
- Events: Standard form events
- UI: Need input components (GPUI has these)

### 4. TB Output (P0)

**Current (Effuse):**
- State: Log lines, auto-scroll, filter
- Events: `toggleAutoScroll`, `clearLogs`, `updateFilter`
- Subscriptions: WebSocket for log stream
- UI: Scrollable log output with ANSI color support

**GPUI Migration:**
- Use `list()` or `uniform_list()` for virtualized log rendering
- ANSI color parsing → GPUI color codes
- Auto-scroll implementation via scroll position tracking
- Filter logic in render

**Migration Complexity:** Medium
- Virtualized list rendering
- ANSI parsing
- Auto-scroll behavior

### 5. TB Results (P0)

**Current (Effuse):**
- State: Test results, selected test, filter, sort
- Events: `selectTest`, `updateFilter`, `changeSort`
- UI: Table with pass/fail indicators, expandable details

**GPUI Migration:**
- Table rendering with sorting
- Expandable rows (conditional rendering)
- Color coding for pass/fail
- Click handlers for row selection

**Migration Complexity:** High
- Complex table with sorting/filtering
- Expandable details
- Multiple interaction modes

### 6. Agent Graph (Complex)

**Current (Effuse):**
- Uses D3.js force simulation
- Canvas-based rendering
- Complex physics simulation
- Node dragging, zoom/pan

**GPUI Migration:**
- **Option 1:** Port D3 force simulation to Rust (most work)
- **Option 2:** Use existing Rust graph library (e.g., `petgraph`)
- **Option 3:** Use GPUI's `canvas()` element for custom rendering
- Physics simulation in background task
- Mouse events for drag/zoom

**Migration Complexity:** Very High
- Physics simulation port
- Canvas rendering
- Complex interaction model

### 7. TestGen Graph (Complex)

**Current (Effuse):**
- Canvas-based directed graph
- Evolution history visualization
- Node expansion/collapse
- Pan/zoom

**GPUI Migration:**
- Similar to Agent Graph
- Use `canvas()` element
- Custom graph layout algorithm
- Event handling for pan/zoom/click

**Migration Complexity:** Very High
- Custom graph rendering
- Layout algorithms
- Interaction handling

### 8. TB Command Center (Complex)

**Current (Effuse):**
- Multi-view dashboard (7 sub-components)
- Tab navigation
- Shared state across views
- Complex routing

**GPUI Migration:**
- Tab bar component (GPUI has `tab_bar`)
- Multiple view structs (one per tab)
- Shared state via Entity
- Tab switching logic

**Sub-components:**
1. Dashboard (overview)
2. Task Browser (task list)
3. Run Browser (run history)
4. TestGen (test generation UI)
5. Shell (command interface)
6. Settings (configuration)

**Migration Complexity:** Very High
- Multiple coordinated views
- Shared state management
- Complex navigation

---

## Example Implementations

Complete example implementations are provided in separate files:
- [apm-view.rs](./examples/apm-view.rs) - APM Widget
- [mc-tasks-view.rs](./examples/mc-tasks-view.rs) - MC Tasks Component
- [tb-controls-view.rs](./examples/tb-controls-view.rs) - TB Controls
- [log-output-view.rs](./examples/log-output-view.rs) - TB Output with virtualized list

---

## Migration Checklist

For each component:

- [ ] **Define State Struct** - Map Effuse state → Rust struct
- [ ] **Create View Struct** - With Entity<State> field
- [ ] **Implement Render Trait** - Port template → element builders
- [ ] **Map Events** - Event delegation → direct handlers
- [ ] **Port Styling** - Tailwind classes → GPUI methods
- [ ] **Handle Async** - Subscriptions → cx.spawn()
- [ ] **Add Services** - Socket, etc. as fields or context
- [ ] **Write Tests** - Unit tests for state logic
- [ ] **Integration Tests** - UI behavior tests

---

## Common Challenges

### 1. Async State Updates

**Effuse:** Effect.gen with yield* syntax
**GPUI:** cx.spawn() with update() callbacks

Solution: Use `cx.spawn()` and update Entity inside async block

### 2. Event Delegation

**Effuse:** `ctx.dom.delegate()` for event bubbling
**GPUI:** Direct handlers on each element

Solution: Each button/clickable gets its own `on_click()`

### 3. Template Composition

**Effuse:** String concatenation, joinTemplates()
**GPUI:** Builder pattern chaining

Solution: Use loops to build element trees

### 4. Service Injection

**Effuse:** Effect Context system
**GPUI:** Struct fields or App-level context

Solution: Pass services as Arc<dyn Trait> fields

### 5. Subscriptions

**Effuse:** Stream-based with automatic cleanup
**GPUI:** Manual spawn with .detach()

Solution: Spawn in init, store Subscription handle if needed

---

## Performance Considerations

### Virtualized Lists

For large lists (logs, tasks, etc.), use GPUI's virtualization:

```rust
uniform_list(cx.view().clone(), "log-list", items.len(), |cx, range| {
    let items = &items[range.clone()];
    items.iter().map(|item| {
        div().text(&item.text)
    }).collect()
})
```

### State Updates

Minimize re-renders by:
- Only calling `cx.notify()` when state changes
- Using `when()` for conditional rendering
- Caching computed values in state

### GPU Rendering

GPUI renders to GPU - styling is cheap compared to DOM:
- Style changes don't cause layout thrashing
- Animations are smooth (60+ FPS)
- Shadow, blur, etc. are GPU-accelerated

---

## Next Steps

1. **Start with P0 components** (MC Tasks, TB Controls, TB Output, TB Results)
2. **Build reusable patterns** (buttons, inputs, tables)
3. **Create shared components** (badges, tooltips, modals)
4. **Port complex components** (graphs, command center)
5. **Integration testing** with full app

---

**Last Updated:** 2025-12-09
**Status:** Planning Document
**See Also:**
- [gpui-complete-guide.md](./gpui-complete-guide.md)
- [rust-migration-plan.md](./rust-migration-plan.md)
