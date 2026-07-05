# QA Swarm for Khala Mobile: Adapting the Standing-Engagement Pattern

Date: 2026-07-05
Status: design + first-pass implementation seed. This document does not flip
a product promise green, publish a price, or claim hosted/self-serve QA Swarm
availability for mobile.

## What QA Swarm Is (per the source material)

Per `docs/transcripts/246.md` (episode 246, "Dogfooding Khala Code"), QA Swarm
is defined as:

> Point a swarm of QA agents at your product and get proof it works. A
> coordinated fleet of autonomous QA agents — scripted scenarios, seeded
> monkeys, LLM explorers, perf probes — drives your app through a real
> browser, a real terminal, and (on macOS) the real native window; every
> discovery distills into a committed, re-runnable e2e test; every run
> produces an honest CONFIRMED/REFUTED verdict, videos, exact accounting, and
> a shareable web URL where the whole swarm is visible as a live, cinematic
> StarCraft-blue board.

Key structural ideas extracted from the transcript, distinct from the
UX-contract registry:

1. **Customer number one is the product itself.** "We are customer number
   one. QA Swarm's standing engagement is Khala Code desktop itself, and its
   first sales artifact is the evidence that engagement produces." For
   mobile, the equivalent is: Khala Mobile is customer number one for a
   mobile QA Swarm engagement.
2. **Coordinated fleet of DIFFERENT kinds of checks**, not one style: scripted
   scenarios (deterministic, human-designed), seeded monkeys (randomized
   fuzz/exploration), LLM explorers (an agent freely poking at the product
   looking for breakage), and perf probes (budget-style latency/resource
   checks) — driving through whatever surface the product actually has (for
   desktop: browser, terminal, native window; for mobile: the actual device
   or emulator screen).
3. **Every discovery becomes a committed, re-runnable regression test** —
   findings are not just filed, they get distilled into permanent coverage so
   the same bug class can never silently return.
4. **Honest verdicts, not vibes**: every run reports CONFIRMED/REFUTED with
   real receipts (videos, traces, exact counts), never a bare "looks good."
5. **A visible, shareable status surface** — for desktop this already exists
   as `/qa/qa-run.khala-code-nightly.latest`
   (`docs/qa/qa-swarm-khala-code-standing-engagement.md`), with a findings
   ledger whose lifecycle is `caught -> filed -> fixed -> distilled`.

The desktop implementation (`docs/qa/khala-code-nightly-matrix.md` and
neighboring `docs/qa/khala-code-*.md` files) is a mature, many-week-deep system
with a nightly matrix, coverage-frontier tracking, perf-budget gates, visual
baselines, and a mechanical corpus. This document does NOT attempt to
replicate all of that depth for mobile in one pass — that would be dishonest
overreach. Instead it adapts the SHAPE (the four check kinds, the findings
ledger states, the standing-engagement framing) to what mobile can actually
support today, and names what a mobile-native nightly matrix would need next.

## What's Genuinely Different About Mobile

The desktop QA Swarm drives a **browser DOM, a terminal, and a native macOS
window** — all things a CI-style headless process can drive directly on the
same machine that built the app. Mobile is structurally harder:

- There is no headless "just run it" path for iOS/Android UI the way there is
  for a DOM (`happy-dom`) or a native macOS accessibility tree. Real
  UI-level driving needs either a simulator/emulator (which needs Xcode/
  Android SDK tooling and is slow to boot) or a physical device.
  Maestro/Detox/XCUITest/Espresso are the standard tools for that layer, and
  none is wired into this repo yet.
  This is exactly why `khala_mobile.platform.launched_app_interaction_smoke.v1`
  in the contract registry stays `pending` — there is no dishonest way to
  claim device-level coverage without the harness to back it. (A narrower,
  real gap in the SAME family — mounting `ChatComposer` itself as a live
  React component tree, as opposed to only its pure intent-builder functions
  — was closed on 2026-07-05 without a device/simulator at all, via a `bun
  test`-only React Native harness; see
  `khala_mobile.composer.rn_component_mount_coverage.v1`, now `enforced`.
  That proves real component STATE/RENDER/EFFECT logic, not real native
  rendering, gesture physics, or Skia/Reanimated execution — the device-level
  claim above is unaffected.)
  It is also why the audit distinguishes "build succeeded" from "app launched
  and was interacted with" as two different, separately-tracked claims.
- Native modules (STT, Apple FM) genuinely cannot be exercised without real
  hardware — there is no useful simulator-level fake for microphone capture
  or the on-device Foundation Models runtime. A "seeded monkey" or "LLM
  explorer" pass on a simulator would only ever exercise the
  availability-probe path, never the actual capture path.
