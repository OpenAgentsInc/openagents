# Zeron — Animation Audit

> Strictly read-only audit of all animations in `zeronsh/zeron` (local checkout at `a7947b72`, `v0.2.132`).
> All source paths below are relative to the `zeron` repository root (`/Users/christopherdavid/work/zeron`).

## Summary by Surface

| Surface | Stack | Key Animation Systems |
| --- | --- | --- |
| **`crates/orb`** | Rust procedural 3D dot/line engine | 14 procedural orb modes (`Orbits`, `Breath`, `Ripple`, `Web`, `Ribbon`, `Scan`, `Pulse`, `Constellation`, `Weave`, `Beacon`, `Focus`, `Gyroscope`, `Echo`, `Morph`), `AnimationClock`, `AudioResponse` mic/TTS envelope follower, 240 ms smoothstep `OrbAnimator` crossfade |
| **`crates/veil` & `crates/proto`** | Rust shared motion math | Adaptive EMA streaming text fade-in (`ElemVeil`, `RowVeil`), `CaptionVeil` (180 ms exit), shared `ZERON_PULSE_MS` (2400 ms) & `GRADIENT_SPIN_MS` (750 ms) curves |
| **`crates/visual` & `crates/visual-gpu`** | Rust + Luau + `wgpu` | 1–60 fps visual playback clock, 3D turntable spin + orbit + 2-pass bloom, Shadertoy-style WGSL `iTime`/`iFrame` + `BufferA..D` ping-pong pipeline |
| **`crates/ui`** | Rust / GPUI desktop UI | ~60 fps `activity_pulse` clock, 6 cubic-bezier curves & 16 `MotionSpec` presets, interruptible tween controllers, Chromium-style `Glide` smooth scroll, rubber-band overscroll, 3D barrel `roll_text`, critically damped `composer_dock`, transcript shimmer/fold/glides, caret & terminal blink |
| **`apps/ios` & `ZeronLiveActivity`** | Swift / UIKit / SwiftUI / WidgetKit | `CADisplayLink` CoreGraphics `OrbView`, `UIView.animate` spring `DraftHandoff`, cross-dissolve captions/wallpapers, `Timer` call clocks & 20 Hz audio level polling, WidgetKit live timer & `WaveBars` interpolation |
| **`apps/android`** | Kotlin / Jetpack Compose | `withFrameNanos` orb/stage/transcript loops, `Animatable` + `tween` (wallpaper, transcript, drag-dismiss, error shake), `rememberInfiniteTransition` (shimmer, skeleton), `animate*AsState` & `AnimatedVisibility`/`AnimatedContent` across screens |
| **`apps/landing` & `apps/card-web`** | HTML / CSS / JS / WebGL2 | CSS hover transitions, `@keyframes spoke-flow` & `drift`, rAF 15 fps ASCII canvas, scroll quote fill, `glyph-field.js` parallax/afterglow/beam sweep, WebGL2 card spring physics + foil sweep + arrive |

---

## 1. Procedural 3D Orb Engine (`crates/orb`)

