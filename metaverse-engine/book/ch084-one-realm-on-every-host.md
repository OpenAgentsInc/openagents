# 84. One realm on every host

> **Status: partly implemented.** The OpenAgents Metaverse Engine ships both
> `aethyr-host-v1` (single-session browser, Linux Vulkan, and macOS Metal
> lifecycle contracts) and **`aethyr-host-v2`** (`#14042`,
> [`metaverse-engine/host/aethyr-host-v2.coax`](../host/aethyr-host-v2.coax),
> `proto/verse/aethyr-host-v2.json`), a context-safe multi-instance native host
> powering the expandable Metaverse Viewer and embedded GameBuilder pane in
> OpenAgents Desktop (`apps/openagents-desktop/page/metaversepane.coax`,
> `#14040`–`#14046`) across macOS Metal 4 (120 Hz ProMotion ping-pong),
> Linux Wayland/Vulkan, and Windows Win32/Vulkan. Full cross-host service and
> realm-wide visual conformance remains separate work.

The host owns platform effects: the window, devices, GPU, audio, files and
sockets. Coax owns gameplay, admission, memory budgeting, refresh cadence, and
recovery policy. [`metaverse-engine/host/README.md`](../host/README.md)
specifies `aethyr-host-v1` (initialization, asynchronous ownership, and failure
behavior), while [`metaverse-engine/host/aethyr-host-v2.coax`](../host/aethyr-host-v2.coax)
and [`metaverse-engine/host/aethyr-host-v2.h`](../host/aethyr-host-v2.h) add
context-safe multi-instance hosting so one desktop process can mount multiple
viewports (a universal 3D model viewer, a live realm, an embedded GameBuilder
session, and inline chat cards) without global-state collisions or runaway VRAM.
Missing a contract version keeps a legacy face; it does not qualify that face
as a versioned host.

## Multi-instance hosting (`aethyr-host-v2`, `#14042`)

Desktop workspaces open more than one 3D surface at once—for example, a chat
thread with an inline preview alongside the right-side Metaverse pane
([`apps/openagents-desktop/page/metaversepane.coax`](../../apps/openagents-desktop/page/metaversepane.coax),
`#14040`). `aethyr-host-v2` replaces global host statics with explicit
instance handles (`AETHYR_MAX_INSTANCES = 16`, four kinds: `VIEWER`, `REALM`,
`GAMEBUILDER`, `INLINE`) governed by one `#authority` Coax budget and refresh
policy:

- **Aggregate VRAM ceiling:** `aethyr_effective_cap` bounds total GPU memory to
  `min(512 MiB, 10% of system RAM)` with a `64 MiB` floor, capping an active
  pane at `384 MiB` (`AETHYR_ACTIVE_TAB_CAP`) and an inline card at `96 MiB`
  (`AETHYR_INLINE_CARD_CAP`). Negative sizes, per-slot overruns, and aggregate
  overflows are refused whole by `aethyr_budget_charge`.
- **LRU eviction watermark:** once aggregate usage crosses `75%`
  (`aethyr_budget_needs_evict`), `aethyr_budget_evict_victim` selects the
  oldest unfocused slot holding GPU bytes, never evicting the focused viewport.
- **Inline deduplication (`OA-SHOW-LIVE`):** when an inline chat card names the
  exact asset or realm already visible in the right-side Metaverse pane,
  `aethyr_inline_dedup` yields the inline GPU context (`0 MiB` duplicate cost).
- **Dirty-gated refresh & thermal policy:** `aethyr_frame_hz` runs hidden or
  static scenes at `0 Hz` (event-driven redraw only), caps battery or unfocused
  viewports at `60 Hz`, and drives focused animated viewports on AC power up to
  `120 Hz` ProMotion.

## `composition-v2`, 120 Hz Metal 4, and Linux/Windows Vulkan parity (`#14046`)

Every native Desktop host composites 3D viewports directly into the Coax UI
scene graph through `composition-v2` external GPU textures rather than WebKit
overlays:

