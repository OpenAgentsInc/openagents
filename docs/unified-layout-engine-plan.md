# Unified Layout Engine Refactor Plan (WGPUI)

## Goal
Unify all WGPUI UI layouts under the same Taffy-backed layout engine used by autopilot-ui,
starting with Storybook and then the rest of the WGPUI component surface.

## Non-goals
- Visual redesigns or UX changes that are unrelated to layout unification.
- Feature work on new components unrelated to layout refactors.

## Phases
### Phase 1: Layout engine parity + hardening
- Ensure LayoutEngine supports all primitives needed by the UI (measure callbacks, flex wrap,
  consistent padding/margin helpers).
- Add tests for measured nodes, flex wrap, and layout helper behaviors.
- Document the layout APIs in WGPUI for reuse.

### Phase 2: Layout helpers + container elements
- Add layout helper utilities for common patterns (stack, row, grid, header/nav/content,
  panel layouts).
- Introduce flex-based container elements (Flex/Stack/Grid) so element trees can be built
  without manual bounds math.
- Export helpers and containers for Storybook and component adoption.

### Phase 3: Storybook conversion
- Refactor Storybook scaffolding to use LayoutEngine instead of manual bounds math.
- Replace sidebar/content layout logic with layout helpers + container elements.
- Validate Storybook usability at multiple window sizes.

### Phase 4: Component migration
- Convert WGPUI components that perform manual layout math to LayoutEngine-driven layout.
- Introduce shared layout utilities or container elements where components share patterns.
- Remove dead layout code once parity is achieved.

### Phase 5: Cleanup + enforcement
- Remove legacy layout utilities and document migration guidance.
- Add tests/benchmarks for key layout flows.
- Capture follow-up optimizations (text measurement, cache invalidation).

## Worklog
- 2026-01-28: Phase 2: added `layout_helpers` utilities (offset/stack/row/grid/header-nav/
  panel), introduced Flex/Stack/Grid elements with per-child layout styles, and exported the
  new APIs from WGPUI. Added unit tests for helper layouts.
