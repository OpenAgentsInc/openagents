# Wayland input SDK projection

The v1 Wayland window now reads the compositor's actual XKB keymap and
serialized state. It no longer relies on the old US evdev translation or
drops every modifier bit. The separate Coax `wayland-input.coax` owns the
stable named-key table, Unicode scalar admission, modifier projection,
text routing and scroll-unit decision. XKB/Wayland execute platform effects.
The appended `VkwTo` callbacks are `realm_key(user,key,mods,down)`,
`text(user,bytes,length)` and `wants_text(user)`; the existing physical `.key`
callback remains available for desktop shortcuts. Old consumers without the
new callbacks retain their existing route. Required callbacks without the
compiled SDK adapter refuse startup explicitly.

A complete keymap must have a measured descriptor containing one
terminal-NUL string, at most 1 MiB, before SDK mapping/parsing. XKB resolves
the actual layout. The client consumes compositor modifier masks through
`xkb_state_update_mask`, separately from physical event projection; it never
mixes the server's `update_key` model into that client state. The authority
includes a physical modifier event itself before the following compositor
mask update, including another held side or latched/locked modifier.
`xkb_keysym_to_utf32` preserves browser-style character identity under
Control; `xkb_state_key_get_utf32` would turn Ctrl+A into a control byte.
These SDK roles follow the [XKB state documentation](https://xkbcommon.org/doc/current/group__state.html).

The actual production listener fixture builds real XKB context/keymap/state,
passes a real descriptor through `vkw_keymap`, invokes the same callbacks as
Wayland dispatch and creates real client pointer proxies. It never opens a
window, starts a GPU or takes keyboard/pointer focus. Its authored
[normal](wayland-input/normal/observations.json) and
[sanitized](wayland-input/asan/observations.json) receipts require **all 53
rows/212 fields**, including:

- Plain and shifted Unicode, Ctrl/Alt/Meta together, all 15 named keys and
  the modifier key's own press/release bits.
- German layout physical Y becomes `z`, from the actual two-layout keymap.
- Active text gets one UTF-8 `a`; local dead-acute composition gets one
  two-byte `é`, with no intermediate/duplicate printable key-down.
- A key releases its delivered identity after a modifier snapshot changes;
  focus loss resets the SDK's delivered-key owner.
- Finger travel 1.25 pixels stays 1.25, two wheel notches become 200 pixels,
  and value120=30 becomes 25 pixels without also applying its discrete/axis
  values. Horizontal travel does not create a vertical offer.
- A pressed pointer survives leave, then forwards outside motion and the
  actual release. Final XKB context/map/state owners are absent.

The [actual emitted keymap](wayland-input/normal/keymap.txt) is retained.
Pointer input uses Wayland's implicit grab. Scroll facts coalesce until the
protocol frame; high-resolution facts take precedence over discrete notches,
and finger/continuous sources keep pixel units. The old-protocol fallback
retains ten CSS pixels per wheel axis unit. This follows the platform event
units and ordering in the [official Wayland protocol](https://wayland.freedesktop.org/docs/html/apa.html).

The Coax [policy gate](wayland-input/mutants.log) kills **201/201** root
mutants; the independent exact-word [observer](wayland-input/observer-mutants.log)
kills **219/219**, with zero survivors or equivalent exclusions. Boundary
survivors in the initial policy iteration exposed the missing Unicode=1
case and a redundant modifier upper-bound guard. The final tests cover that
scalar and enumerate the four permitted text-modifier combinations directly.
No product threshold was relaxed to pass mutation.

The one-command Coax driver normally emits the small companion, generates
protocol bindings using the installed SDK, builds the fixture, executes the
real listeners and checks the source fence. The actual final source/output
joins are retained for [normal](wayland-input/normal/sources.json) and
[ASan/UBSan/LSan](wayland-input/asan/sources.json). Both pass, with empty
listener diagnostics and all ownership fields checked. This is not a
whole-realm emission or full window integration result.

Linux tools use the genuinely qualified 0.50 compiler image at
`~/.cache/coax-release/8012c3c9473f51f98665796608d9194348a67a97904e07f652a72d53b12fa529/coax`.
Its original checked compiler-input closure remains the scope of compiler
admission; current component inputs and generated outputs have their own
recorded hashes. Mac policy/observer tests use the admitted b587 image from
`browser-clip-audio.md`. The final SDK versions are in
[the run log](wayland-input/final-asan.log), and every subprocess argv,
status, stdout and stderr is retained under the two run directories.

The existing realm shipping and Nix package hooks now emit/copy the Coax
companion and SDK header, set `AETHYR_WAYLAND_INPUT_V1` and link
`libxkbcommon` (`pkg-config` name `xkbcommon`). The shell syntax and Nix parse
checks pass; this record does not claim a whole Nix image build. Ordinary
`vk-realm` callback wiring, real producer/room/audio behavior, external
input-method protocol and clipboard services remain separate integration
work under #12869. Local XKB compose is not an external IME or paste claim.
The host contract is not weakened, and native default remains gated on
complete three-host acceptance.

On Linux, rerun only a changed slice from an owned worktree with the
qualified compiler selected explicitly:

```sh
nice -n 15 nix-shell -p gcc pkg-config wayland wayland-scanner \
  wayland-protocols vulkan-headers vulkan-loader libxkbcommon --run \
  'COAX="$QUALIFIED_COAX" COAX_JOBS=1 COAX_CHECK_JOBS=2 \
   coax/web/tool.sh aethyr/host/check-wayland-input.coax "$PWD" "$NEW_OUT"'
# Append --sanitize for the independent instrumented SDK build/run.
```

Each compiler/SDK/run subprocess has a bounded deadline, and the complete
component run takes seconds. The original manually exercised prototype is
not relabelled as the source-fenced final driver output. Tools-check passes
after staging all new files: 1,804 tracked tools, zero without a reason;
its [receipt](wayland-input/tools-check.log) is retained with this record.
