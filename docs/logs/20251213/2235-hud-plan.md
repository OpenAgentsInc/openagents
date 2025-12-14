# HUD Crate Implementation Plan

Port Arwes sci-fi HUD patterns to WGPUI with white/opacity theme on black background.

## Crate Structure

```
crates/hud/
├── Cargo.toml
├── src/
│   ├── lib.rs                  # Public API
│   ├── theme.rs                # White/opacity color constants
│   ├── easing.rs               # Easing functions
│   ├── animator/
│   │   ├── mod.rs
│   │   ├── state.rs            # AnimatorState enum
│   │   ├── animator.rs         # HudAnimator state machine
│   │   └── manager.rs          # AnimatorManager (parallel/stagger/sequence)
│   ├── frame/
│   │   ├── mod.rs
│   │   ├── corners.rs          # FrameCorners (bracket corners)
│   │   └── lines.rs            # FrameLines (edge lines with gaps)
│   ├── background/
│   │   ├── mod.rs
│   │   └── dot_grid.rs         # DotGridBackground
│   ├── button/
│   │   ├── mod.rs
│   │   └── hud_button.rs       # HudButton with frame
│   └── bin/
│       └── hud_demo.rs         # Demo binary
```

## Implementation Steps

### Step 1: Create Crate Foundation

Create `crates/hud/Cargo.toml`:
```toml
[package]
name = "hud"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "hud_demo"
path = "src/bin/hud_demo.rs"

[dependencies]
wgpui = { path = "../wgpui", features = ["desktop"] }
smallvec = "1"
log = "0.4"

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
env_logger = "0.11"
winit = "0.30"
```

### Step 2: Theme Module (`src/theme.rs`)

White-on-black color palette using `Hsla::white().with_alpha()`:

```rust
pub mod hud {
    pub const BG: Hsla = Hsla::black();              // #000000
    pub const FRAME_BRIGHT: Hsla = Hsla::white();    // Full white
    pub const FRAME_NORMAL: Hsla = /* white @ 0.7 */;
    pub const FRAME_DIM: Hsla = /* white @ 0.4 */;
    pub const DOT_GRID: Hsla = /* white @ 0.15 */;
    pub const TEXT: Hsla = /* white @ 0.9 */;
}

pub mod timing {
    pub const ENTER_FRAMES: u32 = 15;  // ~250ms @ 60fps
    pub const EXIT_FRAMES: u32 = 10;   // ~167ms
    pub const STAGGER_OFFSET: u32 = 3; // ~50ms between children
}
```

### Step 3: Easing Module (`src/easing.rs`)

```rust
pub type EasingFn = fn(f32) -> f32;

pub fn linear(t: f32) -> f32;
pub fn ease_out_cubic(t: f32) -> f32;     // Arwes default
pub fn ease_in_cubic(t: f32) -> f32;
pub fn ease_in_out_cubic(t: f32) -> f32;
pub fn ease_out_expo(t: f32) -> f32;      // Dramatic decel
```

### Step 4: Animator System

**`src/animator/state.rs`** - Animation state enum:
```rust
pub enum AnimatorState {
    Exited,    // Not visible
    Entering,  // Transitioning in
    Entered,   // Fully visible
    Exiting,   // Transitioning out
}
```

**`src/animator/animator.rs`** - Core state machine:
```rust
pub struct HudAnimator {
    state: AnimatorState,
    progress: f32,           // 0.0-1.0 (eased)
    frame_count: u32,
    enter_duration: u32,
    exit_duration: u32,
    easing: EasingFn,
}

impl HudAnimator {
    pub fn new() -> Self;
    pub fn enter_duration(self, frames: u32) -> Self;
    pub fn easing(self, f: EasingFn) -> Self;
    pub fn enter(&mut self);
    pub fn exit(&mut self);
    pub fn tick(&mut self) -> bool;  // Returns true if animating
    pub fn progress(&self) -> f32;
    pub fn state(&self) -> AnimatorState;
}
```

**`src/animator/manager.rs`** - Orchestration:
```rust
pub enum ManagerMode {
    Parallel,        // All together
    Stagger,         // Overlapping sequence
    StaggerReverse,
    Sequence,        // One at a time
    SequenceReverse,
}

pub struct AnimatorManager {
    mode: ManagerMode,
    stagger_offset: u32,
    children: Vec<HudAnimator>,
}

impl AnimatorManager {
    pub fn new(mode: ManagerMode) -> Self;
    pub fn add_child(&mut self, animator: HudAnimator);
    pub fn enter(&mut self);
    pub fn exit(&mut self);
    pub fn tick(&mut self) -> bool;
    pub fn child(&self, index: usize) -> Option<&HudAnimator>;
}
```

