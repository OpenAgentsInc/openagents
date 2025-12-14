# Coder Architecture: Own All Six Layers

This document provides an in-depth exploration of Coder's architecture. We implement every layer from domain model to GPU rendering, giving us complete control over the entire stack.

## Philosophy: Why Own the Stack?

Most UI frameworks abstract away complexity, but abstractions leak. When performance matters, when you need fine-grained control, when debugging requires understanding the whole system—abstractions become obstacles.

By owning all six layers, we achieve:

- **Zero Hidden Magic**: Every line of code is ours to read, understand, and modify
- **Optimized for Our Use Case**: No general-purpose framework bloat
- **Direct GPU Access**: No intermediate layers translating to DOM or native controls
- **Full Type Safety**: Rust's type system guards the entire stack
- **Predictable Performance**: No framework internals with unknown cost

## The Six Layers

### Layer 0: Renderer (`wgpui/`)

**Responsibility**: Low-level GPU rendering primitives.

**Location**: `crates/wgpui/` (shared with Coder, but general-purpose)

**Key Components**:

1. **wgpu Integration**:
   - WebGPU API for cross-platform GPU access
   - Render pipelines for quads, text, images
   - Vertex/index buffers, textures, samplers
   - Command encoder for GPU work submission

2. **Layout Engine (Taffy)**:
   - Flexbox layout algorithm
   - Absolute positioning
   - Size constraints and stretch factors
   - Caching for performance

3. **Text Rendering (cosmic-text)**:
   - Font loading and shaping
   - Glyph rasterization
   - Text measurement and wrapping
   - RTL and bidirectional text support

4. **Scene Abstraction**:
   - High-level drawing API (`draw_quad`, `draw_text`, etc.)
   - Batching for efficient GPU usage
   - Clipping regions
   - Transform stack

**APIs**:
```rust
// Drawing primitives
scene.draw_quad(bounds, fill, stroke, corner_radius);
scene.draw_text(text, position, font, size, color);
scene.draw_image(texture, bounds, tint);

// Layout
let layout = taffy.compute_layout(node, available_space);
let bounds = Bounds::from_layout(layout);
```

**Why Not Use a UI Framework?**
- Direct GPU control for maximum performance
- Custom rendering for markdown, syntax highlighting, terminal cells
- No DOM overhead or CSS parsing
- Predictable, deterministic rendering

---

### Layer 1: Domain Model (`coder/domain/`)

**Responsibility**: Event-sourced domain model providing the source of truth.

**Key Concepts**:

#### Entities

Domain entities represent core business objects:

- **Thread**: A conversation thread
- **Message**: A chat message (User/Assistant/System)
- **Run**: A workflow execution with steps
- **ToolUse**: An agent tool invocation
- **Project**: A code project

Each entity is identified by a strongly-typed UUID:
```rust
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub struct ThreadId(Uuid);

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub struct MessageId(Uuid);
```

#### Events

All state changes are captured as **DomainEvent**:

```rust
pub enum DomainEvent {
    // Thread events
    ThreadCreated { thread_id: ThreadId, title: String, timestamp: DateTime<Utc> },
    MessageAdded { thread_id: ThreadId, message: Message },
    MessageStreaming { thread_id: ThreadId, message_id: MessageId, content: String },
    MessageComplete { thread_id: ThreadId, message_id: MessageId },

    // Tool events
    ToolUseStarted { tool_use_id: ToolUseId, run_id: RunId, ... },
    ToolUseComplete { tool_use_id: ToolUseId, result: ToolOutput, ... },

    // Run events
    RunStarted { run_id: RunId, thread_id: ThreadId, ... },
    RunStepUpdated { run_id: RunId, step_id: StepId, status: StepStatus },
    RunFinished { run_id: RunId, status: RunStatus, cost: Cost },

    // Project events
    ProjectCreated { project_id: ProjectId, name: String },
    ProjectUpdated { project_id: ProjectId, changes: ProjectChanges },
}
```

Events are:
- **Immutable**: Once written, never modified
- **Append-only**: Added to event stream, never deleted
- **Timestamped**: Every event has a `DateTime<Utc>`
- **Ordered**: Sequence numbers via `EventEnvelope`

