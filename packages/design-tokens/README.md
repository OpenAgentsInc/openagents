# OpenAgents Design Tokens

`@openagentsinc/design-tokens` is the neutral token authority for shared
OpenAgents UI packages. It intentionally has no Foldkit, style compiler, React,
or app runtime dependency.

Current scope:

- Autopilot dark palette tokens used by web, desktop, and protocol parity.
- Foldkit CSS variable output for coexistence with existing global CSS.
- Plain native-shaped theme output for future mobile/native consumers.
- Flattened protocol token output for `CANONICAL_DARK` parity checks.

Out of scope for this first package:

- Forum, public landing, marketing, and tenant theme palettes.
- Mobile/native theme adoption. The package exposes native-shaped tokens, but
  app migration remains a later track.
- Component-level sizing, spacing, typography, motion, and semantic surface
  tokens that do not yet have an agreed shared contract.

Add new tokens here only when at least two package or app surfaces need the same
value contract. App-only styles should stay local until they become shared.
