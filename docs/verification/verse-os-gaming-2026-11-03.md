# Verse OS gaming, 2026-11-03

Owner sequence: #12724 accelerated desk, #12725 Battle.net/WoW launcher,
#12726 targeted input, #12727 Coax controller. Work proceeds in that order.

## Accelerated desk (#12724)

The Coax compositor advertises linux-dmabuf v4 when its chosen render node
provides an EGL/GBM context and importable ARGB8888/XRGB8888 modifiers.
The default node is /dev/dri/renderD128; --render-node selects another.
Coax owns feedback, params, descriptor ownership, format/size admission and
buffer lifetime. The C effect imports an EGLImage, attaches it to a framebuffer,
and synchronously reads pixels. Implicit DMA-BUF synchronization applies;
this does not implement the explicit syncobj protocol.

This first path retains CPU composition and the existing PPM upload into the
Verse panel. It enables accelerated clients; it is not a zero-copy panel path
or a claim of WoW performance. No driver/compositor on the owner's desktop
was replaced and no VT or visible window was opened.

Observed on coderos-4080, RTX 4080, NVIDIA 595.71.05:

- Private headless Xwayland, upstream glxgears: renderer NVIDIA GeForce
  RTX 4080/PCIe/SSE2, OpenGL 4.6. 24 advertised format/modifier pairs.
- Captured two changing 800x600 desk images, with a fullscreen resize between.
  DMA-BUF rows retain their producer's order; no additional GL row flip applies.
  The integration check independently tests that glxgears' blue gear is above
  its red gear, as the upstream scene specifies. Battle.net text is upright.
- Native ASan/UBSan build: 11 GPU imports, two captures, zero live imported
  images at shutdown, compositor exit 0. LeakSanitizer disabled for the
  external driver; imported-image ownership was counted explicitly.
- `coax test os/compositor.coax --library`: 18 checks passed (after the launcher binding).
- `coax mutate os/desk-buffer-policy.coax --library --jobs 4`:
  43 mutants, 43 killed, zero survivors (the initial admission-only policy;
  the unnecessary row-flip helper has been removed).
- `nix flake check --no-build ./os/nix`: passed. Fixed the existing duplicate
  dynamic `checks.${system}` declaration that initially prevented this gate.
- `coax/web/tool.sh tools-check`: ran; reports eight pre-existing Python
  tools without reason headers (the two vector pin tools and six human/hotbar
  pack tools), plus a port-list cleanup notice. This change adds no non-Coax tool.

The reproducible Coax integration driver is `os/desk-gpu-check.coax`:

```sh
nix-shell -p gcc pkg-config libdrm libglvnd libgbm mesa-demos xwayland
coax/web/tool.sh os/desk-gpu-check.coax COMPOSITOR os/compositor-us.xkb OUTPUT_DIR
```

Build COMPOSITOR from `coax c os/compositor.coax --library`, the standard
tool/process/store/socket/entropy effects, wayland-native.c and screen-native.c,
with `-DCOAX_WAYLAND_GPU` and pkg-config's libdrm/egl/glesv2/gbm flags.
`verse/gpu/realm.sh` and the Nix realms package now do that automatically.
Builds without the macro retain the shared-memory-only host, and a failed GPU
probe preserves shared-memory clients.

## Battle.net and World of Warcraft (#12725)

`os/battlenet.coax`, installed as `verse-battlenet`, runs from a terminal on
the Verse desk, or through Super+G while the desk has the keyboard. It probes
the desk, requires a local Xwayland DISPLAY and the existing installation,
refuses when Battle.net or WoW is already running, and holds a nonblocking
prefix launch lock for the installed runner's lifetime. It reuses
`/run/current-system/sw/bin/coder-battlenet`, retaining that host's GE-Proton,
umu, windowed D3D11 settings and winebrowser integration. It does not ship
that host's private gaming configuration in the public image.

Observed in a private 1280x720 headless desk on coderos-4080:

- `verse-battlenet --check` identified the desk and its actual DISPLAY :62.
  Outside that desk it refused. A second launch while it was running refused.
- Battle.net opened from ~/Games/battlenet using GE-Proton11-6. Its cached
  account reached the launcher without entering or exposing credentials.
- Selected World of Warcraft Classic / Classic Era, pressed Play, reached
  character selection, and entered the world. A desk capture showed the
  character in Coldridge Valley. No account purchase or sign-in was needed.
- Audio went to a temporary null sink; neither application opened on the
  owner's visible desktop.
- Launcher entry: 84 Coax checks passed. Its admission policy: 31 mutants,
  31 killed, zero survivors. Updated compositor: 18 checks passed.
- The accelerated-client regression passed with changing frames, correct
  orientation, resize, zero live GPU imports at shutdown, and exit 0.

