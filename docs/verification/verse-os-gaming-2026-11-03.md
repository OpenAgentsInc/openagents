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