#### Projections

Projections are **read-optimized views** derived from events:

```rust
pub struct ChatView {
    pub thread_id: ThreadId,
    pub entries: Vec<ChatEntry>,
    pub streaming_message: Option<(MessageId, String)>,
    pub message_count: usize,
    pub last_updated: DateTime<Utc>,
}

impl ChatView {
    pub fn apply(&mut self, event: &DomainEvent) {
        match event {
            DomainEvent::MessageAdded { message, .. } => {
                self.entries.push(ChatEntry::Message(message.clone()));
                self.message_count += 1;
            }
            DomainEvent::MessageStreaming { message_id, content, .. } => {
                self.streaming_message = Some((*message_id, content.clone()));
            }
            DomainEvent::ToolUseStarted { tool_use, .. } => {
                self.entries.push(ChatEntry::ToolUse(tool_use.clone()));
            }
            // ... more event handlers
        }
    }
}
```

**Why Event Sourcing?**
- **Time-Travel Debugging**: Replay events to any point in history
- **Audit Trail**: Complete log of all state changes
- **Distributed Systems**: Events can be replayed on different machines
- **Testability**: Pure functions (event → projection update)
- **Flexibility**: Multiple projections from same event stream

---

### Layer 2: UI Runtime (`coder/ui_runtime/`)

**Responsibility**: Fine-grained reactive runtime for UI state management.

**Inspiration**: Solid.js reactivity system (no virtual DOM, automatic dependency tracking)

#### Core Primitives

**Signal<T>**: Reactive state container

```rust
pub struct Signal<T> {
    value: Arc<RwLock<T>>,
    subscribers: Arc<RwLock<SmallVec<[SubscriberId; 4]>>>,
}

impl<T: Clone> Signal<T> {
    pub fn new(value: T) -> Self { /* ... */ }

    pub fn get(&self) -> T {
        // Register current subscriber
        if let Some(subscriber_id) = Runtime::current_subscriber() {
            self.subscribers.write().push(subscriber_id);
        }
        self.value.read().clone()
    }

    pub fn set(&self, value: T) {
        *self.value.write() = value;
        // Notify all subscribers
        for &subscriber_id in self.subscribers.read().iter() {
            Runtime::notify(subscriber_id);
        }
    }
}
```

**Memo<T>**: Cached derived value

```rust
pub struct Memo<T> {
    compute: Arc<dyn Fn() -> T>,
    cached: Arc<RwLock<Option<T>>>,
    dirty: Arc<AtomicBool>,
    subscriber_id: SubscriberId,
}

impl<T: Clone> Memo<T> {
    pub fn new<F>(compute: F) -> Self
    where
        F: Fn() -> T + 'static
    {
        let memo = Memo {
            compute: Arc::new(compute),
            cached: Arc::new(RwLock::new(None)),
            dirty: Arc::new(AtomicBool::new(true)),
            subscriber_id: Runtime::allocate_subscriber_id(),
        };

        // Run compute() with this memo as the subscriber
        Runtime::with_subscriber(memo.subscriber_id, || {
            let value = (memo.compute)(); // Subscribes to any signals read
            *memo.cached.write() = Some(value);
        });

        memo
    }

    pub fn get(&self) -> T {
        if self.dirty.load(Ordering::Acquire) {
            // Recompute
            Runtime::with_subscriber(self.subscriber_id, || {
                let value = (self.compute)();
                *self.cached.write() = Some(value.clone());
                self.dirty.store(false, Ordering::Release);
            });
        }
        self.cached.read().clone().unwrap()
    }
}
```

**Effect**: Side effect that auto-runs when dependencies change

