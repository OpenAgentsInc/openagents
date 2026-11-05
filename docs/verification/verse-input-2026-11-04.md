# Verse desktop input, layout and direct WoW display, 2026-11-04

The owner encountered a hidden cursor, a close shortcut that exited Verse,
missing physical right-clicks, and an installer clipped into a narrow tile.
These were separate defects in the nested desktop path.

The host now enables compositor cursor capture on both newly created and
attached desks. Its Wayland callbacks forward right/middle presses and keep
their releases with the original desktop, including after leaving the window,
crossing the panel boundary or closing the panel. Unowned releases and
unsupported buttons do not fabricate input.

`os/verse-window-policy.coax` decides whether an outer close belongs to the
visible nested desktop. The host forwards that close and keeps its own window
alive. With the panel closed, the ordinary outer close still exits Verse.
The compositor's FIFO close delegates to its existing per-window close path:
Wayland `xdg_toplevel.close` or X11 `WM_DELETE_WINDOW`. The previous FIFO path
incorrectly assumed every focused client was a Wayland toplevel.

The runner's main launcher/game surfaces cover the desktop as floating
windows so a later floating modal can remain above them. Other windows from
that runner retain their initial X11 size and float centrally. Two pixels of
padding absorb fractional-coordinate rounding; title properties can still
promote a helper-sized surface to the main-window rule when they arrive later.
The current blank helper was also hidden with X11 UnmapWindow, leaving its
process and Battle.net running. The install button then became accessible.

Validation:

- Panel checks: 9 passed; close authority: 11/11 mutants killed.
- Compositor/dependency checks: 102 passed; game-window authority: 3/3 killed.
- Independent actual Wayland callback/pipe peer under ASan/UBSan: close,
  right/middle routing, outside/closed/leave release ownership, keyboard
  snapshots and hand EOF passed, with leak detection enabled.
- Independent Xlib SDK peer under ASan/UBSan against the emitted real
  compositor: main window 2548×1428, small helper without a tiled column,
  centered installer at least its requested 920×560 with rounding padding,
  cursor pixels, and exactly the dialog's WM_DELETE_WINDOW passed. The peer
  keeps the other two clients and compositor alive. Leak detection enabled.
- Live outer Super+W closed a disposable inner terminal while the Verse
  process/window stayed alive. A live desktop capture includes its cursor.
- `tools-check` retains the 29 existing missing-reason-header failures.
  Both SDK C oracles declare Firm reason 2.

The exploratory full legacy panel mutation run reported 79 survivors in old
geometry/statistics helpers and six unreached constant adapters. It is not a
passing whole-panel mutation claim. The new close admission has its separate
zero-survivor authority gate; no equivalent mutants were excluded.

## Faster display path

The owner then prioritized maximum responsiveness over the nested Verse
desktop. Its current capture loop asks for at most 30 frames per second,
CPU-composites a native 2548×1428 RGB image, reads it back from the game,
loads it through PPM and uploads it for the outer Verse window. The Grid
also continues stepping/rendering behind the flat panel. GPU utilization
was about 40% during the reported lag; that observation does not establish
a game FPS or end-to-end latency measurement.

The immediate path is the installed host's `coder-battlenet` on outer
DISPLAY :1, with WoW directly on desk 2. This removes the nested compositor,
panel frame cap and its image transfer entirely. Proton and the Windows
graphics-to-Vulkan translation remain. Native mouse buttons/cursor go through
the host desktop's normal Xwayland path. The nested-input binaries were
retained as an optional fallback, not inserted into this direct display path.

The old `verse-game` v2 socket does not acquire a direct outer-desktop game.
A fast native adapter preserving target lifetime, input ownership and evidence
contracts is still required before claiming the same autonomous controller on
this path. Do not send controls to a stale nested session or claim a direct
display is qualified for the earlier Elephant combat loop.

Live direct-path qualification: outer handle `0x53`, X11 window 106954753,
WoW PID 4103330, outer desk 2, fullscreen 2560×1440. The owner entered the
world as Arcturus Ra (level 2) in Thendal Village using the `_forever_`
client; this is a different client/character from the Foob sensor trial.
The saved client configuration selects D3D11. No Verse session, nested
compositor or realm renderer remains running. An owner-opened Chromium
window on desk 2 was covered by fullscreen, not closed: Super+F restores
the tiled layout. No game FPS or latency number was inferred from GPU load.
Native automation is tracked in [#12889](https://github.com/OpenAgentsInc/openagents/issues/12889).

## Space and sound

Removed 61 clean, inactive stale worktrees (41.91 GiB) and 193 local branch
names. Unmerged commits have archive refs; dirty/active trees, the running
Elephant checkout and all unique WoW evidence remain. Free space reached
43 GiB. JSONL inventories and branch-to-archive mappings live under
`~/.openagents/worktree-cleanup-2026-11-04-*`.

WoW's audio stream and HDMI sink were unmuted. A three-second sink-monitor
capture had mean -30.8 dB and peak -11.0 dB, establishing non-silent output,
not that the owner could hear that monitor. The PCH controller had no driver:
boot logs showed deferred audio-component binding and an unsupported Xe probe.
Loading `i915` registered the Intel PCH/ALC897 analog sink, and WirePlumber
automatically restored the owner's configured analog output at 43% and moved
WoW's active stereo links there. No microphone was recorded. This runtime
driver repair has not yet been persisted as a NixOS boot-module change.

Local binaries, before/after hashes, SDK scenes, logs and captures are retained
under `~/.openagents/verse-os/combat/2026-11-04-loot/input-repair/`.
