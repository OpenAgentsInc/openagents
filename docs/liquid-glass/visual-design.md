# Visual Design Updates (Liquid Glass)

## Overview

Liquid Glass drives a cohesive, adaptive visual language across platforms. The system emphasizes clarity, depth, and a tight relationship between interface and content, with subtle refinements to color, typography, and geometry.

## System colors

- Tuned across Light, Dark, and Increased Contrast to harmonize with Liquid Glass.
- Improved hue differentiation while preserving Apple’s optimistic character.
- Works in concert with materials so controls remain legible over content.

## Typography

- Refined to strengthen clarity and structure.
- Bolder and left‑aligned in key moments (e.g., alerts, onboarding) to aid readability and scannability.

## Shapes and concentricity

- Three shape types ensure rhythm and balance:
  - Fixed shapes — constant corner radius.
  - Capsules — radius equals half the container height.
  - Concentric shapes — radius derived by subtracting padding from the parent.
- Concentric layouts align radii and margins around a shared center so shapes nest comfortably.
- Optical balance: center mathematically when it fits; subtly offset when it doesn’t.
- Capsules naturally support concentricity and appear widely (sliders, switches, bars, buttons, grouped table corners).

## Platform fit and control sizes

- iOS/iPadOS: Capsules bring focus and clarity to touch‑centric layouts.
- macOS:
  - Mini/Small/Medium controls: rounded rectangles for dense UIs (e.g., inspectors).
  - Large/X‑Large controls: capsules to provide emphasis in more spacious areas.
  - Used together, sizes and shapes create hierarchy across complex desktop layouts.

## Harmonize your visual language

- “Play in the same key”: your components should complement the system’s rhythm and tone.
- Use the three shape types to align with system APIs and behaviors.
- Watch for corners that feel pinched or flared; tension often means the inner shape should be concentric.
- At device edges:
  - Phone: use a capsule with extra margin for breathing room.
  - iPad/Mac: use a concentric shape aligned to the window edge for better balance.
- Components that live both inside containers and standalone: use a concentric shape with a fallback radius; the concentric value adapts when nested, fallback applies when alone.