```rust
pub struct Effect {
    effect_fn: Arc<dyn Fn()>,
    subscriber_id: SubscriberId,
    scope_id: Option<ScopeId>,
}

impl Effect {
    pub fn new<F>(effect_fn: F) -> EffectHandle
    where
        F: Fn() + 'static
    {
        let subscriber_id = Runtime::allocate_subscriber_id();
        let effect = Arc::new(Effect {
            effect_fn: Arc::new(effect_fn),
            subscriber_id,
            scope_id: Runtime::current_scope(),
        });

        // Run immediately
        Runtime::with_subscriber(subscriber_id, || {
            (effect.effect_fn)();
        });

        // Register for future notifications
        Runtime::register_effect(subscriber_id, effect.clone());

        EffectHandle { id: subscriber_id }
    }
}
```

#### Reactive Graph

When you read a signal inside an effect or memo, it automatically subscribes:

```rust
let count = Signal::new(0);
let doubled = Memo::new(move || count.get() * 2); // ← Subscribes to count

Effect::new(move || {
    println!("Count: {}, Doubled: {}", count.get(), doubled.get());
    // ↑ Subscribes to both count and doubled
});

count.set(5); // ← Triggers doubled recompute and effect re-run
```

**Dependency Graph**:
```
count (Signal)
  ├──> doubled (Memo)
  │      └──> effect
  └──> effect
```

When `count` changes:
1. Notify `doubled` → mark dirty
2. Notify `effect` → re-run
3. Effect reads `doubled.get()` → recomputes if dirty

#### Frame Scheduler

The scheduler runs UI updates in discrete frames:

```rust
pub struct Scheduler {
    phase: Signal<Phase>,
    callbacks: HashMap<Phase, Vec<Box<dyn FnMut()>>>,
    stats: FrameStats,
    target_fps: u32,
}

pub enum Phase {
    Idle,
    Update,    // Run effects, process commands
    Build,     // Construct widget tree
    Layout,    // Compute layout (Taffy)
    Paint,     // Generate GPU commands
    Render,    // Submit to GPU
}

impl Scheduler {
    pub fn frame(&mut self) {
        self.run_phase(Phase::Update);
        self.run_phase(Phase::Build);
        self.run_phase(Phase::Layout);
        self.run_phase(Phase::Paint);
        self.run_phase(Phase::Render);

        // Throttle to target FPS
        let frame_time = self.stats.total_ms();
        let target_ms = 1000.0 / self.target_fps as f32;
        if frame_time < target_ms {
            sleep(Duration::from_millis((target_ms - frame_time) as u64));
        }
    }
}
```

#### Command Bus

Commands represent UI intents:

```rust
pub enum Command {
    // Platform commands
    CopyToClipboard { text: String },
    OpenUrl { url: String },

    // Navigation
    Navigate { route: Route },
    GoBack,
    GoForward,

    // Chat commands
    SendMessage { thread_id: ThreadId, content: String },
    CreateThread { title: String },
    DeleteThread { thread_id: ThreadId },

    // Run commands
    CancelRun { run_id: RunId },
    ApproveStep { run_id: RunId, step_id: StepId },
    RejectStep { run_id: RunId, step_id: StepId },
}

pub struct CommandBus {
    queue: VecDeque<Command>,
    handlers: HashMap<TypeId, Box<dyn Fn(&Command) -> CommandResult>>,
}

impl CommandBus {
    pub fn dispatch(&mut self, command: Command) {
        self.queue.push_back(command);
    }

    pub fn process(&mut self) {
        while let Some(command) = self.queue.pop_front() {
            if let Some(handler) = self.handlers.get(&command.type_id()) {
                handler(&command);
            }
        }
    }
}
```

**Why Fine-Grained Reactivity?**
- **Performance**: Only update what changed (no virtual DOM diffing)
- **Simplicity**: No manual subscription management
- **Composability**: Signals, memos, and effects compose naturally
- **Predictability**: Synchronous updates, clear execution order

---

### Layer 3: Widgets (`coder/widgets/`)

**Responsibility**: Composable UI building blocks.

#### Widget Trait

All widgets implement this interface:

```rust
pub trait Widget {
    /// Paint this widget to the scene
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext);

    /// Handle an input event
    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult;

    /// Optional widget ID for focus/hover tracking
    fn id(&self) -> Option<WidgetId> { None }

    /// Size hints for layout
    fn size_hint(&self) -> (Option<f32>, Option<f32>) { (None, None) }
}

pub enum EventResult {
    Handled,      // Event consumed
    Unhandled,    // Event not relevant
    NeedsFocus,   // Request focus
    LostFocus,    // Release focus
}
```

