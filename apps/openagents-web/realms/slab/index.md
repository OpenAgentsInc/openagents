# SLAB Simulation

SLAB is a floating platform for heavy industry that Palmer Luckey says he has been developing: a seastead with a 1,000-year design life. He has shown one AI-generated concept picture and published no design.

This world is our reconstruction of it. We read the sizes from the picture, worked out what would have to be true for it to float, and built it on an open ocean you can walk around. Every size and system here is our assumption, and the physics and power are engineering models, not measurements.

Open the world: https://openagents.com/worlds/slab

It needs WebGPU: a current Chrome, Edge or Safari. Without it, this page is the tour.

## Pictures

Five stills from the world as it is today.

- The whole platform from the north-east, the view of the concept picture: https://openagents.com/worlds/slab/pictures/platform.jpg
- A person on the deck, with the towers behind: https://openagents.com/worlds/slab/pictures/on-deck.jpg
- The temples, the round tholos and the cypress gardens: https://openagents.com/worlds/slab/pictures/temples.jpg
- The hull cut open, with its cells, the waterline and the cold-water pipes going down: https://openagents.com/worlds/slab/pictures/section.jpg
- Half built: 60 of the 120 hull units joined at the assembly berth: https://openagents.com/worlds/slab/pictures/build.jpg

## How big it is

The deck is 1,000 m long and 600 m wide: 60 hectares, a twelve-minute walk end to end. The wall stands 118 m out of the water, as tall as a 35-storey building, and another 25 m is under it. On top are 23 glass towers of 85 to 205 m, eight stacks, a canal, Greek temples and gardens. The wall height comes from fitting a camera to the concept picture. The length and width are our planning values.

## Why it floats

Concrete sinks, but a hollow concrete box floats, as a steel ship does. It settles until the water it pushes aside weighs as much as it does. For this hull that is 25 m down: a block of seawater 1,000 by 600 by 25 m, about 15.4 million tonnes. So that is what the hull, the city and the ballast water together weigh.

The part that floats is the lowest 50 m: a hull whose inside walls, every 50 m, divide it into cells. Above it, columns carry the deck up to the height of the wall in the picture. Some cells hold ballast water, which pumps move from cell to cell to keep the deck level. Sixteen mooring lines hold it in place.

It is so wide and heavy that waves barely move it. In the rough 4 m sea the world opens on, the model lifts and drops the deck by a few centimetres. You can hole a corner cell, watch the hull lean, and pump ballast to level it again.

The hull changes the sea round it. Waves that meet the east and north walls are sent back, so the water at those walls stands about twice as high as in the open sea. Where a breaking crest meets a wall it throws spray. In the 4 m sea the world opens on, that spray is a few metres high, small against a 118 m wall. In the gale it reaches about 29 m. The concept picture's plumes, about 36 m, need a sea between the gale and the hurricane. West of the hull the sea is calm: under a third of the open sea's height close to the wall, with no whitecaps. Behind the hull the current leaves a strip of slack water with a ragged line of foam along each edge. None of this is painted: turn the wind up and it changes.

## How it is built

Nothing this wide fits through a ship channel, so in our scenario it is built in 120 pieces, each 100 by 50 m.

1. A yard at Corpus Christi, Texas casts one piece every eight days. Each starts as an 8 m shell that floats at the quay while crews raise its walls.
2. Two tugs tow each piece 48 nautical miles to a berth in 40 m of water in the Gulf of Mexico.
3. Crews join the pieces afloat, one seam at a time. The 120th joins on day 983.
4. A freight barge from Brownsville brings about 26,800 tonnes a trip, 200 trips in all. The first 107 bring concrete: crews thicken the sea walls and raise the deck on columns, from 50 m to 143 m over the keel. The rest bring the towers and plant. Ballast brings the hull down to its 25 m.
5. The deck is finished on day 1,783, a little under five years in.

Watch it in 90 seconds: https://openagents.com/worlds/slab?view=build

The yards, dates, tugs and berth are a scenario we assumed. The numbers behind it are further down this page.

## Where it could go

The platform's own power plant decides. An ocean thermal plant (OTEC) makes electricity from the difference between warm water at the surface and cold water piped up from 1,000 m down. It needs about 20 °C of difference all year, which only tropical seas have.

- Off Guam, where the world opens, the surface stays at 27.7 to 29.2 °C and the model's plant makes 95 to 116 MW through the year.
- Off California the plant makes nothing: the water three miles out is 607 m deep, too shallow for pipes that hang 1,000 m, and too cold in winter.