The current game buffer is smaller than the desk's requested fullscreen
rectangle; the remaining area is black. This is functional windowed rendering,
not a fullscreen-resolution or frame-rate qualification. The next issues
provide explicit window targeting, bounded controls and a controller CLI.

## Targeted control (#12726)

Generation 2 is specified in `docs/protocol/desk-control-v2.md` and
`proto/openagents/os/desk/v2/control.proto`. Generation 1 remains available.
The new API exposes the actual display and window/buffer geometry, uses a
fresh OS-entropy session identifier and per-mapping window lifetime revisions,
and grants one connection a lease of at most five seconds. Window handles
now reuse free bounded slots instead of eventually indexing past the window
table; every new mapping has a new revision.

Owned input bypasses compositor keyboard shortcuts. It is released on stop,
disconnect, lease expiry, target/focus loss, physical/legacy input, VT departure
and shutdown. Slow reply writes have a 100 ms deadline, so a client that stops
reading cannot block the compositor indefinitely. The screenshot API returns
a capture sequence and the target's metadata.

Checks and observations:

- Full compositor: 100 Coax checks passed. Lease/slot policy: 17 mutants
  killed, zero survivors. Strict wire decoder: 207 entry-source mutants
  killed, zero survivors; dependency checks retained in that run.
- New protobuf compiled with protoc 34.1 to a temporary descriptor.
- `os/desk-control-check.coax` against upstream xev in an isolated desk,
  native ASan/UBSan: input delivered, relative motion, competing controller,
  malformed/duplicate request, outside-target refusal, disconnect, expiry
  without traffic, emergency stop, legacy input, focus loss, removal and
  recycled-handle lifetime checks all passed. Compositor exit 0, no sanitizer
  error. LeakSanitizer remains disabled for the external graphics driver.
- In actual WoW Classic, Enter pressed through generation 2 entered the
  world. A 200 ms W hold visibly moved the character among the graves in
  Coldridge Valley. An 80-pixel right-button relative drag visibly turned
  the camera. Subsequent info reported inactive, zero keys and zero buttons.
  These were bounded controls on the private headless desk with muted audio.

`os/desk-call.coax SOCKET [--pause-ms N] REQUEST_JSON...` is the developer
probe: its requests share one connection. The controller CLI follows in #12727.

## Controller CLI (#12727)

`os/game.coax` builds as `verse-game`: list, look, press, hold, click, drag,
stop. It selects exactly one title/handle/lifetime, uses the v2 socket and
never assumes an X display number. `look` captures PPM from the desk and can
ask the installed ffmpeg to encode PNG; a JSON sidecar binds the file to the
capture's frame/session/window metadata. Holds are bounded to two seconds.
Closing the connection releases only that connection's lease; explicit
`stop` is the separate emergency stop.

Observed:

- Game entry: 87 Coax checks. Action policy: 193 mutants killed, zero
  survivors. Target-selection policy: 65 entry-source mutants killed,
  zero survivors, dependency checks retained.
- The native ASan/UBSan fixture now also drives a private foot/cat terminal
  through the actual CLI. Capture -> press -> capture changes the visual
  fixture; PNG bytes and metadata are written; an invalid fractional hold
  refuses before input. Killing a CLI while the desk observably holds its
  key clears the key and lease. Emergency stop passes. All cases passed.
- Against actual WoW: CLI PNG capture, a 100 ms W hold, a -40-pixel right
  drag, another PNG capture and stop all succeeded. The earlier API run
  independently established visible movement and camera turning.
- Session checks after adding the tools directory to its desk's PATH:
  34 passed. Nix flake evaluation after packaging the CLI and PNG converter:
  passed. The CLI does not add a new Python, JavaScript or shell tool.

- Built and installed the native newland/Verse bundle at
  `~/.local/share/verse-os/current`, including both gaming tools and the
  optional openagents CLI. The installer preserves that CLI when a checkout
  has not built a replacement. No system rebuild or default-session change.
- Final integrated trial: the installed verse-session ran its actual Vulkan
  Grid inside another private headless compositor, with its own inner desk.
  Plain `verse-battlenet` started from that desk, Play launched Classic Era,
  and the installed CLI entered Coldridge Valley, held W for 100 ms, dragged
  the camera 40 pixels and stopped. The outer capture shows the in-world
  game docked beside the Grid; before/after captures are
  `/tmp/verse-wow-outer/wow-in-grid.png` and `wow-in-grid-after.png`.
  Audio stayed on a temporary null sink throughout.
- That trial exposed host `VK_DRIVER_FILES` and `LD_LIBRARY_PATH` overrides
  leaking from the native Verse into Proton's container. The launcher now
  clears both for its child; the installed fix passed the integrated trial.
  Its updated entry passed 84 Coax checks.

User commands: [Verse gaming](../os/verse-gaming.md).