#### Core Widgets

**Div**: Generic container

```rust
pub struct Div {
    id: Option<WidgetId>,
    background: Option<Hsla>,
    border: Option<(Hsla, f32)>,
    corner_radius: f32,
    padding: f32,
    children: SmallVec<[AnyWidget; 4]>,
}

impl Div {
    pub fn new() -> Self { /* ... */ }
    pub fn background(mut self, color: Hsla) -> Self { self.background = Some(color); self }
    pub fn child(mut self, widget: impl Widget + 'static) -> Self { /* ... */ }
}

impl Widget for Div {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Paint background
        if let Some(bg) = self.background {
            cx.scene.draw_quad(bounds, bg, None, self.corner_radius);
        }

        // Paint children
        for child in &mut self.children {
            child.paint(bounds, cx);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        // Propagate in reverse order (z-order)
        for child in self.children.iter_mut().rev() {
            if child.event(event, bounds, cx) == EventResult::Handled {
                return EventResult::Handled;
            }
        }
        EventResult::Unhandled
    }
}
```

**Text**: Text rendering

```rust
pub struct Text {
    content: String,
    font_size: f32,
    color: Hsla,
    bold: bool,
    italic: bool,
}

impl Widget for Text {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.draw_text(
            &self.content,
            bounds.origin,
            cx.text_system,
            self.font_size,
            self.color,
        );
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let size = cx.text_system.measure(&self.content, self.font_size);
        (Some(size.width), Some(size.height))
    }
}
```

**Button**: Clickable button

```rust
pub struct Button {
    label: String,
    variant: ButtonVariant,
    on_click: Option<Box<dyn FnMut()>>,
    hovered: bool,
    pressed: bool,
}

pub enum ButtonVariant {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

impl Widget for Button {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.variant.color();
        let color = if self.pressed {
            color.darken(0.2)
        } else if self.hovered {
            color.lighten(0.1)
        } else {
            color
        };

        cx.scene.draw_quad(bounds, color, None, 4.0);
        cx.scene.draw_text(&self.label, bounds.center(), ...);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { position } => {
                self.hovered = bounds.contains(*position);
            }
            InputEvent::MouseDown { button: MouseButton::Left, .. } if self.hovered => {
                self.pressed = true;
            }
            InputEvent::MouseUp { button: MouseButton::Left, .. } if self.pressed => {
                self.pressed = false;
                if let Some(ref mut on_click) = self.on_click {
                    on_click();
                }
                return EventResult::Handled;
            }
            _ => {}
        }
        EventResult::Unhandled
    }
}
```

#### Composition Pattern

Widgets compose via builder pattern:

```rust
fn build_ui() -> AnyWidget {
    Div::new()
        .background(theme::bg::SURFACE)
        .padding(16.0)
        .child(
            Text::new("Welcome to Coder")
                .size(24.0)
                .bold(true)
        )
        .child(
            Div::new()
                .child(Button::new("New Chat").on_click(|| { /* ... */ }))
                .child(Button::new("Settings").on_click(|| { /* ... */ }))
        )
        .into()
}
```

**Why Immediate-Mode UI?**
- **Simplicity**: No state synchronization between UI and data
- **Predictability**: Rebuild widget tree every frame from current state
- **Performance**: Signals ensure only changed data triggers rebuilds
- **Flexibility**: Easy to add conditional rendering, loops, etc.

---

### Layer 4: Surfaces (`coder/surfaces_*`)

**Responsibility**: High-level UI features composed from widgets.

#### Chat Surface (`surfaces_chat/`)

**ChatThread**: Main chat view with virtual scrolling

