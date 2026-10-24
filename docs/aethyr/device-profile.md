# The Aethyr device profile and detail controller

[ADR 0007](../adr/0007-capability-and-detail.md) asks for one device profile
and one detail controller. This page describes how they are built, what the
web renderer does with `Detail`, and the CSS contract for interface knobs.
Issue #12589 tracks the work.

| Piece | Where |
| --- | --- |
| Profile, budget, `Detail`, knobs, controller, report (the source) | `crates/aethyr-profile` (root workspace; host and `wasm32-unknown-unknown`) |
| Web knob table | `crates/aethyr-profile/fixtures/web-knobs.json`, copied to `apps/aethyr/packages/runtime/detail-knobs.json` |
| Shared test vectors | `crates/aethyr-profile/fixtures/vectors.json` |
| TypeScript mirror | `apps/aethyr/packages/runtime/detail.ts` |
| Web probe | `apps/aethyr/packages/runtime/device-profile.ts` |
| Session glue (profile, budget, controller, knobs) | `apps/aethyr/packages/runtime/detail-driver.ts` |
| Renderer switches | `apps/aethyr/packages/renderer/detail-settings.ts`, `Renderer.setDetail` |
| Player wiring | `apps/aethyr/apps/player/main.ts`, `WorldSession.attachDetail` |

## The device profile

`DeviceProfile` (`aethyr.profile.v1`, snake_case JSON) holds:

- `gpu`: the graphics API, adapter vendor, architecture, device and
  description, whether it is a fallback adapter, sorted feature names, every
  limit by its WebGPU name, and a coarse `class` (`none`, `software`,
  `baseline`, `extended`) computed from the limits. The classes are
  provisional until qualification.
- `memory`: `navigator.deviceMemory`, when present.
- `display`: viewport size in CSS pixels, `devicePixelRatio`, a refresh rate
  measured from 20 animation frames, and `(dynamic-range: high)`.
- `input`: the primary pointer (`fine`, `coarse`, `none`), `hover`,
  `any-pointer`, touch points, and whether the Gamepad API exists.
- `power`: Battery Status level and charging, when present. The web has no
  low-power or thermal signal, so those stay unknown there.
- `network`: `navigator.connection` effective type, downlink, round-trip time,
  and Save-Data, when present.
- `prefs`: reduced motion, reduced transparency, more contrast, and text
  scale (root font size over 16 px).

The probe never reads the user agent. Its test hands it a `navigator` that
throws on `userAgent`, `userAgentData`, `platform`, `appVersion`, and
`vendor`. Every asynchronous source (adapter, battery, refresh measurement)
is bounded by a timeout; a missing source leaves its field `null`. The crate
parses profiles with unknown fields refused, and the TypeScript test checks
the probe's output has exactly the keys of `fixtures/profile-web.json`, so a
field added on one side fails the other.

## The budget

`Budget::for_profile` gives 60 Hz frames (or the display's rate when it is
slower), 30 Hz under constrained power (battery below 20% and not charging,
low-power mode, or serious thermal state), GPU time at 80% of the frame, a
quarter of device memory, and a 50 MB download cap on slow or data-saving
networks. These numbers are placeholders until the ADR 0007 qualification
sets a budget per device class.

## The controller

The controller moves one `Detail` value between a floor and a ceiling. It
reads GPU time when the host has timestamps and the frame interval
otherwise. Frames are grouped into windows of 30, after 60 warmup frames.
Each window's 90th percentile is compared with the budget:

| Window | Effect |
| --- | --- |
| Above 110% of budget | Lower by 0.1 (0.2 when above 150%) |
| GPU time at or below 75%, or frame interval at or below 105% | Counts toward a raise; four in a row raise by 0.05 |
| In between | Nothing changes; the run of good windows restarts |

The dead band, the run requirement, and the smaller step up are the
hysteresis. A raise that overloads the frame within two windows doubles the
run needed for the next raise, up to 64 windows, so a workload on the edge
between two levels settles on the lower one; 128 quiet windows halve it
again. Frame intervals longer than 250 ms are pauses (a hidden tab, a
debugger) and restart the warmup. Frame pacing can't show headroom on a
display that waits for vertical sync, which is why a pacing window counts as
good when it simply meets the budget; the backoff keeps those raise probes
rare.

When `Detail` is already at its floor and frames are still over budget, the
controller reports `pressure: "at_floor"`. That is the signal for the shell
to tell the person the device can't hold this zone, as ADR 0007 asks.

`hold` pins `Detail`. The player holds it at 1 for screenshots, and frame
benchmarks hold it at its current value for the whole recording, so
measurements stay comparable. Measurements taken while held are dropped.

## Knobs, floors, and preferences

A `KnobDecl` maps `Detail` to a knob's value: `floor` at 0, `full` at 1,
shaped by a curve (`linear`, `ease_in`, `ease_out`, or `steps` with a
count). No `Detail` value produces a value below the floor.

