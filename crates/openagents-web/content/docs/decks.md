# Decks and the Map

OpenAgents for Mac can show a slide deck full screen, and a map of how
OpenAgents routes every message.

## Open a deck

In a chat on the Mac, ask for it by name:

- "Show the Test-Time Capabilities deck"
- "Open the Three DevDays Later deck"

The deck opens over the window. Three decks ship with the app today: Three
DevDays Later, Test-Time Capabilities, and OpenAgents, from episode 289.

| Key | Does |
| --- | --- |
| Right arrow, Page Down, Space | Next slide |
| Left arrow, Page Up | Previous slide |
| Home, End | First slide, last slide |
| F | Full screen on or off |
| Esc | Leave full screen, then close the deck |

The buttons along the bottom do the same, with a slide counter. Some
slides are live: the Grid behind a title, the Map you can drag and zoom,
or link cards that open in your browser when clicked.

Decks open only in the Mac app. Asked on the phone or in the Terminal, the
reply says so.

## The Map

The Map shows OpenAgents as one zoomable graph: the router every message
reaches first, its routes, the prepared answers and knowledge behind them,
the chat model, Coder, the coding agents, the plugins, and the screens an
answer can open. Each kind has its own color, listed in the legend.

Open it from **Map** in the sidebar's footer of
[OpenAgents for Mac](/docs/mac), from **Commands** (Cmd+K,
**Open the map**), or from the **Window** menu. Or ask in a Mac chat: "show
me how you route things".

- Drag to pan; pinch, or Cmd and the wheel, to zoom.
- A click on a node shows its details; a double-click zooms in.
- A ring shows health: no ring is measured and good, a red ring is
  measured and weak, and a dashed ring isn't measured yet. A red dot marks
  a gap, where OpenAgents is thin.
- The **Gaps** panel lists them, and an outline lists every node.
- Each plugin shows the step it has reached: Not packaged, Candidate, a
  verdict, Reproduced, Validated, or Adopted.

The phone and the Terminal answer the same question with a sentence; the
Map is on the Mac.

Next: [Settings](/docs/settings).