```rust
pub struct ChatThread {
    chat_view: Signal<ChatView>,
    scroll_offset: Signal<f32>,
    item_height: f32,
}

impl Widget for ChatThread {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let view = self.chat_view.get();
        let offset = self.scroll_offset.get();

        // Virtual scrolling: only render visible items
        let visible_start = (offset / self.item_height).floor() as usize;
        let visible_count = (bounds.height() / self.item_height).ceil() as usize + 2;

        for i in visible_start..visible_start + visible_count {
            if let Some(entry) = view.entries.get(i) {
                let y = i as f32 * self.item_height - offset;
                let item_bounds = Bounds::new(bounds.x, bounds.y + y, bounds.width, self.item_height);

                match entry {
                    ChatEntry::Message(msg) => {
                        MessageBubble::new(msg.clone()).paint(item_bounds, cx);
                    }
                    ChatEntry::ToolUse(tool) => {
                        ToolUseIndicator::new(tool.clone()).paint(item_bounds, cx);
                    }
                }
            }
        }
    }
}
```

**MessageBubble**: Individual message rendering

```rust
pub struct MessageBubble {
    message: Message,
    markdown_rendered: Option<MarkdownBlocks>,
}

impl Widget for MessageBubble {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Role-based styling
        let bg = match self.message.role {
            Role::User => theme::bg::USER_MESSAGE,
            Role::Assistant => theme::bg::ASSISTANT_MESSAGE,
            Role::System => theme::bg::SYSTEM_MESSAGE,
        };

        cx.scene.draw_quad(bounds, bg, None, 8.0);

        // Render markdown
        if self.markdown_rendered.is_none() {
            let parser = MarkdownParser::new();
            self.markdown_rendered = Some(parser.parse(&self.message.content));
        }

        if let Some(ref blocks) = self.markdown_rendered {
            MarkdownRenderer::render(blocks, bounds, cx);
        }
    }
}
```

#### Terminal Surface (`surfaces_terminal/`)

**Terminal**: ANSI-capable terminal emulator

```rust
pub struct Terminal {
    buffer: TerminalBuffer,
    scroll_offset: Signal<f32>,
    cursor_style: CursorStyle,
}

impl Terminal {
    pub fn write(&mut self, data: &[u8]) {
        let mut parser = AnsiParser::new();
        let segments = parser.parse(data);
        self.buffer.write_segments(&segments);
    }
}

impl Widget for Terminal {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let scroll = self.scroll_offset.get() as usize;
        let visible_rows = (bounds.height() / CELL_HEIGHT) as usize;

        for row in scroll..scroll + visible_rows {
            if let Some(line) = self.buffer.line(row) {
                for (col, cell) in line.cells().iter().enumerate() {
                    let x = bounds.x + col as f32 * CELL_WIDTH;
                    let y = bounds.y + (row - scroll) as f32 * CELL_HEIGHT;

                    // Background
                    let bg = cell.style.effective_bg();
                    cx.scene.draw_quad(
                        Bounds::new(x, y, CELL_WIDTH, CELL_HEIGHT),
                        bg,
                        None,
                        0.0,
                    );

                    // Foreground
                    let fg = cell.style.effective_fg();
                    cx.scene.draw_text(
                        &cell.char.to_string(),
                        Point::new(x, y),
                        cx.text_system,
                        FONT_SIZE,
                        fg,
                    );
                }
            }
        }

        // Cursor
        self.paint_cursor(bounds, cx);
    }
}
```

---

### Layer 5: Shell (`coder/shell/`)

**Responsibility**: Application-level structure (routing, navigation, chrome).

#### Router

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    Home,
    Chat { thread_id: ThreadId },
    Project { project_id: ProjectId },
    Settings,
    NotFound { path: String },
}

pub struct Router {
    current: Signal<Route>,
    history: Vec<Route>,
    position: usize,
}

impl Router {
    pub fn navigate(&mut self, route: Route) {
        // Truncate forward history
        self.history.truncate(self.position + 1);
        self.history.push(route.clone());
        self.position = self.history.len() - 1;
        self.current.set(route);
    }

    pub fn back(&mut self) -> bool {
        if self.position > 0 {
            self.position -= 1;
            self.current.set(self.history[self.position].clone());
            true
        } else {
            false
        }
    }
}
```

#### View Registry

```rust
pub trait View {
    fn id(&self) -> &'static str;
    fn route(&self) -> Route;
    fn widget(&mut self) -> &mut dyn Widget;
    fn title(&self) -> String;
    fn on_activate(&mut self);
    fn on_deactivate(&mut self);
}