- **macOS Metal 4 (`apps/openagents-desktop/MetaverseViewportView.swift`,
  `metaverse-engine/host/apple/PageHost.m`, `metaverse-engine/host/metal/MetalRealm.m`):**
  double-buffered `MTLTexture` ping-pong (`293c698c7c`) eliminates display-link
  frame contention so 3D WASD focus and scene rendering hold `120 Hz` ProMotion
  without stalling the main Coax UI thread.
- **Linux Wayland/Vulkan & Windows Win32/Vulkan (`apps/openagents-desktop/linux/`,
  `apps/openagents-desktop/windows/`, `#14046`):** share the same
  `composition-v2` external-texture contract and triple-target Coax shaders
  (`coax wgsl`, `coax metal`, `coax spirv`), including `.splat` / 3D Gaussian
  Splatting rendering in the universal model viewer.
- **Embedded GameBuilder & CoreAudio (`#14043`–`#14045`):**
  `gamebuilder/core/desktop-bridge.coax` embeds the live GameBuilder editor
  directly inside the Desktop Metaverse pane with native Metal/Vulkan rendering,
  CoreAudio playback, live `--attach` CLI socket round-trips, and the Studio
  drawer (Agents, Claims, Replay Scrubber, Publish, and Nostr Lobby).

## One session, several hosts

[`metaverse-engine/session/lifecycle.coax`](../session/lifecycle.coax) defines
blur, hide, return, socket replacement, device loss, and module trap behavior.
Blur releases held inputs. A long suspension revokes pending commands and asks
for accepted state. Device loss keeps the session while drawing stops and
resources are restored; it does not replay commands.

[Chapter 95](ch095-offline-is-a-local-server.md) runs the same room tick in
native C and a browser worker, including shared math.
[Chapter 98](ch098-versions-deploys-and-reconnect.md) explains the seat, socket,
and owner lifetimes. The server's authoritative clock continues when a page
hides; pausing local prediction does not pause the room.

The lifecycle contract's client and room sides are adopted in Everglade. That
fact does not establish every platform event on every native face. The
versioned host adapters and their evidence are documented in
[`metaverse-engine/host/README.md`](../host/README.md) and
[`docs/metaverse/native-gpu.md`](../../docs/metaverse/native-gpu.md).

## Verify the boundary you use

The bounded streaming WebGPU carrier adopts
[render/loss-policy.coax](../render/loss-policy.coax) for the existing
ten-second recovery deadline and three-losses-per-minute contract. Actual loss
while rendering or building, late device completion, and stopped cleanup have
[separate receipts](../../docs/verification/aethyr-streaming-12990/walk-automatic-loss.md).
The Coax face refuses paid frames during rebuilding and, after a stop, admits
only the session's real closing phase. This fixture is separate from host-v1
adoption. The [native draw-result profile](../../docs/verification/aethyr-streaming-12990/walk-native-loss.md)
retains a submitted command pool/fence and resource borrows through a pending
result, then uses the same checked loss policy for reported draw loss. Real
Vulkan rendering with labelled SDK-result injection qualifies this boundary;
the broader native factory/upload and rescue surface remains unqualified. The
additive [whole-reactor browser rescue profile](../../docs/verification/aethyr-streaming-12990/walk-persistent-rescue.md)
retains allocator-backed sources and queued facts, restores one exact-build
save, and suppresses another save before 300 frame starts. Actual trap,
failed-adapter, and pending-load reloads qualify a two-MiB desktop profile;
they do not establish native or larger-memory rescue.

```shell
coax/bin/coax test metaverse-engine/session/lifecycle.coax --library
coax/bin/coax test metaverse-engine/host/aethyr-host-v2.coax --library
```

Contract checks establish policy, not device behavior. Host qualification also
needs actual input, transport, audio, resource retirement, and recovery
observations. See the
[host verification records](../../docs/verification/host-contract-2026-11-05/README.md)
and the [Mac acceptance record](../../docs/verification/metal-host-2026-11-05/current-85c-final.md).
Do not infer a cross-architecture session digest or a universal recovery
latency from a component's byte comparison. Each shipped realm and host needs
its own complete evidence.
