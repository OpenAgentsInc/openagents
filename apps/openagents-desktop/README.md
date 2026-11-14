# OpenAgents desktop

The recovery shell is a Coax chat application on macOS and Linux. Both builds
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
new desktop release is produced by this plan alone.

## Native environment client

`native/` now contains the first Coax Native desktop clients: AppKit with
JavaScriptCore on Mac, and generated Coax C with native GTK controls on Linux.
They run the same application as iPhone. `coder host serve` owns managed
Claude processes independently of windows; the client uses the account
gateway for remote computers and a protected local adapter for **This computer**.

Build the Mac target with `native/build.sh`. The Linux SDK linker is
`native/build-linux.sh`, after generating `native/program.coax`,
`native/linux-services.coax` and `coder/tui/host.coax` on a build host. It
preserves `com.openagents.desktop`; the Mac package retains the installed
OpenAgents icon and advances the previous1.0.0/build28828 identity.

See [the runtime contract](../../docs/protocol/environment-runtime-v1alpha1.md)
for enrollment, history/continuation bounds and current qualification gaps.
The recovery shell and its saved settings stay available; this slice does
not overwrite native conversations or claim Windows/realm-preview support.