| File & Line | Symbol / Type | Parameters & Timings | Trigger / Purpose |
| --- | --- | --- | --- |
| `crates/orb/src/lib.rs:23` | `REDUCED_MOTION_T` | Static freeze frame at `0.6` s | Static snapshot when reduced motion is enabled |
| `crates/orb/src/motion.rs:5-47` | `AnimationClock` | `accumulated: f64`, `anchor`, `rate` | Phase-preserving clock for smooth speed changes without jumps |
| `crates/orb/src/motion.rs:54-105` | `AudioResponse` | User attack `18.0/s`, release `5.0/s`; agent attack `14.0/s`, release `3.5/s`; speed `1.0 + 0.45*user + 0.65*agent` clamped `[1.0, 1.85]` | Modulates orb speed from live mic and TTS audio levels |
| `crates/orb/src/animator.rs:14-267` | `OrbAnimator` / `crossfade` | `TRANSITION_SECS = 0.24` s (240 ms), Hermite `smoothstep`, dot/line `(x,y,z,r,alpha)` lerp | Morphs smoothly between `OrbState` modes when `!reduce_motion` |
| `crates/orb/src/engine/core.rs:92-122` | `make_proj` | `rot_y = t * 0.42 * spin`, `rot_x = 0.38 + sin(t * 0.27 * spin) * 0.22 * wob_mul`, `FOV = 2.6` | Shared 3D perspective camera yaw/pitch/wobble |
| `crates/orb/src/engine/orbits.rs:10-90` | `Orbits` (`Idle`, `speed = 0.35`) | Up to 6 tilted great-circle rings, particle speed `0.55 + 0.18 * ring` | Ambient idle orbital circulation |
| `crates/orb/src/engine/lattice.rs:17-69` | `Scan` (`Routing`, `speed = 1.00`) | Fibonacci sphere, `scan_y = sin(t * 1.35 * scan_mul)`, Gaussian band `band_w = 0.26 * band_mul` | Travelling latitude scan band |
| `crates/orb/src/engine/lattice.rs:73-112` | `Breath` (`Listening`, `speed = 0.70`) | `breath = 0.76 + 0.22*(0.5 + 0.5*sin(1.15t))`, surface harmonic `0.05*sin(3dx - 1.9t)*sin(3dy + 1.4t)` | Calm inhale/exhale listening sphere |
| `crates/orb/src/engine/lattice.rs:114-213` | `Pulse` (`Connecting`, `speed = 0.95`) | Sawtooth `wave_y = 1.25 - frac(0.48t)*2.5`, radial push `1 + flare*0.08`, 48-dot emissive ring | Periodic connection shockwave |
| `crates/orb/src/engine/lattice.rs:217-293` | `Ripple` (`Speaking`, `speed = 1.05/1.00`) | Dual-pole waves `sin(5.5*a1 - 3.4t)` & `sin(4.5*a2 - 2.6t)` | Speaking voice wave interference |
| `crates/orb/src/engine/web.rs:14-149` | `Web` (`Thinking`, `0.85`) & `Constellation` (`ToolUse`, `0.90`) | `Web`: node drift `0.14`, travelling packets `frac(t*speed + phase)`; `Constellation`: drift `0.045`, squared-sine twinkle | Thinking node graph & tool-use starfield |
| `crates/orb/src/engine/braid.rs:11-86` | `Weave` (`Synthesizing`, `speed = 0.90`) | 3-strand plait, `lat = 0.42*sin(3θ - 1.3t + φ) + 0.14*sin(2θ + 0.9t - φ)`, rot `θ + 0.25t` | Synthesizing braided strands |
| `crates/orb/src/engine/ribbon.rs:9-110` | `Ribbon` (`Working`, `speed = 0.95/0.90`) | Half-twist Möbius strip, width breathe `0.72 + 0.28*sin(0.48t)`, multi-wave undulation | Continuous working Möbius flow |
| `crates/orb/src/engine/concepts.rs:18-72` | `Focus` (`Focusing`, `speed = 0.75`) | Iris `openness = 0.5 + 0.5*sin(0.9t)`, `rot = 0.11t`, blade curl, core breathe `sin(1.5t)` | Focusing iris/spiral orb |
| `crates/orb/src/engine/concepts.rs:75-144` | `Gyroscope` (`Reasoning`, `speed = 0.85`) | Gimbal `rotate_y(0.055t)`, per-ring `tilt_x`, thought dots with 2-dot trail | Reasoning gimbal inference loops |
| `crates/orb/src/engine/concepts.rs:147-195` | `Echo` (`Recalling`, `speed = 0.80`) | `phase = fract(0.14t + ring/rings)`, `life = sin(π*phase)^0.58`, counter-rotation `-0.1t` | Recalling outward concentric ripple rings |

---

## 2. Streaming Veil, Shared Motion Math & Visual/GPU (`crates/veil`, `crates/proto`, `crates/visual`, `crates/visual-gpu`)

| File & Line | Symbol / Type | Parameters & Timings | Trigger / Purpose |
| --- | --- | --- | --- |
| `crates/veil/src/lib.rs:34-220` | `ElemVeil`, `RowVeil` | EMA seed `160 ms` (`0.7*ema + 0.3*min(gap, 1000 ms)`), duration `clamp(3*ema, 120..400 ms)`, backlog speedup `1 + 0.3*max(0, active-2)`, curve `1 - (1-p)^1.6` | Adaptive per-chunk opacity fade-in for streaming markdown/text |
| `crates/veil/src/caption.rs:8-118` | `CaptionVeil` | Entry via `ElemVeil`; exit `CAPTION_EXIT_MS = 180 ms` (`1 - veil_opacity(p)`) | Live voice/transcript rolling tail captions |
| `crates/proto/src/motion.rs:13-120` | Shared loader curves | `ZERON_PULSE_MS = 2400` (`0.5 - 0.5*cos(2π*phase)`), `GRADIENT_SPIN_MS = 750` (trail floor `0.12`), 2×3 `MINI_RING`, `staggered_phase` | Cross-platform desktop/mobile loader timing |
| `crates/visual/src/prelude.luau:5-27` | `ui.lerp`, `ui.smoothstep`, `ui.mix` | Scalar & color interpolation helpers | Agent Luau visual scripting |
| `crates/visual/src/lib.rs:101-130`, `node.rs:880-935` | Visual playback clock | `fps` clamped `1..60` (default `30` when animated, `0` static), `playback` (`loop` / `once` over `duration` s) | Drives `show_visual` canvases, 3D scenes, and shaders |
| `crates/visual/src/scene.rs:20-95`, `visual-gpu/src/scene.rs:1500-1590` | 3D scene turntable & bloom | Continuous yaw `spin` (deg/s), pointer drag orbit, 2-pass GPU Gaussian bloom | Animated 3D `ui.scene` nodes |
| `crates/visual/src/shader.rs:1-185`, `visual-gpu/src/shader.rs:80-145,240-395` | GPU Shadertoy pipeline | `DEFAULT_SHADER_FPS = 30`, WGSL `iTime`/`iTimeDelta`/`iFrame`/`iMouse`/`iResolution`, ping-pong `BufferA..D` + half-res bloom | Live `ui.shader` passes |

