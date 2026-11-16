# Compiled GPU app qualification, 2026-11-16

Issue [#13645](https://github.com/OpenAgentsInc/openagents/issues/13645) remains
open. This record covers the shared compiled page and desktop SDK foundations,
not completion of the desktop replacement, all backend effects or physical
accessibility ceremonies.

## Linux page on Vulkan

The actual iOS/shared page and app core compile through
`coax/native/linux-build.coax`, with the same component atlas and shader
source as the Apple app. The qualification entry is
`apps/openagents-desktop/native/gpu/tests/linux.coax`; its controlled outbox
answers contain no account, credential store, network login or paired agent.
The C entry performs SDK calls only, through committed row IDs.

The real Vulkan pipeline rendered Settings and login, activated their rows,
accepted a committed email value and detached focus on blur. It settled the
actual drawer animation before pressing the page revealed underneath.
`UiApp.busy` now exposes that existing motion fact to the SDK driver.

Both ordinary and fully instrumented C/Coax builds pass, with
`-fsanitize=address,undefined`, `ASAN_OPTIONS=detect_leaks=1` and
`UBSAN_OPTIONS=halt_on_error=1`. The renderer here is Mesa Lavapipe, a software
Vulkan implementation; this is not a physical GPU performance measurement.
The SDK does not yet qualify Wayland IME, focus traversal or live account
and agent services in the compiled Linux application.

![Actual shared login on Linux Vulkan](linux-login.png)

Normal and instrumented PNGs are byte-identical, SHA-256
`75eb724ca0afe0e4faf8e2e03142e247b133dbb886e371a9f96b66331a43998a`.
The picture contains only the disposable `example.test` fixture address.

The build is a Coax tool; the C compiler and Wayland/Vulkan/ATK headers and
link libraries come from the existing native SDK environment. Generated C
and headers are written directly by the compiler, preserving crypto table
lines larger than the process observer's line limit. Compiler/linker flags
use the shared, checked `nc_flags` argv reader, with no shell interpretation.

## Accessibility and Apple SDKs

### iOS 1400: native sidebar activation

The floating control was a bare `UIControl` wired to
`primaryActionTriggered`; a finger tap did not produce that action. It is
now a `UIButton`, retaining the same glass appearance. The open drawer also
records the visible control in its live semantic table, so the accessibility
bridge retains the actual button while the drawer is open. Login opened from
Settings now obtains Back's frame from its committed row even when the
underlying Settings page has no sidebar control.

The new Coax Back regression fails on the previous implementation. All 907
app checks pass after the fix. An actual XCUITest on an isolated iPhone 17 Pro
simulator taps the native menu, opens/closes it repeatedly, visits Settings,
opens login, taps Back and returns home. It passes with no failures; this
tests the UIKit event path rather than calling `oc_activate` directly.

![Sidebar opened by a real UIKit tap](sidebar-1400.png)

Screenshot SHA-256:
`d493169869348d0bba4a00ab36ba8853917bb97e13d572265e5b53f3c1dc227c`.
The isolated app has no account or paired computers. Archive, package gate
and export pass; upload accepted with delivery
`13f947a0-90bb-4ad9-9a0a-74eb32fa818c`. The dSYM is retained as
`a65283d9d69a321586bfaf7cafba90dc`. Apple processing is separate.

The Linux [ATK/AT-SPI record](../../../apps/shared/host/linux/ACCESSIBILITY.md)
covers retained roles/states/text/actions, atomic refusal, structural and
state notifications, retirement and a separate actual D-Bus reader. Its
normal and sanitizer SDK tests pass. The actual shared Vulkan host with
`UI_ATSPI` compiles against generated Wayland input/text and real SDK headers.

The hidden Mac target passes Settings/login, native text insertion, held and
committed IME input, one native editor in accessibility and blur. Its package
links neither JavaScriptCore nor WebKit and contains no JS/Wasm/HTML or old
native projection. Ten standalone Apple SDK tests cover identity custody,
file storage and bounded resident-service carriage. A read-only probe of the
installed Mac service authenticated and decoded DescribeEnvironment and
16 session summaries; no turn was started.

The account/device drawer checks pass in debug and release, including
ASan/UBSan. Backend nesting checks: GPU 40, surface 29, frame 62 and app
gallery 288 passed. The independent native SDK utility has nine passing checks.
Passing unchanged gates were not repeated for this record.

The older generic gallery has two hit-count assertions that fail unchanged
on earlier commit `5ba21554b7`; those assertions were retained. The compiled
OpenAgents qualification uses its actual page rather than that older gallery.

Real iPhone and VoiceOver/Orca/passkey ceremonies remain in `NEEDS_OWNER.md`;
the native SDK and software Vulkan observations do not certify those exits.