### Step 5: Frame Components

**`src/frame/corners.rs`** - Corner brackets `[ ]`:
```rust
pub struct FrameCorners {
    animator: HudAnimator,
    corner_length: f32,      // Length of each bracket arm
    line_width: f32,         // Thickness
    color: Hsla,
    padding: f32,
}

impl FrameCorners {
    pub fn new() -> Self;
    pub fn corner_length(self, len: f32) -> Self;
    pub fn line_width(self, width: f32) -> Self;
    pub fn color(self, color: Hsla) -> Self;
    pub fn animator_mut(&mut self) -> &mut HudAnimator;
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene);
}
```

Corner drawing algorithm:
- For each corner (TL, TR, BL, BR)
- Draw horizontal quad from corner point
- Draw vertical quad from corner point
- Animate length based on `animator.progress()`
- Fade alpha based on `animator.progress()`

**`src/frame/lines.rs`** - Edge lines with center gaps:
```rust
pub struct FrameSides {
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
}

pub struct FrameLines {
    animator: HudAnimator,
    sides: FrameSides,
    gap: f32,                // Gap in middle of each line
    line_width: f32,
    color: Hsla,
}
```

Line drawing algorithm:
- For each enabled side
- Draw two quads (left/right of center gap for horizontal, top/bottom for vertical)
- Animate from center outward using `progress`

### Step 6: Background Component

**`src/background/dot_grid.rs`**:
```rust
pub struct DotGridBackground {
    animator: HudAnimator,
    spacing: f32,            // Distance between dots
    dot_radius: f32,         // Dot size
    color: Hsla,             // White @ ~0.15
}

impl DotGridBackground {
    pub fn new() -> Self;
    pub fn spacing(self, s: f32) -> Self;
    pub fn dot_radius(self, r: f32) -> Self;
    pub fn animator_mut(&mut self) -> &mut HudAnimator;
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene);
}
```

Dot grid algorithm:
- Calculate grid starting at bounds origin
- For each grid point within bounds:
  - Draw small circular quad (`with_uniform_radius(dot_radius)`)
  - Color alpha = `base_alpha * animator.progress()`

### Step 7: HUD Button

**`src/button/hud_button.rs`**:
```rust
pub struct HudButton {
    label: String,
    animator: HudAnimator,
    frame: FrameCorners,
    hovered: bool,
    pressed: bool,
    font_size: f32,
    on_click: Option<Box<dyn FnMut()>>,
}

impl HudButton {
    pub fn new(label: impl Into<String>) -> Self;
    pub fn on_click<F: FnMut() + 'static>(self, f: F) -> Self;
    pub fn tick(&mut self);
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, text: &mut TextSystem);
    pub fn event(&mut self, event: &InputEvent, bounds: Bounds) -> bool;
}
```

Button states:
- Normal: frame at normal opacity
- Hovered: frame brighter (1.0 alpha)
- Pressed: frame slightly dimmed

### Step 8: Demo Binary

**`src/bin/hud_demo.rs`**:

```rust
struct HudDemo {
    platform: Option<DesktopPlatform>,
    background: DotGridBackground,
    main_frame: FrameCorners,
    panel_manager: AnimatorManager,  // 3 staggered panels
    buttons: Vec<HudButton>,
    started: bool,
}
```

Demo layout:
1. Full-screen black background
2. Dot grid covering entire area
3. Main frame with large corners
4. Title text: "HUD SYSTEM v1.0"
5. Three buttons stacked vertically: CONNECT, SCAN, ABORT
6. All components animate in with stagger on start

### Step 9: Integration

Add to workspace `Cargo.toml`:
```toml
[workspace]
members = [
    # ... existing
    "crates/hud",
]
```

Add cargo alias to `.cargo/config.toml`:
```toml
hud = "run --bin hud_demo"
```

## Key Files to Reference

| File | Pattern |
|------|---------|
| `crates/wgpui/src/color.rs` | `Hsla::white()`, `with_alpha()` |
| `crates/wgpui/src/scene.rs` | `Scene`, `Quad`, `draw_quad()` |
| `crates/coder/widgets/src/button.rs` | Widget trait, builder pattern |
| `crates/coder/app/src/main.rs` | Desktop ApplicationHandler |

## Verification

Run demo: `cargo hud`

Expected behavior:
1. Window opens with black background
2. Dot grid fades in
3. Main frame corners animate (grow from 0 to full length)
4. Buttons animate in with stagger (50ms apart)
5. Buttons respond to hover (brighten) and click (callback fires)
