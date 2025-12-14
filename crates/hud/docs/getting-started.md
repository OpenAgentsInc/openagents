# Getting Started with HUD

This guide covers the basics of using HUD components in your WGPUI application.

## Installation

Add HUD to your `Cargo.toml`:

```toml
[dependencies]
hud = { path = "../hud" }
wgpui = { path = "../wgpui" }
```

## Basic Component Usage

Every HUD component follows the same lifecycle pattern:

1. **Create** - Instantiate with builder pattern
2. **Configure** - Set properties using fluent methods
3. **Animate** - Trigger enter/exit animations
4. **Tick** - Update animation state each frame
5. **Paint** - Render to a scene

### Example: Frame with Button

```rust
use hud::{FrameCorners, HudButton, colors};
use wgpui::{Bounds, Scene, TextSystem};

struct MyUI {
    frame: FrameCorners,
    button: HudButton,
}

impl MyUI {
    fn new() -> Self {
        Self {
            // Create a corner frame
            frame: FrameCorners::new()
                .corner_length(20.0)
                .line_width(2.0)
                .color(colors::FRAME_BRIGHT),

            // Create a button
            button: HudButton::new("ACTIVATE")
                .font_size(14.0)
                .padding(20.0, 10.0),
        }
    }

    fn start(&mut self) {
        // Trigger enter animations
        self.frame.animator_mut().enter();
        self.button.animator_mut().enter();
    }

    fn update(&mut self) {
        // Tick animations every frame
        self.frame.tick();
        self.button.tick();
    }

    fn paint(&self, scene: &mut Scene, text_system: &mut TextSystem) {
        let frame_bounds = Bounds::new(100.0, 100.0, 300.0, 200.0);
        self.frame.paint(frame_bounds, scene);

        let button_bounds = Bounds::new(150.0, 150.0, 120.0, 40.0);
        self.button.paint(button_bounds, scene, text_system);
    }
}
```

## Animation System

### HudAnimator

Every component contains a `HudAnimator` that manages its animation state:

```rust
use hud::HudAnimator;

let mut animator = HudAnimator::new()
    .enter_duration(30)  // 30 frames to enter
    .exit_duration(20);  // 20 frames to exit

// Start entering
animator.enter();

// Check state
if animator.is_animating() {
    println!("Currently animating");
}

// Get progress for rendering
let opacity = animator.progress(); // 0.0 to 1.0
```

### Animation States

```
Exited ─────enter()────▶ Entering ───(complete)───▶ Entered
   ▲                                                    │
   │                                                    │
   └───(complete)─── Exiting ◀────────exit()────────────┘
```

### Coordinating Multiple Animations

Use `AnimatorManager` to orchestrate child animations:

```rust
use hud::{AnimatorManager, ManagerMode, HudAnimator};

// Stagger mode: children animate one after another
let mut manager = AnimatorManager::new(ManagerMode::Stagger)
    .stagger_offset(5); // 5 frames between each child

// Add children
manager.add_child(HudAnimator::new());
manager.add_child(HudAnimator::new());
manager.add_child(HudAnimator::new());

// Start all (they will stagger automatically)
manager.enter();

// In update loop:
manager.tick();
```

Available modes:
- `Parallel` - All children animate simultaneously
- `Stagger` - Children animate with offset delays
- `Sequence` - Each child waits for the previous to complete

## Handling Input

Interactive components accept input events:

```rust
use hud::HudButton;
use wgpui::{InputEvent, Bounds};

let mut button = HudButton::new("CLICK ME")
    .on_click(|| println!("Button clicked!"));

// In your event handler:
fn handle_event(&mut self, event: &InputEvent) {
    let button_bounds = Bounds::new(100.0, 100.0, 120.0, 40.0);

    // Returns true if event was handled
    if self.button.event(event, button_bounds) {
        // Request redraw
    }
}
```

## Theme Colors

HUD provides predefined colors for consistency:

```rust
use hud::colors;

// Frame colors (white with varying opacity)
colors::FRAME_BRIGHT  // High visibility
colors::FRAME_NORMAL  // Default
colors::FRAME_DIM     // Subtle

// Text colors
colors::TEXT          // Primary text
colors::TEXT_MUTED    // Secondary text

// Background
colors::BG            // Pure black
colors::DOT_GRID      // Dot grid opacity
```

## Next Steps

- [Animation System](animator.md) - Deep dive into animations
- [Components Reference](components.md) - All available components
- [Theming](theming.md) - Customizing colors
