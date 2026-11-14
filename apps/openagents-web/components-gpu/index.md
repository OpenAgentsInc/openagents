# Components (GPU)

The shared Coax Native GPU component gallery, also embedded in the iOS app at **Settings → Components**. WebGPU and Metal draw the same Coax display list with the same generated shader. The [HTML gallery](/components) remains available for browser-native text and controls.

Use **Motion**, **Controls**, **HUD** and **ARWES** to inspect a section. Replay, pause, reduce motion, change skins or tap a motion card. Drag vertically to scroll on a phone. Motion includes corner frames, activity arcs, progress rings, spring switches and drawer/reply transitions. These are the first Arcade motion ports; full blur, image/composition, swipe actions and all 28 audited specimens remain tracked in `components/rendering.md`.

## What is on the page

Four cards, one for each skin:

- **zeron**: Zeron Dark, the look of [chat](/chat/index.md) and OpenAgents for desktop.
- **everglade**: iron and gold, for Everglade and the Golemancers' Workshop.
- **verse**: the OpenAgents Metaverse's instrument look, grey lines and a cyan accent.
- **terminal**: the same components in a terminal's cells.

Each card holds the same components from the same calls: a panel, buttons, toggles, a slider, a bar, tabs, a list with a section, and Markdown (a paragraph, a list with task boxes, a quote, a code block and a table). They work: buttons light under the pointer, toggles flip, sliders drag, tabs and rows pick.

## How it is built

A component doesn't know where it is drawn or how it looks.

- **Core** holds state and rules. Markdown is parsed in one file for every surface.
- **Layout** measures text through one interface: pixels on a pixel target, cells in a terminal.
- **Paint** is a display list of primitives, not vertices.
- **Skin** is tokens and frames: colours, type, spacing, and how a panel or a button is built.
- **Backend** turns the display list into GPU vertices with one shader, or into a terminal's rows.

The terminal card is laid out in cells and drawn by the terminal backend into a grid; the page then shows the grid.

## ARWES

Below the cards, every ARWES port in the Metaverse skin: the nine frames, four backgrounds, the illuminator, text sequence and decipher, all 31 easings, the animator's child timing, flicker, the three styles, theme token scales, the bleeps' cue model and keyed timers, 25 specimens in all, each labelled. They enter with the ported motion; the controls replay them, exit and enter again, and step the easing, the animator's manager, the seed, the dots, the frames' assembly and the cue model. The cue model shows its state and never plays a sound.

The specimens are painted once (`metaverse-engine/ui/arwes-paint.coax`) and drawn here by the component shader; the HUD laboratory draws the same paint as its own records. ARWES is [arwes/arwes](https://github.com/arwes/arwes/tree/bdbaa0324900ee978d42036d1304a053c1fe54b5), copyright 2014–2024 Romel Perez, under the [MIT license](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/LICENSE).

The source is in the [OpenAgents repository](https://github.com/OpenAgentsInc/openagents) under `components/`. The plan is `components/spec.md`.
