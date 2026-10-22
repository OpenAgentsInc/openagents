# A model group as one machine in the Pylon Bay (#12403)

The Grid's Pylon Bay used to draw a model group as a row of machines with a
pipe on the floor and an HTML label over each one. Now a group reads as one
machine: light conduits join its head and parts into a loop, its name stands
on a sign over its lane, and its state shows in how the loop moves. The live
group is the DeepSeek V4.1 Flash experimental preview (four RTX PRO 6000
parts), so every capture shows it with its preview tag.

## What the Grid draws

Everything below is drawn by Verse, in the world or over it on the canvas.
The group's members keep their HTML buttons as click targets and for screen
readers, but their HTML labels are hidden.

- **The loop.** Raised conduits, two rails of light with ties, arch from
  the head to part 1, then from each part to the next in layer order. The
  floor pipe carries the answer back to the head.
- **The sign.** The model's name in block letters over the lane, and, when
  the launch calls itself a preview, an EXPERIMENTAL PREVIEW tag in a box
  under it. The sign stands behind the machines, inside the group's lane.
- **Devices.** Each part has an emblem in front of it: a hexagon for an RTX
  PRO 6000, a double diamond for an H100, a square for an Ada GPU, a
  crossed square for another NVIDIA GPU, a ring with a dot for a Mac's GPU
  (Metal), and a triangle for a CPU. A part on a Mac stands as a Mac tower.
- **Details.** Within 20 m of the group, or with the pointer over a member,
  a panel over the sign shows the name, the preview note ("Experimental
  preview: its accuracy checks haven't passed yet"), the state, tokens per
  second and time to the first token (from the head's answer profile), the
  parts and answer time limit, the cost an hour at spot prices (from the
  spend guard's price table), and the last answer (from its receipt).
  Within 10 m, each part shows its layers and device under it.

## States

`pylons_view::bay::machine` holds the group-state model, with unit tests.

| State | When | Look |
| --- | --- | --- |
| Idle | The head is ready and no job is running. | Steady, quiet conduits; bright sign. |
| Reading a prompt | The head is busy, until its median first-token time. | Bright conduits; one long sweep with a trail that hops into each part and rests there while it prefills, part by part. The working part glows. |
| Answering | The head is busy, after that. | Bright conduits; a pulse beats around the loop, one pulse per token step, or per a few steps when steps are under 0.4 s (at 29 tokens/s, one pulse per 12 steps). The member the pulse just left glows. |
| Starting, recovering | The head says so, or a part is missing. | Broken conduits. |
| Down | The group left the Grid in the last 10 minutes. | Broken, faint conduits; dark machines with broken beams; "Offline" in its details. |

Beacons don't say when the first token came, so the switch from reading to
answering falls at the head's median first-token time. That median runs
short for a long prompt: in `live-9s.png`, the group still read a prompt
that took about 20 s, but the Grid had already moved to answering. The
head's beacon says when it is busy.

## Captures

1440 × 900, headless Chrome under `openagents browser run`, against a local
build of the site on the production relay with the live group, taken by
`scripts/dev/check-grid-group.py`. Shots marked simulated rewrite only what
the pylons layer hands Verse in that page, to show a state's look on the
live group; they are not evidence of load.

| File | What it shows |
| --- | --- |
| `idle.png` | The live group idle, from the plaza side of its lane. |
| `near-idle.png` | Closer: each part's layers and device under it. |
| `live-9s.png` | A real `pylon ask` 9 s in: bright conduits, a pulse on the loop, and the head's beam up. |
| `live-20s.png` | Another real request 20 s in: answering, the pulse on the loop. |
| `prefill-simulated.png` | The sweep and the working part's glow. |
| `decode-simulated.png` | The answering beat. |
| `selected.png` | The pointer over the group's head: the details at any distance, no HTML label. |
| `mixed-mac-simulated.png` | A mixed group: the last part on a Mac's GPU, as #12420 runs it, with its Mac tower and ring emblem, and a cost for the three rented parts only. |
| `down-simulated.png` | The group just after it left the Grid. |

## Budget

| Measure | Before (`94bf966eed`) | After |
| --- | --- | --- |
| `everglade_web_bg.wasm`, Brotli 11 | 7,586,284 bytes | 7,598,817 bytes (+12.5 KB) |
| `pylons_web_bg.wasm`, Brotli 11 | 81,215 bytes | 91,579 bytes (+10.4 KB) |
| Grid content step, median of 6 | 80 ms | 95 ms |
| First frame, median of 6 | 213 ms | 222 ms |

Both builds were measured on the same base, before this change was rebased
onto the build billboard, with an earlier model set of 110 models; the final
set has 53. The times are local (Apple Silicon, localhost, headless Chrome). The new
models are built at start: one conduit hop in three looks, stretched to each
hop's length, two pulses, a glow, seven emblems, the sign's letters, the
preview tag, and a post. With the build billboard's models, the Grid's pack
passed the engine's limit of 256 models, so the limit is now 512
(`verse_engine::assets::MAX_PACK_MODELS`).
