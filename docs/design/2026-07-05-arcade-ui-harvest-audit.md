# Arcade UI/Animation Harvest Audit — for Khala Mobile

**Date:** 2026-07-05
**Scope:** `/Users/christopherdavid/work/projects/repos/arcade` (read-only reference clone) → `clients/khala-mobile` in this monorepo.

## 1. What Arcade is, and why it's worth mining

Arcade (`thearcadeapp`, ArcadeLabsInc) was the founder's previous company's Nostr/AI-chat mobile app. It is an **Ignite-boilerplate, bare React Native 0.71 app** (RN CLI, not Expo-managed — Expo SDK ~48 packages are autolinked piecemeal, but there's no `expo start`/EAS in the loop) with Expo SDK version so old it predates Expo Router entirely. Despite the dated stack, the actual design language people remember — the cyan-glow sci-fi "Arwes" HUD chrome, the corner-unfolding frames, the swipe-to-reply donut, the blurred long-press context menu — is genuinely excellent and maps almost perfectly onto what we already want for a StarCraft/Protoss-blue Khala aesthetic (see `docs/design/starcraft.md`: "ancient, psionic, ceremonial... gold, bronze, deep blue, cyan glow, engraved bevels... luminous seams... buttons should feel like glyphic controls"). Arcade already built the "glyphic control, powered by light" feel Protoss calls for — just in the wrong blue (`cyan400 #22d3ee`, one-off `tint #5BC6E0`) instead of ours (`accent #4fd0ff`).

The catch: **Arcade's actual rendering engine for all the good stuff is `@shopify/react-native-skia` + Reanimated 3 worklets, not CSS/NativeWind and not `react-native-svg`.** Nearly every polished visual (the Arwes button, the frame, the donut, the blur popup, the activity indicator, the splash screen, the background gradient) is drawn on a Skia `<Canvas>`, not built from styled `View`s. Khala is Expo SDK 57 + NativeWind 4 + Tailwind v3 + Reanimated 4.5.1 (already installed, currently **completely unused** — confirmed zero references to `Animated`/`Reanimated`/`useSharedValue` anywhere in `clients/khala-mobile`) + Gesture Handler 3.0.2 (also installed, also currently only exercised passively by `@react-navigation/drawer`'s stock behavior). So porting is real engineering, not copy-paste:

- **Reanimated-only patterns** (swipe gestures, toggle knobs, fade/stagger, interpolated color/transform morphs, blur-popup choreography) port almost directly — Khala already has the exact same Reanimated 3/4-generation API surface installed and unused.
- **Skia-drawn patterns** (Arwes frame corners, donut, activity indicator, background gradient blur) need **`@shopify/react-native-skia` added as a new dependency** (it has an Expo config plugin and works fine in the Expo Dev Client / EAS Build model Khala already uses for its own native modules — see `modules/khala-push-to-talk-stt/app.plugin.js` for precedent on how this repo already wires local native config plugins). This is a "medium" lift per component, not a blocker, but it's the single biggest technical fact to plan around: **the Arwes look is Skia-drawn, not SVG-drawn.**
- A few things (the `AnimatedTyping` character-by-character typewriter, the static-GIF typing indicator, `setTimeout`-driven splash blur) are explicitly **worse** patterns than what Reanimated already offers, and should be reimplemented rather than ported (see §3 "Do NOT adopt as-is").

Everything below cites exact Arcade source paths so a future engineer can jump straight to it.

---

## 2. Prioritized harvest list

### 2.1 The Arwes Frame (corner-unfold sci-fi chrome) — **the single highest-value harvest**

**What it is / where:** `app/components/Frame/{Frame.tsx,AnimatedRectBorder.tsx,FrameSquare.tsx,Scaler.tsx,index.tsx}`. A Skia-drawn rectangular frame with 4 small stroked corner squares and 4 independent border lines. Toggling a `visible` boolean drives a `withTiming(..., {duration:200})` Reanimated value that's bridged into Skia's own value system via `useSharedValueEffect` (`progress.current = rProgress.value`), and each corner square/border line is wrapped in a `Scaler` (a Skia `<Group origin={...} transform={[{scale|scaleX|scaleY: n}]}>` primitive) so the frame visually **unfolds outward from its corners** and each border line **grows out from its own midpoint**, rather than the whole thing fading or clipping in. There's also a `highlighted` glow state (fed from `ArwesButton`, see 2.2) that pushes a background-fill opacity from 0.2→0.7 over 200ms.