> Note: `crates/theme` contains only static color/surface tokens and static alpha compositing (no animations or timers).

---

## 3. Desktop UI (`crates/ui` — GPUI)

### 3.1 Core Motion, Scroll & Text Engines

- **`crates/ui/src/motion.rs:60-285` & `motion/windows_pulse.rs:1-75` (`AnimationClock`, `activity_pulse`)**: Shared ~60 fps (`16 ms`, high-res waitable timer on Windows) pulse clock with `250 ms` frame lease, `speed_scale`, and `set_reduced_motion`.
- **`crates/ui/src/motion.rs:330-585` (Cubic-bezier solver & `MotionSpec` catalog)**:
  - **Curves**: `EASE (0.25, 0.1, 0.25, 1)`, `EASE_OUT_CUBIC (0.215, 0.61, 0.355, 1)`, `EASE_OUT_QUINT (0.22, 1, 0.36, 1)`, `EASE_OUT_EXPO (0.16, 1, 0.3, 1)`, `EASE_SNAPPY_OUT (0.2, 0, 0, 1)`, `SPRING_BOUNCE (0.34, 1.56, 0.64, 1)`.
  - **Presets**: `FADE_IN` (500 ms expo + 4 px rise), `FADE_OUT` (140 ms ease), `HOVER`/`SUBMENU` (150 ms), `COLLAPSE` (200 ms), `DIALOG_IN` (160 ms expo, scale `0.97→1`), `MENU_IN` (120 ms quint, 4 px slide), `MENU_OUT` (90 ms ease), `POPOVER_IN` (150 ms quint), `RESIZE` (280 ms snappy), `RESIZE_BOUNCE` (260 ms, 5 px C1 parabola), `SELECTION_SLIDE` (220 ms quint), `TRANSCRIPT_SETTLE` (160 ms quint), `TRANSCRIPT_GLIDE` (500 ms ease-in-out), `GLYPH_STATE` (280 ms quint), `WALLPAPER_CROSSFADE` (320 ms cubic), `ROW_REVEAL` (700 ms, `1 - 0.35*(1-t)*sin(πt)`).
- **`crates/ui/src/motion.rs:730-1120`**: Interruptible tween controllers — `CollapseHeight` (200 ms), `SelectionSlide` (220 ms y/height retarget), `HoverFade`/`HoverFades` (150 ms reversible hover lerp).
- **`crates/ui/src/smooth_scroll.rs:55-485` (`Glide`)**: Chromium `ScrollAnimator` impulse bezier `(0.25, 0, 0.5, 1)`, `40 px/line`, `150..350 ms` by √delta, velocity-preserving retarget.
- **`crates/ui/src/overscroll.rs:45-220`**: Rubber-band overscroll (`0.55` stretch resistance + exponential spring snapback, `~110 ms` tau).
- **`crates/ui/src/edge_fade.rs:1-120`**: Scroll-offset-proportional edge alpha gradient mask.
- **`crates/ui/src/roll_text.rs:1-68,270-360`**: Per-glyph 3D cylindrical barrel roll (`240 ms` + `18 ms/char` stagger, `EASE_OUT_QUINT`) for chip/button label changes.

### 3.2 Component & Surface Animations