The platform's fixed loads (data halls, desalination and industry) need 163 MW, more than the ocean plant alone gives even off Guam. So the world opens with two small nuclear modules beside it, and together they carry the loads. No OTEC plant has yet delivered more than about 0.1 MW net, so treat every power figure here as a model.

Each site is a real place on a chart, with its depth, its distance to land and the legal lines round it. Under **Change**, **What would it take?** lets you change the hull, the load, the place and the power, and says what follows. Every setting has an address you can share:

- The Gulf Coast pilot, one caisson that fits the ship channel: https://openagents.com/worlds/slab?preset=pilot
- A hull too small for its load, sinking to its deck: https://openagents.com/worlds/slab?long=400&wide=300&weight=6
- Three miles off California, where the ocean thermal plant stops: https://openagents.com/worlds/slab?preset=california

Try the other sites: Puerto Rico, Kona in Hawaii, the equator, California, the Gulf of Mexico off Texas and a Norwegian fjord. For example https://openagents.com/worlds/slab?site=california

## In the world

The first time you open it, a guided tour of seven stops takes about two minutes: the picture it comes from, how big it is, why it floats, what holds it, what runs on it, how it could be built and where it could go. Each stop is one sentence and one number. Tap or press Next to go on, Back to go back, and "Leave the tour" at any stop. After that the world opens in **Explore**, and **Tour** in the bar plays it again.

The names of the districts stand over the deck while you are high above it. Press one in a box, such as "Tower district", and the camera goes there. **Look** lists nine places to go, and **Build it** plays the construction.

- The `prime` layout, built to match the concept image: a concrete platform 1,000 m long and 600 m wide, with sharp corners. A watertight hull 50 m deep floats it, and a lighter structure of columns on that hull puts the deck 143 m over the keel. It floats with 25 m under the water and its wall 118 m above it, as in the picture. It opens on the whole platform from the picture's side, at its golden hour.
- The deck's districts, from west to east:
  - the OTEC power plant's turbine halls, moonpools and ammonia tanks along the north wall;
  - industry, with process units, a tank farm, a desalination plant and eight stacks of 70 to 95 m;
  - twenty-three glass towers of 85 to 205 m;
  - a canal lined with temple fronts and cypresses, a domed rotunda and two parterres;
  - Greek temples with real columns, steps and pediments, a round tholos, cypress groves and a fountain plaza;
  - two data halls, a mining yard, a medical block and a colonnaded hall along the south wall.
- The hull floats. It rides on the waves under it, held by sixteen mooring lines, pushed by the wind, a current and the slow push of the waves. It turns against its lines as well as moving off. **See inside**, **What it is doing now** gives its rise and fall, its lean and how far it is pushed off its place.
- The mooring lines have their own shape and a seabed. In deep water (400 m or more) they are taut rope lines from the keel to anchors about 2 km out. In shallow water (Texas, 89 m; the Norwegian fjord, 134 m) they are heavy chains from the waterline that lie along the seabed. **See inside**, **What holds it in place** says which, how hard the hardest-pulled line is pulled, and has **Lose a line**: the most loaded line breaks, the platform moves off and turns over several minutes, and its neighbours take the load. `?lose=N` opens the world with N lines already broken.
- A hurricane is a storm warning, so 20 storm lines in 12 legs, square to the sides, are made fast, as they are during the build. With them the platform holds the hurricane from every side. Off Texas, on chain, single waves still pull the hardest line past its safe load, and losing a line there can make another part. **Let the storm lines go** shows the sixteen alone: beam on to a hurricane they part one after another.
- What this is not: the world does not work out how the platform changes the waves round it, beyond the white water at its walls and the calm behind it, or the forces a full hydrodynamic analysis gives for each wave frequency. The water the hull carries with it and its damping are stand-ins.
- The power runs. The world opens on both: a 120 MW ocean thermal (OTEC) plant and two nuclear modules, which together carry the data halls, desalination, industry and most of the miners. Switch to OTEC alone (or open [`?power=otec`](/worlds/slab?power=otec)) and the dispatcher sheds load: the stacks stop smoking and the data halls go to backup. **Change**, **Power source** also runs four modules alone, and the report's 10 MW demonstration plant.
- Damage control: **Change**, **Break something** can hole cell A1, the south-west corner cell, and pump ballast to the opposite corner to level the hull again.
- Two cutaway views:
  - **Section** (V): the hull cut open across, with its concrete walls, bulkheads every 50 m, the ballast water in blue, a flooded cell in red, the waterline, and the columns that carry the deck above the 50 m hull. Its parts are named on the cut.
  - **Under the sea** (U): swim under the hull to see its bottom, the four cold-water pipes hanging 1,000 m down and the mooring lines as the mooring model has them now. A broken line lies on the seabed.
