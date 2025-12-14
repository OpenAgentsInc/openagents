# Animation System

HUD's animation system is built around two core types: `HudAnimator` for individual component animations and `AnimatorManager` for orchestrating groups of animations.

## HudAnimator

The `HudAnimator` is a state machine that transitions between four states:

```
┌─────────┐    enter()    ┌──────────┐   complete   ┌─────────┐
│ Exited  │ ───────────▶ │ Entering │ ───────────▶ │ Entered │
└─────────┘               └──────────┘              └─────────┘
     ▲                                                   │
     │                                                   │
     │         complete    ┌──────────┐     exit()       │
     └──────────────────── │ Exiting  │ ◀────────────────┘
                           └──────────┘
```

### Creating an Animator

```rust
use hud::HudAnimator;
use hud::easing::ease_out_expo;

let animator = HudAnimator::new()
    .enter_duration(30)     // 30 frames (~0.5s at 60fps)
    .exit_duration(20)      // 20 frames for exit
    .easing(ease_out_expo); // Custom easing curve
```

### Controlling Animation

```rust
let mut animator = HudAnimator::new();

// Start enter animation
animator.enter();

// Start exit animation (from Entered or Entering)
animator.exit();

// Toggle between enter/exit
animator.toggle();

// Skip animation, jump to final state
animator.set_entered();
animator.set_exited();
```

### Animation Loop

Call `tick()` every frame to advance the animation:

```rust
// In your update loop:
if animator.tick() {
    // Returns true if still animating
    request_redraw();
}
```

### Reading Animation State

```rust
// Current state enum
let state = animator.state();  // AnimatorState::{Exited, Entering, Entered, Exiting}

// Eased progress (0.0 to 1.0) - use for rendering
let progress = animator.progress();

// Raw linear progress (without easing)
let raw = animator.raw_progress();

// Boolean checks
if animator.is_animating() { /* Entering or Exiting */ }
if animator.is_visible() { /* Not Exited */ }
```

### Using Progress for Rendering

The `progress()` value is your primary tool for animated rendering:

```rust
fn paint(&self, scene: &mut Scene) {
    let progress = self.animator.progress();

    // Skip rendering if fully exited
    if progress <= 0.0 {
        return;
    }

    // Animate opacity
    let color = Hsla::new(0.0, 0.0, 1.0, 0.5 * progress);

    // Animate size
    let width = 100.0 * progress;

    // Animate position (slide in)
    let x = target_x - (20.0 * (1.0 - progress));
}
```

## AnimatorManager

For coordinating multiple animations, use `AnimatorManager`:

### Manager Modes

```rust
use hud::{AnimatorManager, ManagerMode};

// Parallel: All children animate at once
let parallel = AnimatorManager::new(ManagerMode::Parallel);

// Stagger: Children start with delays
let stagger = AnimatorManager::new(ManagerMode::Stagger)
    .stagger_offset(5); // 5 frames between each

// Sequence: Each child waits for previous to complete
let sequence = AnimatorManager::new(ManagerMode::Sequence);
```

### Adding Children

```rust
use hud::HudAnimator;

let mut manager = AnimatorManager::new(ManagerMode::Stagger);

// Add individual animators
manager.add_child(HudAnimator::new());
manager.add_child(HudAnimator::new().enter_duration(20));

// Access children
if let Some(child) = manager.child(0) {
    println!("First child progress: {}", child.progress());
}
```

### Coordinating with Components

Typical pattern for a list of components:

```rust
struct AnimatedList {
    items: Vec<MyItem>,
    manager: AnimatorManager,
}

impl AnimatedList {
    fn new(count: usize) -> Self {
        let mut manager = AnimatorManager::new(ManagerMode::Stagger)
            .stagger_offset(3);

        for _ in 0..count {
            manager.add_child(HudAnimator::new());
        }

        Self {
            items: (0..count).map(|_| MyItem::new()).collect(),
            manager,
        }
    }

    fn start(&mut self) {
        self.manager.enter();
    }

    fn tick(&mut self) {
        self.manager.tick();

        // Sync component animators with managed children
        for (i, item) in self.items.iter_mut().enumerate() {
            if let Some(managed) = self.manager.child(i) {
                if managed.state().is_entered() {
                    item.animator_mut().set_entered();
                }
            }
            item.tick();
        }
    }
}
```

## Easing Functions

HUD provides 30+ easing functions in `hud::easing`:

### Naming Convention

- `ease_in_*` - Slow start, fast end
- `ease_out_*` - Fast start, slow end
- `ease_in_out_*` - Slow start and end

### Available Curves

| Family | Functions |
|--------|-----------|
| Linear | `linear` |
| Quadratic | `ease_in_quad`, `ease_out_quad`, `ease_in_out_quad` |
| Cubic | `ease_in_cubic`, `ease_out_cubic`, `ease_in_out_cubic` |
| Quartic | `ease_in_quart`, `ease_out_quart`, `ease_in_out_quart` |
| Quintic | `ease_in_quint`, `ease_out_quint`, `ease_in_out_quint` |
| Sinusoidal | `ease_in_sine`, `ease_out_sine`, `ease_in_out_sine` |
| Exponential | `ease_in_expo`, `ease_out_expo`, `ease_in_out_expo` |
| Circular | `ease_in_circ`, `ease_out_circ`, `ease_in_out_circ` |
| Back | `ease_in_back`, `ease_out_back`, `ease_in_out_back` |
| Elastic | `ease_in_elastic`, `ease_out_elastic`, `ease_in_out_elastic` |
| Bounce | `ease_in_bounce`, `ease_out_bounce`, `ease_in_out_bounce` |

### Using Custom Easing

```rust
use hud::HudAnimator;
use hud::easing::{ease_out_expo, ease_in_out_elastic};

// Set on animator creation
let animator = HudAnimator::new().easing(ease_out_expo);

// Or use directly in calculations
fn animate_bounce(t: f32) -> f32 {
    ease_in_out_elastic(t)
}
```

### Default Easing

HUD uses `ease_out_cubic` as the default easing for most components, matching Arwes conventions.

## Timing Constants

Default timing values are in `hud::theme::timing`:

```rust
use hud::theme::timing;

timing::ENTER_FRAMES    // 20 frames (~0.33s at 60fps)
timing::EXIT_FRAMES     // 15 frames
timing::STAGGER_OFFSET  // 3 frames between staggered items
```

## Best Practices

### 1. Always Check Progress Before Rendering

```rust
fn paint(&self, scene: &mut Scene) {
    let progress = self.animator.progress();
    if progress <= 0.0 {
        return; // Skip rendering when fully exited
    }
    // ... render with progress-based values
}
```

### 2. Use Stagger for Lists

Lists look more dynamic with staggered item animations:

```rust
let manager = AnimatorManager::new(ManagerMode::Stagger)
    .stagger_offset(3);
```

### 3. Match Enter/Exit Durations

For symmetrical animations, use similar enter and exit durations:

```rust
let animator = HudAnimator::new()
    .enter_duration(20)
    .exit_duration(20);
```

### 4. Use Fast Easing for UI Feedback

`ease_out_*` functions feel responsive for enter animations:

```rust
.easing(ease_out_expo)  // Dramatic deceleration
.easing(ease_out_cubic) // Smooth default
```