- **`crates/ui/src/loaders.rs:71-255`**: Wave loader (`2400 ms`, `0.15 s` stagger, opacity `0.08..1`, scale `0.9..1`), gradient spin (`750 ms`), 2×3 status glyph.
- **`crates/ui/src/pull_request_skeleton.rs:8-40` & `running_pill.rs:1-95`**: `ZERON_PULSE` (`2400 ms`, `0.08` bar stagger, opacity `0.45..0.85`) for PR skeletons and running-subagent dot.
- **`crates/ui/src/orb/orb.rs:1-125`**: GPUI Orb host ticking `orb::OrbAnimator` on `activity_pulse` (~60 fps).
- **`crates/ui/src/markdown/veil.rs:1-110`**: Applies `ElemVeil`/`RowVeil` alpha to streaming markdown runs.
- **`crates/ui/src/new_thread_background_effects.rs:35-165`**: `WALLPAPER_CROSSFADE` (`320 ms` `EASE_OUT_CUBIC`).
- **`crates/ui/src/composer_dock.rs:15-95,220-520` (`Glide`)**: Critically damped spring (`omega = 12/duration`, `420 ms`/`470 ms` staged bounds + opacity handoff) for composer dock choreography.
- **`crates/ui/src/composer.rs:163-172,2335-2350`**: Caret blink (`CARET_BLINK_MS = 500 ms`, solid while typing).
- **`crates/ui/src/terminal/panel.rs:54-58,400-440,1230-1245`**: Terminal cursor blink (`500 ms`, stops after `15 s` idle) + drawer `RESIZE` (`280 ms`).
- **`crates/ui/src/transcript.rs:127-130,2240-2260,2985-3012,6795-6818`**: Tool-group title shimmer (`3400 ms` loop, `2 px` strips, `0.36` half-width), user-bubble fold height tween + `UserCollapseScroll` compensation, and `TRANSCRIPT_GLIDE` (`500 ms`)/`TRANSCRIPT_SETTLE` (`160 ms`).
- **`crates/ui/src/pickers/compact.rs:95-125,870-910`**: Compact picker `RESIZE` (`280 ms`) + `request_animation_frame` phase sweep (`dt <= 50 ms`).
- **`crates/ui/src/popover.rs:67-195,1385-1445`**: Popover entrance/exit (`POPOVER_IN` / `MENU_IN` / `MENU_OUT`).

---

## 4. iOS & Live Activity (`apps/ios`)

> No Metal shaders (`.metal`, `MTKView`, `.shaderEffect`) are used; all orb/motion effects use `CADisplayLink`, CoreGraphics, UIKit, or SwiftUI.

### 4.1 Voice & Orb

- **`apps/ios/Zeron/Voice/OrbView.swift:38-130`**: `CADisplayLink` loop + CoreGraphics `OrbMath.frame`; pauses on `.idle`, window detach, or `isReduceMotionEnabled` (static `t = 0`); lerps `speakingLevel` (`+0.38` attack / `+0.16` release per tick).
- **`apps/ios/Zeron/Voice/VoiceStageViewController.swift:287-291`**: `UIView.animate` (`0.18 s`, `.beginFromCurrentState`) for stage button colors and `0.94` press scale.
- **`apps/ios/Zeron/Voice/VoiceStageViewController.swift:309-316`**: `UIView.transition` (`0.28 s`, `.transitionCrossDissolve`) for live caption updates (instant under Reduce Motion).
- **`apps/ios/Zeron/Voice/VoiceStageViewController.swift:429-435`**: `1.0 s` repeating `Timer` for `MM:SS` call duration.
- **`apps/ios/Zeron/Voice/VoiceControls.swift:28-30,165-169,200-209`**: Mic button highlight (`0.15 s`, `alpha: 0.55`), `1.0 s` `VoiceBar` duration `Timer`, and `BarButton` press scale (`0.12 s`, scale `0.92`, `alpha: 0.78`).
- **`apps/ios/Zeron/Voice/RemoteVoiceController.swift:267-273`**: `0.05 s` (20 Hz) `Timer` polling remote WebRTC audio level to drive `OrbView`.
- **`apps/ios/Zeron/Voice/VoiceViewController.swift:192-196`**: `1.4 s` `Timer` clearing ended call banner state.

### 4.2 Live Activity & Dynamic Island

- **`apps/ios/ZeronLiveActivity/VoiceLiveActivity.swift:52-55,64-66,109-111,196-198`**: WidgetKit 1 Hz `Text(timerInterval:countsDown:false)` call timer.
- **`apps/ios/ZeronLiveActivity/VoiceLiveActivity.swift:125-130`**: SwiftUI `.opacity.combined(with: .scale(scale: 0.8))` + `.easeInOut(duration: 0.2)` mute badge swap.
- **`apps/ios/ZeronLiveActivity/VoiceLiveActivity.swift:141-175`**: SwiftUI `.contentTransition(.interpolate)` + `.easeInOut(duration: 0.25)` morphing 4 `WaveBars` heights across call phases (pushed via `VoiceActivityController.swift:59-83`, throttled to `0.5 s`).

### 4.3 Session & Navigation Transitions

- **`apps/ios/Zeron/Session/DraftHandoff.swift:72-97`**: Spring shared-element flight (`0.42 s`, damping `0.86`, `.curveEaseOut`) with nested fades (`0.18 s` draft out, `0.20 s` delay `0.14 s` bubble in); skipped under Reduce Motion.
- **`apps/ios/Zeron/Session/NewSessionViewController.swift:419-421,603-604`**: Wallpaper `.transitionCrossDissolve` (`0.2 s`) + `UIActivityIndicatorView(.medium)`.
- **`apps/ios/Zeron/Session/SessionViewController.swift:448-451`**: Transcript fade-in (`0.18 s`, `.curveEaseOut`).

