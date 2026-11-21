# Ocean Atomics' marine reactor: video analysis

Source: a 78-second explainer video from Nick Touran, shared on X from his
account (`whatisnuclear`). The file is
`ocean-atomics-reactor.mp4` (archived out of git under `~/.openagents/archive/docs/research/ocean/ocean-atomics-reactor.mp4`), originally named
`whatisnuclear-2106132133043650587.mp4`.

**Who.** Nick Touran is a nuclear engineer and reactor designer (Ph.D.,
P.E.) who runs whatisnuclear.com. His X profile lists him as CTO at Ocean
Atomics.

**The company.** Public information on Ocean Atomics is thin. A job
listing describes it as an American company designing, licensing and
assembling a standardized nuclear power plant for maritime use
([startup.jobs](https://startup.jobs/marine-engineer-ocean-atomics-8728073)).
A job-aggregator profile says it was founded in 2025 in Bethesda, Maryland,
and calls Touran its chief nuclear officer, not CTO
([Simplify](https://simplify.jobs/c/Ocean-Atomics)). The company's own
website (oceanatomics.com, shown at the end of the video) was not reviewed
for this doc.

Everything below about the reactor comes from the video. None of it has
been checked against the company's filings, papers or licensing
documents.

## The video

- **Format:** 78.1 seconds, 1920×1080 at 60 fps (4,681 frames).
- **Audio:** music only. A Whisper transcription (`medium.en`) found no
  speech, and the track goes silent from 74.8 s.
- **Style:** an animated technical illustration in thin blue lines on a dark
  blueprint grid, in the company's brand.
  - Captions sit bottom-left: a numbered category in orange, a large title,
    and one line of description.
  - From 9 s, an inset of the reactor compartment stays in the top-right
    corner.
  - From 31 s, a "▶▶ Speeding up" marker appears top-left, and it becomes
    "▶▶▶ Faster" at 52 s as the scenes shorten.

**Method:** one frame per second across the whole video, full-resolution
crops of every label and caption, and a transcription of the audio. Every
caption below is copied from the screen.

### Shot list

| Time | Category | Title | Caption on screen | What it shows |
|---|---|---|---|---|
| 0–8 s | Ocean Atomics Reactor | **One reactor, one compartment** | "A standardized 50 MWe integral pressurized-water reactor, designed to drop into any large hull, 500 ft and up." | A deck line drops into place, the compartment builds around the reactor, then the labels appear (see below) |
| 9–19 s | 01 Stationary power | **Powership** | "Moored hull exports 50 MWe to the shore grid, enough for roughly 40,000 homes." | The compartment slides into a hull at a quay. A power line runs from the hull to pylons and lit city blocks ashore |
| 20–30 s | 02 Propulsion | **Container ship** | "Same compartment, now driving the shaft. Years between refuellings, no bunkering." | A full container ship. A crane lifts boxes on and off. The compartment sits amidships, low in the hull |
| 31–36 s | 03 Propulsion | **Ro-ro car carrier** | "Vehicles roll aboard over the stern ramp. The power plant is unchanged." | A car carrier with cars rolling up the stern ramp |
| 37–41 s | 04 Propulsion | **Bulk carrier** | "Grain, ore and coal trades at constant speed, with no fuel cost exposure." | The compartment alone, then a bulk carrier being loaded through a chute |
| 42–47 s | 05 Hotel and propulsion | **Passenger ship** | "Thousands of cabins and full propulsion from a single 50 MWe module." | A cruise ship with rows of lit cabins |
| 48–51 s | 06 Stationary compute | **Floating data center** | "50 MW of steady power for compute, cooled by seawater, linked to shore by fibre." | A moored hull with six lit server halls on deck. Dashed lines run down into the sea (seawater cooling), and a dashed fibre cable runs along the seabed to a building ashore |
| 52–53 s | Future hulls | **Icebreaker** | "Arctic escort with months of endurance." | An icebreaker among ice floes |
| 54 s | Future hulls | **LNG carrier** | "Cargo delivered without burning it." | Five spherical tanks |
| 56 s | Future hulls | **Product tanker** | "Long-haul liquid cargo at steady speed." | A tanker |
| 57 s | Future hulls | **Desalination ship** | "Fresh water for a coastal city, made on site." | A hull piping water ashore |
| 58 s | Future hulls | **Hydrogen and e-fuel plant** | "Electrolysis and synthesis at sea, shipped out by tanker." | Process blocks and spherical tanks on a hull |
| 59 s | Future hulls | **Offshore charging hub** | "Recharging electric ferries and workboats from one hull." | Small boats plugged in on either side of a hull |
| 60–69 s | Ocean Atomics Reactor | **One design, many ships** | "One licence, one supply chain and one fuel cycle across every application. Scale nuclear energy. Power generations." | A montage of the earlier hulls, each with the same compartment, ending on the compartment alone at the waterline. "oceanatomics.com" appears bottom-right |
| 70–78 s | — | **Ocean Atomics** | "Scale nuclear energy. Power generations." and "oceanatomics.com" | End card over a dark sea |

### The reactor, as labelled (0–8 s)

| Label | Text on screen |
|---|---|
| Ocean Atomics Reactor | 50 MWe integral PWR |
| Standard compartment | 18 m long, 12.5 m high, same in every hull |
| Reactor core | ≈160 MWt for 50 MWe |
| Control rod drives | Top-mounted |
| Steam generators | Inside the reactor vessel |
| Gas pressurizer | Separate vessel inside containment |
| Canned circulation pumps | Mounted directly on the vessel |
| Steam out, feedwater in | To the turbine room aft |
| Steel containment | — |
| Iron–water shielding tank | Surrounds the containment |

**What the labels imply.** These are our arithmetic, not the company's
claims.
- **Efficiency.** 50 MWe from 160 MWt is about 31% thermal-to-electric,
  typical of a pressurized-water reactor with a steam turbine.
- **Waste heat.** Each module rejects about 110 MW of heat. At a 10 °C
  seawater temperature rise, its condenser needs about **2.7 m³/s** of
  seawater.
- **Compartment size.** The compartment is 18 m long and 12.5 m high. Its
  width isn't labelled.
- **Turbine room.** The turbine and generator are in a separate room aft,
  so a whole plant takes more length than the 18 m compartment.
- **Minimum hull.** The minimum hull ("500 ft and up") is about 152 m.
- **Homes powered.** "40,000 homes" from 50 MWe works out to about
  1.25 kW per home, close to average U.S. household use.

**Design features named.** All the main primary-circuit parts sit in or on
one vessel: steam generators inside it, canned pumps mounted directly on it
(so the pumps have no shaft seals), and control rod drives on top. The
pressurizer is a separate gas-filled vessel. Containment is a steel shell
inside an iron-and-water shielding tank.

### What the video does not say

- **Unstated quantities:**
  - fuel type and enrichment;
  - refuelling interval (only "years between refuellings");
  - the compartment's width and mass;
  - cost or schedule;
  - licensing status;
  - whether 50 MWe is gross or net (after the plant's own use).
- **Missing systems:** the turbine room, shore connection details, crew
  numbers, emergency cooling and spent-fuel handling.
- **Missing from the data-center hull:** how many modules run it, and
  backup power.

## Relevance to SLAB

The [OTEC assessment](otec.md) found OTEC credible but unproven above about
0.1 MW net, limited to tropical sites, and needing large seawater systems.
For the SLAB world spec's 120 MW net plant, that means about 846 m³/s of
seawater through four 6.7 m pipes 1,000 m long. A compact marine reactor
changes several of those constraints. This is our first-pass comparison,
before any research.

| | OTEC (120 MW net, per otec.md) | Ocean Atomics-type modules |
|---|---|---|
| Power per unit | 120 MW net from one large plant | 50 MWe per module; 3–4 modules for SLAB's loads |
| Size | Plant spans 3 hull districts, plus 4 pipes of 6.7 m × 1,000 m | 18 × 12.5 m compartment plus a turbine room, per module |
| Seawater | About 846 m³/s, warm plus cold | About 2.7 m³/s per module for the condenser |
| Where it works | Tropics only; California ruled out | Anywhere, including cold water (which helps the condenser) |
| Proven at scale | No: largest verified net output is about 103 kW | Marine pressurized-water reactors are proven in naval and icebreaker service, and Russia's Akademik Lomonosov is a 70 MWe floating plant built from two 35 MWe units. This particular design is not proven |
| Weather | Output varies with sea temperature through the year | Steady output, except during refuelling and maintenance outages |
| Regulation | 1980 OTEC Act, with no current implementing regulations | Among the most heavily regulated industries. Expect nuclear licensing, flag-state and port-state approval, security and fuel controls. This works against SLAB's stated aim of escaping state and local rules |
| Coproducts | Cold water for cooling and desalination | Waste heat; process heat for desalination or hydrogen |

**As an alternative to OTEC.** Nuclear removes the tropical constraint and
the pipe risk, and makes the power steady. It is also the only one of the
two that works at the California site. The trade is regulation, security
and public acceptance.

**As a supplement to OTEC.** One or two modules provide firm power while an
OTEC plant is demonstrated and grown in steps, as the OTEC report
recommends. OTEC's cold water still serves data-center cooling and
desalination.

**Waste heat into OTEC.** Could reactor waste heat warm OTEC's warm side?
The OTEC report lists Taiwan's Ho-Ping project as "waste-heat-assisted".
At SLAB's scale the effect is small: a module's roughly 2.7 m³/s of warmer
condenser water, mixed into 564 m³/s of warm seawater, raises it by well
under 0.1 °C. A dedicated design would be needed for it to matter.

**Dome layout.** Silone's dome is 122 m (400 ft) across. That is smaller
than the "500 ft and up" hull the video says a module needs. The monolith
(1,000 m) has room for several modules.

The [SLAB world spec](slab-world-spec.md) now includes this reactor as a
power option (section 6.6). The research to firm up these numbers is still
to be done.

## Other floating nuclear projects in the news

These come from a quick search, not research:
- **Bluecore Energy** is developing a 10 MWe floating reactor barge at the
  Port of Long Beach, California, after a $50 million seed round
  ([gCaptain](https://gcaptain.com/bluecore-advances-floating-nuclear-power-project-at-port-of-long-beach/),
  [ESG Today](https://www.esgtoday.com/bluecore-energy-raises-50-million-to-build-floating-nuclear-power-plants/)).
- **Core Power** is working with Glosten on a floating nuclear plant for U.S.
  ports by the early 2030s.
- **ThorCon** is developing a 500 MW molten-salt floating plant
  ([E&E News](https://www.eenews.net/articles/coming-soonish-to-a-port-near-you-floating-nuclear-plants)).
- **On regulation:** POWER magazine covers how nuclear, maritime and port
  regulators overlap
  ([POWER](https://www.powermag.com/nuclear-at-sea-navigating-regulatory-confluence/)).