- The flow overlay (L) draws the plant's warm and cold water, its used water going out 60 m down and a power line to each load, each named with its megawatts.
- The section also draws the hull's bending, 100 times exaggerated: in a gale the ends of the 50 m hull flex 8 to 15 cm.
- Two more layouts (K, or `?layout=dome` and `?layout=cluster`):
  - **Dome**: one Silone dome, 122 m across, with five planted terraces and a glass lantern. It heaves and rolls with the swell: its roll period falls inside the band of common wave periods.
  - **Cluster**: seven domes joined by twelve bridges with sliding joints. Each bridge's card gives its slide and hinge; in a 5 m swell the joints run into their stops.
  - With nuclear or hybrid power, a 160 m powership is moored 300 m off with a cable, because the modules need a longer hull than a dome.
- Click anything for a card. Every card says what the thing is, why it is here, three numbers you can picture (each marked From Luckey, From a source or Our assumption), what it is doing now, and the maths if you ask for it. Click the sea for the sea's card.
- Seven sites (Guam, Puerto Rico, Kona, the equator, California, the Gulf of Mexico off Texas and a Norwegian fjord) and six sea states, from calm to a hurricane. The sun follows the site, the month and the hour.
- Three ways to move:
  - **Orbit**: drag or the arrow keys turn the view, the wheel, a pinch or R and F zoom, W, A, S and D move.
  - **Fly** (F): W, A, S and D along the view, Space or E to rise, Shift or Q to drop, the arrow keys to turn, the wheel to set the speed.
  - **Walk** (G): GameBuilder's character controller on the deck, which moves with the hull. Space jumps; Space twice flies.
- The controls are a bar at the top, in plain words, with a sheet under it (at the foot of a phone):
  - **Tour**, **Explore** and **Build it** are the three ways to be in the world.
  - **Look**: orbit, walk or fly, closer and farther, and a list of nine places to go.
  - **See inside**: cut it open, go underwater, water and power, and what the platform is doing now.
  - **Change**: where it floats, the sea, the hour and month, the power source, the shape, and things to break.
  - **Help**: how to get around, the keys, and links to the research.
