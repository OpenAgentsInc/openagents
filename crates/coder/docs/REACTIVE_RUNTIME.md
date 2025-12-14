# Reactive Runtime: Fine-Grained Reactivity

The reactive runtime is Coder's state management system. Inspired by Solid.js, it provides **fine-grained reactivity**: automatic dependency tracking and minimal re-computation.

## Why Fine-Grained Reactivity?

Traditional UI frameworks use different approaches:

| Approach | How It Works | Trade-Off |
|----------|--------------|-----------|
| **Virtual DOM** (React) | Rebuild entire component tree, diff against previous | Simple mental model, but wastes CPU diffing unchanged nodes |
| **Coarse-Grained** (Vue 2) | Track component-level dependencies | Still re-renders entire component |
| **Fine-Grained** (Solid, Svelte) | Track signal-level dependencies | Only update what actually changed |

Fine-grained reactivity wins on performance: **only changed data triggers updates**.

## Core Primitives

### Signal<T>

A **Signal** is a reactive container that notifies subscribers when its value changes.

```rust
pub struct Signal<T> {
    value: Arc<RwLock<T>>,
    subscribers: Arc<RwLock<SmallVec<[SubscriberId; 4]>>>,
}

impl<T: Clone> Signal<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: Arc::new(RwLock::new(value)),
            subscribers: Arc::new(RwLock::new(SmallVec::new())),
        }
    }

    pub fn get(&self) -> T {
        // Automatic dependency tracking
        if let Some(subscriber_id) = Runtime::current_subscriber() {
            let mut subs = self.subscribers.write();
            if !subs.contains(&subscriber_id) {
                subs.push(subscriber_id);
            }
        }

        self.value.read().clone()
    }

    pub fn set(&self, value: T) {
        *self.value.write() = value;

        // Notify all subscribers
        let subs = self.subscribers.read().clone();
        for &subscriber_id in subs.iter() {
            Runtime::notify(subscriber_id);
        }
    }

    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        {
            let mut value = self.value.write();
            f(&mut *value);
        }

        // Notify after update
        let subs = self.subscribers.read().clone();
        for &subscriber_id in subs.iter() {
            Runtime::notify(subscriber_id);
        }
    }
}
```

**Key Points**:
- `get()` subscribes the current context (effect/memo)
- `set()` or `update()` notifies all subscribers
- Thread-safe: `Arc<RwLock<T>>` allows shared ownership

**Example**:
```rust
let count = Signal::new(0);

Effect::new(move || {
    println!("Count: {}", count.get()); // ← Subscribes
});

count.set(5); // ← Triggers effect re-run
// Output: "Count: 5"
```

### ReadSignal / WriteSignal

For more control, signals can be split:

```rust
pub struct ReadSignal<T> {
    signal: Signal<T>,
}

pub struct WriteSignal<T> {
    signal: Signal<T>,
}

impl<T: Clone> Signal<T> {
    pub fn split(self) -> (ReadSignal<T>, WriteSignal<T>) {
        (
            ReadSignal { signal: self.clone() },
            WriteSignal { signal: self },
        )
    }
}

impl<T: Clone> ReadSignal<T> {
    pub fn get(&self) -> T {
        self.signal.get()
    }
}

impl<T: Clone> WriteSignal<T> {
    pub fn set(&self, value: T) {
        self.signal.set(value);
    }

    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        self.signal.update(f);
    }
}
```

**Use Case**: Pass only read or write capability to different parts of code.

```rust
let (read, write) = Signal::new(0).split();

// UI can only read
fn render(count: ReadSignal<i32>) {
    println!("Count: {}", count.get());
}

// Logic can only write
fn increment(count: WriteSignal<i32>) {
    count.update(|n| *n += 1);
}
```

### Memo<T>

