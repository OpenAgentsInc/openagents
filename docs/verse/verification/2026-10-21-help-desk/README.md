# The help desk on the Grid homepage (#12443)

A help desk now stands on the Grid plaza, ahead of the spawn and to its
right, on the walk in. A counter signed ASK US holds the launch film's
MacBook, open, and its iPhone on a stand. Walking up to it shows "Press E
to ask" ("Tap here to ask" on a touch screen). E, or a tap on the prompt,
opens the homepage chat over the world, the same GPUI chat the terminal
opens, pointed at our free hosted agent. Escape or walking away closes it.

## Where it stands

The counter's middle is at (−6, 0, −5), facing (0, −7). Visitors ask from
1.3 m in front of it. A test in `crates/grid-help-desk` keeps it 6 to 10 m
from the spawn and clear of the plaza's middle, the terminal, every arch
(open or not), Game Lab, the Pylon Bay, and the Gym. The desk's reach and
Game Lab's never meet.

## Models

`scripts/blender/help_desk_devices.py` builds the film's devices with
`scripts/film/studio/devices.py`, as the `four-apps-intro` shot poses them,
with no video, and writes glTF under `assets/verse/generated/help_desk/`.
`crates/grid-help-desk` compiles them into the Grid's line art: dark faces,
sharp edges as white lines (the keys quieter), and each display as a
textured panel with a still of the OpenAgents chat (laptop) or the
OpenAgents app (phone). The Grid admits the compiled file and the two
stills with the pack, so the page downloads no glTF and links no glTF
reader.

| Asset | Triangles | Bytes |
| --- | ---: | ---: |
| `macbook.glb` | 3,050 | 153,980 |
| `iphone.glb` | 850 | 37,928 |
| `help-desk-devices.bin` (both, compiled) | 3,800 | 54,542 |
| `macbook-screen.png` (384 × 249) | | 20,311 |
| `iphone-screen.png` (144 × 313) | | 5,527 |

The desk's models hold about 4,900 vertices. The devices are drawn at twice
life size so they read from the walk in.

## Chat

The desk reuses the homepage chat. While it is open at the desk,
`openagentsChatDesk` is set, and the chat:

- Sends without the `home` chat surface, so the site answers as it does for
  any signed-out visitor: the hosted agent (Gemini, the answer bank, and the
  product knowledge base, in the "we" voice), with the visitor limits and
  abuse controls unchanged.
- Keeps a chat of its own, apart from the terminal's, and offers no model
  choice or Kiln.

## Visit events

| Event | When | Detail |
| --- | --- | --- |
| `help_desk_reached` | The visitor walks into the desk's reach. | none |
| `help_desk_opened` | The chat opens at the desk. | none |
| `help_desk_message` | A message is sent at the desk. | `nth`: `first` or `later` |

None carries what anyone typed. The Grid report counts visits that reached
the desk and that asked there.

## Captures

1440 × 900, headless Chrome with WebGPU, under `openagents browser run`,
against a local website (`scripts/dev/full-local.sh`'s gateway and chat
worker, with this branch's Grid, chat, and Pylons builds) as a signed-out
visitor. `scripts/dev/check-grid-home-desk.py` takes them and checks the
visit's events.

| File | What it shows |
| --- | --- |
| `spawn.png` | The view after joining: the desk right of the walk to the Gym. |
| `approach.png` | The desk from the walk in, outside its reach. |
| `closeup.png` | Zoomed in: the MacBook with the chat on its screen and the iPhone on its stand. |
| `prompt.png` | At the counter: "Press E to ask" over the sign. |
| `open.png` | After E: the chat, focused, labeled "Ask OpenAgents". |
| `answer.png` | "What is OpenAgents?", answered from the answer bank in 1.5 s. |
| `followup.png` | "How do I connect my phone to Coder on my Mac?", answered by Gemini from the knowledge base in 56 s. |
| `closed.png` | After Escape: the chat closed and the prompt back. |
| `capture.json` | The run's positions, chat states, and the visit's events. |

## Budget

| Measure | `origin/main` | With the desk |
| --- | --- | --- |
| `everglade_web_bg.wasm`, Brotli | 7,586,284 bytes | 7,650,818 bytes (+64.5 KB, about 26 ms at 20 Mbit/s) |
| Grid content step, median of 6 | 80 ms | 101 ms |
| First frame, median of 6 | 201 ms | 233 ms |

The lander chat's WebGPU module is 1,431,084 bytes Brotli, under its
1,500,000-byte budget. Times are local (localhost, headless Chrome).