- Two platforms means every device-level scenario needs to be authored (or at
  least verified) twice, and the honest evidence quality currently differs
  between them (see the audit's iOS/Android gap section).

None of this means QA Swarm doesn't apply to mobile — it means the mobile
adaptation's FIRST layer has to be the thing that IS fully automatable today
(pure logic, unit-tested, real CI-speed), with the device-level layer named
explicitly as the next investment rather than faked.

## The Four Check Kinds, Adapted

| Kind | Desktop today | Mobile today | Mobile next |
| --- | --- | --- | --- |
| Scripted scenarios | DOM-mounted `happy-dom` oracle tests + QA-harness RPC scenarios | The 22-file `bun test` suite, including `tests/ux-contracts.test.ts`'s contract oracles AND (as of 2026-07-05) `tests/chat-composer.test.tsx`'s real `react-test-renderer` mounts of the production `ChatComposer` — deterministic, human-designed, real, and now includes actual component-tree assertions, not just pure-function ones | Extend the same `bun test` React Native harness (`tests/support/rn-test-environment.ts`) to the next screen/component; separately, a Maestro (or Detox) scripted flow per platform: sign-in resolves, open a thread, send a message, see the lane picker — this is exactly what `khala_mobile.platform.launched_app_interaction_smoke.v1` is waiting on |
| Seeded monkeys | fuzz/regression corpus (`khala-code-mechanical-corpus.md`) | none yet | A bounded input-fuzz pass over the pure builder functions (e.g. `validateDelegationPrompt`, `mergeTranscriptIntoDraft`) is cheap to add now with property-based generation; a real on-device monkey (random taps) needs the same device harness as scripted scenarios |
| LLM explorers | an agent freely driving Khala Code desktop looking for UX breakage (this is literally how the desktop registry was seeded — see episode 246's "conversation history mining pass") | this audit itself is one instance of an LLM explorer pass — reading every real source file and test, and recording exactly what's proven vs. assumed | a recurring version of this exact audit process, re-run periodically as the app grows, feeding new pending/enforced contracts the same way |
| Perf probes | `docs/qa/khala-code-latency-budgets.md`, perf-trend regressions | none yet | cold-launch time, thread-switch latency, and OTA-check overhead are the natural first mobile perf budgets, but all need a real device/emulator to measure meaningfully — not simulator-only |

## Findings Ledger (mobile instance)

Mirroring `docs/qa/qa-swarm-khala-code-standing-engagement.md`'s lifecycle
(`caught -> filed -> fixed -> distilled`), seeded honestly from this audit
pass:

| Finding | Lifecycle state | Evidence |
| --- | --- | --- |
| Android Kotlin STT module `AsyncFunction` reified-generic build failure | fixed, distilled | `khala_mobile.android.stt_module_typed_asyncfunction_signature.v1` (enforced regression oracle) + `clients/khala-mobile/README.md` real-build receipt |
| No React Native component-mount test harness in this package | fixed, distilled | `khala_mobile.composer.rn_component_mount_coverage.v1` moved `pending` -> `enforced` on 2026-07-05: `clients/khala-mobile/tests/support/rn-test-environment.ts` (a `bun test` React Native harness — Flow-strips `react-native` on the fly and stubs the handful of native-bridge-touching leaves) plus `clients/khala-mobile/tests/chat-composer.test.tsx` (6 real `react-test-renderer` mounts of the production `ChatComposer`, asserting idle/active button state, controlled-input typing, and real `push()` call shapes on Send/Stop). Scoped to `ChatComposer` only — see the follow-up list below for extending it to other screens/components. |
| No device/emulator launch-and-interact proof for either platform | filed | `khala_mobile.platform.launched_app_interaction_smoke.v1` (pending contract) |
| Push-to-talk STT never actually captures audio (both platforms, by design) | filed (known, not a regression — pre-existing scoped limitation) | `khala_mobile.stt.real_device_capture_proof.v1` (pending contract) |
| Apple Foundation Models bridge never actually calls the FM API (by design) | filed (known, not a regression) | `khala_mobile.applefm.real_device_bridge_proof.v1` (pending contract) |

Counted honestly: **2 fixed+distilled, 3 filed, 0 caught-but-unfiled.** This
ledger is intentionally conservative — a finding only advances when a real
receipt (a passing oracle, a real build log, a dated manual-check) backs the
next state, exactly like the desktop ledger's rule.

## Copy Gate (mobile)

Allowed public wording, mirroring the desktop copy gate:

> Khala Mobile's behavior-contract registry and QA findings ledger are seeded
> from a 2026-07-05 audit: every enforced contract has a real, currently
> passing oracle; every known device-level or native-capture gap is recorded
> as an explicit pending contract with a named blocker, not silently assumed.

Disallowed until separately gated (identical spirit to the desktop QA Swarm
copy gate):

- "Khala Mobile is fully tested on real devices."
- "Push-to-talk / Apple Foundation Models work end to end."
- "The mobile app has been verified on Android and iOS with equal rigor."
  (It has not — see the audit's explicit iOS/Android gap section.)
- Any price, SLA, settlement, or third-party customer claim about a mobile QA
  Swarm offering.

## What Was Actually Built in This Pass

1. `clients/khala-mobile/src/contracts/ux-contracts.ts` — the mobile
   behavior-contract registry, reusing the exact same
   `@openagentsinc/behavior-contracts` schema, coverage checker, and markdown
   renderer as desktop (added as a real workspace dependency, not copied).
   9 enforced contracts (each with a real, currently-passing oracle) and 4
   honest `pending` contracts with named blockers.
2. `clients/khala-mobile/tests/ux-contracts.test.ts` — the oracle file: 14
   tests, all real, all passing (mechanical registry validation, coverage
   check, doc-sync check, and one describe block per enforced contract that
   imports and exercises the actual production function).
3. `docs/khala-mobile/khala-mobile-ux-contract.md` — the human-rendered
   registry doc, generated via the shared `renderBehaviorContractMarkdown`
   helper (not hand-typed), so it cannot silently drift from the machine
   registry.
4. `docs/khala-mobile/2026-07-05-mobile-qa-swarm-audit.md` — the audit this
   registry is seeded from.
5. This document.

## What's Left for a Follow-Up Pass

In priority order:

1. ~~**Wire a real RN component-mount test harness into `bun test`.**~~ **Done
   2026-07-05.** `clients/khala-mobile/tests/support/rn-test-environment.ts`
   is the harness (Flow-strips `react-native` via a Bun `onLoad` plugin using
   `@react-native/babel-preset`, and stubs the small set of native-bridge-
   touching leaves — `View`, `Text`, `TextInput`, `Pressable`, `Platform` —
   that have no meaning without a device/simulator host); the working
   example is `clients/khala-mobile/tests/chat-composer.test.tsx`, which
   mounts the REAL `ChatComposer` and asserts the Steer/Queue/Stop button
   swap, the idle-only lane picker's visibility, controlled-input typing,
   and the real `push()` call shape for Send and Stop. This closed
   `khala_mobile.composer.rn_component_mount_coverage.v1` (now `enforced`).
   Scoped to `ChatComposer` only — extending the SAME harness to other
   screens/components is the next highest-leverage step here, since the hard
   part (making `react-native` importable at all under `bun test`) is done;
   each new component only needs its own leaf-component mocks (Skia/gesture-
   handler/native-module boundaries), not new harness work.
2. **Stand up Maestro (or Detox) flows for both platforms** covering the
   scenario in `khala_mobile.platform.launched_app_interaction_smoke.v1`:
   launch, sign-in resolves, open a thread, send a message, see the lane
   picker. This is the mobile equivalent of desktop's DOM-mounted scenarios
   and is the single highest-leverage next investment, since it unblocks
   BOTH the "launched app" contract and gives a real harness for future
   scripted scenarios. Real device/simulator rendering, gesture/touch
   physics, Skia drawing, and Reanimated worklets remain fully out of scope
   for the `bun test` harness above — it exercises React
   state/render/effect logic only, never real native rendering.
3. **A dated physical-device manual-check pass** for push-to-talk capture and
   the Apple FM bridge, once the underlying native implementations move past
   their current always-reject/always-unavailable shells. Until then those
   contracts should stay `pending` — do not implement device automation for
   capabilities that do not exist yet.
4. **Close the iOS/Android evidence gap** named in the audit: get Android to
   at least TestFlight-equivalent evidence (an installed, launched APK on a
   real device or emulator), not just a clean Gradle assemble.
5. **A nightly matrix analog for mobile**, once (2) exists, mirroring
   `docs/qa/khala-code-nightly-matrix.md`'s shape: run the unit suite +
   Maestro flows + the behavior-contract coverage check on a schedule, write
   a `qa-status-surface.json`-equivalent artifact, and stand up a stable
   share URL the same way `docs/qa/qa-swarm-khala-code-standing-engagement.md`
   did for desktop.
