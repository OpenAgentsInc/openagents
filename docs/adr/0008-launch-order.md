# 0008: Launch order: Kiki on the web first

Status: Accepted (owner, 2026-10-24: "the focus for what we need to launch
first on the web application focuses on the Kiki MVP, so we need that all
solid").

## Decision

1. **Kiki MVP on the web, solid, on desktop and phone browsers.** The routes
   are `/new-lander`, `/new-lander/verse`, and `/new-lander/house`, with
   their sign-in, guest, planet, studio, player, and invitation flows. This is
   the launch.
2. **Aethyr applied to the web app.** The Kiki surfaces are rebuilt in this
   repository on the controller and renderer split
   ([0005](0005-effect-controls-the-ui.md)), the design system
   ([0006](0006-one-design-system.md)), and the device profile
   ([0007](0007-capability-and-detail.md)). Live routes keep their current
   pinned releases until each rebuilt surface passes the same browser checks.
3. **Mobile app demos.** Working demos of Kiki surfaces and zones in the
   OpenAgents mobile app, using [0010](0010-mobile-zone-loading.md), to show
   how the same framework runs there.
4. **Mobile app zones for users:** the Haunted House and the Grid
   ([0009](0009-zones-and-the-grid.md)).

Native rendering of zones in the mobile app, and desktop, follow.

## Consequences

Work that doesn't serve step 1 or 2 waits unless it is cheap and unblocks a
later step.