---

## 5. Android (`apps/android` — Jetpack Compose)

> No `RuntimeShader`/AGSL/GLSL shaders are used; all animations use Compose `withFrameNanos`, `Animatable`, `rememberInfiniteTransition`, `animate*AsState`, `AnimatedVisibility`, and `AnimatedContent`.

### 5.1 Voice & Orb

- **`apps/android/.../voice/VoiceOrbView.kt:72`**: `withFrameNanos` custom frame loop driving `VoiceOrbView`.
- **`apps/android/.../voice/VoiceStageScreen.kt:248,324`**: `AnimatedContent` (`fadeIn`/`fadeOut`) for ended call controls + `withFrameNanos` stage loop.
- **`apps/android/.../voice/VoiceHost.kt:79`**: `AnimatedVisibility` for voice notice banner.

### 5.2 Transcript & Design

- **`apps/android/.../design/WallpaperHero.kt:46-47`**: `Animatable(0f)` + `tween(120)` wallpaper fade-in.
- **`apps/android/.../transcript/Transcript.kt:122-124,168,174,229,235-236`**: `withFrameNanos` scroll/clock loops, `Animatable` + `tween(durationMs, CubicBezierEasing(0.16, 1, 0.3, 1) / FastOutSlowInEasing)` item motion, and `tween(280)` newly arrived item fade-in.
- **`apps/android/.../transcript/Widgets.kt:147,328,377-378`**: `animateFloatAsState` (`fastEffectsSpec()`) press wash, `AnimatedVisibility` fade, and `rememberInfiniteTransition` shimmer (`tween(3400, LinearEasing)`).

### 5.3 Terminal

- **`apps/android/.../terminal/TerminalView.kt:199,246`**: `postInvalidateOnAnimation()` custom `View` redraws.
- **`apps/android/.../terminal/TerminalScreen.kt:303,320,330,344,462,499`**: `AnimatedVisibility` fades + vertical slide for IME accessory bar, `animateColorAsState` (`fastEffectsSpec()`) key background, `animateFloatAsState` (`0.92f`, `fastSpatialSpec()`) key press scale.

### 5.4 UI Screens & Components

- **`SessionScreen.kt:261,339,362,491,496`**: Header `animateColorAsState`, banner `AnimatedVisibility`, and error shake (`Animatable` + `tween(50)` over `[-8, 8, -5, 5, -2, 0]`).
- **`ModelPicker.kt:183,309,318,319,362-363`**: Pager `animateDpAsState` (`defaultSpatialSpec()`), plate color, press scale (`0.985f`), selection check, and `rememberInfiniteTransition` skeleton pulse (`tween(900, delayMillis = seed * 120)`, `RepeatMode.Reverse`).
- **`SubagentScreen.kt:131,159,161`**: Header color, row reveal alpha (`defaultEffectsSpec()`), loading `AnimatedVisibility`.
- **`ComposerParts.kt:105,144,264` & `ComposerSurface.kt:76`**: `AnimatedVisibility` (`fadeIn/Out` + vertical expand/shrink) for thumbnails/card/mentions + `AnimatedContent` surface switch.
- **`Companions.kt:126,135,138` & `TabletShell.kt:141,156,168`**: Horizontal expand/shrink + fade for companion chips and tablet sidebar/detail pane.
- **`ImageViewer.kt:61`**: `Animatable(0f)` vertical drag-to-dismiss.
- **`OnboardingScreen.kt:186,254,271,367`**: Step `AnimatedContent`, background veil alpha (`0.82f`/`0.55f`), color transition, wallpaper controls `AnimatedVisibility`.
- **`GitHubConnect.kt:143`, `QuestionPanel.kt:125`, `NewSessionScreen.kt:251`, `ZeronRoot.kt:81`, `Expressive.kt:87,130,208`**: `AnimatedContent`/`AnimatedVisibility` state fades and `animateColorAsState` token transitions.

---

## 6. Web Surfaces (`apps/landing` & `apps/card-web`)

### 6.1 `apps/landing`

- **`apps/landing/public/index.html`**:
  - **CSS transitions**: `color .15s` (lines `111, 118, 254, 326, 767`), `background .15s` (`144, 323`), `background-color .2s` (`374`), `opacity .2s` (`303, 305`), `border-color .25s` (`676`), `box-shadow .15s ease-out, scale .15s ease-out` (`187`, disabled on `.sponsor-chip` at `192`).
  - **`@keyframes spoke-flow`** (`462, 468, 470`): `stroke-dashoffset: -12px`, `2.2s linear infinite` on `.spoke-line`/`.spoke-accent` (disabled at `592` under reduced motion).
  - **`@keyframes drift`** (`664, 667`): `translate3d(0, -50%, 0) -> none`, `var(--dur, 70s) linear infinite` on `.wall.is-live .wall-col` (disabled at `692` under reduced motion).
  - **rAF 2D ASCII canvas** (`1395, 1442, 1444`): Throttled to ~15 fps (`ms - prev > 66`), `draw(ms * 0.00022)` (static `draw(0)` under reduced motion).
  - **rAF scroll-driven quote fill** (`1544, 1551`): Lerps `shown += (target - shown) * 0.2` to update `--fill` on quote words.
