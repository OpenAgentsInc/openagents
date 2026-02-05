Hatchery frame animation notes

Context
- Goal: Arwes-style Nefrex frame assembly animation on the hatchery page.
- Stack: `@arwes/react-animator`, `@arwes/react-frames`, custom hatchery wrapper.

Key implementation details
- The Nefrex frame uses `useFrameAssembler` and `FrameNefrex` with `animated={false}`.
  This matches Arwes' own assembler sandbox and avoids double animations.
- The hatchery page uses an `Animator` with `root` and `active` toggled by UI.
- Activation is gated until the frame SVG has rendered its elements.
  The `AssemblingFrame` component waits for `[data-frame]` and `[data-name=line]`
  to exist before calling `onReady()`.
- A small controller inside the animator explicitly sends `enter`/`exit` on changes
  so the assembler always receives `entering`/`exiting` transitions.

Background fade behavior
- The frame background is intentionally darker (very dark purple).
- Background fade is delayed relative to the corner/line draw:
  - enter delay: 240ms
  - exit delay: 160ms
  - fade duration: 220ms
- This is implemented in `AssemblingFrame` by subscribing to animator state and
  applying delayed opacity changes to `[data-name=bg]`.

Files touched
- `apps/web/src/components/hatchery/LiteClawHatchery.tsx`
  - `Animator` root with active toggle
  - `FrameAnimatorController` for explicit enter/exit
  - frame activation deferred until `AssemblingFrame` signals readiness
- `apps/web/src/components/hatchery/AssemblingFrame.tsx`
  - `useFrameAssembler` integration
  - delayed background fade relative to line draw

Notes
- If the frame renders solid (no animation), ensure the animator is activating
  only after frame elements exist and that `FrameAnimatorController` is present.
