# Design method

How spacecraft are designed, in our own words, and what each step means for
the typed spacecraft document in Lagrange 2. See [the README](README.md) for
the confidence marks.

## Phases

NASA and ESA both split a project into phases that each end in a review.
The names below follow NASA's life cycle [M]; ESA uses 0, A, B, C, D, E, F
with similar content [M].

| Phase | Work | Ends with |
| --- | --- | --- |
| Pre-A, concept studies | Is the mission worth doing; rough options | Mission concept review |
| A, concept and technology | Requirements, a baseline concept, budgets with large margins | System requirements and definition reviews |
| B, preliminary design | Subsystem designs, trades closed, interfaces defined | Preliminary design review (PDR) |
| C, final design and fabrication | Detailed drawings, parts built | Critical design review (CDR) |
| D, assembly, integration, test, launch | Integration and verification, launch | Flight readiness |
| E, operations | The mission | |
| F, closeout | Disposal | |

**For us:** the spacecraft document's version history can carry a phase
label, and the margin policy below can depend on it. A sample ship at
"phase C" maturity would carry smaller margins than a chat-sketched concept.

## Requirements

A requirement is a single, verifiable statement ("the ship shall deliver
3 km/s of Δv with its full payload"). Good requirements are:

- **Traceable:** each derives from a parent (mission goal → system →
  subsystem).
- **Verifiable:** by test, analysis, inspection, or demonstration. Each
  requirement names its method.
- **Unambiguous:** one number, one unit, one condition.

**For us:** each check in the spacecraft document (mass budget, center of
mass, structural limits) is a requirement with an analysis-type verification.
Writing them this way gives the chat a clear refusal message: which
requirement failed, by how much.

## Budgets

A budget allocates a limited resource across subsystems and tracks the
current estimate against the allocation. The common budgets:

- **Mass:** dry mass by subsystem, propellant, payload.
- **Power:** load per mode (assembly, cruise, burn, safe) against
  generation and storage.
- **Δv:** every maneuver, plus station-keeping and contingency.
- **Propellant:** Δv converted through the rocket equation, plus residuals
  and unusable propellant (a few percent [M]).
- **Pointing:** attitude error contributors combined.
- **Link:** see [subsystems.md](subsystems.md#communications).
- **Data and thermal:** onboard storage, heat in and out.

## Margins

Two kinds of margin are layered:

- **Contingency (growth allowance)** on each line item, by maturity. Typical
  practice [M]: about 20–30% on a new design that exists only as a concept,
  10–20% on a modified existing design, 3–5% on an off-the-shelf flown item,
  and near 0% on measured hardware. AIAA S-120 (mass properties control)
  and ANSI/AIAA guidance codify such tables [M, verify the standard and its
  numbers].
- **System margin** held at system level above the summed estimates with
  contingency, often 10–25% of dry mass early in a project [M].

For power, a similar contingency applies to loads, and arrays are sized for
end-of-life output after degradation.

For Δv, mission planners commonly hold several percent of total Δv, or a
fixed allotment, for navigation errors and contingencies [M].

**For us:** the spacecraft document stores each part's mass with a maturity
class, computes the margin, and refuses an edit that drives the system
margin negative. The sample ship should carry real-looking contingencies.

## Trade studies

A trade study compares design options against weighted criteria:

1. List options (for example, chemical vs. electric propulsion for the
   departure burn).
2. Choose criteria and weights (mass, Δv capability, burn time, cost, risk).
3. Score each option per criterion, ideally from a sizing calculation rather
   than opinion.
4. Check sensitivity: does the winner change if a weight moves?
5. Record the decision and its rationale.

**For us:** a chat request such as "make it electric" can run a small trade:
compute propellant mass and burn time for each option with the rocket
equation and the thrust, and show both before applying the change.

## Configuration and mass properties

The assembled ship's mass properties come from its parts:

- Total mass: the sum of part masses.
- Center of mass: the mass-weighted mean of part centers of mass.
- Inertia tensor about the ship's center of mass: each part's inertia about
  its own center, rotated into the ship frame, plus the parallel-axis term
  m (|d|² I − d dᵀ) for its offset d.
- Products of inertia show asymmetry; a ship meant to burn along its axis
  wants the thrust line through the center of mass (offset in the order of
  centimeters for a large ship [M]) and small products of inertia.

Propellant depletion moves the center of mass during a burn. Designers
place tanks near the center of mass to limit that shift.

## Verification

The four methods are test, analysis, inspection, and demonstration. Large
structures that cannot be tested whole on the ground are verified by
analysis correlated with tests of components. A ship assembled in orbit is
the extreme case: the first test of the whole is in space.

**For us:** our "tests" are Rust tests that check the simulated ship against
its document: the simulated mass properties equal the document's, the burn
matches the rocket equation, and limits are respected. See
[sim-mapping.md](sim-mapping.md).
