# OpenAgents desktop

The existing app is a Coax chat application on macOS and Linux. Both builds
use `zeron/linux.coax` and the shared components; Mac uses the shared
AppKit/Metal host and Linux uses the shared Wayland/Vulkan host. Existing
implementation and checks are described in the [desktop reference](../../docs/desktop/coax-desktop.md)
and [apps roadmap](../../docs/apps/2026-11-10-apps-roadmap.md).

## Replacement direction — 2026-11-13

Replace this shell incrementally with **OpenAgents on Coax Native**, on Mac
and Linux first, then Windows. The application is the normal way to set up
this computer's environment and chat/control any enrolled computer. A
versioned resident Coax service owns local Claude execution independently
of windows; the same service can run through the CLI without a desktop.

The canonical [environment, Claude and realm-authoring specification](../../docs/protocol/environments-claude-and-world-editing.md#31-one-desktop-product-two-independent-lifetimes)
owns this plan, its shared protocol, local/remote authorization, service
lifecycle, migration and replacement gates. [Coax Native](../../coax/native.md)
owns the application framework. This direction is specified, not implemented
by this documentation change; the current app still owns its agent processes
through the GUI lifetime.

Preserve `com.openagents.desktop`, app/signing identity, icons, credentials,
settings and conversation references. Reuse the current components, engines
and platform hosts rather than restoring the deleted Rust app or starting a
second agent stack. Windows backend selection remains open. Closing the new
client must eventually detach from ongoing turns, not kill them; do not claim
that guarantee for the current launch path.

Builds/checks run in a worktree through the repository's remote build workflow.
The existing platform entry points are `build.sh` and `linux/build.sh`; no
new desktop release is produced by this plan.