**Why it's good:** this is exactly the "engraved bevels... luminous seams... powered by light rather than by screens" quality the StarCraft doc calls for the Protoss treatment, and it's *directional* — the frame visibly assembles itself from its corners rather than a generic fade/scale, which reads as far more "activating alien technology" than a plain opacity transition. The center-out line growth (`AnimatedRectBorder`'s per-edge `Scaler` with `scaleOrigin` at each line's midpoint) is a small, specific detail that's the difference between "looks templated" and "looks bespoke."

**Recommendation for Khala:** Add `@shopify/react-native-skia` (config plugin + prebuild) and port `Frame`/`Scaler`/`AnimatedRectBorder`/`FrameSquare` nearly verbatim, recoloring `color={colors.palette.cyan400}` → `accent` (`#4fd0ff`) and the background fill from `almostBlack` → `surface`/`surfaceRaised`. Target surfaces: wrap the fleet-run/connected-account/worker cards in `app/(drawer)/settings.tsx` (currently plain `rounded-xl border border-border bg-surfaceRaised` `View`s with **zero motion**) so they visually "power on" when the settings screen mounts or when a card's status changes; also strong for a redesigned `AppHeader` framing device, and for the tool-call cards in `TranscriptPartRow` (`src/components/transcript-part-row.tsx`'s `tool` case) to make a live tool invocation feel like it's "activating."
**Effort/risk: medium.** New native dependency (Skia) + non-trivial Skia/Reanimated bridging code, but the bridging idiom (`useSharedValueEffect`) is small and reusable once written once.

### 2.2 ArwesButton (gesture-driven highlight state for Frame)

**What it is / where:** `app/components/ArwesButton/index.tsx`. Not its own visual — it's a `Gesture.Tap()` wrapper (`.onBegin`/`.onTouchesUp`/`.onFinalize`, `.maxDuration(5000)`) that flips a Skia `useValue(false)` boolean, consumed by `Frame`'s `highlighted` prop, so press-in brightens the frame's glow fill and press-out (or release) dims it back down. Demoed in `app/screens/DemoScreen/ArwesButtonScreen.tsx` with uppercase, letter-spaced (`letterSpacing:2, textTransform:"uppercase", fontWeight:"800"`) label text — classic HUD button typography.

**Why it's good:** it decouples "is this pressed" from React re-renders entirely (Skia's own reactive value, mutated directly from a gesture worklet via `runOnJS`), so the glow response is immediate and frame-perfect — no React state round-trip lag on a physical button.

**Recommendation for Khala:** Pair directly with the ported `Frame` (2.1) as Khala's primary CTA/action button treatment — a good fit for the composer's Send/Stop button in `src/components/chat-composer.tsx` (currently instant `bg-accent`/`bg-danger` className swaps with no press feedback at all) and for any future primary "launch"/"confirm" action. Also directly reuse the uppercase-letterspaced label convention for command-style buttons, echoing the StarCraft doc's command-card guidance.
**Effort/risk: small** (once Frame exists) — it's a thin gesture wrapper.

### 2.3 SwipeableItem + AnimatedDonut (swipe-to-reply with progress ring)

