# SLAB Simulation

An unlisted reconstruction of SLAB at https://openagents.com/realms/slab. SLAB is Palmer Luckey's proposed floating platform for American industry. This world rebuilds it on open ocean from the research in the OpenAgents repository. It is unlisted: the [worlds list](https://openagents.com/worlds) leaves it out, so you open it by its address.

SLAB has published no design. The dimensions here are this world's own planning values, read from the AI-generated concept image and set out in the world spec.

## Build SLAB from the ports

[Play the construction timelapse](/realms/slab/?view=build). In the world, open **Build** or press **B**. Drag the gold timeline in either direction. Space plays or pauses, the arrow keys step through the timeline, and Escape returns to the original camera. **Tow**, **Join** and **Outfit** jump to the first operation of each kind. Drag the scene to orbit and use the wheel to zoom. A link with `?view=build&day=983.8` pauses on that project day.

| Follow the work | What to watch |
| --- | --- |
| [Corpus Christi casting yard](/realms/slab/?view=build&day=14) | An 8 m starter shell floats at the quay. As crews raise the walls, place the deck and load low ballast, the caisson draws more water. |
| [First Gulf tow](/realms/slab/?view=build&day=30.28) | Two tugs take one 100 × 50 m caisson through an assumed 8 nautical miles of channel at 3 knots and 40 offshore at 4 knots. A 20 m/s headwind holds the first departure for two days. |
| [Assembly berth](/realms/slab/?view=build&day=503.8) | Tugs warp each caisson along an open row. The first caisson takes the moorings. A work barge attends each subsequent joint. Only a completed connection adds that unit to the joined displacement. The 120th joins on day 983. |
| [Brownsville freight run](/realms/slab/?view=build&day=984.6) | A separate 120 × 32 m lighter brings 25,000 t per call. The route assumes 15 nautical miles of channel and 130 offshore. Watch the lighter rise as cargo transfers to the hull, its towers go up and ballast brings it to operating draft. |
| [Commissioned deck](/realms/slab/?view=build&day=1383) | At 25 m draft the joined hull displaces 15.375 million metric tonnes. The sequence ends at the original skyline. The 40 m berth rules out the main world's kilometre-long OTEC pipes. |

