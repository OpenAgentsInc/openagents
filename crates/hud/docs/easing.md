# Easing Functions

HUD includes a comprehensive library of easing functions for smooth animations.

## Overview

Easing functions transform linear progress (0.0 to 1.0) into curved motion:

```rust
use hud::easing::ease_out_cubic;

// Linear: t -> t
// Eased:  t -> ease_out_cubic(t)

let linear_half = 0.5;                  // = 0.5
let eased_half = ease_out_cubic(0.5);   // = 0.875 (already near the end)
```

## Using with HudAnimator

```rust
use hud::HudAnimator;
use hud::easing::ease_out_expo;

let animator = HudAnimator::new()
    .easing(ease_out_expo);
```

## Function Types

### Ease In
Slow start, accelerating toward the end.
Best for: exit animations, elements leaving the screen.

### Ease Out
Fast start, decelerating toward the end.
Best for: enter animations, UI feedback.

### Ease In Out
Slow start and end, fast middle.
Best for: looping animations, transitions.

## Available Functions

### Linear

```rust
use hud::easing::linear;

// No easing - constant speed
linear(0.5) // = 0.5
```

### Quadratic (Power of 2)

Gentle curve, subtle easing.

```rust
use hud::easing::{ease_in_quad, ease_out_quad, ease_in_out_quad};

ease_in_quad(0.5)     // = 0.25
ease_out_quad(0.5)    // = 0.75
ease_in_out_quad(0.5) // = 0.5
```

### Cubic (Power of 3)

**Default for HUD** - Smooth, natural motion.

```rust
use hud::easing::{ease_in_cubic, ease_out_cubic, ease_in_out_cubic};

ease_in_cubic(0.5)     // = 0.125
ease_out_cubic(0.5)    // = 0.875
ease_in_out_cubic(0.5) // = 0.5
```

### Quartic (Power of 4)

Stronger acceleration/deceleration.

```rust
use hud::easing::{ease_in_quart, ease_out_quart, ease_in_out_quart};
```

### Quintic (Power of 5)

Even more dramatic curve.

```rust
use hud::easing::{ease_in_quint, ease_out_quint, ease_in_out_quint};
```

### Sinusoidal

Smooth trigonometric curve.

```rust
use hud::easing::{ease_in_sine, ease_out_sine, ease_in_out_sine};
```

### Exponential

Dramatic acceleration - fast and snappy.

```rust
use hud::easing::{ease_in_expo, ease_out_expo, ease_in_out_expo};

// ease_out_expo is excellent for UI "pop" effects
ease_out_expo(0.3) // Already at ~0.875
```

### Circular

Follows a circular arc.

```rust
use hud::easing::{ease_in_circ, ease_out_circ, ease_in_out_circ};
```

### Back (Overshoot)

Goes past the target then settles back.

```rust
use hud::easing::{ease_in_back, ease_out_back, ease_in_out_back};

// ease_out_back overshoots slightly past 1.0
ease_out_back(0.8) // > 1.0 momentarily
```

Use for: bouncy buttons, playful UI elements.

### Elastic

Spring-like oscillation.

```rust
use hud::easing::{ease_in_elastic, ease_out_elastic, ease_in_out_elastic};
```

Use for: attention-grabbing effects, notifications.

### Bounce

Bouncing ball effect.

```rust
use hud::easing::{ease_in_bounce, ease_out_bounce, ease_in_out_bounce};
```

Use for: playful interactions, landing effects.

## Visual Reference

```
         ease_out (fast start)
         ╭────────────────
        ╱
       ╱
      ╱
     ╱
────╯
    ease_in (fast end)
```

```
         linear
        ╱
       ╱
      ╱
     ╱
    ╱
───╱
```

```
         ease_in_out
       ╭───────╮
      ╱         ╲
     ╱           ╲
    ╱             ╲
   ╱               ╲
──╯                 ╰──
```

## Recommendations by Use Case

| Use Case | Recommended Easing |
|----------|-------------------|
| Enter animations | `ease_out_cubic`, `ease_out_expo` |
| Exit animations | `ease_in_cubic`, `ease_in_quad` |
| Button hover | `ease_out_quad` |
| Modal appear | `ease_out_expo` |
| Loading bars | `linear` |
| Attention pulse | `ease_in_out_elastic` |
| Drop effects | `ease_out_bounce` |
| Smooth loops | `ease_in_out_sine` |

## Creating Custom Easing

All easing functions have the signature `fn(f32) -> f32`:

```rust
use hud::easing::EasingFn;

// Custom: sharp start, gradual end
fn my_easing(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(6)
}

let animator = HudAnimator::new()
    .easing(my_easing as EasingFn);
```

## Combining with Progress

Manual easing application:

```rust
use hud::easing::ease_out_expo;

fn paint(&self, scene: &mut Scene) {
    let raw_progress = self.animator.progress();

    // Apply extra easing for specific properties
    let scale = ease_out_expo(raw_progress);
    let opacity = raw_progress; // Linear fade

    // Different properties, different curves
    let width = 100.0 * scale;
    let color = Hsla::new(0.0, 0.0, 1.0, opacity);
}
```

## Testing Easing Functions

All easing functions satisfy:

- `f(0.0) ≈ 0.0`
- `f(1.0) ≈ 1.0`

Except for `back` variants which overshoot slightly.

```rust
#[test]
fn test_easing_bounds() {
    assert!((ease_out_cubic(0.0) - 0.0).abs() < 0.01);
    assert!((ease_out_cubic(1.0) - 1.0).abs() < 0.01);
}
```
