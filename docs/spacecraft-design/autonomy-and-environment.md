# Autonomy, environment and operating rules

The other notes cover orbits, subsystems, assembly mechanics and the phased
network. This one covers three things they leave open, each of which shapes
how the robots work:

- how far away the ground is;
- what space does to the electronics;
- the rules for a station in low Earth orbit.

Marks follow the [README](README.md): [V] verified with a cited source,
[C] computed by us, [M] from memory (verify before it becomes a code constant
or a test limit), [P] a planning target.

## 1. The ground is too far away to fly the arms

**Light time.**
- Earth to the Moon is about 1.28 s one way, on the mean distance of
  384,400 km [C].
- Earth to the L2 point (about 448,900 km [V]) is about 1.50 s one way, so
  about 3 s round trip [C].
- Ground stations, relays and processing add to that, typically to several
  seconds [M].

**Coverage gaps.**
- A low Earth orbit station is out of contact with any one ground station
  for most of each ~90-minute orbit [M], unless it buys relay service.
- Cislunar contact depends on the antenna network and on the orbit's
  geometry; the [cislunar strategy](cislunar-strategy.md) plans for
  LunaNet-compatible service plus an independent command path.

**What this means for assembly.** Contact moves happen in tens to hundreds
of milliseconds [M]:
- a gripper closing;
- a peg meeting a socket;
- a latch seating;
- an arm's reaction kicking the station.

A human or a ground computer seconds away can't close those loops, so:

- **The robot closes every fast loop itself.** That covers force and torque
  limits, compliant contact, collision avoidance, and stopping when a reading
  is out of bounds. The ground and the planner never send joint commands in
  real time.
- **The ground supervises at the level of stages, not motions.** A person
  approves a plan, the robot runs it stage by stage, and each stage ends in a
  check the robot can compute from its own sensors. A failed check means
  step back, retry within limits, or make safe and wait. It never means push
  on.
- **Make safe is always available without the ground.** That means:
  - releasing or holding a part in a known state;
  - stopping all arms;
  - restoring the station's attitude;
  - going quiet.

  It must work through loss of contact, which the cislunar strategy already
  requires of every vehicle.

### Stage-checked assembly, the loop we should build

This is the "plan, simulate, check, act" loop from our
[Aether AI research](../research/aether-ai/README.md), applied to
construction:

1. **Plan** the assembly as discrete stages: which part, which arm, which
   port, in what order. A sequence is a typed document, versioned like our
   Lab documents, so it can be reviewed and compared.
2. **Simulate** each stage before it runs, in the same physics the
   [simulation roadmap](sim-mapping.md) builds. Check reach, joint and torque
   limits, collisions, the station's attitude impulse, centre-of-mass
   shift, and the loads on parts already latched.
3. **Check** each stage after it runs, against what the simulation predicted:
   - the part is latched;
   - the pose is within tolerance;
   - the load path holds under a small proof load;
   - the station's attitude is still inside its box.
4. **Act** on the result: continue, step back, retry, or make safe.
   Small decision models choose among these discrete options, and the
   simulator and sensors decide whether the choice worked. A model never
   outputs joint commands.

In Verse this is directly testable. Lagrange 2's factory can inject faults:
- a stuck latch;
- a part off by a few millimetres;
- a motor at its limit;
- a lost link to the ground.

A test then checks that the sequence recovers or makes safe, never that it
pushes through.

## 2. Radiation changes which computers can do the work

**Low Earth orbit sits inside Earth's magnetic field.** It is the
gentlest place to fly ordinary (commercial, non-hardened) electronics, apart
from passes through the South Atlantic Anomaly [M]. That is one more reason
the strategy starts there.

**Cislunar space and L2 are outside most of that protection.** Spacecraft
there see:
- galactic cosmic rays all the time;
- solar particle events, sometimes for hours to days;
- a large halo orbit periodically passes through Earth's magnetotail and
  magnetosheath.