- **`apps/landing/public/glyph-field.js`**:
  - **Scroll parallax** (`222`): rAF `translate3d(0, y, 0)` when `!reduced && o.parallax !== 1`.
  - **Pointer afterglow** (`251, 267`): rAF 2D canvas decaying pointer trail (`LIFE = 650 ms`, `SPACING = 12 px`).
  - **Beam sweep** (`272, 281`): Continuous rAF `beamLoop(t)` calling `applyBeam(t)` when `!reduced`.

### 6.2 `apps/card-web`

- **`apps/card-web/site/card.js:373-440,975,993,1011-1012`**: WebGL2 shader pipeline + rAF `wake()`/`tick(now)` loop driving spring physics (`rotX`, `rotY`, `shift`), `strength` interpolation, foil `sweep` (`120 ms` delay, `900 ms` duration, `inOutSine`, `lerp(-0.4, 1.4)`), `arrive` (`820 ms`), and continuous `depth > 0` layer parallax.
- **`apps/card-web/site/card-page.js:61` & `tools/browser.mjs:29`**: Double `requestAnimationFrame` paint-frame synchronization.

---

## 7. Coder Desktop Port Status / Assessment

Coder Desktop lives in `apps/coder-desktop/` (Swift/AppKit + C Metal host + Coax UI) and builds on the shared Coax UI libraries in `components/` (`components/core/`, `components/paint/`, `components/skin/`, `components/kit/`, `components/app/`, `components/zeron/`). Many of Zeron's core desktop motion primitives and conversation animations are already ported; visual flourishes (shimmers, rolling text, orb, backdrop frost on all overlays) and transition polish remain.

### Status Matrix