**What it is / where:** `app/components/SwipeableItem/index.tsx` + `app/components/AnimatedDonut/index.tsx`. A `Gesture.Pan().activeOffsetX([-10,10])` (10px threshold specifically chosen to not fight FlashList's vertical scroll — explicit comment in source) drags a row via `translateX`, clamped with a hand-rolled worklet `clamp`, normalized to `progress = clamp(|translateX|/scrollableAmount, 0, 1)`. That `progress` is **squared** (`progress.value ** 2`) before being bridged (again via `useSharedValueEffect`) into a Skia `AnimatedDonut` circular-progress ring (drawn via a single Skia `Path.addCircle` with `start`/`end` props controlling the stroke arc) — so the "pull to confirm" ring fills with an eased, accelerating curve rather than linearly. Past 30% swipe progress, a reveal-icon container fades in and parallax-translates at `translateX/10`. On completing a full swipe, the donut springs (`withSpring` with `overshootClamping:true, mass:0.5`) from scale 1→2 while fading out, replaced by the action icon; `useAnimatedReaction` flips a `hasBeenFullySwiped` boundary flag exactly at 0/1. Row snaps back via `withTiming(0)` after release regardless of outcome.

**Why it's good:** this is a fully-realized, iMessage/Telegram-quality swipe-to-reply gesture with real polish details (eased-not-linear fill, parallax reveal, spring pop-and-fade completion) that most apps get only partially right. The `DirectMessageReply.tsx` companion (banner shown after a successful swipe) additionally uses **`ReText`** (§2.4) so the quoted sender/content text updates purely on the UI thread with zero React re-renders, and animates its own open/close via a single `progress` value driving both `height` and `opacity` together.

**Recommendation for Khala:** This is the standout candidate for the thread message list (`app/thread/[threadId].tsx`, which today renders `TranscriptPartRow` items in a plain `FlatList` with no swipe interaction at all) — swipe-to-reply or swipe-to-quote a specific transcript part/message. Reskin the donut ring to `accent` (`#4fd0ff`) over `surfaceMuted`. The donut needs Skia (2.1's dependency); the Pan gesture, clamp worklet, spring-pop, and parallax reveal are pure Reanimated/Gesture-Handler and need no new dependency at all.
**Effort/risk: medium** (gesture + Skia donut + wiring into the existing FlashList/FlatList row), but decomposes cleanly into a Reanimated-only phase and a Skia-donut phase that can ship independently.

### 2.4 ReText (UI-thread-only animated text)

**What it is / where:** `app/components/ReText/index.tsx`. A ~15-line component: `Animated.createAnimatedComponent(TextInput)` with `editable={false}` and `Animated.addWhitelistedNativeProps({text:true})`, fed by `useAnimatedProps(() => ({text: text.value}))` where `text` is a Reanimated shared value. Text updates never touch React's render cycle.

**Why it's good:** it's the standard Reanimated/Redash trick (explicitly credited to `wcandillon`) for any text that needs to update at high frequency without JS-thread cost — token counters, live elapsed-time displays, streaming character counts.

**Recommendation for Khala:** Directly applicable to `TranscriptPartRow`'s `usage` case (token counts, `src/components/transcript-part-row.tsx`) if it's ever driven by a live/streaming counter rather than a static final value, and to any future "streaming LLM text" reveal in the `text`/`reasoning` cases — feed characters into a shared value as they arrive over the sync WebSocket instead of `setState`-ing per token. Trivial, no new dependency (pure Reanimated, already installed).
**Effort/risk: small.**

### 2.5 BlurredPopup (long-press blurred context menu)

**What it is / where:** `app/components/BlurredPopup/{BlurredContext.tsx,BlurredPopupProvider.tsx,TouchablePopupHandler.tsx,index.tsx}`. On long-press (`Gesture.LongPress().minDuration(500)`, measured on the UI thread via Reanimated's `measure()`, with `react-native-haptic-feedback`'s `impactLight` fired at the same time), the provider takes a full-screen Skia screenshot of the current view (`makeImageFromView`), renders it twice into a full-screen Skia canvas under **two layered `<Blur>` filters at different intensities** (a subtle 1/3-strength base blur plus the full blur — this two-layer trick reads noticeably better than a single blur pass), re-renders the long-pressed node at its exact original screen position on top (non-blurred, so it looks like it "pops forward"), and shows a context menu that auto-positions itself based on available screen space (`yAlignment`/`xAlignment` computed from canvas size minus page position). Dismissal is choreographed: the menu fades first, then 200ms later the blur animates back to 0, and only once `useAnimatedReaction` observes blur hitting exactly 0 does it `runOnJS` unmount the whole overlay — i.e. **animate-out-then-unmount**, not unmount-then-hope-the-animation-finishes.

**Why it's good:** it's essentially iOS's native context-menu "Peek & Pop" feel, built entirely in userland, with real production-grade details (two-layer blur depth, haptic confirmation, auto-positioning, correct pointer-events pass-through when idle via `useAnimatedProps` toggling `pointerEvents`, and the animate-out-before-unmount sequencing that avoids the classic "content vanishes before the fade finishes" bug).

**Recommendation for Khala:** This is the right shape for message/transcript-part long-press actions (copy text, copy as markdown, quote/reply, re-run tool call) in `app/thread/[threadId].tsx` — mount the provider once at the app root (Arcade mounts it wrapping the whole `<AppStack/>` in `AppNavigator.tsx`; Khala's equivalent root is `app/_layout.tsx`). Recolor the popup menu chrome to `surfaceRaised`/`border`/`accent`. Needs `@shopify/react-native-skia` (for `makeImageFromView` + `<Blur>`) — if that dependency is added for 2.1/2.3 anyway, this becomes a natural next slot; if Skia is deliberately skipped, this specific pattern could be approximated with `expo-blur`'s `BlurView` (not currently installed either) at lower fidelity (no view-screenshot-and-freeze trick, just a live blur overlay) as a smaller-lift fallback.
**Effort/risk: large** (Skia screenshot capture + two-layer blur + auto-positioning + choreographed dismiss is the most involved single pattern in Arcade), **or medium** if the `expo-blur` fallback approach is accepted instead of the exact Skia technique.

### 2.6 DrawerIconButton (hamburger↔X morph)

**What it is / where:** `app/components/DrawerIconButton.tsx`. Three bars, each animated off one shared `progress: SharedValue<number>` (0=closed, 1=open) using `interpolate`/`interpolateColor` for every property simultaneously: top/bottom bars rotate ±45° (RTL-aware), shrink 18→12px wide, shift `marginBottom`/`marginStart`, and cross-fade color from `colors.text`→`colors.tint`; the middle bar just shrinks and fades to become the X's gap.

**Why it's good:** it's a clean, complete, single-file example of a multi-property `interpolate` morph — genuinely one of the most re-usable files in the whole repo, and directly relevant since Khala's `AppHeader` (`src/components/app-header.tsx`) currently renders its menu/back glyphs as **plain static `Text` characters** (`☰`/`‹`) with zero animation.

**Recommendation for Khala:** Port close to verbatim into a new small component, driven by the drawer's open/close state (Khala's drawer is stock `expo-router/drawer` today — see §3 for the nuance there), recolored `colors.text`→`text` and `colors.tint`→`accent`. Replace `AppHeader`'s static `☰` glyph with this morphing icon.
**Effort/risk: small** — pure Reanimated `interpolate`/`interpolateColor`, no new dependency.

### 2.7 TouchableFeedback (UI-thread press-highlight)

**What it is / where:** `app/components/TouchableFeedback.tsx`. `Gesture.Tap()` flips a shared `active` boolean; `useAnimatedStyle` cross-fades `backgroundColor` between `defaultColor`("transparent") and `highlightColor`("rgba(255,255,255,0.1)") over 100ms — entirely on the UI thread, no React state involved in the visual feedback at all.

**Why it's good:** it's the smallest, most reusable primitive in the whole codebase — a drop-in `TouchableOpacity` replacement with genuinely better perceived responsiveness (100ms UI-thread crossfade vs `active:` NativeWind class swap, which still round-trips through the JS/style-recalc pipeline on every touch).

**Recommendation for Khala:** Use as the base "pressable row" primitive across the app — directly replaces the current `Pressable className="... active:bg-surfaceActive"` idiom used in the thread-list row (`app/(drawer)/index.tsx`) and the settings/fleet cards. Recolor `highlightColor` to a translucent `accent` (e.g. `accent/10`-equivalent) to match the token system's existing opacity-modifier convention (already used in `src/components/shell.tsx`'s `Pill` component, e.g. `border-accent/60 bg-accent/10`).
**Effort/risk: small.**

### 2.8 Toggle / Switch knob animation

**What it is / where:** `app/components/Toggle.tsx`. Three variants (checkbox/radio/switch); the interesting one is the switch knob, animated via `useAnimatedStyle` computing `start: withTiming(on ? "100%":"0%")` and `marginStart: withTiming(on ? -(knobWidth)-offsetRight : offsetLeft)` — i.e. the knob slides between two percentage anchors offset by its own measured width, rather than computing absolute pixel positions.

**Why it's good:** it's a small, correct, reusable "sliding knob between two percentage anchors" technique that avoids hardcoding container widths.

**Recommendation for Khala:** Directly useful if/when Khala adds any settings toggle (currently `app/(drawer)/settings.tsx` has no toggles at all, only read-only status cards) — e.g. a future "auto-approve tool calls" or notification-preference switch. Low priority until such a toggle is actually needed, but cheap to have ready.
**Effort/risk: small.**

### 2.9 BackgroundGradient (breathing glow card)

**What it is / where:** `app/components/BackgroundGradient.tsx`. A Skia `RoundedRect` with a `SweepGradient` (angular/conic, cyan-toned) and a `BlurMask` whose blur radius oscillates via `withRepeat(withTiming(10,{duration:2000}), -1, true)` (infinite yoyo) bridged in via `useSharedValueEffect` — a slow 2-second "breathing" pulse.

**Why it's good:** it's a cheap, very effective "this surface is alive/thinking" ambient animation — exactly what an in-progress AI turn or an active tool call should feel like, and matches the StarCraft doc's guidance that ambient/embient loops (as opposed to snappy <160ms feedback motion) are reserved for factional flavor/"living" surfaces.

**Recommendation for Khala:** Use as the "active turn" background treatment behind the composer's turn-status line in `src/components/chat-composer.tsx` (currently a static `"● turn {status}"` text row with no motion) or behind an in-flight tool-call card in `TranscriptPartRow`. Recolor the `SweepGradient` stops to `accent`/`accentSoft` tones.
**Effort/risk: medium** (needs Skia, but it's a small, self-contained, already-`React.memo`'d component).

### 2.10 Staggered list entrance (`FadeIn.delay(n * index)`)

**What it is / where:** Recurring idiom across `app/navigators/TabBar.tsx` (dead code, see §3, but pattern still valid) and `app/screens/HomeMessagesScreen.tsx`'s `FlashList` `renderItem`: `<Animated.View entering={FadeIn.delay(100 * index).duration(800)}>`. Reanimated's built-in `entering` layout-animation prop, not hand-rolled `useAnimatedStyle`.

**Why it's good:** cheapest possible way to make a list "feel alive" on first paint — each row cascades in 100ms after the previous, with no bespoke animation code required at all (it's a one-line prop).

