# Design a part in CAD Lab

CAD Lab designs parts by talking: say what you want, and it builds the
part, checks it, measures it, and keeps every version so you can compare
them. This guide makes a small bracket on openagents.com and downloads it.

CAD Lab on openagents.com is part of Pro (see [Pricing](/docs/pricing)).
CAD on your own computer is free; see the end of this guide.

## 1. Ask for a part

Log in at [openagents.com/login](https://openagents.com/login), start a [new chat](/new), and
describe the part with its sizes:

```text
Design a bracket to hold a 40 mm fan: a flat plate with the fan's four
mounting holes, and two slots at the bottom for M4 screws.
```

A card with the part shows under the answer: the part in 3D (drag to turn
it, scroll or pinch to zoom), with its size and mass. Every change is
rebuilt from its history and checked before it's saved: the solid has to be closed, and parts can't overlap
where you didn't say they should.

## 2. Change it

Ask for changes the same way:

```text
Make the plate 3 mm thick and round the corners.
```

In the card, a tap on a face or an edge shows its name. Then talk about it:
"make the top face 2 mm lower". Each change is a new version, and the
earlier ones are kept; **Compare with** on the card shows what changed
from the version before.

## 3. Open the design page

Each design has its own page, listed under **Your designs** on the
[CAD Lab page](/cad). It has the 3D view, **Files**, and **Versions**.

- **Files:** download the part as GLB, STEP, STL, or 3MF for a printer or
  another CAD program, or as a drawing (PDF or DXF).
- **Versions:** every version, newest first, with what it changed.
- **Pull a face:** pick a face and drag **Pull**, or type a distance in
  millimeters and press **Preview**, then **Save change**.
- **Delete this design** removes every version.

## In a project

A chat in a [project](/docs/share-a-project) keeps its designs in that
project. Viewers of the project can open them; editors and the owner can
change them.

## Without Pro

Designs you already made stay yours: their page, 3D view, files, and
versions stay open, and you can delete them. Making or changing a part
needs Pro.

## CAD on your own computer, free

The same CAD engine runs on your own computer with no login and no plan.
It keeps designs in a folder on your computer and makes no calls to
openagents.com. Today it's built from source: from a checkout of the
[OpenAgents repository](https://github.com/OpenAgentsInc/openagents), run
`cargo build --release -p cad-lab` and put the `cad-lab` program on your
`PATH`. Then, from a terminal:

```sh
openagents cad list
openagents cad open DOC
openagents cad render DOC --view iso
openagents cad export DOC --formats step,stl
```

`openagents cad` starts the CAD service on your computer when it isn't
running. `openagents cad --help` lists every command, including 2D
drawings and comparing versions.

Next: [Follow a run from your phone](/docs/follow-from-your-phone).