Preferences are limits, not inputs. Reduced motion holds `motion` and
`ui_effects` at their floor; reduced transparency holds `backdrop_blur` at
its floor. They never move `Detail`. So that a preference and a floor never
disagree, those three knobs must declare a floor of 0; the crate and the
mirror both refuse a table that doesn't.

Text, focus indicators, and hit targets have no knob.

### The web renderer's knobs

At `Detail` 1 every switch has the value the renderer had before this work,
and a test checks that.

| Knob | Floor | Curve | What it changes |
| --- | --- | --- | --- |
| `render_scale` | 0.5 | 5 steps | Drawing-buffer size over the viewport size. Applied by `WorldSession.resize`, so the canvas stretches to fill the page |
| `shadow_quality` | 0 | linear | How far the camera moves (2 m at full, 8 m at the floor) or turns (dot 0.995 to 0.98) before the sun shadow cascades refit |
| `foliage_density` | 0.4 | linear | Grass draw distance over the manifest's `maxDistance`, never below `lodDistance + 5` |
| `crowd_tier_distance` | 0.4 | linear | Battle crowd LOD and draw distances over their configured values |

`aa`, `occlusion`, and `cluster_lod_bias` are not declared yet. Four-sample
anti-aliasing is fixed in every pipeline the renderer creates, so changing it
needs pipeline variants; occlusion and cluster LOD have no runtime switch to
drive.

## The CSS contract for interface knobs

`DetailDriver.applyInterface(element)` writes these custom properties on the
element (the player uses `document.documentElement`). Each is a unitless
number from 0 to 1 with three decimals, so a stylesheet multiplies its own
token by it:

| Property | Knob | Example use |
| --- | --- | --- |
| `--detail` | `Detail` itself | Rarely needed directly |
| `--detail-blur` | `backdrop_blur` | `backdrop-filter: blur(calc(var(--blur-md) * var(--detail-blur, 1)))` |
| `--detail-shadow` | `ui_shadows` | Mix shadow opacity: `color-mix(in oklab, var(--shadow) calc(var(--detail-shadow, 1) * 100%), transparent)` |
| `--detail-motion` | `motion` | `transition-duration: calc(200ms * var(--detail-motion, 1))` |
| `--detail-effects` | `ui_effects` | Opacity or count of decorative layers (glow, particles) |

The element also gets `data-detail-motion="on"` or `"off"` (off when
`--detail-motion` is 0), for animations that can't be scaled, such as
keyframes: `[data-detail-motion="off"] .pulse { animation: none; }`.

Always write a fallback of 1 in `var()`, so a page without a detail driver
looks as it does today. The interface knobs use two steps, so the properties
change at most at `Detail` 0.5 and 1, not on every controller step.
`oa-tokens` and `apps/verse-ui` don't read these properties yet; adopting
them there is follow-up work for whoever owns that package.

## Reports

`DetailReport` (`aethyr.detail-report.v1`) carries the profile, the budget,
the controller state (`detail`, floor, ceiling, source, last window, budget,
pressure, change count, backoff, whether held), and every knob's value and
whether a preference limits it. It is included in:

- frame benchmark results (`WorldSession.benchmark`, the `detail` field);
- `WorldSession.inspect()`, and so the player's `aethyrforgePlayer.status()`;
- the player's failure record: when the world fails to open or a frame
  throws, `status().failure` holds the message and the report from that
  moment.

## Why a TypeScript mirror and not Wasm

The controller is about 150 lines of arithmetic that runs on every frame in
the browser's main thread, next to a renderer written in TypeScript. A Wasm
build would add a second module to load before the first frame, a
JavaScript-to-Wasm call per frame, and a build step for this root-workspace
crate beside the engine's own `kernel.wasm` toolchain, for no change in
behavior. The mirror avoids that, and the shared vectors stop drift:
`fixtures/vectors.json` holds controller runs (fixed frame times and a
closed-loop workload whose cost follows `Detail`), knob values with and
without preferences, CSS strings, and budgets. The crate's test and the
TypeScript test both run every case and compare within 1e-9; both round
`Detail` to six decimals after each step and format CSS values the same way,
so they agree exactly. After a deliberate change, run
`AETHYR_PROFILE_WRITE=1 cargo test -p aethyr-profile`, review the diff, then
update the mirror until `npm test` passes. The Wasm-ready crate stays the
source for native hosts and the mobile app.

## Not done yet

- **The phone measurement.** Issue #12589 asks for a run on a phone browser
  under sustained load, showing the frame budget holds by lowering `Detail`
  without going below a floor. It has not been run; the controller is only
  checked against simulated workloads in both test suites.
- GPU timestamps reach the controller only when the renderer was built with
  its profiling experiment and the adapter has `timestamp-query`; ordinary
  player builds use frame pacing.
- Zones don't declare knobs and floors in their manifests yet, and the shell
  doesn't yet refuse a zone whose floor the device can't meet.
- The budget numbers and GPU classes are provisional until qualification.
