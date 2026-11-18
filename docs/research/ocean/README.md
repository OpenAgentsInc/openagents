# Ocean research: SLAB and offshore industry

This folder looks at building industry on the ocean, starting from SLAB.
SLAB is Palmer Luckey's proposed floating platform for American industry,
announced on November 17, 2026. The folder holds:
- the announcement and a reply that proposes a design;
- research reports on SLAB and on powering it with ocean thermal energy
  conversion (OTEC);
- a spec for rebuilding SLAB as a simulated world in the OpenAgents
  Metaverse Engine.

SLAB has published no design: no dimensions, site, materials or power
system. The reports keep apart what Luckey said, what independent evidence
shows, and their own analysis. The world spec marks which of its numbers
are sourced and which are its own assumptions. Everything here is current
as of November 18, 2026.

## Reading order

1. [palmer-luckey-post.md](palmer-luckey-post.md): what SLAB claims to be.
2. [slab.md](slab.md): whether it holds up.
3. [silone-dome.md](silone-dome.md): one outside design proposal.
4. [otec.md](otec.md): whether OTEC can power it, and where.
5. [slab-world-spec.md](slab-world-spec.md): turning all of it into a
   simulated world.

## The documents

### [palmer-luckey-post.md](palmer-luckey-post.md)

**What it is:** Luckey's announcement on X, word for word.
- SLAB is a seastead with a 1,000-year design life.
- It is meant to be cheaper than land and free of state and local rules.
- It targets metal processing, medicine, desalination, refining and
  chemicals.
- It was funded at a $1B valuation in 2017.
- Luckey objects to it being called a floating AI data center.

**Includes:** the concept image, [slab.jpeg](slab.jpeg), which was labelled
AI-generated on X.

### [slab.md](slab.md)

**What it is:** the first research assessment. Its full PDF is
[SLAB_Research_Report_2026-11-18.pdf](SLAB_Research_Report_2026-11-18.pdf)
(15 pages, 30 references).

**What it covers:**
- What has actually been disclosed. A $1B valuation is not $1B raised.
- Engineering precedents: Shell's Prelude floating gas plant, Japan's
  Mega-Float and Monaco's concrete floating dock.
- The 1,000-year lifespan claim.
- Maritime law: state waters versus the U.S. territorial sea, and whether
  SLAB counts as a vessel or a fixed installation.
- Cost against land.
- AI data centers at sea.

**Conclusion:** the engineering category is credible. SLAB's claims about
cost, lifespan and regulatory relief are unproven, and power is the biggest
open question.

### [silone-dome.md](silone-dome.md)

**What it is:** Ben Silone's reply in the announcement thread. He proposes
400 ft floating concrete domes, built with the inflatable-form and sprayed
concrete method Monolithic Dome has used on land for 50 years.

**Includes:**
- His post, word for word.
- A shot-by-shot analysis of his 46-second render, with the video itself as
  [silone-dome.mp4](silone-dome.mp4).

**Conclusion:** the render shows homes and offices on calm water. It doesn't
show the construction method, behaviour in waves, or domes joined together.
Silone isn't known to be part of SLAB.

### [otec.md](otec.md)

**What it is:** the follow-up research on whether OTEC can power SLAB, with
a focus on data centers and Bitcoin mining. Its full PDF is
[OTEC_for_SLAB_Research_Report_2026-11-18.pdf](OTEC_for_SLAB_Research_Report_2026-11-18.pdf)
(33 pages, 87 references).

**What it covers:**
- How OTEC works.
- Every plant built or planned.
- Why the cold-water pipe is the bottleneck.
- Cost estimates by plant size.
- A ranked site atlas.
- Data-center cooling.
- Mining economics.
- The 1980 OTEC Act.

**Conclusions:**
- OTEC is credible in the tropics but unproven at 120 MW. The largest
  verified net output is about 103 kW.
- California is ruled out: the water is too cold.
- The best U.S. sites are Guam, Puerto Rico and Kona.
- Mining earns about $79/MWh, so it suits leftover power, not anchoring the
  plant.
- The recommended path runs from a Kona test, to a 5–10 MW plant in Guam,
  and grows only on measured results.

### [slab-world-spec.md](slab-world-spec.md)

**What it is:** the brief for a coding agent to build `/realms/slab`, an
unlisted world in the OpenAgents Metaverse Engine.

**What it specifies:**
- **Three layouts:**
  - the monolith from the concept image: 1,000 × 600 m, 15.4 Mt;
  - a single Silone dome;
  - a cluster of seven joined domes.
- **Physics:** hull buoyancy sampled across 6,000 columns of the hull, so
  wave forces cancel along it in the simulation itself; mooring lines,
  ballast and flooding.
- **Sites and seas:** five sites with sourced or planning water
  temperatures, and six sea states up to a typhoon.
- **On board:** the OTEC plant, data halls, mining and desalination, using
  the figures in otec.md.
- **For the build:** deck layout, controls, 12 tests and seven build phases.

**Status:** nothing in it is built yet.

## Files without their own doc

| File | What it is | Described in |
|---|---|---|
| [slab.jpeg](slab.jpeg) | SLAB's AI-generated concept image | palmer-luckey-post.md |
| [silone-dome.mp4](silone-dome.mp4) | Ben Silone's dome render (46 s, no sound) | silone-dome.md |
| [SLAB_Research_Report_2026-11-18.pdf](SLAB_Research_Report_2026-11-18.pdf) | Full SLAB research report | slab.md |
| [OTEC_for_SLAB_Research_Report_2026-11-18.pdf](OTEC_for_SLAB_Research_Report_2026-11-18.pdf) | Full OTEC research report | otec.md |
