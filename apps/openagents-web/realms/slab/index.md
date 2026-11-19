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

Nothing this wide fits through a ship channel, so in our scenario it is built from 120 caissons: hollow concrete boxes 100 by 50 m and 50 m deep that float.

1. A yard at Ingleside, Texas, on the Corpus Christi ship channel, casts them in six dry cells. Each cell floods to float its caisson out.
2. Two caissons are joined in the yard's basin, and tugs tow each pair down the channel, past the Aransas Pass jetties and 40 nautical miles out to a berth in 89 m of water.
3. The first pair is moored there. Every other pair is joined to the hull at sea, in a calm, one joint at a time. The hull is closed on day 1,177.
4. On each caisson, columns and walls climb from 50 m to 143 m over the keel and carry the deck. Barges from a wharf at Ingleside bring the city's buildings, and gantries lift them on.
5. Ballast water brings the hull down to its 25 m draft. It is commissioned on day 1,510, about four years in.

Watch it: https://openagents.com/worlds/slab?view=build

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

The days to build under **Building it** are one run of our build simulation for the hull you set: the yard, the tows, the joints at sea and the weather.

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
- The deck is lived in. People walk the promenades and the sidewalks and stand at the parapet; trucks, carts and forklifts drive the streets and yards and stop at each crossing; a helicopter lifts off the hospital's roof, flies out and comes back; seabirds circle the corners; flags fly with the wind. A red supply ship goes round the platform to the dock in the sheltered south wall, lies at the landing stage for four minutes and leaves; a tug patrols off the north wall and a bulk carrier lies far off. The ships ride the waves under them and leave a wake when they move. After sunset the ships show red and green sidelights and white masthead lights, and the vehicles and the helicopter show their lights. Marble statues three times life size stand on the promenade. Click a ship, a vehicle or the helicopter for its card. In walk mode with the photoreal look (`?style=photoreal`) you are one of the people. A phone draws fewer of them.
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

[Watch the build](/worlds/slab?view=build). In the world, open **Build it** or press **B**. What you see is one run of our build simulation, drawn day by day: every caisson, tug, barge and line is where the simulation has it, at its own draft. The camera follows a film plan, about three and a half minutes from the empty yard to the commissioned platform. Drag **Where in the build** to any day, pick a chapter, or change the speed. Space plays or pauses, the arrow keys step five seconds of film, and Escape returns you to the platform at its berth. Drag the scene to take the camera, and **Back to the film** gives it back. Click a caisson, a tug or a barge for its weight, its draft, its GM (its margin against rolling over) and what its towline pulls. A link with `?view=build&day=600` pauses on that day.

| Follow the work | What to watch |
| --- | --- |
| [The casting yard](/worlds/slab?view=build&day=20) | Six dry cells at Ingleside. In each, the bottom slab is poured over its steel, the walls climb in a slipform, and the top slab closes the box. |
| [Floating out](/worlds/slab?view=build&day=50) | The cell floods to the sea's level, its gate opens, and the caisson is warped out into the basin. |
| [The first tow](/worlds/slab?view=build&day=59.5) | Two caissons joined in the basin leave as one tow, down the channel and past the jetties. |
| [The first joint at sea](/worlds/slab?view=build&day=70) | The second pair waits at stand-by for a calm, then is pulled in, sealed and stressed to the first. |
| [The deck climbs](/worlds/slab?view=build&day=330) | Columns and walls climb on the joined caissons while new pairs arrive. |
| [The city goes up](/worlds/slab?view=build&day=900) | Barges bring the city's buildings, and gantries on the hull's edge lift them on. |
| [Commissioned](/worlds/slab?view=build&day=1510) | At its 25 m draft, with its city standing. |

**What is worked out and what is placed.** Each day's weather, each tow's forces and draft, each joint's wait for a calm, each pour and each lift come from the simulation ([how it works](https://github.com/OpenAgentsInc/openagents/blob/main/docs/research/ocean/slab-construction-method.md)). Moves inside the yard, a pair's approach from stand-by to its joint, and tugs and barges coming back are placed along paths, not worked out from forces. The ballast's free surface is not modelled. The land round the yard is schematic, and the lines are drawn thicker than they are so they can be seen. The dates, the yard, the berth and the method are a scenario we assumed.

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
