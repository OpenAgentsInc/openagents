# Action and Keymap System

This document is the comprehensive reference for the action and keymap system
added to WGPUI. It covers action definition, keystroke parsing, keymap resolution,
key contexts, dispatch integration, and the fluent Interactive API.

## Scope and source files

The action/keymap system spans these modules:

**Action module** (`crates/wgpui/src/action/`):
- `mod.rs` - Module exports and documentation
- `action.rs` - Core `Action` trait and `AnyAction` type erasure
- `keystroke.rs` - `Keystroke` parsing from strings like "cmd-shift-s"
- `binding.rs` - `KeyBinding` connecting keystrokes to actions
- `registry.rs` - `ActionRegistry` for name lookups and default bindings
- `dispatch.rs` - `ActionListeners` and dispatch utilities
- `macros.rs` - `actions!` and `action!` macros for concise definitions
- `standard.rs` - 21 built-in UI actions (MoveUp, Copy, Cancel, etc.)

**Keymap module** (`crates/wgpui/src/keymap/`):
- `mod.rs` - Module exports
- `context.rs` - `KeyContext` stack for scoped bindings
- `keymap.rs` - `Keymap` with precedence-based resolution
- `defaults.rs` - Default keybindings (cmd-s for Save, etc.)

**Interactive module** (`crates/wgpui/src/interactive.rs`):
- `Interactive` trait for fluent `.on_action()` API
- `WithAction<C, A>` wrapper for action handling
- `WithContext<C>` wrapper for key context scoping

**Integration** (`crates/wgpui/src/components/context.rs`):
- `EventContext` extended with key context and action dispatch

## High-level architecture

The action/keymap system provides keyboard-first workflows with user-customizable
keybindings. The flow is:

```
User presses key
       |
       v
Keymap.match_keystroke(key, modifiers, context)
       |
       | (finds binding with highest precedence)
       v
Action (type-erased Box<dyn AnyAction>)
       |
       v
EventContext.dispatch_action(action)
       |
       | (bubbles through component hierarchy)
       v
Component with .on_action::<A>() handler
       |
       v
Handler executes, returns true if handled
```

Key separation of concerns:

- `Action` defines what can be done (Save, Copy, MoveUp)
- `Keystroke` defines how it's triggered ("cmd-s")
- `KeyBinding` connects keystroke to action with optional context
- `Keymap` resolves keystrokes to actions with precedence
- `KeyContext` provides scoping for context-sensitive bindings
- `Interactive` trait enables declarative action handlers on components

## Action system

### Goals

- Type-safe action dispatch with compile-time guarantees
- Runtime type erasure for storage in keymaps and registries
- Cloneable boxed actions for dispatch and replay
- Human-readable names for debugging and UI display

### Core types

```rust
pub trait Action: Any + Debug + Clone + Send + Sync + 'static {
    fn name() -> &'static str where Self: Sized;
    fn action_id(&self) -> ActionId { TypeId::of::<Self>() }
    fn boxed_clone(&self) -> Box<dyn AnyAction>;
}

pub trait AnyAction: Any + Debug + Send + Sync {
    fn action_id(&self) -> ActionId;
    fn name(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
    fn boxed_clone(&self) -> Box<dyn AnyAction>;
}
```

- `Action`: Implemented by concrete action types
- `AnyAction`: Type-erased trait object for storage
- `ActionId`: Type alias for `TypeId`, uniquely identifies action types
- `NoAction`: Placeholder action for testing

### Defining actions

**Manual implementation:**

```rust
#[derive(Debug, Clone, Default)]
struct Save;

impl Action for Save {
    fn name() -> &'static str { "editor::Save" }
    fn boxed_clone(&self) -> Box<dyn AnyAction> { Box::new(self.clone()) }
}
```

**Using the `actions!` macro:**

```rust
actions!(editor, [
    Save,
    Undo,
    Redo,
    Copy,
    Paste,
]);

// Creates: editor::Save, editor::Undo, etc.
// Each is a unit struct implementing Action
```

**Using the `action!` macro for actions with fields:**

```rust
action!(editor::GoToLine {
    line: usize,
});

// Creates struct with field, implements Action
let action = GoToLine { line: 42 };
```

### Standard actions

