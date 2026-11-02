# ARWES source provenance

Upstream: [arwes/arwes](https://github.com/arwes/arwes).
Pinned commit: [`bdbaa0324900ee978d42036d1304a053c1fe54b5`](https://github.com/arwes/arwes/tree/bdbaa0324900ee978d42036d1304a053c1fe54b5).
The pinned standalone package version is `1.0.0-next.25020502`.
The pin is the source identity; it is not a dependency range.

License: [the exact upstream MIT notice](LICENSE), including
`Copyright (c) 2014 - 2024 Romel Perez (romelperez.dev)`.
The license file is copied byte for byte. Preserve it with translated or
adapted portions. The original project is not a runtime dependency.

This record inventories all ten vanilla/core package families and all
23 package entry indexes. Every family is classified in the
[porting report](../../../docs/verse/2026-11-01-arwes-coax.md).
The tables below pin all 72 non-test, non-index TypeScript source
files under those ten core `src/` trees, plus the 23 package entry indexes,
license, README and standalone package metadata (98 files total).
They include unported host code as an audit boundary. A file hash does not
claim that every feature in that file was ported.

Downloaded code was read as source only. Its scripts, package installation,
builds and framework demos were not run. The Coax files are authored
translations/adaptations checked in this repository.

## Translation and adaptation map

The implementation map is described in the report. Core formulas and
geometry become Coax; DOM/SVG/CSS object operations, framework ownership,
Web Audio decoding/playback and browser scheduling remain host mechanisms
or explicit exclusions. The existing Coaxial host and twelve-float vertex
ABI are preserved. There is no JavaScript API-compatibility claim.

## Complete vanilla/core public-family inventory

Every family re-exported by `packages/standalone/src/index.ts` is listed
here. Types are assessed with their owning runtime family rather than
copied as TypeScript declarations.

| Package | Public exports/families at the pin | Coax disposition |
| --- | --- | --- |
| `tools` | `isBrowser`, `isBrowserSafari`, `cx`, `loadImage`, `createTOScheduler`, `randomizeList`, `filterProps`; `PartialDeep`, `NoInfer`, `Writable` types | Adapt keyed timeout scheduling and seeded Fisher–Yates. Browser detection/image loading belong to the host. Class-name merging, undefined-property filtering and TypeScript type utilities are replaced by typed records/composition, not exported as DOM helpers. |
| `styles` | `styleSteps`, `styleStrip`, `styleSeparator` and their prop shape | Port the visible/transparent band arithmetic into scalar masks and Coax drawing. CSS gradient strings are excluded. |
| `theme` | `createThemeMultiplier`, `createThemeUnit`, `createThemeColor`, `createThemeStyle`, `createThemeBreakpoints`, `createCreateTheme`; multiplier/unit/color/style/breakpoint creator types | Port multiplication, bounded series selection, numeric breakpoints and typed color channels. Adapt style/unit/theme assembly to typed values. Arbitrary CSS strings, deep JavaScript-object factory behavior, arbitrary callbacks and LCH/CSS parsing are not compatibility targets. |
| `animated` | `easing`, `easeAmong`, `easeSteps`, `formatAnimatedCSSProps`, `applyAnimatedCSSProps`, `transition`, `fade`, `flicker`, `draw`, `createAnimation`, `createAnimatedElement`, `createAnimatedXElement`, `animateDraw`; easing/animation/settings types | Port all 31 easing curves, piecewise/step interpolation, transitions and path-progress arithmetic. Adapt time/direction/repeat into a pure caller-time sample and typed properties. CSS/DOM application, RAF callbacks, promises and element ownership stay outside Coax. |
| `animator` | `createAnimatorSystem`; `ANIMATOR_STATES`, `ANIMATOR_ACTIONS`, `ANIMATOR_MANAGER_NAMES`, default duration/settings; node, system, condition, manager and duration types | Adapt four lifecycle phases and six scheduling modes to bounded Coax motion/timeline/switch records. Preserve useful order, reverse, offset, stagger, limit and switch concepts; do not recreate a JavaScript subscriber graph. |
| `frames` | `createFrame`; nine builders: Underline, Lines, Corners, Octagon, Nero, Nefrex, Kranox, Header, Circle; `animateFrameAssembler`; `styleFrameClipOctagon`, `styleFrameClipKranox`; frame/path/element/settings types | Port preset geometry, family-specific numeric transition tracks and clip-point arithmetic; adapt generic drawing/shared assembly to Coax paths and twelve-float stroke output. Generic DOM/SVG construction, class names, definitions, masks, arbitrary SVG attributes and CSS clip strings are excluded. |
| `bgs` | `createBackgroundDots`, `createBackgroundPuffs`, `createBackgroundGridLines`, `createBackgroundMovingLines`; settings/props/cancel types | Adapt all four visual families to deterministic seeded Coax geometry/samples and the existing GPU. Preserve origin/phase/envelope concepts. ResizeObserver, Canvas 2D, browser DPR reads and timer ownership remain host responsibilities. |
| `effects` | `createEffectIlluminator`, `createEffectIlluminatorSVG`; props/cancel types | One typed pointer/radial-field model covers both visual concepts; Coax renders the glow through the existing GPU. DOM insertion, SVG gradient IDs, global document listeners and CSS opacity transitions are excluded. |
| `text` | `animateTextSequence`, `animateTextDecipher`, `getAnimationTextDuration`; text animation settings and controller types | Port duration and adapt reveal/decipher to bounded UTF-8 scalar strings and existing GPU text. No DOM cloning/walking, hidden text mutation or CSS visibility ownership. |
| `bleeps` | `createBleep`, `createBleepsManager`, `BLEEPS_CATEGORIES`; source/general/master/category/manager/update/controller types | Adapt categories, inheritance, gains, mute/disable, play/stop ownership and cue/loop state as pure Coax. Audio fetch, MIME choice, decoding, Web Audio nodes, window focus and actual playback remain platform effects. The demo visualizes silent cues. |

`standalone` is the eleventh vanilla package, an aggregator of those ten,
not an extra implementation family. Its npm metadata also names `tslib`;
that TypeScript runtime is unnecessary for the Coax translation.

## Every framework wrapper and aggregator

The remaining twelve packages contain integration rather than additional
independent visual mathematics. They are reviewed, classified and excluded
as framework runtime code. Their useful core behavior comes from the
Coax equivalents above.

| Package | Public wrapper surface | Treatment |
| --- | --- | --- |
| `react-tools` | `memo`, `mergeRefs`, `NoSSR`, `useUpdateEffect` | React lifecycle/ref/SSR glue; not applicable to a Coax frame loop. |
| `react-animator` | `Animator`, `useAnimator`, `AnimatorGeneralProvider`, `useAnimatorGeneral`, related types | Replace provider/context/subscriptions with caller-owned motion state and explicit parameters. |
| `react-animated` | `useAnimated`, `Animated`, `useAnimatedX`, `AnimatedX` | Use typed Coax animation samples; no React component or element ref is retained. |
| `react-bleeps` | `BleepsProvider`, `useBleeps` | Cue settings/state can be passed explicitly; hooks/providers are not ported. |
| `react-core` | `BleepsOnAnimator` | Adapt lifecycle-to-cue intent as data; do not trigger sound during headless or owner-screen review. |
| `react-text` | `Text` | Use reveal/decipher output with the existing font and fitted text. |
| `react-frames` | `FrameBase`, the nine frame components, `useFrameAssembler` | Use Coax frame geometry and assembler progress directly. |
| `react-bgs` | `Dots`, `Puffs`, `GridLines`, `MovingLines` | Use the four Coax background families directly. |
| `react-effects` | `Illuminator`, `IlluminatorSVG` | Use the single Coax pointer/radial model directly. |
| `react` | Re-exports `arwes` and all nine React packages above | No React dependency or compatibility aggregator is introduced. |
| `solid-animator` | `useGetAnimator`, `Animator` | Solid ownership/context glue; replace with explicit Coax state. |
| `solid` | Re-exports `arwes` and `solid-animator` | No Solid dependency or compatibility aggregator is introduced. |

The source repository's documentation, playgrounds, performance apps,
community examples, build scripts and publishing infrastructure are not
library runtime functionality. They are not imported into OpenAgents.

## Coax implementation mapping

| Coax target | Exact upstream source groups | Treatment |
| --- | --- | --- |
| `../arwes-frames.coax`: `af_settings`, `af_frame_settings`, `af_frame` | `packages/frames/src/createFrame{Underline,Lines,Corners,Octagon,Nero,Nefrex,Kranox,Header,Circle}Settings/*.ts` | Direct preset coordinate/default translation into `AfPoint` paths; resolved dimensions replace SVG percentage/relative commands. |
| `../arwes-frames.coax`: `af_clip`, `af_clip_contains`, `af_assembly` | `packages/frames/src/styleFrameClip{Octagon,Kranox}/*.ts`, `animateFrameAssembler/animateFrameAssembler.ts`; draw-progress concepts from `packages/animated/src/animateDraw/animateDraw.ts` | Clip polygons/containment and explicit shared assembler tracks with overall-track easing; 64-segment ellipse/dash approximation. Source family transitions are separately exposed below; no arbitrary SVG mask engine. |
| `../arwes-frames.coax`: `AfTransform`, `af_family_transition`, `af_family_point`, `af_frame_family` | All nine `packages/frames/src/createFrame*Settings/*.ts` numeric transition settings; Motion One 10.18.0 default easing resolved by the pinned lock and read-only archives below | Numeric source-family fade/draw/translation/rotation/scale/delay tracks; CSS ease cubic-bezier(.25,.1,.25,1) via scalar bisection. Progress 0–1 per entering/exiting direction; separate from shared assembler and per-element browser interruption/inheritance. |
| `../arwes-motion.coax`: `ar_ease`, `ar_steps`, `ar_among`, `ar_draw`, `ar_transition`, `ar_flicker` | `packages/animated/src/{easing,easeSteps,easeAmong,animateDraw,transitions}/*.ts` | Direct numerical curves/interpolation/property tracks with Coax bounds and `f32` drawing profile. |
| `../arwes-motion.coax`: `ar_animation_sample`, `ArMotion`, child/timeline/switch APIs | `packages/animated/src/createAnimation/createAnimation.ts`, `packages/animator/src/internal/{createAnimatorMachine,createAnimatorManager,isNodeConditionCorrect}.ts` and animator constants/types | Caller-time lifecycle/scheduling adaptation; continuity-capture extension, bounded timelines, caller booleans; no RAF/promise/subscriber graph. |
| `../arwes-motion.coax`: `ar_text_duration`, `ar_text_sequence`, `ar_text_decipher`, `ar_text_blink` | `packages/text/src/{getAnimationTextDuration,animateTextSequence,animateTextDecipher}/*.ts`, `internal/setTextNodesContent/setTextNodesContent.ts`, `packages/tools/src/randomizeList/randomizeList.ts` | Duration math and bounded UTF-8 scalar reveal/seeded decipher; no DOM text-node mutation, unsupported/malformed inputs refused. |
| `../arwes-fx.coax`: theme/style/random/timer APIs | `packages/theme/src/createTheme{Multiplier,Unit,Color,Style,Breakpoints}/*.ts`, `packages/styles/src/style{Steps,Strip,Separator}/*.ts`, `packages/tools/src/{randomizeList,createTOScheduler}/*.ts` | Numeric channel/series/breakpoint/mask ports; bounded keyed due-action state replaces timeout callbacks with FIFO tie ordering and refusal of due times outside the finite clock domain; deterministic seed replaces `Math.random`. Generic JS deep factory/CSS serialization excluded. |
| `../arwes-fx.coax`: background and illuminator APIs | All `packages/bgs/src/createBackground*/*.ts`, including dots distance helper; `packages/effects/src/createEffectIlluminator{,SVG}/*.ts` | Sample/phase/envelope/radial arithmetic adapted to Coax buffers/shaders; no Canvas, DOM insertion, global listeners or ResizeObserver. The `aw_background` adapter draws centered/dashed grid lines through the existing Coax line writer. |
| `../arwes-fx.coax`: `AxBleepConfig`, `AxBleep`, `ax_bleep_*` | `packages/bleeps/src/{types,constants}.ts`, `createBleep/*.ts`, `createBleepsManager/*.ts` | Pure category inheritance/gain/caller/restart/loop/load-state adaptation. One current pending timestamp, load-generation ticket and bounded caller bitset replace upstream per-play load callbacks; pending callers enter accounting earlier and `ax_bleep_loaded_for` refuses stale generations, and a stop that ends the cue, unload, and disable invalidate the load ticket. Audio fetch/MIME/decode/playback are not ported. |
| `../arwes-draw.coax`: `aw_frame`, `aw_frame_family`, `aw_frame_profile`, `aw_background`, `aw_visual`, `aw_stroke`, `aw_fill`, `aw_disc`, `aw_quad` | Geometry/sample concepts above, composed through existing OpenAgents HUD, font and stroke ABI | Coax renderer adaptation; even/odd concave fill scan conversion capped at 256 rows; actual radial/solid/disc output in the twelve-float ABI. |
| `../../../apps/openagents-web/demos/hud/{arwes-demo,gpu}.coax` | Core concepts above plus original OpenAgents interaction and projects shader integration | Sixth ARWES tab with ASSEMBLY/FAMILY frame profile control: 25 specimens in Frames9/Motion6/Fields5/Tokens5, plus original18 =43. Coax fragment modes5 radial,6 rectangle,7 disc; silent cue visualization; framework packages are not runtime dependencies. |

The brace notation groups exact files listed individually with hashes below.
Coax implementation links are relative to this directory. They are source
mapping, not a claim that every declaration in an upstream file is supported.

## Supplemental Motion default-easing evidence

The pinned [package-lock.json](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/package-lock.json#L2099)
resolves `@motionone/animation`, `@motionone/dom` and `@motionone/utils` to
10.18.0, with their integrity values recorded there. The following locked
archives were downloaded/unpacked and read as inert source only to resolve
omitted frame easing defaults; no package script or runtime was executed.
They establish CSS `ease = cubic-bezier(.25,.1,.25,1)` and the numeric
bisection behavior consulted by the Coax family-transition port. The archive
references supplement the 98 ARWES-core file hashes below; they are not
extra copied runtime packages or a Motion API-compatibility commitment.

| Locked archive | Consulted paths |
| --- | --- |
| [@motionone/utils 10.18.0](https://registry.npmjs.org/@motionone/utils/-/utils-10.18.0.tgz) | `dist/defaults.es.js` |
| [@motionone/animation 10.18.0](https://registry.npmjs.org/@motionone/animation/-/animation-10.18.0.tgz) | `dist/utils/easing.es.js`, `dist/Animation.es.js` |
| [@motionone/dom 10.18.0](https://registry.npmjs.org/@motionone/dom/-/dom-10.18.0.tgz) | `dist/animate/animate-style.es.js` |

## Exact upstream file hashes

Paths are relative to the pinned repository root. Hashes are SHA-256 of the
raw file bytes, not Git blob IDs or generated artifacts. Links resolve to
the pinned revision rather than mutable `main`.

| Upstream path | SHA-256 |
| --- | --- |
| [LICENSE](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/LICENSE) | `c4c322f2a1047a2fa9612387520938b569a4fd5ff54ca1af9166cabc5e04f51f` |
| [README.md](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/README.md) | `f20e7418b46d73d2dfdb399609a50fed6c6c8c9ad11a518170c48c0170953d61` |
| [packages/standalone/package.json](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/standalone/package.json) | `8b87dfb6c732228027ab4e4b1ffe6fbd8d238b7b70ae5d1069eb4fbd127942a6` |
| [packages/animated/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animated/src/index.ts) | `37c4f9267bca21f0a94fd6f65207f4d8fbffa142d6cb82e8a340cc0ba9d2bee5` |
| [packages/animator/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animator/src/index.ts) | `bca8742d5379a4b2a92ed2df0b1fc42458b0d65cb2113c5b882b47fd95d11024` |
| [packages/bgs/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/bgs/src/index.ts) | `9070882582e686a0648cb4eed99b37154de87514173a3c50f656aaf9b672ba94` |
| [packages/bleeps/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/bleeps/src/index.ts) | `2ba838f3b2e7c3c987303fb8a3603a1d520022c78b17fc2947f80405e580d734` |
| [packages/effects/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/effects/src/index.ts) | `1d75e54bb9f5fac7d2f40dda2d92dbc81964866ee9b9df5053e68d0ac19b1716` |
| [packages/frames/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/index.ts) | `8be2c3e4dad7cc6e7c182ae9894aea48e8d8d8617530b310500f75cdb128597d` |
| [packages/react/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/react/src/index.ts) | `9057ad7f715d70ba7ddc56f666d09ab3bfebf9877bf5413d37326bdb1d7b69fe` |
| [packages/react-animated/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/react-animated/src/index.ts) | `94d52333877b5e153dca5d0b44cdd91082065bf76edeb3295771c28753af78ac` |
| [packages/react-animator/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/react-animator/src/index.ts) | `ec07d545007f7e3dc169fd0e2eace2bf0c68fc91e7d75fb22dd4ec2b5f7a7509` |
| [packages/react-bgs/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/react-bgs/src/index.ts) | `d050859e46b96c97af0eb21699f74f4314588060c4a18febe6c0110b6af321e9` |
| [packages/react-bleeps/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/react-bleeps/src/index.ts) | `6a87a8527fea529ec3285576144e25de2bcdddfeb8ed20bc29b73020fc98444e` |
| [packages/react-core/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/react-core/src/index.ts) | `1d81acbfcb3c9e0e9344e3815412a9ba747c62489c415420d22519b7cdc32d1d` |
| [packages/react-effects/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/react-effects/src/index.ts) | `4c8c11a98150ccf1e73dbc91cbfa66da5cac6c02fd77cdf953a9bb0da588c951` |
| [packages/react-frames/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/react-frames/src/index.ts) | `fb001c4f632a125da0d9f6943310a37a2ccabb5e4da4dd3698814d35ff6f0e48` |
| [packages/react-text/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/react-text/src/index.ts) | `bce084331b053d873330885d247f649ead27d7c16769c0e45498c5b6239796be` |
| [packages/react-tools/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/react-tools/src/index.ts) | `00cff51d873b21f389449aaeb36072679b704162cb0ec020435a10e74d8be558` |
| [packages/solid/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/solid/src/index.ts) | `47951b15965a68a33db8c23b24a86c3f9b2840cd83014409c5c8f46e48ef2a12` |
| [packages/solid-animator/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/solid-animator/src/index.ts) | `468dc41edce12dc30129c6e14fd1a692556ea8c595b5bff70203db279cd10b49` |
| [packages/standalone/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/standalone/src/index.ts) | `bce5e4352de195ad58396ee3c179559714f28240f963c67b188417aa0c2276cf` |
| [packages/styles/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/styles/src/index.ts) | `65bd2cd9e101057f329a4da76ef14c46a66986b87d0bcc0c43f77e38f9f3b550` |
| [packages/text/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/text/src/index.ts) | `f0dbd593df42c2d1fab0597dc51a0506d3622cb54f667a3ef3cfef0882c99887` |
| [packages/theme/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/theme/src/index.ts) | `7f050e1bca82e32393ffdcddcfc3a8e5be35dfa10b961c3be6e2e196caf71fe7` |
| [packages/tools/src/index.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/tools/src/index.ts) | `94890f1c379d4227a5a191570df8899c0c1533a86c7aa902d241a309061a4e78` |
| [packages/tools/src/createTOScheduler/createTOScheduler.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/tools/src/createTOScheduler/createTOScheduler.ts) | `cf9b1b9cb7006ebf8c1d4af905eb89a06299eee31795d6ec20b63388afed655b` |
| [packages/tools/src/cx/cx.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/tools/src/cx/cx.ts) | `c355f6b3bcdac0d0dd13ac748c09b598e52725b48896d64909c4cf6e78e515df` |
| [packages/tools/src/filterProps/filterProps.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/tools/src/filterProps/filterProps.ts) | `26f4514de7097d955a16d60a780508e15116fa5e69e33e173a5f87fb74372de9` |
| [packages/tools/src/isBrowser/isBrowser.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/tools/src/isBrowser/isBrowser.ts) | `d5658615995ccb99139ad5c0c55ef0277c4c3ae1df4ba16f00a598935dac1f7f` |
| [packages/tools/src/loadImage/loadImage.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/tools/src/loadImage/loadImage.ts) | `93bb040935d9a0789369e4efa1b92330b1d6a25369206e9591a4e83252ee9687` |
| [packages/tools/src/randomizeList/randomizeList.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/tools/src/randomizeList/randomizeList.ts) | `eb0c9cc99af7d98442420a82324dd72f7ce1a6812cbe103934bd1ff13573cf13` |
| [packages/tools/src/types.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/tools/src/types.ts) | `63c81d85de9690a80e14155b7cb483be9bba5b6cbfa8b21c98e099d97bf3134d` |
| [packages/styles/src/styleSeparator/styleSeparator.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/styles/src/styleSeparator/styleSeparator.ts) | `8c358a691b3c5818238311a9e9b53fab60fcd0c9f089421011e5e5826d299371` |
| [packages/styles/src/styleSteps/styleSteps.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/styles/src/styleSteps/styleSteps.ts) | `3fdb503bf866f08212aa67f3d590e3a3245d40e1c202452537eccccc5322f15a` |
| [packages/styles/src/styleStrip/styleStrip.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/styles/src/styleStrip/styleStrip.ts) | `b7b3c002b91acb9411887c8bf483d62ce95ab41b1da0447c916923be45ce3e2e` |
| [packages/theme/src/createCreateTheme/createCreateTheme.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/theme/src/createCreateTheme/createCreateTheme.ts) | `c72822482f3ce593b5e5a73396561bfa481d4feabbcd6a1e07bcad2fe9ef6d65` |
| [packages/theme/src/createThemeBreakpoints/createThemeBreakpoints.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/theme/src/createThemeBreakpoints/createThemeBreakpoints.ts) | `c769bd261d9ec9972ef3ca9730848d13e3da01688c59b2f708161cd3ab2f3c5a` |
| [packages/theme/src/createThemeColor/createThemeColor.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/theme/src/createThemeColor/createThemeColor.ts) | `2ae987056699cf2f68d25861a3edc88952d8c16e742dbdaea386616de4a5f6c5` |
| [packages/theme/src/createThemeMultiplier/createThemeMultiplier.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/theme/src/createThemeMultiplier/createThemeMultiplier.ts) | `3254613ebba840f8171e1ccca9f809fa6bb86d2a39719a5d63571412b84759a9` |
| [packages/theme/src/createThemeStyle/createThemeStyle.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/theme/src/createThemeStyle/createThemeStyle.ts) | `70e70161fca38c4fffe7e0201f3281bc13d3ae3292c9b17f91d4c0526baa3b95` |
| [packages/theme/src/createThemeUnit/createThemeUnit.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/theme/src/createThemeUnit/createThemeUnit.ts) | `3cf1433efedf10f80d7936042f7e1ea5544db82c5c14e07a33f0d87c0d83564b` |
| [packages/theme/src/types.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/theme/src/types.ts) | `933bdd88faf5dbf619e9f5f9c8e91a295d2e7b51ff823596feb4703cfed4cfd9` |
| [packages/animated/src/animateDraw/animateDraw.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animated/src/animateDraw/animateDraw.ts) | `1600ef3637bed13465bd72a714213011978f191a083b38015c10c09f78bd1fa0` |
| [packages/animated/src/applyAnimatedCSSProps/applyAnimatedCSSProps.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animated/src/applyAnimatedCSSProps/applyAnimatedCSSProps.ts) | `c6d178dd41acbc7a75cc00d27a09033b103ac91ab46873ddbb5ec4b756602424` |
| [packages/animated/src/createAnimatedElement/createAnimatedElement.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animated/src/createAnimatedElement/createAnimatedElement.ts) | `e8a3f2a7534cb980bdb8df6dc2f34c115dd81dfd07d012d246cfe82c462ac0b9` |
| [packages/animated/src/createAnimatedXElement/createAnimatedXElement.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animated/src/createAnimatedXElement/createAnimatedXElement.ts) | `38871f8b79bd8c1e5b47fb3afb8903442801bc1e0c4fe4465e117175675484e5` |
| [packages/animated/src/createAnimation/createAnimation.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animated/src/createAnimation/createAnimation.ts) | `6f3d1ba8ddc94448df40a1b03e3aed2a4a888dda515ed33c135634a3b9712ef5` |
| [packages/animated/src/easeAmong/easeAmong.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animated/src/easeAmong/easeAmong.ts) | `95d6b56e9c23a0ab00e6935f964f9afab5301ab9f6a4f28a127341522f8183db` |
| [packages/animated/src/easeSteps/easeSteps.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animated/src/easeSteps/easeSteps.ts) | `54b626e31858a05ceb9baa004ec7d03b1240c43a2c2a939e7a488c0998284b1d` |
| [packages/animated/src/easing/easing.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animated/src/easing/easing.ts) | `07e46ac039b81305b25ab7619106ed1d495582ce11c86998186a89715be091bf` |
| [packages/animated/src/formatAnimatedCSSProps/formatAnimatedCSSProps.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animated/src/formatAnimatedCSSProps/formatAnimatedCSSProps.ts) | `ffed3fb0a09d0a8c67774e7a59a840710bbb6dcecaf00702f16f11200e410cd8` |
| [packages/animated/src/transitions/transitions.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animated/src/transitions/transitions.ts) | `db4c10caf753cf6308996ed290a5a1bfc076b01b2b31be4fdb8a4cbe45615650` |
| [packages/animated/src/types.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animated/src/types.ts) | `e9d925d8d3cb7815feb8a517fb8a3b7019bedd505632b8375d4630605e2af88d` |
| [packages/animator/src/constants.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animator/src/constants.ts) | `651c6f479c8849b003cfcb126fcc3d43ae276752eaa4514afb173b483ab0fa80` |
| [packages/animator/src/createAnimatorSystem/createAnimatorSystem.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animator/src/createAnimatorSystem/createAnimatorSystem.ts) | `18106b0eefd644d4fa1f338824f92f19d624268658b9a2467c130cba6b3e87c5` |
| [packages/animator/src/internal/createAnimatorMachine.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animator/src/internal/createAnimatorMachine.ts) | `9e5f82fb1fd30fca0ed33e2a18a4e1ef1a1a4fb6631ce4530094be3314473220` |
| [packages/animator/src/internal/createAnimatorManager.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animator/src/internal/createAnimatorManager.ts) | `6a9e9eeba356efd9ac6d4a3122077710965c3e4f60f551a5b7cfcadcad7396d9` |
| [packages/animator/src/internal/isNodeConditionCorrect.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animator/src/internal/isNodeConditionCorrect.ts) | `fdc0cecb56ca91061197b9b4b0999584acdf57d19157cc89cb8eaab2fb9088da` |
| [packages/animator/src/types.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/animator/src/types.ts) | `944cdbcc0c1b1d7e6bb350db98a53fd173cbf36449b360050591ac09d0096cc8` |
| [packages/frames/src/animateFrameAssembler/animateFrameAssembler.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/animateFrameAssembler/animateFrameAssembler.ts) | `7576062715513ddf166c539c1066600b048a0666192f3b5754e8209c3c4d794e` |
| [packages/frames/src/createFrame/createFrame.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/createFrame/createFrame.ts) | `5f2bc89e57c41fb989aa5a6fdcf190199f91e0af25b7a030539748d111f242ea` |
| [packages/frames/src/createFrameCircleSettings/createFrameCircleSettings.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/createFrameCircleSettings/createFrameCircleSettings.ts) | `bbf36e4b2ad65bac2e521c59384cbbb4bbf9e2fa88b4da14f562ca6613c53079` |
| [packages/frames/src/createFrameCornersSettings/createFrameCornersSettings.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/createFrameCornersSettings/createFrameCornersSettings.ts) | `43ae440ed1ea1fc25e9afb07171a66471db9d85e663c279f2886d9ad9daf1ea3` |
| [packages/frames/src/createFrameHeaderSettings/createFrameHeaderSettings.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/createFrameHeaderSettings/createFrameHeaderSettings.ts) | `d89ee60b3ca859ae4508e983dd8682ee1638cfea822832297ab24927e6f11e5e` |
| [packages/frames/src/createFrameKranoxSettings/createFrameKranoxSettings.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/createFrameKranoxSettings/createFrameKranoxSettings.ts) | `deac734b7d7713df7f364c0e76a0c7db2b27accd8b5f10fabf8f12f93d14e17a` |
| [packages/frames/src/createFrameLinesSettings/createFrameLinesSettings.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/createFrameLinesSettings/createFrameLinesSettings.ts) | `590593cc08661f73db831e1aca12b6ad7d5bb4ccdb518daf2499d638da7e2a98` |
| [packages/frames/src/createFrameNefrexSettings/createFrameNefrexSettings.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/createFrameNefrexSettings/createFrameNefrexSettings.ts) | `357c6ecd1170535e2777bf4f3462c6f46225cf03b227350acef601c30f15522e` |
| [packages/frames/src/createFrameNeroSettings/createFrameNeroSettings.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/createFrameNeroSettings/createFrameNeroSettings.ts) | `579047af3f1030e3228c7e71ec277b350c726c6d17b91fc45a6ff0af7184248c` |
| [packages/frames/src/createFrameOctagonSettings/createFrameOctagonSettings.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/createFrameOctagonSettings/createFrameOctagonSettings.ts) | `9189661c6927cddcf26c4805edd863022ec14982b78a433c9d8668e8e40c6aca` |
| [packages/frames/src/createFrameUnderlineSettings/createFrameUnderlineSettings.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/createFrameUnderlineSettings/createFrameUnderlineSettings.ts) | `a109e82240dc1d4abd59221cec71f26a2e66534a899d2d203b36c506dd71da3d` |
| [packages/frames/src/internal/drawFrameElements.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/internal/drawFrameElements.ts) | `487d2577abc68c490c85fea1dffcec83da66bd37374ceceec7d18f24ebe5733b` |
| [packages/frames/src/internal/formatFrameDimension.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/internal/formatFrameDimension.ts) | `19304659a6910d4b5c28d694d0ad7e1b9e5d19b17b23ca82234521c2425b1958` |
| [packages/frames/src/internal/formatFramePath.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/internal/formatFramePath.ts) | `62826ba21e5b1f4b21d5464b52fb925bc534142050b938af952b2ce86d517e58` |
| [packages/frames/src/internal/renderFrameElements.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/internal/renderFrameElements.ts) | `7f02e1f6110095b98413a20805b5d55cec50bd53defb31e0b115882da4e9d593` |
| [packages/frames/src/internal/transitionFrameElements.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/internal/transitionFrameElements.ts) | `6164c7100e62756019771f0b7a34c77c2f13961effd689492bbe9f03f78fd402` |
| [packages/frames/src/styleFrameClipKranox/styleFrameClipKranox.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/styleFrameClipKranox/styleFrameClipKranox.ts) | `42ce76c2bcbcbf4710e1939b7b58b8c7a7cec4a726c7bbd1ddb5554831be0b92` |
| [packages/frames/src/styleFrameClipOctagon/styleFrameClipOctagon.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/styleFrameClipOctagon/styleFrameClipOctagon.ts) | `c9b85ecaf5bbfa241092aa56103ebe95c143b93b62a0d6f61a0b02d0bdc0176f` |
| [packages/frames/src/types.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/frames/src/types.ts) | `cb296af23304427f422f6bd5007c4c73905e0f154c2784891ad44454fc396f7c` |
| [packages/bgs/src/createBackgroundDots/createBackgroundDots.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/bgs/src/createBackgroundDots/createBackgroundDots.ts) | `79060e56ef22553a21964246e8b593a92d049528c7e2bd6f6d8224f8187d6f5a` |
| [packages/bgs/src/createBackgroundDots/getDistanceFromOriginToCornerProgress.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/bgs/src/createBackgroundDots/getDistanceFromOriginToCornerProgress.ts) | `eb70a585f426d14989e17dda402a3b32993c94220ed2a619dbb550785ab0ad2b` |
| [packages/bgs/src/createBackgroundGridLines/createBackgroundGridLines.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/bgs/src/createBackgroundGridLines/createBackgroundGridLines.ts) | `bf957aaa4c17a42550382f1c6aed32d841508107ffdae1b5f7a398f373abaccc` |
| [packages/bgs/src/createBackgroundMovingLines/createBackgroundMovingLines.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/bgs/src/createBackgroundMovingLines/createBackgroundMovingLines.ts) | `95f8d5aecedfdd13746d0c3d47a98e971e6a0eac1fc5035c2b49583d62456388` |
| [packages/bgs/src/createBackgroundPuffs/createBackgroundPuffs.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/bgs/src/createBackgroundPuffs/createBackgroundPuffs.ts) | `7fd9bbf93552fc391c7d8fca8d4a337eb35ff8055d60b65d7973b88c04580d75` |
| [packages/effects/src/createEffectIlluminator/createEffectIlluminator.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/effects/src/createEffectIlluminator/createEffectIlluminator.ts) | `7b622c690875be416919d5c4c2620c5d334e9cfa5b466235c38297ec6ffb383a` |
| [packages/effects/src/createEffectIlluminatorSVG/createEffectIlluminatorSVG.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/effects/src/createEffectIlluminatorSVG/createEffectIlluminatorSVG.ts) | `02b436d1fd48f24ad917318409bf696200d0c5638ea177f1078b027ae16b07c3` |
| [packages/text/src/animateTextDecipher/animateTextDecipher.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/text/src/animateTextDecipher/animateTextDecipher.ts) | `ef070f0b6cb6ce2c9e030488dcfbff800552072bece2878f124eecb0d6d49d9c` |
| [packages/text/src/animateTextSequence/animateTextSequence.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/text/src/animateTextSequence/animateTextSequence.ts) | `bcb1a4aa92cf3bc3d498a67677a7ec73e93d1651f98c1bbd73cf03543ba454ff` |
| [packages/text/src/getAnimationTextDuration/getAnimationTextDuration.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/text/src/getAnimationTextDuration/getAnimationTextDuration.ts) | `b31f8a9e55f6c70609f4c5d02436a5a36a78b6e64846ba0d96ae4720007ceef2` |
| [packages/text/src/internal/setTextNodesContent/setTextNodesContent.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/text/src/internal/setTextNodesContent/setTextNodesContent.ts) | `e6a9532f9ea0423407452335dbb81f80e77ced39a9924173f392afa386ac2f8d` |
| [packages/text/src/internal/walkTextNodes/walkTextNodes.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/text/src/internal/walkTextNodes/walkTextNodes.ts) | `feae464e7300cc8fc2148add4cba946b3943aa5cbc1f3e19a43e80a382ecb6ce` |
| [packages/text/src/types.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/text/src/types.ts) | `ef7a68858208ef52ec674dd6079c581bedf30f5690631061dc39637e53bbeedb` |
| [packages/bleeps/src/constants.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/bleeps/src/constants.ts) | `6d28ae9fc39f021e90f952064ec046a9c3fc8d44512555fe2a57273598d2e4b7` |
| [packages/bleeps/src/createBleep/createBleep.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/bleeps/src/createBleep/createBleep.ts) | `1bcf1e3efd995ec81299ad4a94dcc0ebaf39b2ec95adc86ee8b88b5c85e0be53` |
| [packages/bleeps/src/createBleep/createBleepLoader.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/bleeps/src/createBleep/createBleepLoader.ts) | `a6ac38b19718a500f588ca4a061472af8dcc6116f9b8d79db21a9f0c3ad7f32f` |
| [packages/bleeps/src/createBleep/createBleepSource.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/bleeps/src/createBleep/createBleepSource.ts) | `e2b286ba1b906b446b9307d01b4b59a30db0e5d71431bc6f8c19aca83f7cbbae` |
| [packages/bleeps/src/createBleepsManager/createBleepsManager.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/bleeps/src/createBleepsManager/createBleepsManager.ts) | `530428f0f0c4e356eb3b92f7a1806d013af1cff1869cd435d5571e0c43c5fd8f` |
| [packages/bleeps/src/types.ts](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/packages/bleeps/src/types.ts) | `fcdb8f8db62c68dd078076fc19752d199835446266c01e54bdce836df6e9205b` |
