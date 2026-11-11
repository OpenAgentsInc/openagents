# Components

The OpenAgents component library. One set of UI components, written once in Coax, used by every OpenAgents and Verse app. This page shows every component in every skin, drawn by the library itself.

## What is on the page

Four cards, one for each skin:

- **zeron**: Zeron Dark, the look of [chat](/chat/index.md) and OpenAgents for desktop.
- **everglade**: iron and gold, for Everglade and the Golemancers' Workshop.
- **verse**: the Verse's instrument look, grey lines and a cyan accent.
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

The source is in the [OpenAgents repository](https://github.com/OpenAgentsInc/openagents) under `components/`. The plan is `components/spec.md`.