| Zeron System | Status | Coder Desktop Implementation (Paths & Symbols) | Notes / Gaps |
| --- | --- | --- | --- |
| **Frame Loop & Reduced Motion** (`crates/zgx-platform`) | **Ported** | `apps/coder-desktop/Window.swift` (`CADisplayLink`, `blinkAt`, `cd_blink`, `cd_env`, `NSWorkspace.shared.accessibilityDisplayShouldReduceMotion`); `components/kit/frame.coax` (`UiFrame.clock`, `UiFrame.dt`, `UiFrame.reduced_motion`) | 120 Hz ProMotion loop; skips draws when idle, throttles non-moving work to 60 Hz, schedules wakeups for caret blink. |
| **Streaming Veil** (`crates/veil`) | **Ported** | `components/core/veil.coax` (`Veil`, `VeilChunk`, `veil_push`, `veil_alpha_at`, `veil_fade` `(1-p)^1.6`, `veil_boost`, 160 ms EMA, 120–400 ms clamp); `apps/coder-desktop/page/transcript.coax` (`veil_push`); `apps/coder-desktop/page/layout/conversation.coax` (`MdVeil`, `app_message_agent_linked_veiled`) | Full cadence-adaptive chunk reveal ported and wired to live agent replies. |
| **Easing Curves, Timelines & Springs** (`crates/scene/src/motion.rs`, `ui/ds/motion.luau`) | **Ported** | `components/core/arwes/motion.coax` (`ArMotion`, `ar_ease` with 31 curves, `ar_motion_to`/`ar_motion_tick`, `ArTimeline`, `ArSwitch`); `components/core/spring.coax` (`UiSpring`, `ui_spring_step`); `components/core/keyed.coax` (`KeyedStore`, `keyed_toward`, `keyed_press`, `keyed_spring`, `KEYED_DT_MAX = 0.064`) | Analytic damped spring and 31 easing curves with reduced-motion snap. No standalone CSS `cubic-bezier(x1,y1,x2,y2)` solver (polynomial curves like `AM_OUT_QUAD`, `AM_OUT_QUART`, `zc_out3`, `app_ov_out3` are used instead). |
| **Scroll Physics, Overscroll & Rail** (`ui/ds/scroll.luau`) | **Ported** | `components/core/scroll.coax` (`UiScroll`, `ui_scroll_by`, `ui_scroll_step`, `ui_scroll_rail`); `apps/coder-desktop/page/state.coax` (`scr`, `side_scr`, `pane_scr`) | Quartic ease-out momentum coast, rubber-band overscroll + rebound spring, 1.4 s overlay rail linger + 0.26 s fade. |
| **Transcript Glide & Jump-to-Latest** (`ui/ds/transcript.luau`) | **Ported** | `apps/coder-desktop/page/layout/conversation.coax` (`cp_follow`, `CP_GLIDE = 0.38s` `AM_OUT_QUAD`, `CP_AWAY = 0.15s`); `apps/coder-desktop/page/input.coax` (`cp_glide_reset`); `components/app/chat.coax` (`app_jump_pill`, `pill_in = 0.18s` `AM_OUT_QUAD`, `pill_out = 0.12s`, `pill_rise = 8`) | Full hysteresis, smooth glide on send/jump, and rising/fading floating pill. |
| **Composer Pill Morph** (`ui/ds/composer.luau`) | **Ported** | `components/zeron/composer.coax` (`ZcMotion`, `zc_motion_step`, `zc_moving`, 220 ms `zc_out3`); `apps/coder-desktop/page/state.coax` (`pill`); `apps/coder-desktop/page/layout/conversation.coax` | Compact-to-expanded pill height/padding interpolation with hysteresis and reduced-motion snap. |
| **Caret Blink** (`ui/ds/composer.luau`, `editor`) | **Ported** | `components/zeron/composer.coax` (`since`, `caret_blink`); `components/zeron/editor.coax` (`ue_blink`); `components/app/base.coax` (`UiApp.caret_blink = 0.55s`) | Restarts on input/selection change; pauses under reduced motion; host schedules timer wakeups rather than spinning at 120 Hz. |
| **Activity Tree, Loader & Working Status** (`ds/activity.luau`, `tree_line.luau`, `working.luau`) | **Ported** | `components/app/activity.coax` (`app_loader` 8-dash ring, `app_working`, tree trunk/branch `UiSpring` stiffness=220/damping=26 + glow, thought fold `UiSpring` + `prim_fade`); `components/app/chat.coax` (`app_thinking` pulsing dots); `components/kit/animated.coax` (`activity` arc sweep) | Ported and wired in `conversation.coax` and `pane.coax`. |
| **User Message Fold & Edge Fades** (`ds/user_message.luau`) | **Ported** | `components/app/chat.coax` (`app_message_user_fold`, `bubble_ease = 0.26s` `AM_OUT_QUART`); `components/paint/list.coax` (`prim_fade`, `prim_unfade`, `prim_alpha`, `prim_opacity`) | Smooth height expand/collapse with bottom gradient fade (`bubble_fade = 1.5` lines). |
| **Dialogs, Context Menus & Command Palette** (`ds/dialog.luau`, `ds/menu.luau`) | **Ported (Partial Frost)** | `components/app/overlay.coax` (`AppOverlaySpec`, `app_overlay_zeron`, `app_ov_out3` 200 ms + 4 pt rise, `app_ov_in2` 130 ms exit, 120 ms menu fade); `components/app/palette.coax`; `apps/coder-desktop/page/overlay.coax` | Motion timings and soft shadows (`UiShadows` in `components/skin/skin.coax`) match Zeron, but dialogs/menus still paint solid `#1c1c1f` rather than frosted backdrop blur. |
| **Live Message Entrance & Control Press/Switch** (`ui/ds/motion.luau`) | **Ported** | `components/app/base.coax` (`uiframe_entry`, `uiframe_press`, `uiframe_spring`); `apps/coder-desktop/page/skin.coax` (`k.motion.enter = 0.12s`, `k.app.rise = 3.0pt`); `apps/coder-desktop/page/layout/conversation.coax` (`live_ask`, `live_reply`, `live_pane`) | Live-only entrance gating prevents re-animating history on chat switch. Switches use `uiframe_spring`. |
| **Backdrop Blur / Frosted Glass** (`blur.wgsl`) | **Partially Ported** | `components/paint/list.coax` (`PRIM_LAYER`, `prim_backdrop`); `apps/coder-desktop/page/compose.coax` (`CpCompose`, `cp_compose`, `cp_glass`, `CP_BLUR_DEVICE = 31.5`); `apps/coder-desktop/host.c` (`composition.metal`) | Composition pipeline works and is enabled on the models menu (`conversation.coax`), but not yet wired into `app_overlay_paint_panel` (dialogs, context menus, command palette) or floating pills. |
| **Hover / Focus Color Transitions** | **Missing** | `components/kit/frame.coax` (`uiframe_hovered`) | Controls currently snap hover fills/borders instantaneously instead of easing over 100–150 ms. |
| **Shimmer / Skeleton Loaders & Rolling Text** (`roll_text`, shimmer) | **Missing** | *(none)* | No sweeping shimmer gradient for loading states or rolling text transitions for changing status verbs/counts. |
| **Sidebar / Split-Pane / Panel Transitions** | **Missing** | `apps/coder-desktop/page/layout/sidebar.coax`, `pane.coax`, `review.coax`, `filepane.coax` | Sidebars, agent panes, review panes, and file panes toggle instantaneously without width/slide easing. |
| **Toast Notifications & Micro-Interactions** | **Missing** | *(none)* | No animated toast stack, copy-button check morph, tab indicator slide, or error-shake feedback. |
| **Procedural 3D Orb Engine** (`crates/orb`) | **Not Ported** | *(none)* | No 3D orb shader, spring-physics orb states, or audio-reactive ring in Coder Desktop. |
| **Embedded Terminal Animations** | **Not Applicable** | *(none)* | Coder Desktop does not embed Zeron's GPU terminal surface. |

