# SLAB Simulation

An unlisted reconstruction of SLAB at https://openagents.com/realms/slab. SLAB is Palmer Luckey's proposed floating platform for American industry. This world rebuilds it on open ocean from the research in the OpenAgents repository. It is unlisted: the [worlds list](https://openagents.com/worlds) leaves it out, so you open it by its address.

SLAB has published no design. The dimensions here are this world's own planning values, read from the AI-generated concept image and set out in the world spec.

## What it shows

- The `prime` layout: a concrete hull 1,000 m long, 600 m wide and 50 m deep, with rounded corners. It floats with 25 m under the water and its deck 25 m above it.
- The deck's districts, from west to east:
  - the OTEC power plant's turbine halls, heat exchangers, moonpools and ammonia tanks;
  - industry, with process units, a tank farm, a desalination plant and six 60 m stacks;
  - four data halls with their cooling yard;
  - twenty towers of 80 to 180 m;
  - a mining yard of fifty containers;
  - a medical campus;
  - a canal, colonnades and gardens around a domed civic hall.
- The hull floats. It rides on the waves under it, held by sixteen mooring lines, pushed by the wind and a current. The motion panel shows its heave, roll, pitch and offset.
- The power runs. OTEC, or nuclear modules, or both, feed the data halls, desalination, industry and the miners, and the dispatcher sheds load when the power falls short.
- Click anything for a card: what it is, how it is built, and its live numbers. Click the sea for the sea state.
- Five sites (Guam, Puerto Rico, Kona, the equator, California) and six sea states, from calm to a hurricane. The sun follows the site, the month and the hour.
- Three ways to move:
  - **Orbit**: drag or the arrow keys turn the view, the wheel or R and F zoom, W, A, S and D move.
  - **Fly** (F): W, A, S and D along the view, Space or E to rise, Shift or Q to drop, the arrow keys to turn, the wheel to set the speed.
  - **Walk** (G): GameBuilder's character controller on the deck, which moves with the hull. Space jumps; Space twice flies.
- Keys 1 to 9 travel to stations around the platform. N and M change the site and the sea, [ and ] the hour, P the power, I the stats, H hides the panels, ? shows the help.
- Address options:
  - `?site=` and `?sea=` pick a site and a sea state; `?wind=` sets the wind in metres a second.
  - `?hour=`, `?month=` and `?rate=` set the clock and how fast it runs.
  - `?power=otec`, `nuclear` or `hybrid`; `?otec=` scales the OTEC plant; `?modules=` and `?swac=1`.
  - `?mode=fly` or `walk` starts moving.
  - `?t=` holds one moment.
  - `?yaw=`, `?pitch=`, `?dist=`, `?x=` and `?z=` place the view.
  - `?hud=0` hides the panels.
  - `?tier=phone` draws the phone's waves on any screen.
- Without WebGPU (a current Chrome, Edge or Safari), the page shows only its name and a link here.

The physics and power are engineering models, not measurements. No OTEC plant has delivered more than about 0.1 MW net. The dome layouts, the section and underwater views and the flow overlay aren't built yet.

## How it is made

- `metaverse/slab-plan.coax` is the platform as data: the hull's pieces and the masses on its deck, with its checks.
- `metaverse/slab-sim.coax` is the physics and the power: the sites, sea states, hull motion, moorings and dispatch, with the spec's tests.
- `apps/openagents-web/slab.coax` is the world.
- `slab-face.c` binds it for the browser, and `slab.wgsl` (from `gpu/slab.coax`) draws it.
- `docs/research/ocean/` in the OpenAgents repository holds the research:
  - `slab-world-spec.md` is the spec this world is built from;
  - `slab.md` and `otec.md` are the reports behind it.

## Make your own world

An agent can build one from a shallow clone of the repository and open a pull request: [metaverse/SKILL.md](https://github.com/OpenAgentsInc/openagents/blob/main/metaverse/SKILL.md).