pub struct ViewRegistry {
    views: HashMap<&'static str, Box<dyn View>>,
    active: Option<&'static str>,
}

impl ViewRegistry {
    pub fn activate_for_route(&mut self, route: &Route) {
        // Find view matching route
        for (id, view) in &mut self.views {
            if std::mem::discriminant(&view.route()) == std::mem::discriminant(route) {
                if self.active != Some(*id) {
                    // Deactivate old view
                    if let Some(old_id) = self.active {
                        self.views[old_id].on_deactivate();
                    }

                    // Activate new view
                    view.on_activate();
                    self.active = Some(*id);
                }
                return;
            }
        }
    }
}
```

---

### Layer 6: Application (`coder/app/`)

**Responsibility**: Application bootstrap, main event loop.

#### AppState

```rust
pub struct AppState {
    /// All chat threads
    threads: HashMap<ThreadId, Signal<ChatView>>,

    /// Active thread
    active_thread: Signal<Option<ThreadId>>,

    /// Connection status
    connected: Signal<bool>,

    /// Event queue from backend
    event_queue: VecDeque<DomainEvent>,
}

impl AppState {
    pub fn apply_event(&mut self, event: DomainEvent) {
        match &event {
            DomainEvent::ThreadCreated { thread_id, .. } => {
                let view = Signal::new(ChatView::new(*thread_id));
                self.threads.insert(*thread_id, view);
            }
            DomainEvent::MessageAdded { thread_id, .. } |
            DomainEvent::MessageStreaming { thread_id, .. } => {
                if let Some(view_signal) = self.threads.get(thread_id) {
                    view_signal.update(|view| view.apply(&event));
                }
            }
            // ... more event handlers
        }
    }
}
```

#### App

```rust
pub struct App {
    state: AppState,
    navigation: Navigation,
    views: ViewRegistry,
    chrome: Chrome,
    scheduler: Scheduler,
    commands: CommandBus,
}

impl App {
    pub fn init(&mut self) {
        // Navigate to home
        self.navigation.navigate(Route::Home);

        // Start event loop
        self.run();
    }

    fn run(&mut self) {
        loop {
            // Handle input
            if let Some(event) = platform::poll_event() {
                self.handle_event(event);
            }

            // Update phase
            self.update();

            // Paint phase
            self.paint();

            // Submit to GPU
            platform::present();
        }
    }

    fn handle_event(&mut self, event: InputEvent) {
        let chrome_bounds = Bounds::new(0.0, 0.0, self.width, CHROME_HEIGHT);
        if self.chrome.event(&event, chrome_bounds, &mut EventContext::new(&mut self.commands)) == EventResult::Handled {
            return;
        }

        if let Some(view_id) = self.views.active {
            let view_bounds = Bounds::new(0.0, CHROME_HEIGHT, self.width, self.height - CHROME_HEIGHT);
            self.views[view_id].widget().event(&event, view_bounds, &mut EventContext::new(&mut self.commands));
        }
    }

    fn update(&mut self) {
        // Process commands
        self.commands.process();

        // Apply domain events
        while let Some(event) = self.state.event_queue.pop_front() {
            self.state.apply_event(event);
        }

        // Run scheduler
        self.scheduler.frame();
    }