The `action::standard` module provides common UI actions:

| Category | Actions |
|----------|---------|
| Navigation | `MoveUp`, `MoveDown`, `MoveLeft`, `MoveRight`, `MoveToStart`, `MoveToEnd` |
| Editing | `Cancel`, `Confirm`, `Delete`, `Backspace`, `SelectAll` |
| Clipboard | `Copy`, `Cut`, `Paste` |
| Undo/Redo | `Undo`, `Redo` |
| Focus | `FocusNext`, `FocusPrevious` |
| UI | `ToggleCommandPalette`, `Close`, `Refresh` |
| File | `Save`, `Open`, `New` |

## Keystroke system

### Keystroke parsing

`Keystroke::parse()` converts human-readable strings to structured keystrokes:

```rust
let ks = Keystroke::parse("cmd-shift-s").unwrap();
assert!(ks.modifiers.meta);
assert!(ks.modifiers.shift);
assert_eq!(ks.key, Key::Character("s".to_string()));
```

**Supported modifiers:**
- `cmd`, `meta`, `super` - Meta/Command key
- `ctrl`, `control` - Control key
- `alt`, `opt`, `option` - Alt/Option key
- `shift` - Shift key

**Supported keys:**
- Single characters: `a`, `b`, `1`, `/`, etc.
- Named keys: `enter`, `escape`, `tab`, `backspace`, `delete`,
  `up`, `down`, `left`, `right`, `home`, `end`

**Examples:**
- `"cmd-s"` - Command+S (Save)
- `"ctrl-shift-p"` - Control+Shift+P (Command palette)
- `"escape"` - Escape key alone
- `"alt-up"` - Alt+Up arrow

### KeyBinding

Connects a keystroke (or sequence) to an action with optional context:

```rust
// Simple binding
let binding = KeyBinding::new("cmd-s", Save).unwrap();

// Binding with context requirement
let binding = KeyBinding::new("escape", CloseModal)
    .unwrap()
    .with_context("Modal");
```

## Keymap system

### Goals

- Store multiple bindings with precedence rules
- Resolve keystrokes to actions based on current context
- Support user overrides (later bindings win)
- Context-aware matching (deeper context wins)

### Keymap structure

```rust
pub struct Keymap {
    bindings: Vec<KeyBinding>,
}

impl Keymap {
    pub fn add(&mut self, binding: KeyBinding);
    pub fn match_keystroke(
        &self,
        key: &Key,
        modifiers: &Modifiers,
        context: &KeyContext,
    ) -> Option<Box<dyn AnyAction>>;
}
```

### Precedence rules

When multiple bindings match the same keystroke:

1. **Context depth** - More specific contexts win
   - Binding for "TextInput" beats binding for "Editor" when in `[Editor > TextInput]`
   - Binding with no context matches anywhere but loses to contextual bindings

2. **Binding order** - Later bindings override earlier
   - User bindings added after defaults take precedence
   - Enables customization without removing default bindings

**Example:**

```rust
let mut keymap = Keymap::new();

// Default escape binding (global)
keymap.add(KeyBinding::new("escape", Cancel).unwrap());

// Modal-specific binding (wins when in Modal context)
keymap.add(KeyBinding::new("escape", CloseModal)
    .unwrap()
    .with_context("Modal"));

// User override (wins over both due to later order)
keymap.add(KeyBinding::new("escape", CustomEscape).unwrap());
```

### Default keymap

`default_keymap()` returns a keymap with common bindings:

```rust
let keymap = default_keymap();
// Includes: cmd-s (Save), cmd-c (Copy), escape (Cancel),
// arrow keys (Move*), tab (FocusNext), etc.
```

| Keystroke | Action |
|-----------|--------|
| `up/down/left/right` | MoveUp/Down/Left/Right |
| `home/end` | MoveToStart/MoveToEnd |
| `tab` | FocusNext |
| `shift-tab` | FocusPrevious |
| `escape` | Cancel |
| `enter` | Confirm |
| `backspace` | Backspace |
| `delete` | Delete |
| `cmd-c` | Copy |
| `cmd-x` | Cut |
| `cmd-v` | Paste |
| `cmd-a` | SelectAll |
| `cmd-z` | Undo |
| `cmd-shift-z` | Redo |
| `cmd-s` | Save |
| `cmd-o` | Open |
| `cmd-n` | New |
| `cmd-shift-p` | ToggleCommandPalette |
| `cmd-w` | Close |
| `cmd-r` | Refresh |