**Recommendation for Khala:** Apply directly to the thread-list `FlatList` rows in `app/(drawer)/index.tsx` and the fleet/settings cards in `app/(drawer)/settings.tsx` — both currently render with zero entrance motion. Also apply to `TranscriptPartRow` items as new transcript parts stream in, so new tool-call/reasoning/text cards cascade in rather than popping instantly. Trivial: `import { FadeIn } from 'react-native-reanimated'`, wrap row in `Animated.View entering={FadeIn.delay(60 * index).duration(220)}` (tune timing shorter than Arcade's 800ms per the StarCraft doc's "keep most motion under 160ms" guidance — Arcade's own timing is looser than what we'd want).
**Effort/risk: small** — the single best effort-to-impact ratio item in this whole audit.

### 2.11 DirectMessageReply (single-value open/close banner)

**What it is / where:** `app/components/DirectMessageReply.tsx`. One `progress = useDerivedValue(() => withTiming(replyInfo.value ? 1 : 0))` value drives **both** `height: progress.value * height` and `opacity` on the same `useAnimatedStyle` — the banner opens/closes as one coherent motion rather than separately-timed height and fade.

**Why it's good:** small but correct pattern — single source-of-truth animated value driving multiple properties avoids the visual "jank" of independently-timed height/opacity animations drifting out of sync.

