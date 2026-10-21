# The Pylon Bay on the Grid homepage (#12423)

The Grid homepage at `/` used to show each Pylon as a white DOM beam on a
ring around the plaza. Labels piled up, every pillar always showed its full
label, a model group's parts stood far apart, and one part stood in the
camera's foreground. Now Verse draws the Pylons as machines in one place,
the Pylon Bay, and the page labels them only when it helps.

## Layout

The Bay lies east of the plaza, left of the walk from the spawn to the Gym,
and faces the plaza. It spans x 14.8 to 42 and z −3.5 to 26.5, clear of the
spawn disc, the Gym, the shared computer, the Lagrange 2 arch, and the Grid
robots' patrols. `pylons_view::bay` holds the layout, and a test checks
those clearances.

- **Sections.** The front holds four sections: Mac, GPU, CPU, and secure.
  Each has six spots, two across and three deep, with its name painted on
  the floor. A label that names sealed hardware (Intel TDX, AMD SEV-SNP, a
  confidential or attested machine) counts as secure, because beacons don't
  say so yet.
- **Model groups.** Behind the sections, each model group has its own lane:
  the head, a router with a halo, then its parts in layer order, 2.3 m
  apart. A pipe on the floor joins them, with marks in front of each part
  that count its place.
- **Stable spots.** A Pylon keeps the spot it took when it arrived. A new
  Pylon takes the first free spot in its section, and a new group takes the
  first free lane. Spots free up when Pylons leave.
- **The sign.** PYLON BAY stands raised behind the last lanes, as the Gym's
  name stands over the Gym.

## Graphics

Each machine is line art in the Grid's ladder, with dark faces that hide
what stands behind it. It stands on a sloped plinth.

- A Mac is a slim tower with a crystal cap.
- A GPU is a rack of blades with two fans on top.
- A CPU is a squat box with a chip on its front and fins on top.
- A secure machine is a box inside a locked cage.
- A group's head is an eight-sided router with a halo, and a part is a slim
  rack.

The beam over a machine shows its load. An idle machine has a short, faint
beam. A busy one has a taller, brighter beam, a full one has the tallest,
and a machine that is winding down has a broken beam. The details on a
machine light up while it works, and a spark climbs its beam. A job's
receipt sends a ring across the floor. While a group answers, its pipe
lights up and a packet runs from the head down its parts.

## Labels

- From farther than 48 m, including the overview, a machine shows no label.
- From 16 to 48 m, a Pylon or a group's head shows its name.
- Within 16 m, it shows its name, its machine, how busy it is, and its
  models. A head says how many parts its group has.
- A part shows its label only within 9 m: "Part 2 of 4" and its layers.
  From farther away, the group's one label is its head's.
- A pointer over a machine, or its open card, shows the full label at any
  distance and marks the machine with brackets on the floor.
- Nearest labels come first, and a label that would cover one already shown
  waits until it has room.

A click opens the Pylon's card, as before. The #12415 fixes hold: a click
leaves focus on the world (`after_click` in the check's JSON), there's no
white focus box, and a pillar hides only when it leaves the screen. The
screen-reader list of Pylons and its keyboard navigation are unchanged.

## Captures

All captures are 1440 × 900 against a local build on the live relay, with
the live Pylons of the time: four ordinary Pylons and one to three DeepSeek
V4.1 groups. `scripts/dev/check-grid-home-bay.py` takes them under
`openagents browser run`.

| File | What it shows |
| --- | --- |
| `before-overview.png`, `before-joined.png` | The old ring of DOM beams with piled labels, before this change. |
| `after-overview.png` | The overview: the Bay as one place, with no labels from that far. |
| `after-joined.png` | The view after joining: the Bay on the left, with names only. |
| `after-near.png` | Walking up to the Bay: full labels near, names further off. |
| `busy-near.png` | The busy look (simulated with `--simulate-busy`): bright beams, sparks, pulses, lit pipes, and packets. |
| `hover.png` | The pointer over a group's head: its full label and floor brackets. |
| `card.png` | The head's card, opened by a click. |

`scripts/dev/check-grid-home.py` also passes against the build: each
label's foot is the exact projection of its Bay spot in every frame, and the
spots stay put through the orbit and a resize.

## Budget

| Measure | `origin/main` | With the Bay |
| --- | --- | --- |
| `everglade_web_bg.wasm`, Brotli | 7,515,647 bytes | 7,525,241 bytes (+9.6 KB, about 4 ms at 20 Mbit/s) |
| `pylons_web_bg.wasm`, Brotli | 70,175 bytes | 79,614 bytes |
| Grid content step, median of 6 | 75 ms | 94 ms |
| First frame, median of 6 | 326 ms | 344 ms |

The start times are local (M5 Max, localhost, headless Chrome). The Bay's
models are built at start and share their corners, because the start
digests every vertex.