## Key context system

### Goals

- Scope keybindings to specific UI contexts
- Support hierarchical contexts (Window > Editor > TextInput)
- Enable context-dependent behavior (escape closes modal, not app)

### KeyContext structure

```rust
pub struct KeyContext {
    identifiers: SmallVec<[String; 4]>,
}

impl KeyContext {
    pub fn push(&mut self, identifier: impl Into<String>);
    pub fn pop(&mut self) -> Option<String>;
    pub fn contains(&self, identifier: &str) -> bool;
    pub fn depth(&self) -> usize;
}
```

### Usage pattern

Components push/pop their context during event handling:

```rust
// In component event handler
cx.push_context("Modal");

// Handle events - "Modal" bindings now active

cx.pop_context();
```

Or use the `WithContext` wrapper:

```rust
let modal = MyModal::new()
    .key_context("Modal");  // Auto push/pop
```

### Context matching

A binding matches if its context is present anywhere in the stack:

```rust
// Context stack: [Window, Editor, TextInput]

// These bindings match:
KeyBinding::new("escape", Cancel).unwrap()  // No context requirement
KeyBinding::new("escape", X).unwrap().with_context("Editor")
KeyBinding::new("escape", Y).unwrap().with_context("TextInput")

// This binding does NOT match:
KeyBinding::new("escape", Z).unwrap().with_context("Modal")
```

## Dispatch integration

### EventContext extensions

`EventContext` is extended with action dispatch capabilities:

```rust
impl EventContext {
    // Key context
    pub fn push_context(&mut self, identifier: impl Into<String>);
    pub fn pop_context(&mut self) -> Option<String>;
    pub fn key_context(&self) -> &KeyContext;

    // Action listeners
    pub fn on_action<A: Action>(
        &mut self,
        component_id: u64,
        handler: impl FnMut(&A) -> bool + 'static,
    );

    // Action dispatch
    pub fn dispatch_action(&mut self, action: Box<dyn AnyAction>);
    pub fn take_pending_action(&mut self) -> Option<PendingAction>;
    pub fn try_handle_action(&mut self, action: &dyn AnyAction, id: u64) -> bool;
    pub fn dispatch_to_hierarchy(&mut self, action: &dyn AnyAction, ids: &[u64]) -> Option<u64>;
}
```

### ActionListeners

Per-component storage for action handlers:

```rust
pub struct ActionListeners {
    listeners: HashMap<ActionId, ActionHandler>,
}

impl ActionListeners {
    pub fn on_action<A: Action>(&mut self, handler: impl FnMut(&A) -> bool + 'static);
    pub fn handle(&mut self, action: &dyn AnyAction) -> bool;
}
```

### Dispatch flow

1. Key event triggers `Keymap::match_keystroke()`
2. If action found, call `cx.dispatch_action(action)`
3. Event loop calls `cx.take_pending_action()`
4. Action bubbles through component hierarchy via `dispatch_to_hierarchy()`
5. First handler returning `true` stops propagation

## Interactive trait

### Goals

- Fluent API for adding action handlers to components
- Chainable with other component methods
- Automatic context management via wrappers

### Interactive trait

```rust
pub trait Interactive: Component + Sized {
    fn on_action<A: Action>(self, handler: impl FnMut(&A) -> bool + 'static) -> WithAction<Self, A>;
    fn key_context(self, context: impl Into<String>) -> WithContext<Self>;
}

// Blanket implementation for all Components
impl<C: Component + Sized> Interactive for C {}
```

### Usage examples

**Single action handler:**

```rust
let button = Button::new("Save")
    .on_action::<Save>(|_| {
        save_document();
        true
    });
```

**Multiple action handlers:**

```rust
let modal = Modal::new()
    .on_action::<Cancel>(|_| {
        close_modal();
        true
    })
    .on_action::<Confirm>(|_| {
        submit_form();
        true
    });
```

**With key context:**