---

## 8. Suggested Porting & Implementation Order for Coder Desktop

### Phase 1 — High-Leverage Polish on Existing Primitives (Low Complexity, High UX Impact)

1. **Wire Backdrop Blur (`prim_backdrop`) into `app_overlay_paint_panel` (`components/app/overlay.coax`)**
   - *Why*: `composition-v1` (`apps/coder-desktop/page/compose.coax` + `composition.metal`) is already live for the models menu. Extending it to dialogs, context menus, and the command palette (`components/app/palette.coax`) with automatic `reduced_transparency` fallback immediately unifies floating surface depth across the app.
2. **Smooth Hover & Focus Transitions (`components/core/keyed.coax`, `components/app/base.coax`)**
   - *Why*: `keyed_toward` already exists. Adding a ~100–120 ms ease on button/row hover washes and focus rings removes visual harshness across the sidebar, conversation actions, and menus with minimal code.
3. **Copy-Confirmation & Inline Action Micro-Interactions (`apps/coder-desktop/page/layout/conversation.coax`)**
   - *Why*: `copied_kind`/`copied_turn` state already exists; crossfading the copy icon to a checkmark (and back) over 120–150 ms gives crisp feedback without adding new architecture.

### Phase 2 — Structural Layout & Feedback Transitions (Medium Complexity, High UX Impact)

4. **Animated Sidebar & Split-Pane Open/Close (`apps/coder-desktop/page/layout/sidebar.coax`, `pane.coax`, `filepane.coax`, `review.coax`)**
   - *Why*: Driving panel width or slide offset with `UiSpring` (or 180–220 ms `AM_OUT_QUART`) makes opening agent sub-panes, file diffs, and the sidebar feel spatial and prevents jarring content jumps. Respect `reduced_motion` via instant snap.
5. **Shimmer / Skeleton Loading Primitive (`components/paint/`, `components/app/`)**
   - *Why*: Useful for initial chat/thread hydration, Foreman HUD loading, and file/diff fetches. Can be implemented as a lightweight clock-driven linear sweep band (static muted fill under `reduced_motion`) without heavy shaders.
6. **Rolling Status Text (`roll_text` equivalent for `app_working` / Foreman status)**
   - *Why*: When an agent transitions between phases (e.g., *Thinking → Reading → Editing → Testing*), a 160–200 ms vertical clip+fade roll makes live activity readable at a glance without layout jitter.

### Phase 3 — Notification & Secondary Polish (Medium Complexity, Medium UX Impact)

7. **Toast Notification Stack (`components/app/`)**
   - *Why*: Non-blocking feedback for background events (git worktree actions, daemon reconnects, update ready, clipboard copy) using the existing `AppOverlaySpec` slide+fade timings (`200 ms` `out3` in, `130 ms` `in2` out).
8. **Active Tab / Selection Indicator Glide (`components/app/pane.coax`, `settings.coax`)**
   - *Why*: Animating the active tab highlight/underline position with `UiSpring` or `AM_OUT_QUART` gives polish to multi-agent panes and settings tabs.
9. **Standalone Cubic-Bezier Evaluator (Optional, `components/core/`)**
   - *Why*: Only needed if future Zeron design tokens require exact CSS `cubic-bezier(...)` parity beyond the 31 analytic curves already in `components/core/arwes/motion.coax`.

### Explicitly Out of Scope (Do *Not* Port to Coder Desktop)

- **Procedural 3D Orb Engine (`crates/orb`)**: Built for Zeron's voice/ambient assistant identity; Coder Desktop is a keyboard-first coding workspace where `app_loader` and `app_working` communicate agent state more cleanly with far lower GPU cost. (Only reconsider if voice mode is added to Coder Desktop.)
- **iOS / Android Native Animations (`apps/ios`, `apps/android`)**: Mobile sheet detents, haptic-coupled drag gestures, and platform SwiftUI/Compose transitions do not apply to the macOS Coax/Metal desktop host.
- **Marketing & Card-Web Animations (`apps/landing`, `apps/card-web`)**: 3D WebGL2 foil tilt cards, ASCII background canvases, pointer afterglows, beam sweeps, and infinite marquee drifts are web marketing effects and would distract in a developer tool.
- **Embedded Terminal Cursor/Bell Animations (`ui/ds/terminal`)**: Not needed unless Coder Desktop embeds a GPU terminal surface.
