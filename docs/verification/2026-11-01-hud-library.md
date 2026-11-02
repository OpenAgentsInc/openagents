# Aethyr HUD library verification

Date: 2026-11-01. Base: `71c1d3fcdb`. The implementation adds Coax HUD
geometry and drawing components, a standalone catalog and four compositions,
Coax build/verification tools, the reference study, and chapter 15's partial
implementation record. It leaves the production homepage and its verified
observations unchanged.

## Checks

| Check | Result |
| --- | --- |
| `coax test apps/openagents-web/demos/hud/projects.coax --library` | 25 checks passed, including loaded frame, font, geometry and analytics checks. |
| `coax test apps/openagents-web/demos/hud/build.coax --library` | 9 checks passed, including bounded shell quoting and loaded tool helpers. |
| `coax/web/tool.sh apps/openagents-web/demos/hud/build.coax` | Builds the actual page Wasm and Coax-authored shader with the existing projects C face and browser host. |
| `coax/web/tool.sh apps/openagents-web/demos/hud/check.coax` | Final native and actual browser journeys passed. |
| `python3 psionic/tests/book.py --book aethyr` | 19 programs, 107 source-linked listings and local links pass. This is structural validation, without running every book example. |
| `coax/web/check-visuals.sh apps/openagents-web/build/hud` | Pass: generated visible surface is the Coax canvas; explanatory HTML is hidden when the GPU page runs. |
| `git diff --check` | Pass. |

Native rendering exercises all five compositions at 320×700 @2, 390×844 @3,
900×420 @2, 1440×900 @1 and 1920×1080 @2. Every frame produces finite
vertices, keeps actions within the viewport and stays below the declared
buffer. Peak use is 30,684 of 240,000 vertices. The check reaches the end of
scrollable content and exercises component inspection, modal focus, pointer
release/cancel, clipped controls, pause/reduced motion and all six local
communications commands.

The browser check launches muted headless Chrome through Coax's existing CDP
library. It observes the real page's imported Wasm memory only in the QA
tool; application source has no test observer. It uses real browser pointer
and keyboard events to change all scenes, open and close an inspector, trap
modal focus, change accent, freeze time and arm a route. It reloads with
reduced motion, then checks a 390×844 @2 phone and keyboard scrolling. No
JavaScript exceptions, unhandled rejections or `console.error` calls were
observed. Generic-host missing-resource network messages are outside that
error assertion.

Visual inspection covered the catalog's upper and lower components, the
enlarged orbital inspector, all four composed scenes and narrow phone
rendering. It led to clipping the Grid to its working surface, clipping
scrolling panel glass below the navigation, fitting narrow text and reducing
phone label density. Screenshot tools opened no visible browser and played
no sound. The local dev server is left running at `http://127.0.0.1:8815/`
for the owner's requested review; restart instructions are in the
[demo README](../../apps/openagents-web/demos/hud/README.md).

## Compiler and mutation scope

The available compiler is `coax0 0.47.0`, SHA-256
`6018024e023446d34e0bdd4b74926b9d4edc982e5f5021da430070a94067c648`,
with recorded source digest
`dc9a799262ce259b1d4b46a719620b37dc9b5681beb7c97c2dac53d5c7d1e42d`.
It is reused from the existing checkout through an ignored local executable
link. Compiler sources were newer than this binary; a separate compiler
build was already running elsewhere. This task makes no compiler change and
does not claim a fresh compiler release gate.

An optional mutation exploration of the visual foundation
`coax mutate aethyr/ui/hud.coax --library --jobs 4` killed 106 of 112
mutants and left six survivors (exit 1). No equivalent annotations or waivers
were added, and no zero-survivor claim is made. The surviving sites are the
focus helper's current-index boundary and initial forward index, the
viewport-height zero boundary, and three segment-count boundary comparisons.
This file is not an authority unit and decides no money, admission or
production policy. The required authority mutation gate is not applicable.

## Limits

Readings, contacts and events are explicit synthetic fixtures. Navigation
drawings are illustrative, and communications commands change local demo
state. The full semantic UI tree, fixed-step action integration, dynamic
platform accessibility mirrors, general text scaling and native rendering
adapter remain planned work. The standalone demo is built by its focused
tool and is not installed into the production site's release route table.

The [research report](../verse/2026-11-01-hud-library.md) records the 137-image
visual inventory, twelve larger inspections, source links, eighteen component
APIs and the distinction between observed screen composition and designed
interaction semantics.