Getting there crosses the radiation belts [V].
[NASA space environments overview](https://exoplanets.nasa.gov/system/internal_resources/details/original/983_isat_environments_miller_cleared.pdf)

**Effects that matter for a construction robot** [M]:
- **Total ionizing dose** builds up over the mission and slowly shifts a
  device's behaviour until it fails [V].
  [NASA radiation environment note](https://hawk.cfd.rit.edu/products/publications/IDTL/ngstradenv2.pdf)
  A shipyard meant to run for years has to budget it per box.
- **Single-event effects** come from one particle:
  - bit flips;
  - a processor hang;
  - occasionally a latch-up that damages a part unless power is cut fast.

  For an arm, this means a controller can reset in the middle of a move.

**Design consequences** [P]:
- Use hardened or radiation-tolerant controllers for the safety loop and
  make-safe.
- Use faster commercial compute only for planning and perception, behind
  that safety loop, with watchdogs and checked restarts.
- After any reset, the arm resumes from the last completed stage, never from
  the middle of a motion. The stage-checked loop above makes this natural:
  the last passed check is the restart point.
- Store the station's sequence and state redundantly, with checksums, and
  scrub memory.
- During a solar particle event, plan quiet periods: hold parts, park the
  arms and pause construction.

**Dose numbers for a specific orbit aren't settled here.** Our searches
didn't find Gateway-specific dose budgets. Before any number becomes a
design limit, take it from a mission environment specification or a
radiation model run for the chosen orbit.

## 3. Operating rules for the low Earth orbit station

- **Disposal within five years.** In 2022 the US FCC adopted a rule that
  satellites it licenses in low Earth orbit (below 2,000 km) must be
  disposed of as soon as practicable, and within five years of the end of
  the mission. It replaced the older 25-year guideline [V]. The station,
  every free-flying demonstrator, and any discarded hardware need a disposal
  plan that meets it.
  [Satellite Today](https://www.satellitetoday.com/government-military/2022/09/30/fcc-adopts-5-year-rule-for-deorbiting-satellites/),
  [Broadband Breakfast](https://broadbandbreakfast.com/as-leo-industry-grows-fcc-adopts-rule-to-limit-space-debris/)
- **Nothing floats free.** Every part, tool and package is held, latched or
  tethered at every stage. The assembly sequence must prove this: a stage
  that would leave an unrestrained object fails its check. The cislunar
  strategy makes the same point about cargo near a station.
- **Collision avoidance** with tracked debris and other spacecraft is a
  routine station manoeuvre [M]. It has to pause construction safely, so the
  sequence needs well-defined hold points.
- **Licensing** for spectrum, launch and operation, debris mitigation, and
  export rules starts in parallel with engineering, as the cislunar strategy
  says.

## 4. What this adds to the simulation

These become scenarios and tests next to the ones in
[sim-mapping.md](sim-mapping.md):

| Scenario | What it proves | Phase it serves |
| --- | --- | --- |
| `assembly_survives_link_loss` | With the ground link cut mid-sequence, the robot finishes the current stage or makes safe, and resumes on reconnect | All |
| `stuck_latch_steps_back` | A latch that doesn't seat triggers a step back and a bounded retry, then makes safe; it never forces | LEO station onward |
| `reset_resumes_from_last_check` | A controller reset mid-motion resumes from the last passed stage with the part in a known state | All |
| `solar_event_quiet_hold` | A warning parks the arms and secures parts within a set time | Cislunar |
| `no_unrestrained_objects` | Every stage of a sequence keeps every object held, latched or tethered | All |
| `debris_avoidance_hold` | A collision-avoidance burn request waits for the next hold point, and the burn doesn't disturb latched work | LEO station |

The order of Verse zones follows the strategy's phases. A small LEO work
station zone, with prepared parts and a single arm, comes first: it is the
first real product and the easiest scene. The lunar port and the Lagrange 2
shipyard (#11625) reuse its arms, documents and checks at larger scale.
