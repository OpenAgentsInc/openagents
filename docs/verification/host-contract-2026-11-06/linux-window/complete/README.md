# Current ordinary Vulkan window

The actual current Coax 0.52 producer and closed native SDK pass the
[normal window receipt](normal.json) and
[full instrumented window receipt](asan.json), with their immutable input
fence unchanged. ASan, UBSan and leak detection stay enabled. No sanitizer
suppression is installed. A private headless Weston compositor owns the
window; a private current room binary owns the seat. The run does not use
the owner's display, clipboard, speakers or room process.

The 89 authored fields require real input movement, the same room seat,
continued hidden room messages, held producer/GPU/audio clocks while hidden,
resume, a 640×360 swapchain resize, an actual JPEG, and complete GPU, decoded
image, audio-device and audio-context retirement. Vulkan validation must be
observed enabled, with zero errors and warnings. The live media allocation
is exactly 68,208 bytes: seven 48×48 stills and six 11×14 video placeholders.
The separate seven-still codec workload retains its original 64,512-byte
expectation.

`linux-window-session.coax` authors the finite workload. After the initial
Enter, the character picker can arrive asynchronously, so the play step
confirms it and closes chat before holding W. The hidden step waits at least
one second and requires an observed room message within the existing
30-second diagnostic deadline. It cannot turn a quiet interval into a
passing receipt. The [red regression](hidden-wait-red.log) catches the old
unconditional timed transition. The schedule kills
[106/106 root mutants](session-mutants.log); the independent full receipt
authority kills [132/132](observer-mutants.log), without equivalents.

## SDK fixes

The [zero-timeout attempt](zero-acquire.stderr) presents only the initial three images on the
actual Wayland/NVIDIA path; resize permits three more. The
[one-nanosecond attempt](one-nanosecond-acquire.stderr) also fails.
A bounded one-millisecond acquire lets WSI make progress.
The preceding submission fence still refuses before producer advancement;
timeout still offers no producer frame. This fixes #12964. It changes no
simulation delta, submission-fence deadline or render capacity.

The ordinary callbacks use the measured whole-text dispatcher, cancel
composition at focus/visibility boundaries and connect the clipboard SDK.
The text Coax dependency exports stay private in `libhosttext.so`, avoiding
duplicate WebSocket/crypto symbols without a duplicate-symbol waiver.
The independently recorded text/action observations in the parent directory
remain the UTF-8, empty-commit, overflow and deferred-604 qualification for
#12963.

The initial full `-O1` instrumented DSP cannot keep up with the device clock.
A bounded diagnostic interruption and a parent debugger locate the callback
in `ac_block`, with the realm waiting on the audio/device mutex. DSP keeps
production `-O2` with both sanitizers enabled; the other instrumented SDK
and producer units keep their recorded sanitizer flags. This is a test-build
optimization choice, not a claim that a timed-out run passed.

The resulting run exposes [9,742 leaked TLS bytes](tls-red.stderr).
Engine retirement does not join the process-owned socket workers. The
ordinary process now joins them before global TLS teardown, fixing #12976.
The subsequent [96-byte Wayland leak](proxy-red.stderr) identifies the lost
registry proxy. The window retains its registry and optional decoration and
destroys its owned proxies in dependency order before disconnect, fixing
#12977. The final [stderr](asan.stderr) has no sanitizer diagnostic.

## Replay

Build the current producer and small companions separately through the
documented source-checked emission/cache paths. Generate `RenderIdentity.h`
from the actual common render package with `verse/native/render-package.coax`.
Then use a fresh output directory:

```sh
coax/web/tool.sh aethyr/host/build-linux-world.coax \
  "$PWD" RENDER_C SHELL_C NETWORK_C PRODUCER.so OPUSFILE.so NEW_BUILD \
  --sanitize
coax/web/tool.sh aethyr/host/check-linux-window.coax \
  NEW_BUILD FACTS CURRENT_ROOM "$PWD/coax/web/room/dev" NEW_RESULT
```

The build needs the installed Wayland, XKB, PipeWire, Vulkan and Opus SDKs.
Run inside an owned headless compositor with `ASAN_OPTIONS=detect_leaks=1`,
the installed Vulkan validation layer explicitly selected and a private
Wayland socket. The instrumented receipt uses two-worker Lavapipe; the
normal current-package window uses the RTX 4080. These are SDK integration
observations, not an external IME-engine or complete Nix-image qualification.