**What the numbers mean.** The project assumes parallel casting positions producing one closed unit every eight days. Each needs about 22,372 m³ of reinforced concrete at 2.5 t/m³, including the separate end walls needed during tow. With 3,000 t of low ballast the tow weighs about 58,931 t, draws 11.50 m in seawater and has an initial transverse metacentric height of 2.48 m under the assumed mass distribution. At 4 knots in a 10 m/s headwind, the steady drag estimate including a 30% margin calls for 1.73 MN of the assumed 2 MN available underway. At 20 m/s it exceeds that allowance. The port’s published channel is 530 ft wide and 54 ft deep. The assumed screening margins are 10 m on each side and 2 m below the keel. The 600 m assembled beam excludes a tow of the finished city through that channel. [Corpus Christi channel dimensions](https://portofcc.com/wp-content/uploads/PCCA-Press-Release-CIP-Completion-vf.pdf) · [Brownsville port directory](https://www.portofbrownsville.com/wp-content/uploads/2026/01/Port-of-Brownsville-Directory-2025-Web-V1.pdf)

**How to read the time and sea.** The 90-second playback slows down for the first tow, join and freight call. The day counter follows project time. The distance strip uses nautical miles and the 3D view compresses the space between ports and berth. Draft follows mass and immersed volume, assuming ballast is distributed to hold an even keel during outfitting. The moving hulls follow the mean sampled wave elevation beneath their waterplanes. The mass transfer, static initial stability and steady tow resistance are planning calculations. The scenario assumes a 40 m Gulf berth and prescribed tow paths. A real operation would need surveys, permission, tug performance curves, six-degree-of-freedom seakeeping, damaged stability, dynamic line loads, joint strength and weather-window analysis. The dates, yards, freight, ballast schedule and berth represent a construction scenario. [SLAB feasibility research](https://github.com/OpenAgentsInc/openagents/blob/main/docs/research/ocean/slab-image-feasibility.md)

## What it shows

- The `prime` layout, built to match the concept image: a concrete hull 1,000 m long, 600 m wide and 143 m deep, with sharp corners. It floats with 25 m under the water and its wall 118 m above it, as in the picture. It opens on the picture's own view, at its golden hour.
- The deck's districts, from west to east:
  - the OTEC power plant's turbine halls, moonpools and ammonia tanks along the north wall;
  - industry, with process units, a tank farm, a desalination plant and eight stacks of 70 to 95 m;
  - twenty-three glass towers of 85 to 205 m;
  - a canal lined with temple fronts and cypresses, a domed rotunda and two parterres;
  - Greek temples with real columns, steps and pediments, a round tholos, cypress groves and a fountain plaza;
  - two data halls, a mining yard, a medical block and a colonnaded hall along the south wall.
- The hull floats. It rides on the waves under it, held by sixteen mooring lines, pushed by the wind and a current. The motion panel shows its heave, roll, pitch and offset.
- The power runs. OTEC, or nuclear modules, or both, feed the data halls, desalination, industry and the miners, and the dispatcher sheds load when the power falls short. The power button also runs the report's 10 MW demonstration plant.
- Damage control: the stats panel can hole cell A1, the south-west corner cell, and pump ballast to the opposite corner to level the hull again.
- Two cutaway views:
  - **Section** (V): the hull cut open across, with its concrete walls, bulkheads every 50 m, the ballast water in blue, a flooded cell in red and the waterline.
  - **Under the sea** (U): swim under the hull to see its bottom, the four cold-water pipes hanging 1,000 m down and the sixteen mooring lines.
- The flow overlay (L) draws the plant's warm and cold water, its discharge at −60 m and a power line to each load, with their flows.
- The section also draws the hull's bending, 1,000 times exaggerated: the deep hull is stiff, and in a gale its ends flex 1 to 1.5 cm.
- Two more layouts (K, or `?layout=dome` and `?layout=cluster`):
  - **Dome**: one Silone dome, 122 m across, with five planted terraces and a glass lantern. It heaves and rolls with the swell: its roll period falls inside the band of common wave periods.
  - **Cluster**: seven domes joined by twelve bridges with sliding joints. Each bridge's card gives its slide and hinge; in a 5 m swell the joints run into their stops.
  - With nuclear or hybrid power, a 160 m powership is moored 300 m off with a cable, because the modules need a longer hull than a dome.
- Click anything for a card: what it is, how it is built, and its live numbers. Click the sea for the sea state.
- Five sites (Guam, Puerto Rico, Kona, the equator, California) and six sea states, from calm to a hurricane. The sun follows the site, the month and the hour.
- Three ways to move:
  - **Orbit**: drag or the arrow keys turn the view, the wheel or R and F zoom, W, A, S and D move.
  - **Fly** (F): W, A, S and D along the view, Space or E to rise, Shift or Q to drop, the arrow keys to turn, the wheel to set the speed.
  - **Walk** (G): GameBuilder's character controller on the deck, which moves with the hull. Space jumps; Space twice flies.
- Keys 1 to 9 travel to stations around the platform. N and M change the site and the sea, [ and ] the hour, P the power, K the layout, I the stats, V the section, U the underwater view, L the flow overlay, H hides the panels, ? shows the help.
- Address options:
  - `?site=` and `?sea=` pick a site and a sea state; `?wind=` sets the wind in metres a second.
  - `?hour=`, `?month=` and `?rate=` set the clock and how fast it runs.
  - `?layout=dome` or `cluster` picks a layout.
  - `?power=otec`, `nuclear` or `hybrid`; `?plant=demo10`; `?otec=` scales the OTEC plant; `?modules=` and `?swac=1`.
  - `?mode=fly` or `walk` starts moving; `?view=section` or `under` opens a cutaway; `?hole=1` holes cell A1; `?flow=1` shows the flow overlay.
  - `?t=` holds one moment.
  - `?yaw=`, `?pitch=`, `?dist=`, `?x=`, `?y=` and `?z=` place the view.
  - `?hud=0` hides the panels.
  - `?tier=phone` draws the phone's waves on any screen.
- Without WebGPU (a current Chrome, Edge or Safari), the page shows only its name and a link here.

The physics and power are engineering models, not measurements. No OTEC plant has delivered more than about 0.1 MW net. The cluster's domes are drawn level: their heave and drift show, their small tilts do not. The powership is drawn at rest, and its cable above the water.

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