A **Memo** is a cached computed value that recomputes only when dependencies change.

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
        F: Fn() -> T + 'static,
    {
        let subscriber_id = Runtime::allocate_subscriber_id();

        let memo = Memo {
            compute: Arc::new(compute),
            cached: Arc::new(RwLock::new(None)),
            dirty: Arc::new(AtomicBool::new(true)),
            subscriber_id,
        };

        // Initial computation
        Runtime::with_subscriber(subscriber_id, || {
            let value = (memo.compute)();
            *memo.cached.write() = Some(value);
            memo.dirty.store(false, Ordering::Release);
        });

        memo
    }

    pub fn get(&self) -> T {
        // Register as a dependency
        if let Some(current_subscriber) = Runtime::current_subscriber() {
            Runtime::subscribe_to(current_subscriber, self.subscriber_id);
        }

        // Recompute if dirty
        if self.dirty.load(Ordering::Acquire) {
            Runtime::with_subscriber(self.subscriber_id, || {
                let value = (self.compute)();
                *self.cached.write() = Some(value);
                self.dirty.store(false, Ordering::Release);
            });
        }

        self.cached.read().clone().unwrap()
    }
}

// When a dependency notifies this memo
impl Subscriber for Memo {
    fn notify(&self) {
        self.dirty.store(true, Ordering::Release);
        // Don't recompute yet - wait for next get()
    }
}
```

**Key Points**:
- Lazy: Only recomputes on `get()` when dirty
- Caches result between dependencies changing
- Own subscriber ID for dependency tracking

**Example**:
```rust
let count = Signal::new(5);
let doubled = Memo::new(move || {
    println!("Computing doubled...");
    count.get() * 2
});

println!("{}", doubled.get()); // Output: "Computing doubled..." "10"
println!("{}", doubled.get()); // Output: "10" (cached, no recompute)

count.set(10);
println!("{}", doubled.get()); // Output: "Computing doubled..." "20"
```

### Effect

An **Effect** is a side effect that automatically re-runs when dependencies change.

```rust
pub struct Effect {
    effect_fn: Arc<dyn Fn()>,
    subscriber_id: SubscriberId,
    scope_id: Option<ScopeId>,
}