```rust
let editor = Editor::new()
    .key_context("Editor")
    .on_action::<Save>(|_| { /* ... */ true })
    .on_action::<Undo>(|_| { /* ... */ true });
```

### WithAction wrapper

Intercepts pending actions and calls handler if type matches:

```rust
pub struct WithAction<C, A> {
    inner: C,
    handler: Box<dyn FnMut(&dyn Any) -> bool>,
    _action: PhantomData<A>,
}

impl<C: Component, A: Action> Component for WithAction<C, A> {
    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        // Check for pending action of our type
        if let Some(pending) = cx.take_pending_action() {
            if pending.action.action_id() == TypeId::of::<A>() {
                if (self.handler)(pending.action.as_any()) {
                    return EventResult::Handled;
                }
            }
            // Put back if not handled
            cx.dispatch_action(pending.action);
        }
        self.inner.event(event, bounds, cx)
    }
}
```

### WithContext wrapper

Pushes context before event handling, pops after:

```rust
pub struct WithContext<C> {
    inner: C,
    context: String,
}

impl<C: Component> Component for WithContext<C> {
    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        cx.push_context(&self.context);
        let result = self.inner.event(event, bounds, cx);
        cx.pop_context();
        result
    }
}
```

## Complete integration example

```rust
use wgpui::{
    Interactive, Keymap, KeyContext, KeyBinding, default_keymap,
    action::standard::{Save, Cancel, Confirm},
};

// 1. Create keymap with defaults + custom bindings
let mut keymap = default_keymap();
keymap.add(KeyBinding::new("cmd-enter", SubmitForm).unwrap()
    .with_context("Form"));

// 2. Build component tree with action handlers
let app = Window::new()
    .key_context("Window")
    .child(
        Form::new()
            .key_context("Form")
            .on_action::<SubmitForm>(|_| {
                submit();
                true
            })
            .on_action::<Cancel>(|_| {
                close();
                true
            })
    );

// 3. In event loop, process key events
fn handle_key(key: Key, modifiers: Modifiers, keymap: &Keymap, cx: &mut EventContext) {
    if let Some(action) = keymap.match_keystroke(&key, &modifiers, cx.key_context()) {
        cx.dispatch_action(action);
    }
}
```

## Design decisions

### Why not auto-registration (inventory crate)?

Zed uses the `inventory` crate to auto-register actions at startup. We chose
manual registration for simplicity:

- No proc-macro dependencies
- Explicit control over which actions are available
- Easier debugging (no hidden registration)
- Works in all compilation targets

### Why simple string contexts (not predicates)?

Zed supports complex context predicates like `"Editor && mode == vim"`. We use
simple string matching:

- Covers 95% of use cases
- Much simpler implementation
- Can extend to predicates later if needed

### Why bubble-only dispatch (no capture)?

Zed has capture and bubble phases. We only have bubble:

- Simpler mental model
- Matches DOM event bubbling
- Capture rarely needed in practice

### Why single keystrokes (not sequences)?

Zed supports keystroke sequences like "ctrl-k ctrl-c". We only support single
keystrokes:

- Covers most common cases
- Simpler state management
- Can extend to sequences later if needed

## Testing

The system includes comprehensive tests:

- `action::action::tests` - Action trait, downcasting
- `action::keystroke::tests` - Parsing, matching, display
- `action::binding::tests` - Binding creation, context
- `action::registry::tests` - Registration, building
- `action::dispatch::tests` - Listeners, dispatch
- `action::macros::tests` - `actions!` and `action!` macros
- `keymap::context::tests` - Push/pop, matching
- `keymap::keymap::tests` - Precedence, resolution
- `keymap::defaults::tests` - Default bindings
- `interactive::tests` - Fluent API, wrappers

Run tests with:

```bash
cargo test -p wgpui action::
cargo test -p wgpui keymap::
cargo test -p wgpui interactive::
```

## Future enhancements

Potential extensions (not currently implemented):

1. **Multi-key sequences** - Support "ctrl-k ctrl-c" style chords
2. **Context predicates** - Support "Editor && mode == vim" matching
3. **Action parameters from keymap** - Load action fields from JSON config
4. **Capture phase** - Dispatch from root to focused before bubble
5. **Key repeat handling** - Distinguish initial press from repeat
6. **IME integration** - Proper input method handling