    fn paint(&mut self) {
        let mut scene = Scene::new();
        let mut cx = PaintContext::new(&mut scene, ...);

        // Paint chrome
        let chrome_bounds = Bounds::new(0.0, 0.0, self.width, CHROME_HEIGHT);
        self.chrome.paint(chrome_bounds, &mut cx);

        // Paint active view
        if let Some(view_id) = self.views.active {
            let view_bounds = Bounds::new(0.0, CHROME_HEIGHT, self.width, self.height - CHROME_HEIGHT);
            self.views[view_id].widget().paint(view_bounds, &mut cx);
        }

        // Submit scene
        scene.submit(&self.renderer);
    }
}
```

---

## Data Flow Examples

### Example 1: User Sends Message

1. **Input**: User types in `ChatInput` widget and presses Enter
2. **Event**: `ChatInput::event()` receives `KeyDown { key: Enter }`
3. **Command**: Callback dispatches `Command::SendMessage { thread_id, content }`
4. **Bus**: `CommandBus::dispatch()` queues command
5. **Handler**: In `update()`, command handler sends message to backend
6. **Backend**: Generates `DomainEvent::MessageAdded`
7. **Protocol**: `ServerMessage::Events` streams event back
8. **State**: `AppState::apply_event()` updates `ChatView` signal
9. **Reactive**: Signal update triggers effects/memos subscribed to view
10. **Scheduler**: Next frame runs reactive updates
11. **Paint**: `ChatThread::paint()` renders updated view
12. **GPU**: wgpui submits render commands

### Example 2: Tool Use Streaming

1. **Backend**: Sends `DomainEvent::ToolUseStarted`
2. **AppState**: Adds tool to `ChatView.entries`
3. **Signal**: `chat_view` signal notifies subscribers
4. **Effect**: Virtual list effect recalculates visible range
5. **Paint**: `ToolUseIndicator` renders with "Running" status
6. **Backend**: Streams `DomainEvent::ToolUseProgress` events
7. **AppState**: Updates tool status in view
8. **Paint**: Indicator updates with new status (e.g., "50% complete")
9. **Backend**: Sends `DomainEvent::ToolUseComplete`
10. **AppState**: Marks tool complete, adds result
11. **Paint**: Indicator shows "Success" with result preview

---

## Performance Characteristics

### Memory

- **Signals**: 24 bytes (Arc + RwLock overhead)
- **Memos**: 48 bytes (cached value + dirty flag)
- **Effects**: 32 bytes (function pointer + subscriber ID)
- **Widgets**: Varies (Div ~200 bytes, Button ~150 bytes)

### CPU

- **Frame time**: <16ms for 60 FPS
  - Update: ~2ms (effects, commands)
  - Layout: ~3ms (Taffy)
  - Paint: ~5ms (widget tree traversal)
  - Render: ~3ms (GPU submission)

- **Virtual scrolling**: O(visible_items), not O(total_items)
- **Reactive updates**: O(affected_dependents), not O(all_components)

### GPU

- **Draw calls**: Batched by texture/shader
- **Text rendering**: Glyph atlas for caching
- **Overdraw**: Minimal (opaque widgets skip background)

---

## Design Decisions

### Why Not Use a Framework?

| Framework | Why Not |
|-----------|---------|
| **Dioxus** | Virtual DOM overhead, limited GPU access, abstracts too much |
| **Electron** | 150MB bundle, slow cold start, Chromium overhead |
| **Tauri** | Still uses web technologies (HTML/CSS/JS) |
| **egui** | Immediate-mode, but no reactivity, limited styling |
| **iced** | Elm architecture verbose, limited flexibility |

### Trade-Offs

**Gained**:
- Full control over every layer
- Direct GPU rendering
- Type-safe UI state management
- Predictable performance

**Lost**:
- No browser DevTools
- Smaller ecosystem
- Must implement features ourselves
- Learning curve for contributors

### Future Enhancements

- **Hot Reload**: Live code updates without restart
- **Inspector**: UI tree visualization and debugging
- **Profiler**: Per-widget performance metrics
- **Accessibility**: ARIA-like semantics for screen readers
- **Animations**: Interpolation and transitions
- **Gestures**: Touch, pinch, rotate support

---

## Summary

By owning all six layers, Coder achieves:

1. **Performance**: Native code, direct GPU, fine-grained reactivity
2. **Control**: Every line of code is ours to modify
3. **Type Safety**: Rust's type system guards the entire stack
4. **Simplicity**: No hidden framework magic
5. **Flexibility**: Easy to add custom rendering, interactions, etc.

The architecture is designed for **long-term maintainability** and **extensibility**. As requirements evolve, we can adapt every layer to meet our needs.