- Keys 1 to 9 travel to nine places around the platform: the temples, the power plant, the data halls, the mining yard, the towers, the fountain, the bow, the picture's view and a view from 1.5 km. [ and ] change the hour, V opens the section, U the underwater view, L the flow overlay. N, M, P and K open **Change**, I opens **See inside**, ? opens **Help**, and H hides the controls.
- Address options:
  - `?site=guam`, `puerto-rico`, `kona`, `equator`, `california`, `texas` or `fjord` picks a site and `?sea=` a sea state; `?wind=` sets the wind in metres a second and `?winddir=` the bearing it comes from in degrees (225 is Guam's wet-season south-westerly).
  - `?hour=`, `?month=` and `?rate=` set the clock and how fast it runs.
  - `?layout=dome` or `cluster` picks a layout.
  - `?power=hybrid` (the default), `otec` or `nuclear`; `?plant=demo10`; `?otec=` scales the OTEC plant; `?modules=` and `?swac=1`.
  - `?mode=fly` or `walk` starts moving; `?view=section` or `under` opens a cutaway; `?hole=1` holes cell A1; `?flow=1` shows the flow overlay.
  - `?t=` holds one moment.
  - `?yaw=`, `?pitch=`, `?dist=`, `?x=`, `?y=` and `?z=` place the view.
  - `?card=hull` opens on a card (`sea`, `hull`, `dock`, `pipe`, `plant`, `data`, `mining`, `tower`, `temple` and the rest: the spec lists them).
  - `?hud=0` hides the panels; `?tour=0` skips the tour, and `?tour=1` to `?tour=8` opens it at that stop (`?intro=0` and `?intro=1` still work: no tour, and the tour from its start).
  - `?tier=phone` draws the phone's waves on any screen.
- Without WebGPU, the page shows only its name and a link here.

The cluster's domes are drawn level: their heave and drift show, their small tilts do not. The powership is drawn at rest, and its cable above the water.

## The build, step by step

[Play the construction timelapse](/worlds/slab?view=build). In the world, open **Build it** or press **B**. Drag **Where in the film** in either direction. Space plays or pauses, the arrow keys step through the film, and Escape returns to the original camera. **The first tow**, **The first join** and **Fitting out** jump to the first operation of each kind. Drag the scene to orbit and use the wheel to zoom. A link with `?view=build&day=983.8` pauses on that project day.

| Follow the work | What to watch |
| --- | --- |
| [Corpus Christi casting yard](/worlds/slab?view=build&day=14) | An 8 m starter shell floats at the quay. As crews raise the walls, place the deck and load low ballast, the caisson draws more water. |
| [First Gulf tow](/worlds/slab?view=build&day=30.28) | Two tugs take one 100 × 50 m caisson through an assumed 8 nautical miles of channel at 3 knots and 40 offshore at 4 knots. A 20 m/s headwind holds the first departure for two days. |
| [Assembly berth](/worlds/slab?view=build&day=503.8) | Tugs warp each caisson along an open row. The first caisson takes the moorings. A work barge attends each subsequent joint. Only a completed connection adds that unit to the joined displacement. The 120th joins on day 983. |
| [Brownsville freight run](/worlds/slab?view=build&day=984.6) | A separate 120 × 32 m lighter brings about 26,800 t per call, 200 calls in all. The first 107 land concrete for the lining of the sea walls and for the structure that raises the deck from 50 m to 143 m over the keel. The rest land the buildings and liquids. The route assumes 15 nautical miles of channel and 130 offshore. Watch the lighter rise as cargo transfers to the hull, its towers go up and ballast brings it to operating draft. |
| [Commissioned deck](/worlds/slab?view=build&day=1783) | At 25 m draft the joined hull displaces 15.375 million metric tonnes. The sequence ends at the original skyline. The 40 m berth rules out the main world's kilometre-long OTEC pipes. |

**What the numbers mean.** The project assumes parallel casting positions producing one closed unit every eight days. Each needs about 22,372 m³ of reinforced concrete at 2.5 t/m³, including the separate end walls needed during tow. With 3,000 t of low ballast the tow weighs about 58,931 t, draws 11.50 m in seawater and has an initial transverse metacentric height of 2.48 m under the assumed mass distribution. At 4 knots in a 10 m/s headwind, the steady drag estimate including a 30% margin calls for 1.73 MN of the assumed 2 MN available underway. At 20 m/s it exceeds that allowance. The port’s published channel is 530 ft wide and 54 ft deep. The assumed screening margins are 10 m on each side and 2 m below the keel. The 600 m assembled beam excludes a tow of the finished city through that channel. [Corpus Christi channel dimensions](https://portofcc.com/wp-content/uploads/PCCA-Press-Release-CIP-Completion-vf.pdf) · [Brownsville port directory](https://www.portofbrownsville.com/wp-content/uploads/2026/01/Port-of-Brownsville-Directory-2025-Web-V1.pdf)

**How to read the time and sea.** The 90-second playback slows down for the first tow, join and freight call. The day counter follows project time. The distance strip uses nautical miles and the 3D view compresses the space between ports and berth. Draft follows mass and immersed volume, assuming ballast is distributed to hold an even keel during outfitting. The moving hulls follow the mean sampled wave elevation beneath their waterplanes. The mass transfer, static initial stability and steady tow resistance are planning calculations. The scenario assumes a 40 m Gulf berth and prescribed tow paths. A real operation would need surveys, permission, tug performance curves, six-degree-of-freedom seakeeping, damaged stability, dynamic line loads, joint strength and weather-window analysis. The dates, yards, freight, ballast schedule and berth represent a construction scenario. [SLAB feasibility research](https://github.com/OpenAgentsInc/openagents/blob/main/docs/research/ocean/slab-image-feasibility.md)

## How it is made

- `metaverse/slab-plan.coax` is the platform as data: the hull's pieces and the masses on its deck, with its checks.
- `metaverse/slab-sim.coax` is the physics and the power: the sites, sea states, hull motion, moorings and dispatch, with the spec's tests.
- `apps/openagents-web/slab.coax` is the world.
- `apps/openagents-web/slab-hud.coax` is what its controls hold. The browser draws them from the components kit (`scene-pane.coax`, `components/kit/scene.coax`).
- `slab-face.c` binds it for the browser, and `slab.wgsl` (from `gpu/slab.coax`) draws it.
- `docs/research/ocean/` in the OpenAgents repository holds the research:
  - `slab-world-spec.md` is the spec this world is built from;
  - `slab.md` and `otec.md` are the reports behind it.

## Make your own world

An agent can build one from a shallow clone of the repository and open a pull request: [metaverse/SKILL.md](https://github.com/OpenAgentsInc/openagents/blob/main/metaverse/SKILL.md).
