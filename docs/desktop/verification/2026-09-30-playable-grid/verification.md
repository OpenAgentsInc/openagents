# Desktop playable Grid verification

Recorded September 30, 2026 for [#10038](https://github.com/OpenAgentsInc/openagents/issues/10038).
The implementation is on `main` in `d95c70d9d4`, after the shared surface
(`42e7220e3f`) and native input/GPU adapter (`33bfc84c3d`) slices.
The final fixture correction (`ffce137252`) serves committed RESULTS data over
loopback HTTP and asserts that the shared board loads it successfully. Desktop
and adapter checks also pass after rebasing on the foreground texture-refresh
fix (`2c83ae4daf`).

## Scope and isolation

Play uses `coder_mobile::verse_surface::GridSurface` over the phone's Rust
scene. Watch remains `verse::spectator::Overlook`. The mobile C ABI is
unchanged. No resident-host behavior changed, so no host deployment is needed.
The `openagents-mobile` workspace has no file or manifest change.

Checks use temporary homes, injected protected-store functions, offline app
fixtures, and local relay fixtures. They create no owner chats or tasks, read
no owner keychain item, register no agent, and pair no real device. The GPU
fixture uses an offline player and a spectator pointed at an unused loopback
port; it reads the committed published-results corpus through a bounded,
cancellable loopback HTTP mirror with a temporary cache.

The wrapper's explicit loopback fixture directs its XP readers to that same
relay. It does not silently reach the public relay while testing. Actual Play
uses the configured/public presence relay and the existing public XP reader.

## Passing checks

| Check | Result |
| --- | --- |
| Pinned toolchain formatting | Pass for desktop, mobile wrapper, native adapter, and Verse |
| Desktop library | 91 passed |
| Desktop binary | 49 passed; 6 opt-in fixtures ignored in the ordinary run |
| Rust Native desktop adapter | 57 passed, including viewport clips, shader validation, and composition uniforms |
| Shared Grid surface | 4 passed, including a desktop and mobile-equivalent client, body reports, event budget, distinct identities, and signed own-pose re-entry |
| Verse presence, spectator, and Gym relay integration | 4 passed |
| Verse library, excluding the recorded existing palette failure | 323 passed; 1 ignored; 1 excluded |
| Clippy for desktop, mobile, and adapter, all targets | Pass with warnings denied |
| Offline GPU fixture | Pass; native views plus the actual desktop world layer at both sizes and scales |
| Linux desktop executable | Builds successfully |

The broader mobile run passed 106 tests, ignored 2, and failed its existing
terminal paste test. That test failed again in an isolated retry. The initial
Verse library run passed 323 tests and failed the existing amber-palette
assertion. See [separate failures](unrelated-failures.md); neither failing
implementation was changed by this issue.

The native input fixture covers key/button ownership, relative look, Escape,
focus loss, hidden windows, capture failure, a chat modal, navigation, and
preserving an unsent chat draft. Chord fixtures exercise diagonal
normalization, backpedaling, sprint, simultaneous buttons, key releases,
jump repeat suppression, and drag-versus-click classification. Shared scene
checks cover first-person entry/exit, all three boards' proximity admission,
movement pause, and inactive/unadmitted launch refusal. Existing Verse tests
cover shared collision, ball/block/reset behavior, stale presence, sparse
interpolation, and public-relay publication bounds.

## Reproduction

Run from the repository root with the pinned toolchain. Give another checkout
its own target directory.

```sh
export CARGO_TARGET_DIR="$PWD/target"
cargo fmt -p openagents-desktop -p coder-mobile -p rust-native-desktop -p verse -- --check
cargo clippy -p openagents-desktop -p coder-mobile -p rust-native-desktop --all-targets -- -D warnings
cargo test -p openagents-desktop -p rust-native-desktop --lib --bins
cargo test -p coder-mobile verse_surface --lib
cargo test -p verse --no-default-features --features xp-host --test spectator --test presence --test gym_hall
cargo test -p verse --no-default-features --features xp-host --lib -- --skip palette::tests::full_amber_is_the_terminal_amber
OPENAGENTS_GRID_EVIDENCE="$PWD/docs/desktop/verification/2026-09-30-playable-grid" cargo test -p openagents-desktop playable_grid_gpu --bin openagents-desktop -- --ignored --nocapture
cargo build -p openagents-desktop --bin openagents-desktop
```

The explicit skip records an unrelated failure; it is not a claim that the
unfiltered Verse suite passes. No full workspace release gate was required.

## Captures and timing

The GPU fixture renders the same `grid::Layer` used in the desktop window,
using the window adapter's GPU target format. It captures native Rust views
with `capture_views` and composites them into the same logical viewport.
This is an offline GPU/layout capture, not an installed native-window test.
Readback happens only in the fixture; Play never reads GPU frames to the CPU.

| Mode | Default, 1200×840 points | Minimum, 760×540 points |
| --- | --- | --- |
| Watch | [1×](watch-1200x840-1x.png), [2×](watch-1200x840-2x.png) | [1×](watch-760x540-1x.png), [2×](watch-760x540-2x.png) |
| Play | [1×](play-1200x840-1x.png), [2×](play-1200x840-2x.png) | [1×](play-760x540-1x.png), [2×](play-760x540-2x.png) |
| GYM | [1×](gym-1200x840-1x.png), [2×](gym-1200x840-2x.png) | [1×](gym-760x540-1x.png), [2×](gym-760x540-2x.png) |
| RESULTS | [1×](results-1200x840-1x.png), [2×](results-1200x840-2x.png) | [1×](results-760x540-1x.png), [2×](results-760x540-2x.png) |
| EVALS | [1×](evals-1200x840-1x.png), [2×](evals-1200x840-2x.png) | [1×](evals-760x540-1x.png), [2×](evals-760x540-2x.png) |

The labeled offline preview starts near the Gym. Its EVALS board states that
it is offline; it invents no published result. GYM uses the existing explicit
preview rows, and RESULTS verifies the committed publication. The fixture
walks between boards using native input and tests switching back to Watch.

[timings.json](timings.json) records 90 moving frames on an NVIDIA GeForce
RTX 4080 in a debug build. The measurement includes creation of a test target,
GPU submission, and waiting for completion at 900×600 pixels. It records one
native input dispatch separately. It does not measure an installed window's
swapchain latency, release performance, or another device's sustained frame
rate. Play requests 60 Hz; these measurements describe this fixture only.

## Installed-device checks

[NEEDS_OWNER.md](../../../../NEEDS_OWNER.md#desktop-playable-grid-10038)
contains macOS and Linux Wayland/X11 cursor grabs, native protected-store
prompts, the signed Mac bundle, and desktop/phone checks. A failed protected
store leaves local Play available offline, and a failed grab releases the
pointer with a visible keyboard fallback. Windows Play remains unavailable
until the Verse portability work in #10027. These owner checks do not hold
#10038 open; a defect found there gets a new issue.
