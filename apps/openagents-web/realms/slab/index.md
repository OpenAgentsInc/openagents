# SLAB Simulation

An unlisted reconstruction of SLAB at https://openagents.com/realms/slab. SLAB is Palmer Luckey's proposed floating platform for American industry. This world rebuilds it on open ocean from the research in the OpenAgents repository. It is unlisted: the [worlds list](https://openagents.com/worlds) leaves it out, so you open it by its address.

SLAB has published no design. The dimensions here are this world's own planning values, read from the AI-generated concept image and set out in the world spec.

## What it shows

- The `prime` layout: a concrete hull 1,000 m long, 600 m wide and 50 m deep, with rounded corners. It floats with 25 m under the water and its deck 25 m above it.
- The deck's districts, from west to east:
  - the OTEC power plant's turbine halls, heat exchangers, moonpools and ammonia tanks;
  - industry, with process units, a tank farm, a desalination plant and six 60 m stacks;
  - four data halls with their cooling yard;
  - twelve towers of 80 to 180 m;
  - a mining yard of fifty containers;
  - a medical campus;
  - gardens around a domed civic hall.
- The sea is the water lab's: waves from a wind-sea spectrum, made fresh every frame, with whitecaps and white water along the hull.
- Drag, or use the arrow keys, to turn the view.
- The mouse wheel, or R and F, brings it closer or takes it back.
- W, A, S and D move the point it looks at.
- Address options:
  - `?wind=` sets the wind in metres a second (10 by default).
  - `?t=` holds one moment.
  - `?yaw=`, `?pitch=`, `?dist=`, `?x=` and `?z=` place the view.
  - `?tier=phone` draws the phone's waves on any screen.
- Without WebGPU (a current Chrome, Edge or Safari), the page shows only its name and a link here.

This is the first phase of the world spec. The hull doesn't move with the waves yet. The power plant, data halls and miners don't run yet. The dome layouts aren't built.

## How it is made

- `metaverse/slab-plan.coax` is the platform as data: the hull's pieces and the masses on its deck, with its checks.
- `apps/openagents-web/slab.coax` is the world.
- `slab-face.c` binds it for the browser, and `slab.wgsl` (from `gpu/slab.coax`) draws it.
- `docs/research/ocean/` in the OpenAgents repository holds the research:
  - `slab-world-spec.md` is the spec this world is built from;
  - `slab.md` and `otec.md` are the reports behind it.

## Make your own world

An agent can build one from a shallow clone of the repository and open a pull request: [metaverse/SKILL.md](https://github.com/OpenAgentsInc/openagents/blob/main/metaverse/SKILL.md).