impl Effect {
    pub fn new<F>(effect_fn: F) -> EffectHandle
    where
        F: Fn() + 'static,
    {
        let subscriber_id = Runtime::allocate_subscriber_id();
        let scope_id = Runtime::current_scope();

        let effect = Arc::new(Effect {
            effect_fn: Arc::new(effect_fn),
            subscriber_id,
            scope_id,
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

// When a dependency notifies this effect
impl Subscriber for Effect {
    fn notify(&self) {
        // Re-run the effect
        Runtime::with_subscriber(self.subscriber_id, || {
            (self.effect_fn)();
        });
    }
}

pub struct EffectHandle {
    id: SubscriberId,
}

impl EffectHandle {
    pub fn stop(self) {
        Runtime::unregister_effect(self.id);
    }
}

impl Drop for EffectHandle {
    fn drop(&mut self) {
        Runtime::unregister_effect(self.id);
    }
}
```

**Key Points**:
- Runs immediately on creation
- Re-runs automatically when dependencies change
- Can be stopped via `EffectHandle`
- Cleaned up on scope disposal

**Example**:
```rust
let name = Signal::new("Alice".to_string());
let age = Signal::new(30);

Effect::new(move || {
    println!("{} is {} years old", name.get(), age.get());
});
// Output: "Alice is 30 years old"

name.set("Bob".into());
// Output: "Bob is 30 years old"

age.set(25);
// Output: "Bob is 25 years old"
```

## Runtime

The **Runtime** manages the reactive graph: scopes, subscribers, and notifications.

```rust
pub struct Runtime {
    scopes: SlotMap<ScopeId, Scope>,
    effects: HashMap<SubscriberId, Arc<Effect>>,
    current_subscriber: Option<SubscriberId>,
    current_scope: Option<ScopeId>,
    notification_queue: VecDeque<SubscriberId>,
}

thread_local! {
    static RUNTIME: RefCell<Runtime> = RefCell::new(Runtime::new());
}

impl Runtime {
    fn new() -> Self {
        Self {
            scopes: SlotMap::with_key(),
            effects: HashMap::new(),
            current_subscriber: None,
            current_scope: None,
            notification_queue: VecDeque::new(),
        }
    }

    pub fn with_runtime<F, R>(f: F) -> R
    where
        F: FnOnce(&mut Runtime) -> R,
    {
        RUNTIME.with(|runtime| f(&mut runtime.borrow_mut()))
    }

    pub fn current_subscriber() -> Option<SubscriberId> {
        Self::with_runtime(|rt| rt.current_subscriber)
    }

    pub fn with_subscriber<F, R>(subscriber_id: SubscriberId, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        Self::with_runtime(|rt| {
            let prev_subscriber = rt.current_subscriber;
            rt.current_subscriber = Some(subscriber_id);

            let result = f();

            rt.current_subscriber = prev_subscriber;
            result
        })
    }

    pub fn notify(subscriber_id: SubscriberId) {
        Self::with_runtime(|rt| {
            // Queue notification to avoid recursion
            rt.notification_queue.push_back(subscriber_id);
        });

        // Process queue
        Self::flush_notifications();
    }

    fn flush_notifications() {
        loop {
            let subscriber_id = Self::with_runtime(|rt| rt.notification_queue.pop_front());

            match subscriber_id {
                Some(id) => {
                    // Notify effect
                    if let Some(effect) = Self::with_runtime(|rt| rt.effects.get(&id).cloned()) {
                        effect.notify();
                    }
                }
                None => break,
            }
        }
    }
}
```

**Key Points**:
- Thread-local: Each thread has its own runtime
- Subscriber tracking: `current_subscriber` during signal reads
- Notification queue: Prevents infinite recursion
- Scope management: Cleanup when scopes are disposed

## Scopes

**Scopes** manage lifecycle and cleanup:

```rust
pub struct Scope {
    id: ScopeId,
    parent: Option<ScopeId>,
    children: Vec<ScopeId>,
    effects: Vec<SubscriberId>,
    cleanup_fns: Vec<Box<dyn FnOnce()>>,
}

impl Scope {
    pub fn new() -> ScopeId {
        Runtime::with_runtime(|rt| {
            let parent = rt.current_scope;
            let scope = Scope {
                id: ScopeId::default(),
                parent,
                children: Vec::new(),
                effects: Vec::new(),
                cleanup_fns: Vec::new(),
            };

            let scope_id = rt.scopes.insert(scope);

            // Add as child to parent
            if let Some(parent_id) = parent {
                if let Some(parent_scope) = rt.scopes.get_mut(parent_id) {
                    parent_scope.children.push(scope_id);
                }
            }

            scope_id
        })
    }

    pub fn with_scope<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let scope_id = Scope::new();

        let result = Runtime::with_runtime(|rt| {
            let prev_scope = rt.current_scope;
            rt.current_scope = Some(scope_id);

            let result = f();

            rt.current_scope = prev_scope;
            result
        });

        result
    }

    pub fn on_cleanup<F>(f: F)
    where
        F: FnOnce() + 'static,
    {
        Runtime::with_runtime(|rt| {
            if let Some(scope_id) = rt.current_scope {
                if let Some(scope) = rt.scopes.get_mut(scope_id) {
                    scope.cleanup_fns.push(Box::new(f));
                }
            }
        });
    }

    pub fn dispose(scope_id: ScopeId) {
        Runtime::with_runtime(|rt| {
            if let Some(scope) = rt.scopes.remove(scope_id) {
                // Dispose children first
                for child_id in scope.children {
                    Scope::dispose(child_id);
                }

                // Stop effects
                for effect_id in scope.effects {
                    rt.effects.remove(&effect_id);
                }

                // Run cleanup functions
                for cleanup in scope.cleanup_fns {
                    cleanup();
                }
            }
        });
    }
}
```

**Example**:
```rust
Scope::with_scope(|| {
    let count = Signal::new(0);

    Effect::new(move || {
        println!("Count: {}", count.get());
    });

    Scope::on_cleanup(|| {
        println!("Cleaning up!");
    });

    count.set(5);
});
// Output: "Count: 0", "Count: 5", "Cleaning up!"
```

## Scheduler

The **Scheduler** runs UI updates in discrete frames with defined phases.

```rust
pub struct Scheduler {
    phase: Signal<Phase>,
    callbacks: HashMap<Phase, Vec<Box<dyn FnMut()>>>,
    stats: FrameStats,
    target_fps: u32,
    last_frame_time: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Phase {
    Idle,
    Update,    // Run effects, process commands
    Build,     // Construct widget tree
    Layout,    // Compute layout (Taffy)
    Paint,     // Generate GPU commands
    Render,    // Submit to GPU
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            phase: Signal::new(Phase::Idle),
            callbacks: HashMap::new(),
            stats: FrameStats::default(),
            target_fps: 60,
            last_frame_time: Instant::now(),
        }
    }

    pub fn on_phase<F>(&mut self, phase: Phase, callback: F)
    where
        F: FnMut() + 'static,
    {
        self.callbacks.entry(phase).or_insert_with(Vec::new).push(Box::new(callback));
    }

    pub fn frame(&mut self) {
        let frame_start = Instant::now();

        self.run_phase(Phase::Update);
        self.run_phase(Phase::Build);
        self.run_phase(Phase::Layout);
        self.run_phase(Phase::Paint);
        self.run_phase(Phase::Render);

        self.stats.total_ms = frame_start.elapsed().as_secs_f32() * 1000.0;

        // Throttle to target FPS
        let target_frame_time = 1000.0 / self.target_fps as f32;
        if self.stats.total_ms < target_frame_time {
            let sleep_ms = target_frame_time - self.stats.total_ms;
            std::thread::sleep(Duration::from_millis(sleep_ms as u64));
        }

        self.last_frame_time = Instant::now();
        self.phase.set(Phase::Idle);
    }

    fn run_phase(&mut self, phase: Phase) {
        let start = Instant::now();
        self.phase.set(phase);

        if let Some(callbacks) = self.callbacks.get_mut(&phase) {
            for callback in callbacks {
                callback();
            }
        }

        let elapsed = start.elapsed().as_secs_f32() * 1000.0;
        self.stats.set_phase_time(phase, elapsed);
    }

    pub fn current_phase() -> Phase {
        // Access via signal
        Phase::Idle // placeholder
    }
}

#[derive(Default)]
pub struct FrameStats {
    pub update_ms: f32,
    pub build_ms: f32,
    pub layout_ms: f32,
    pub paint_ms: f32,
    pub render_ms: f32,
    pub total_ms: f32,
}
```

**Frame Lifecycle**:
```
┌─────────────────────────────────────┐
│ Frame N                             │
├─────────────────────────────────────┤
│ 1. Update (2ms)                     │
│    - Run effects                    │
│    - Process commands               │
│    - Handle events                  │
├─────────────────────────────────────┤
│ 2. Build (1ms)                      │
│    - Construct widget tree          │
│    - Call widget constructors       │
├─────────────────────────────────────┤
│ 3. Layout (3ms)                     │
│    - Compute sizes/positions        │
│    - Run Taffy                      │
├─────────────────────────────────────┤
│ 4. Paint (5ms)                      │
│    - Traverse widget tree           │
│    - Generate draw commands         │
│    - Fill Scene                     │
├─────────────────────────────────────┤
│ 5. Render (3ms)                     │
│    - Submit to GPU                  │
│    - Swap buffers                   │
├─────────────────────────────────────┤
│ Total: 14ms (71 FPS)                │
└─────────────────────────────────────┘
```

## Command Bus

Commands represent UI intents separate from immediate execution:

```rust
pub enum Command {
    // Platform
    CopyToClipboard { text: String },
    PasteFromClipboard,
    OpenUrl { url: String },

    // Navigation
    Navigate { route: Route },
    GoBack,
    GoForward,

    // Chat
    SendMessage { thread_id: ThreadId, content: String },
    CreateThread { title: String },
    DeleteThread { thread_id: ThreadId },
    ClearThread { thread_id: ThreadId },

    // Runs
    CancelRun { run_id: RunId },
    ApproveStep { run_id: RunId, step_id: StepId },
    RejectStep { run_id: RunId, step_id: StepId },

    // Project
    OpenProject { path: String },
    CloseProject { project_id: ProjectId },
}

pub struct CommandBus {
    queue: VecDeque<Command>,
    handlers: HashMap<TypeId, Box<dyn Fn(&Command) -> CommandResult>>,
}

impl CommandBus {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            handlers: HashMap::new(),
        }
    }

