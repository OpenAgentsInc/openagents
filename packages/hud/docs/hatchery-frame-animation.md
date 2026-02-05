Autopilot frame animation notes

Context
- Goal: Arwes-style Nefrex frame assembly animation on the autopilot page.
- Stack: Fully inlined autopilot page components (no `@arwes/*` imports).

Key implementation details
- The Nefrex frame is rendered with a custom SVG path generator in `AssemblingFrame`.
- The assembling animation is handled by `animateFrameAssemblerCompat` (requestAnimationFrame)
  instead of Arwes' Motion/WAAPI implementation to avoid `linear()` easing errors.
- Activation is gated until the frame SVG has rendered its elements.
  The `AssemblingFrame` component waits for `[data-frame]` and `[data-name=line]`
  to exist before calling `onReady()`.
- The autopilot page drives the frame animation with a simple `active` boolean,
  passed into `AssemblingFrame`.

Background fade behavior
- The frame background is intentionally darker (very dark purple).
- Background fade is delayed relative to the corner/line draw:
  - enter delay: 240ms
  - exit delay: 160ms
- This is implemented in the compat assembler which animates `[data-name=bg]`
  with its own delay relative to the line draw.

Files touched
- `apps/web/src/components/hatchery/AutopilotPage.tsx`
  - `frameVisible` toggle and delayed activation
- `apps/web/src/components/hatchery/AssemblingFrame.tsx`
  - custom SVG frame rendering
  - requestAnimationFrame-based assembler animation
- `apps/web/src/components/hatchery/animateFrameAssemblerCompat.ts`
  - inline animation engine for the frame

Notes
- If the frame renders solid (no animation), ensure `frameVisible` is toggling
  and the frame SVG contains `[data-name=line]` paths before activation.