**Recommendation for Khala:** Same technique applies to the composer's steer/queue row in `src/components/chat-composer.tsx`, which today just pops in/out of layout with no transition when an active turn + non-empty draft state appears — this is a directly-named gap in the current-app survey and a perfect, cheap slot for this exact single-value height+opacity pattern.
**Effort/risk: small.**

### 2.12 ActivityIndicator (Skia spinner, two-arc rotation)

**What it is / where:** `app/components/ActivityIndicator/{index.tsx,AnimatedArc.tsx}`. A static two-quarter-arc Skia `Path` (two 90° arcs at 0° and 180°, forming a broken ring) that's continuously **rotated as a transform** (not redrawn/stroked) via Skia's own `useTiming`/`useSpring` hooks (not Reanimated). The `"large"` variant layers a second, bigger, counter-rotating arc driven by a looping spring (`mass:1.5, velocity:100, damping:15`) for an organic secondary wobble, plus a soft glow halo via `BlurMask`.

**Why it's good:** far more distinctive than a generic RN `<ActivityIndicator/>` spinner — the counter-rotating dual-arc plus soft glow reads as "psionic energy," not "loading."

**Recommendation for Khala:** Replace the plain `ActivityIndicator` currently used in `src/components/chat-composer.tsx`'s sending state with this. Recolor to `accent`.
**Effort/risk: medium** (needs Skia; otherwise self-contained and low-risk to integrate since it's a drop-in spinner replacement).

---

## 3. Do NOT adopt as-is

- **`AnimatedTyping.tsx`** (`app/components/Faerie/AnimatedTyping.tsx`) — a character-by-character typewriter built on raw `setState` + `setTimeout(fn, 1)`/`setInterval(fn, 250)`, i.e. a per-character React re-render loop on the JS thread. This is a worse version of what Reanimated + `ReText` (2.4) already gives us for free with zero JS-thread cost. It's also dead code in Arcade itself (not wired into any live screen) — do not port the implementation, only the *idea* (character-reveal for streaming LLM text), and build it on `ReText`/shared values instead.
- **The static-GIF typing indicator** (`images.typing` / `typing-message.gif` in `app/components/Faerie/Message.tsx`) — a bought/found GIF asset, not a bespoke animation. If Khala wants an "agent is responding" indicator, build a proper three-dot Reanimated pulse or reuse the `AnimatedDonut`/`ActivityIndicator` (2.12) treatment instead of sourcing another GIF.
- **`SplashScreen.tsx`'s blur ramp** (`app/screens/SplashScreen/SplashScreen.tsx`) — increments a Skia blur value via raw `useState` + `setTimeout(fn, 20)` stepping by 2 up to 300, i.e. React-thread-driven frame-by-frame animation that will jank under any load. The same file's sibling patterns (`Frame.tsx`, `BackgroundGradient.tsx`) already show the correct fix — drive it with `withTiming(300, {duration: 3000})` bridged via `useSharedValueEffect` — so if Khala ever wants a splash-to-app blur-dissolve transition, use that idiom, not this one.
- **`ScreenWithSidebar.tsx`'s `DrawerLayout`** (`app/components/ScreenWithSidebar.tsx`) — uses `react-native-gesture-handler`'s older imperative `DrawerLayout` component API, not the modern declarative `Gesture.*()` builder API used everywhere else in the same codebase. Khala's drawer is already the modern `expo-router/drawer` (React Navigation drawer, itself built on the current Reanimated/Gesture-Handler generation) — there's no reason to introduce this older, inconsistent gesture API just to get the icon-rail layout idea; if an icon-rail nav is wanted, build it as custom drawer content inside the existing `expo-router/drawer` rather than swapping navigators.
- **`app/navigators/TabBar.tsx`** — confirmed dead code (not mounted anywhere in Arcade's live nav tree; the app actually uses React Navigation's default tab bar). The *staggered fade-in idiom it contains* (2.10) is worth harvesting, but the component itself, and its Skia `Frame`-wrapped tab bar chrome, was apparently abandoned mid-build in the original app — don't assume it's production-tested.
- **`app/navigators/HudNavigator.tsx`** — also unreferenced/unmounted, kept only "to show how to transition" per its own code comment. Reference only.
- **Ad-hoc color literals scattered outside the theme** — Arcade's own theme system has real inconsistencies worth learning from, not copying: `colors.tint = "#5BC6E0"` is a bespoke hex not in the `cyan` ramp; `HomeMessagesScreen.tsx`'s `RefreshControl` uses a local one-off `{logo:"#155e75", logoActive:"cyan"}` object instead of the theme; `timing.ts` defines only `{quick:300}` and is barely used, with nearly every animation hardcoding its own duration literal instead. When porting any of the above patterns, route every color through Khala's existing `nativewind-tokens.cjs` (`accent`/`surface*`/`border*`/`text*`) and consider adding a small shared `motion.ts`/`timing.ts` token file up front so Khala doesn't repeat Arcade's "everyone hardcodes their own ms value" drift.
- **Bare `Animated` API usage** — Arcade's Ignite-boilerplate leftovers like `ExpandedButton.tsx` and various demo-screen chrome use plain `TouchableOpacity` with no animation at all; don't mistake DemoScreen-only scaffolding for design-system components worth harvesting.

---

## 4. If we only harvest 3 things first

1. **Staggered list entrance (`entering={FadeIn.delay(n*index)}`, §2.10)** — the cheapest possible change (a one-line Reanimated prop, already-installed dependency, zero new native code) applied to the thread list (`app/(drawer)/index.tsx`), settings/fleet cards (`app/(drawer)/settings.tsx`), and streaming `TranscriptPartRow` items — turns three currently-static, currently-lifeless screens into ones that feel alive on every load, for near-zero engineering cost.
2. **The Arwes Frame + ArwesButton pair (§2.1 + §2.2)** — the actual signature "Protoss glyphic control" look the user loves, recolored to `accent #4fd0ff`; highest ceiling of any single item here, and once the Skia dependency and the `useSharedValueEffect` bridge idiom exist for this, every other Skia-based item (donut, activity indicator, background gradient, blur popup) becomes a much smaller incremental lift.
3. **SwipeableItem + AnimatedDonut swipe-to-reply (§2.3)** — the single most complete, highest-polish interaction pattern in the whole Arcade codebase (eased fill, parallax reveal, spring-pop completion), and it slots directly into the one screen (`app/thread/[threadId].tsx`) that most needs a signature gesture — nothing in Khala today lets you act on an individual transcript part/message at all.