    pub fn register<C, F>(&mut self, handler: F)
    where
        C: 'static,
        F: Fn(&C) -> CommandResult + 'static,
    {
        let type_id = TypeId::of::<C>();
        self.handlers.insert(
            type_id,
            Box::new(move |cmd| {
                // Downcast and call handler
                handler(cmd.downcast_ref::<C>().unwrap())
            }),
        );
    }

    pub fn dispatch(&mut self, command: Command) {
        self.queue.push_back(command);
    }

    pub fn process(&mut self) {
        while let Some(command) = self.queue.pop_front() {
            let type_id = command.type_id();

            if let Some(handler) = self.handlers.get(&type_id) {
                let result = handler(&command);

                match result {
                    CommandResult::Success => {}
                    CommandResult::Error(err) => {
                        eprintln!("Command error: {}", err);
                    }
                    CommandResult::Retry => {
                        self.queue.push_front(command);
                        break; // Stop processing to avoid infinite loop
                    }
                }
            }
        }
    }
}

pub enum CommandResult {
    Success,
    Error(String),
    Retry,
}
```

**Why Commands?**
- **Separation of Concerns**: UI emits intents, handlers execute
- **Testing**: Easy to test handlers in isolation
- **Logging**: All user actions go through command bus
- **Undo/Redo**: Commands can be serialized and replayed

## Reactive Graph Example

```rust
// Create signals
let first_name = Signal::new("Alice".to_string());
let last_name = Signal::new("Smith".to_string());

// Create derived value (memo)
let full_name = Memo::new(move || {
    format!("{} {}", first_name.get(), last_name.get())
});

// Create side effect
Effect::new(move || {
    println!("Full name: {}", full_name.get());
});
// Output: "Full name: Alice Smith"

// Update signals
first_name.set("Bob".into());
// Output: "Full name: Bob Smith" (effect re-runs)

last_name.set("Jones".into());
// Output: "Full name: Bob Jones" (effect re-runs)
```

**Dependency Graph**:
```
first_name (Signal) ──┐
                      ├──> full_name (Memo) ──> effect
last_name (Signal) ───┘
```

When `first_name` changes:
1. Notifies `full_name` → marks dirty
2. Effect reads `full_name.get()`
3. `full_name` recomputes (reads both signals)
4. Effect prints new value

## Performance Tips

1. **Use Memos for Expensive Computations**:
   ```rust
   // Bad: Recomputes every time
   let expensive = move || expensive_calculation(data.get());

   // Good: Caches result
   let expensive = Memo::new(move || expensive_calculation(data.get()));
   ```

2. **Split Signals for Granularity**:
   ```rust
   // Bad: One big signal
   let state = Signal::new(AppState { count: 0, name: "Alice" });

   // Good: Separate signals
   let count = Signal::new(0);
   let name = Signal::new("Alice".to_string());
   ```

3. **Avoid Reading Signals in Loops**:
   ```rust
   // Bad: Re-subscribes on every iteration
   for i in 0..count.get() {
       println!("{}", i);
   }

   // Good: Read once
   let n = count.get();
   for i in 0..n {
       println!("{}", i);
   }
   ```

4. **Use Batching for Multiple Updates**:
   ```rust
   // Updates separately (triggers effect twice)
   first_name.set("Bob".into());
   last_name.set("Jones".into());

   // Better: Batch updates (future feature)
   batch(|| {
       first_name.set("Bob".into());
       last_name.set("Jones".into());
   }); // Triggers effect once
   ```

## Summary

The reactive runtime provides:

1. **Signals**: Reactive state containers
2. **Memos**: Cached derived values
3. **Effects**: Automatic side effects
4. **Scopes**: Lifecycle management
5. **Scheduler**: Frame-based execution
6. **Commands**: Decoupled intent/execution

Together, these primitives enable **fine-grained reactivity**: only changed data triggers updates, resulting in optimal performance and predictable behavior.
