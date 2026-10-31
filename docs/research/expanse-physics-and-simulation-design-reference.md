**Full spoilers for all nine novels, the short fiction, and television Seasons 1 through 6.**

The strongest foundation for an Expanse simulation is a real spacecraft and orbital mechanics model, an explicitly fictional Epstein propulsion model, and separate alien rule systems. Keep all three inspectable. The ordinary rules make distance, acceleration, human vulnerability, and logistics meaningful; their consistency is what makes the protomolecule disturbing.

This reference provides a proposed implementation rather than a claim that the franchise supplies complete engineering specifications. Numerical examples are independently calculated and labeled. Detailed late-book alien mechanisms are identified by their original narrative sources; their underlying equations remain fictional or unspecified.

# Evidence and continuity

Maintain four kinds of evidence in the project: novel text; screen continuity and production references; licensed game material; and original engineering or gameplay assumptions. A statistic from a roleplaying supplement should never silently become a measurement of the television ship. An estimate from an episode should retain the episode, scene, assumed scale, and uncertainty.

For each tunable parameter, store continuity, historical state, value and units, source, and whether it is explicit, inferred, or invented for the simulation. Unknown quantities can remain unknown in the player interface even when the scenario requires an executable internal value. Preserve the distinction between what the simulation knows and what characters know.

The creators describe the Epstein drive as exceptionally efficient fusion propulsion accelerating reaction mass, while deliberately withholding a physically defensible design. Their later discussion explicitly acknowledges unresolved heat rejection. Those statements establish the fictional premise; they do not specify a unique exhaust velocity or reactor design. [Creator explanation](https://www.syfy.com/the-expanse/season-2/blogs/mirrors-how-do-they-work-the-science-of-the-expanse-episode-6) · [Authors discussing the thermal concession](https://www.iheart.com/podcast/105-daniel-and-kellys-extraord-29862087/episode/the-expanse-featuring-james-sa-corey-246769937/)

Use relative historical epochs unless a particular continuity provides an exact date. The television Epstein flashback is 137 years before its Season 2 episode; Orbit describes the story Drive as approximately 150 years before the main novels. These should be separate timeline configurations. [Official television recap](https://www.syfy.com/the-expanse/season-2/blogs/episode-recap-paradigm-shift) · [Publisher guide to the short fiction](https://www.orbit-books.co.uk/orbit-books-news/orbit-reading-recommendations/2022/03/16/take-a-closer-look-at-the-short-stories-in-memorys-legion/)

# Historical eras and their simulation rules

The following scenario structure combines established narrative milestones with proposed gameplay consequences. The suggested early-adoption stages are design choices, not a published year-by-year history.

| Era | World state | Simulation emphasis |
| - | - | - |
| Before Epstein | Earth, Luna and Mars infrastructure; less capable fusion propulsion | Propellant budgets, coast phases, orbital opportunities and costly logistics |
| First Epstein adoption | Experimental drives coexist with old vessels | Retrofits, flight testing, training, certification, supply depots and rescue coverage |
| Expansion of the Belt | New commercial routes become viable | Prospecting, station construction, shipping capacity and industrial ownership |
| Mature solar civilization | Established Earth, Mars and Belt societies and fleets | Shipping, policing, combat, ice and food dependencies, maintenance |
| Eros and Venus | Localized protomolecule activity violates familiar expectations | Investigation, containment, uncertain sensor interpretation and evacuation |
| First Ring exploration | A new spatial domain and an active defensive station | Probes, threshold experiments, capture events and emergency spin gravity |
| Colonization and Free Navy | Interstellar access coexists with Sol dependence and conflict | Traffic, migration, convoys, disrupted agriculture and infrastructure repair |
| Transport Union and Laconia | Decades of colony development and alien-derived military superiority | Transit administration, asymmetric conflict, advanced materials and fabrication |
| Final alien conflict | Escalating threats from the builders' enemies | Network-wide consequences, observation failures and evacuation choices |
| After network collapse | Interstellar links disappear | Colony self-sufficiency, technological regression or recovery, local politics |
| Distant epilogue | Later human interstellar capability | A separate travel ruleset and reconnection scenarios |

The seventh novel follows a roughly three-decade gap. Television Season 6 resolves the sixth-book conflict and introduces Strange Dogs material, while the final three novels carry the later arc. [Interview with the authors and television creators](https://www.theringer.com/2021/12/09/tv/expanse-season-6-finale-amazon-books-ending)

For the late eras, the primary narrative anchors are Persepolis Rising, Tiamat's Wrath, and Leviathan Falls. The last novel ends the Ring network; its epilogue occurs approximately a millennium later and introduces a different form of human interstellar travel. Treat those as distinct configurations. These specific events are referenced to Leviathan Falls, the final sequence and Epilogue: The Linguist. The precise mechanics of the epilogue drive are not an engineering specification.

The post-collapse short-fiction coda is The Sins of Our Fathers. The authors confirm the placement of the coda after the main action of Leviathan Falls. [Author interview and coda discussion](https://www.space.com/the-expanse-leviathan-falls-abraham-franck-interview)

## The first years after Epstein

Pre-Epstein is not synonymous with chemical rockets. The authorized Leviathan Wakes excerpt describes the Knight's older fusion torch and its steam maneuvering thrusters. Legacy propulsion remains useful long after the breakthrough. [Leviathan Wakes chapter 3](https://www.abebooks.com/9780316129084/Leviathan-Wakes-Corey-James-S.A-0316129089/plp)

A convincing early campaign should model technology spreading through an existing economy. A new engine can exceed a hull's structural capacity, cooling capability, crew tolerance or safe control authority. Its nominal performance does not mean the yard has replacement parts or that a stranded vessel can obtain rescue. Existing ports may lack safe approach corridors or procedures for long burns at unfamiliar velocities.

Good missions include qualifying a retrofit, establishing a remote depot, towing a failed prototype, surveying an asteroid for a first permanent settlement, and racing a legacy ship against an Epstein vessel under identical cargo and crew constraints. The important comparison is often sustained acceleration and total useful velocity change, rather than one ship having a higher arbitrary speed limit.

Model adoption with manufacturing capacity, trained labor, maintenance availability, financing, insurance, infrastructure and political control. New trade routes can exist before their destinations have mature cities. Factional development should emerge over time instead of importing the main story's complete political map into the first post-invention year.

# The ordinary physics model

Use metres, kilograms and seconds internally. Keep inertial positions and velocities separate from ship-local coordinates. Each ordinary spacecraft needs position, velocity, attitude, angular velocity, mass, center of mass, inertia and a collection of applied forces and torques.

```math
\dot{\mathbf r}=\mathbf v,\qquad
\dot{\mathbf v}=\sum_j \mu_j\frac{\mathbf r_j-\mathbf r}{|\mathbf r_j-\mathbf r|^3}+\frac{R(q)\mathbf F_{\rm body}+\mathbf F_{\rm other}}{m}
```

This is a Newtonian point-mass gravitational model with a rigid spacecraft. Add atmosphere, nonspherical gravity, radiation pressure or tides where a scenario requires them. Do not calculate every rock's attraction on every other rock merely because the world contains many objects. Planetary ephemerides plus restricted dynamics for vessels are a useful initial level of fidelity.

A reaction engine consumes mass while changing velocity. For constant effective exhaust velocity and negligible external forces, use the rocket equation. NASA provides the underlying momentum derivation. [NASA ideal rocket equation](https://www1.grc.nasa.gov/beginners-guide-to-aeronautics/ideal-rocket-equation/)

```math
F=\dot m_{\rm exhaust}v_e,\qquad
\dot m=-\dot m_{\rm exhaust},\qquad
\Delta v=v_e\ln\left(\frac{m_0}{m_f}\right)
```

Fuel that supplies energy, reaction mass that carries momentum, coolant, oxygen, water and cargo should be distinct accounting categories where the design requires them. Their plumbing may share resources; the accounts still need to say what mass and energy are doing. Do not subtract the same material twice when a fuel's reaction products also contribute to exhaust.

Thrust creates torque when its line of action misses the center of mass. Cargo shifts, a failed maneuvering jet, a detached module and a leaking tank can therefore change handling. For a rigid body with constant inertia, with all vectors expressed in the same body frame:

```math
I\dot{\boldsymbol\omega}+\boldsymbol\omega\times(I\boldsymbol\omega)=\boldsymbol\tau,\qquad \boldsymbol\tau=\sum_i(\mathbf r_i-\mathbf r_{\rm COM})\times\mathbf F_i
```

Recompute mass distribution after changes. A simple rigid-body model can update inertia between steps; a detailed variable-mass model also needs the corresponding momentum flux and inertia-rate terms.

## Three fidelity configurations

| Configuration | Ordinary motion | Technology assumptions | Intended use |
| - | - | - | - |
| Engineering baseline | Physical forces, mass, heat, sensors and calibrated contacts | No Epstein performance or alien exceptions | Realistic mission studies and robotics-oriented experiments |
| Expanse continuity | Same ordinary motion and contacts | Calibrated Epstein performance and specified fictional heat handling | Faithful games and narrative scenarios |
| Teaching and cinematic presentation | Same underlying state where practical | Optional time compression, enlarged visual markers and control assistance | Readability, education and accessible play |

Presentation assistance should not quietly alter the trajectory. If a mode truly changes physical rules, record that change in the scenario configuration.

# Epstein propulsion and the thermal budget

Represent the engine through observable outputs and constraints: available thrust, exhaust performance, propellant flow, power requirements, thermal coupling, throttle response, restart behavior and failure states. It is unnecessary to invent a detailed reactor microphysics model to make a faithful game.

Keep four limits independent: engine thrust, available energy, heat removal and tolerable crew loading. At a given thrust, a heavily loaded freighter accelerates more slowly. With a fixed acceleration command, demanded thrust decreases as mass is consumed. A maneuverable warship may have ample thrust but still need to protect its occupants.

## A deliberately hypothetical example

Assume a 1,000-metric-tonne spacecraft initially accelerating at 1 g with an ideal, directed, nonrelativistic exhaust speed of 10,000 km/s. These are selected simulation inputs, not Rocinante specifications. Pressure thrust is neglected in this illustration.

```math
F=ma,\qquad P_{\rm jet}=\frac12\dot m_{\rm exhaust}v_e^2=\frac12Fv_e
```

| Quantity | Initial result |
| - | -: |
| Thrust | 9.807 MN |
| Exhaust mass flow | 0.981 kg/s |
| Exhaust kinetic power | 49.03 TW |
| Ship heating equal to 0.1 percent of jet power | 49.03 GW |

The parameters are coupled: in this ideal exhaust model, available jet power imposes a thrust limit of twice that power divided by exhaust speed. If the input budget is reactor power, first apply propulsion efficiency to obtain available jet power. A configuration cannot independently maximize thrust and exhaust velocity while assigning arbitrarily small power.

Most exhaust energy can leave with the exhaust; it is incorrect to dump all engine power into the hull. But even a small absorbed fraction can be enormous. In vacuum, thermal radiation is the ordinary way to reject heat without expelling material. Internal conduction or fluid circulation moves heat to the radiator; it does not make the heat disappear. [NASA thermal control](https://www.nasa.gov/smallsat-institute/sst-soa/thermal-control/)

```math
P_{\rm rad}=\epsilon\sigma A_{\rm emit}(T^4-T_{\rm background}^4)
```

For the 49.03 GW example, radiators at 1,000 K with emissivity 0.9 require about 0.961 square kilometres of total emitting area, assuming an unobstructed cold background and negligible incoming radiation. At 1,500 K the emitting area is about 0.190 square kilometres. A panel with two equally useful radiating faces has half the geometric panel area. Those hot radiator temperatures are themselves material and coolant design constraints.

Choose a declared thermal model: fictional heat rejection preserving screen silhouettes; exceptionally low heat coupling with hazardous exhaust; conspicuous radiators; consumable cooling; limited high-power duty cycles; or a combination. The franchise does not supply a complete numerical budget that settles the choice.

A practical game model needs several thermal nodes, not full computational fluid dynamics: reactor/drive, electrical equipment, weapons, occupied spaces, heat sinks and radiators. Connect them through transfer capacities and failures. A basic energy-storage update is:

```math
C_i\dot T_i=P_i+\sum_j Q_{j\to i}-P_{{\rm rad},i}-P_{{\rm expelled},i}
```

Here C is heat capacity in joules per kelvin, P includes generated and externally absorbed heat, Q is heat exchanged with other nodes, and expelled cooling consumes finite material and enthalpy. Radiator damage, a jammed valve or depleted coolant can then create operational emergencies without simulating every pipe molecule. Heat sinks buy time instead of deleting heat.

Drive plumes should affect docking, formation flight and surface operations. A port approach should transition to appropriate low-power maneuvering propulsion. Atmospheric entry also requires aerodynamic and thermal treatment; possession of an Epstein drive does not make every hull an all-purpose lander.

# Orbital travel and rendezvous

Interplanetary travel is a boundary-value problem: reach a future position with the required future velocity. A ship aimed at the current location of Mars will not perform a rendezvous. Even a successful intersection can be a lethal flyby if the relative velocity is wrong.

Display a predicted path, burn segments, a destination ephemeris, arrival relative velocity, propellant reserve, peak crew loading and an arrival-time estimate. Let the player choose a tradeoff such as fastest tolerable route, economical cargo route or a route with a rescue reserve. Initial guesses can use familiar two-body methods; finite-thrust trajectories require numerical propagation and optimization. NASA's trajectory-design work uses direct shooting and collocation for thrust histories. [NASA Goddard trajectory optimization](https://ntrs.nasa.gov/citations/20230006127)

## An ideal flip and burn benchmark

For stationary endpoints separated by distance D, starting and ending at rest, accelerate at constant a for half the distance and brake at the same magnitude for the other half. Ignore gravity, target motion, starting orbital velocity, finite turning time and reserves.

```math
t=2\sqrt{\frac Da},\qquad v_{\max}=\sqrt{aD},\qquad \Delta v=2\sqrt{aD}
```

Using 1 AU = 149,597,870,700 metres and standard gravity = 9.80665 metres per second squared:

| Acceleration | Travel time over 1 AU | Peak speed | Total ideal velocity change |
| - | -: | -: | -: |
| 0.1 g | 9.041 days | 383.0 km/s | 766.0 km/s |
| 0.3 g | 5.220 days | 663.4 km/s | 1,326.8 km/s |
| 1.0 g | 2.859 days | 1,211.2 km/s | 2,422.4 km/s |

These are independently calculated benchmarks, not Earth–Mars timetables. With the hypothetical 10,000 km/s exhaust speed above, the ideal mass expenditure is about 12.4 percent of departure mass at 0.3 g or 21.5 percent at 1 g, before additional maneuvers and reserves. Distance and acceleration have square-root effects on the benchmark travel time.

The ideal halfway flip is not a universal flight plan. Unequal acceleration limits, changing mass, target motion, coasts, exclusion zones and arrival conditions move the transition. The real flip also takes time and maneuvering propellant. A planner should solve for the actual control history rather than rotate at 50 percent of an arbitrary progress bar.

## Coasting and gravity assists

Engine shutdown does not stop a vessel. A stable orbit remains an orbit. Deorbiting requires a trajectory change or an ongoing force such as atmospheric drag. Gravity assists exchange energy and momentum with a moving gravitating body; they are not free speed multipliers independent of encounter geometry.

Powerful propulsion reduces the necessity of carefully timed low-energy transfers, but planets still move, docking still requires velocity matching, and losing a drive at the wrong moment can turn an arrival into an unrecoverable flyby. Keep a trajectory after propulsion failure rather than freezing or dropping the ship.

## Relativistic boundary

The ideal 1 AU, 1 g trip peaks near 0.00404 times light speed, making Newtonian flight adequate for this example. Sustained acceleration over interstellar distances eventually requires special relativity. Define an operating envelope and a separate relativistic propagation mode rather than allowing ordinary engines to cross light speed.

For constant proper acceleration from rest, with proper time tau:

```math
v=c\tanh(a\tau/c),\qquad t=(c/a)\sinh(a\tau/c),\qquad x=(c^2/a)[\cosh(a\tau/c)-1]
```

These equations describe an ideal acceleration segment; fuel limits and braking remain separate constraints. [Relativistic rocket derivation hosted at UCR](https://math.ucr.edu/home/baez/physics/Relativity/SR/Rocket/rocket.html)

# Gravity and life inside spacecraft

The concept artists describe designing ships vertically because thrust supplies the apparent floor. This is the starting point for both exterior silhouettes and playable interiors. [Interview with the ship designers](https://magazine.artstation.com/2016/02/scenes-concept-art-expanse/)

During an ordinary main-engine burn, down points approximately toward the engine. During braking the ship turns and uses that same engine, so the same physical decks remain floors. During the turn, the changing rotational state can move or strike unsecured occupants and cargo. During a coast, loose objects float.

Do not add planetary gravity to the interior as though the hull were held fixed in space. In free fall, ship and contents share almost the same gravitational acceleration. Their relative motion depends on proper acceleration, rotation and tidal differences, plus contacts and other local forces.

```math
\mathbf a_{\rm local}=R^{-1}(\mathbf a_{\rm inertial}-\mathbf a_{\rm origin})-2\boldsymbol\omega\times\mathbf v_{\rm local}-\boldsymbol\omega\times(\boldsymbol\omega\times\mathbf r_{\rm local})-\dot{\boldsymbol\omega}\times\mathbf r_{\rm local}
```

This supplies the Coriolis, centrifugal and Euler terms when using a rotating ship frame. Alternatively, integrate objects inertially and derive local observations. Do not apply both treatments and double-count the same effects.

## Rotating habitats

For ideal spin gravity, acceleration at radius r is omega squared times r. At a selected radius of 500 metres, producing 0.3 g requires approximately 0.7325 rpm. At the axis the centrifugal contribution vanishes. These are kinematic results, not a universal human comfort prescription.

Dropped objects retain their initial inertial velocity. Their paths curve relative to the rotating floor; the effect depends on release location and velocity. Walking in different directions changes the experienced load. Docking near the rotation axis and riding outward can therefore be distinct activities. [NASA artificial gravity review](https://ntrs.nasa.gov/citations/19740010644)

The novels' spun-up Ceres is a fictional megastructure assumption. Preserve it for continuity, but distinguish spinning a reinforced habitat from spinning a natural asteroid far beyond its gravity-supported rotation. A strict engineering mode would need a structural model or replace the settlement geometry with supported rotating habitats.

Magnetic boots apply attachment forces at the feet. They do not make liquids settle or loose tools fall. Implement foot constraints, adhesion limits, release, body rotation and different movement animations. Crew should use handholds and tethers when appropriate.

## Crew load and habitability

Represent acceleration effects with magnitude, body-axis direction, duration, posture, restraint and accumulated exposure. The fictional juice and advanced couches can modify a scenario's tolerance model, but they do not establish a universal safe acceleration. NASA's occupant-protection guidance explicitly treats direction, duration and posture as separate factors. [NASA acceleration technical brief](https://www.nasa.gov/wp-content/uploads/2023/12/ochmo-tb-024-acceleration.pdf)

Useful gameplay outcomes precede death: impaired fine movement, inability to stand, interrupted repairs, fatigue, nausea and loss of consciousness. Individual history can affect the fictional model. Avoid treating all Belters as having one identical tolerance or presenting the franchise's developmental biology as measured human science.

Life support should track oxygen, carbon dioxide, pressure, temperature, water, food and power dependencies. A compartment model is usually sufficient. Decompression depends on opening size, gas state and connected volume. A puncture does not instantly empty every isolated room, and vacuum does not instantly freeze a person or make every object explode.

# Spacecraft design and damage

Design vessels around mission, loaded mass, heat, structural paths, maintenance and crew access. The following archetypes are proposed simulation roles inspired by the setting rather than a table of canonical specifications.

| Archetype | Design priorities | Distinctive gameplay |
| - | - | - |
| Early fusion shuttle | Short hops, modest endurance, safe maneuvering near other vessels | Docking, passenger transfer, propellant discipline |
| Epstein retrofit | New propulsion constrained by an older hull | Qualification, trim, thermal trouble and maintenance |
| Ice or cargo hauler | Payload fraction, capture equipment, tanks and repeated routes | Mass-dependent handling, freight margins and emergencies |
| Corvette or gunship | Defensive coverage, maneuvering authority, magazines and crew protection | Small-crew tactical operations and repair under pressure |
| Capital warship | Redundancy, reactors, armor, weapons and extensive internal subdivision | Damage control, fleet coordination and logistics |
| Racer or courier | Low carried mass, powerful drive, minimal redundancy | Time-critical travel and limited survival margins |
| Generation ship or mobile station | Habitat volume, spin system, agriculture and long-term maintenance | Large rotating interiors and infrastructure management |
| Laconian warship | Alien-derived construction and exceptional capabilities | A separate materials and weapon ruleset |

Separate the visual exterior mesh from the collision geometry, pressure boundaries, armor layers, structure, machinery, fuel network and occupied space. These representations need to agree spatially but do not need identical detail.

A practical vessel can be a graph of compartments and components. Doors connect gas volumes. Buses connect power supplies and consumers. Pipes connect tanks, pumps and heat exchangers. Structural joints connect load-bearing assemblies. Hits remove or degrade graph edges as well as damaging surfaces. The result is an understandable causal chain: a strike severs a coolant line, the drive overheats, thrust falls, the arrival solution changes and crew gravity disappears.

Keep ship variants by era. A retrofit can add a weapon, relocate cargo, change armor or alter landing arrangements. A later Rocinante configuration should not be the default for every earlier scenario. Similarly, Nauvoo, Behemoth and Medina represent different operational uses and fits of a large structure.

A walkable cockpit is only part of the ship. Add accessible maintenance spaces, isolation valves, suit storage, crash couches, emergency routes, cargo restraints, airlocks, ladders and handholds. These spaces generate cooperative tasks and explain the spacecraft's shape.

## Armor and high-speed impacts

Use penetration paths through layers and components instead of only a global health bar. Hit outcome depends on relative velocity, projectile mass and shape, incidence, material and what lies behind the surface. Hypervelocity damage often involves fragmentation, shock and vaporization; a familiar low-speed bullet model will not cover every regime.

Whipple shielding uses a sacrificial bumper to disrupt a particle and spread its debris before it strikes the rear wall. It is a useful engineering reference for debris protection, not a universal solution to heavy weapons. [NASA shield development](https://hvit.jsc.nasa.gov/shield-development/)

For large-scale combat, use calibrated response models or lookup tables rather than a full impact hydrocode. Preserve important failures and let small visual debris be cosmetic. Larger fragments with meaningful collision risk remain physical entities with their own trajectories.

# Worlds and settlement environments

Begin with actual celestial bodies, then layer fictional infrastructure and historical alterations over them. The ordinary environment should include gravity, rotation, terrain, atmospheric properties where present, solar flux, eclipse conditions and relevant radiation exposure.

NASA's Dawn material emphasizes that the asteroid belt is mostly empty space. Closely packed hazards should occur for a reason: a debris cloud, a mining operation, rings or a recent breakup. Planet-scale navigation should not look like flying through a continuous boulder maze. [NASA Dawn FAQ](https://science.nasa.gov/mission/dawn/faq/)

Earth, Luna, Mars, Ceres, Eros, Ganymede, Tycho and later colony worlds need different movement and infrastructure. A station's centrifugal floor is unlike a planetary surface; a small body's irregular gravity field can require careful anchoring; an atmospheric world introduces lift, drag, weather and landing heat. Low gravity alone does not imply a safe surface.

For Ganymede-style agriculture, model illumination, power, protected growing volume, water, nutrients, machinery and transport. A magnetic field is not a substitute for a complete radiation-protection assessment. Mirror damage should change agriculture and heat balance before it appears as an abstract food penalty. Solar flux scales approximately with inverse distance squared, so the same collector area performs differently throughout the system.

Treat lighting separately from camera exposure. Include inverse-square sunlight, occlusion, eclipses, hard shadows and thermal emission. Stars can remain in the astronomical background while being invisible at an exposure chosen to show a brightly illuminated hull. Camera readability settings should not change the power collected by solar panels or the temperature of a surface.

For colony worlds, breathable air, compatible food and biological safety are separate properties. An alien biosphere may supply none, some or all of what humans require. Use surveyed values and uncertainty before colonization; do not derive habitability from green terrain textures.

Keep strategic logistics linked to actual ships: population consumes supplies, factories require inputs, ports have throughput and labor, and storage provides buffers. A cheap transport drive does not create food or spare parts. After the gates open, economic change can move labor and capital away from old projects. After they close, the same dependency graph determines which settlements endure.

# Sensors and communication

Maintain authoritative truth, emitted signals, received measurements and estimated tracks as different data. A target's current position is not automatically the position the player can observe. NASA's SPICE documentation distinguishes geometric state from state corrected for light travel time. [JPL aberration corrections](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/abcorr.html)

At a separation of 1 AU, the calculated one-way light time is approximately 499.0 seconds, or 8 minutes 19 seconds. A radar or command-and-reply exchange requires outbound and return paths plus processing, with geometry changing during the exchange.

```math
t_{\rm receive}-t_{\rm emit}=\frac{|\mathbf r_{\rm target}(t_{\rm emit})-\mathbf r_{\rm observer}(t_{\rm receive})|}{c}
```

The tactical interface should distinguish last observation, predicted present position and uncertainty. A contact may have uncertain identity, thrust, acceleration history and intent. Receding signal quality is a reason to show a growing confidence region rather than a perfectly sharp target marker.

Detectability depends on sensor coverage, sensitivity, integration time, geometry, background, emissions and distance. A bright fusion burn is difficult to hide from an appropriately placed sensor, but neither universal invisibility nor universal detection is a useful physical law. Coasting removes the drive plume; the occupied ship still produces heat. Directional emission, temporary storage, occlusion and decoys can be limited tactical tools.

Keep out-of-game networking latency separate from fictional light delay. A server should distribute observations when they become available in-world, while ordinary network code carries those results to clients. AI should receive the same kind of limited evidence as players. It should not aim with hidden authoritative positions unless the scenario explicitly grants such knowledge.

# Space combat

The official science discussion connects hull punctures with loss of air and uncontrolled gas forces, and explains deliberate depressurization before battle. This suggests a coupled damage and atmosphere model. [Creator discussion of combat and pressure](https://www.syfy.com/the-expanse/season-2/blogs/gravity-sex-and-e-mail-addiction-the-science-of-the-expanse-episode-2)

Torpedoes should be autonomous spacecraft with propulsion, steering limits, seekers, guidance updates, warheads and finite endurance. Railgun rounds are ballistic objects after launch. Point-defense cannons need tracking, traverse limits, line of fire, ammunition, heating and an interception solution. A weapon's effective range depends on geometry and target maneuverability; empty space does not make a projectile disappear after an arbitrary range.

For a deliberately invented example, a projectile crossing 1,000 km at 10 km/s relative closing speed needs about 100 seconds if relative motion stays simple. A target capable of an unexpected 1 g lateral maneuver could move about 49 km from its original extrapolated path over that interval. This calculation explains the predictive disadvantage of unguided fire at distance; it is not a canonical railgun muzzle velocity.

Recoil changes vessel motion. Counter-thrust is another action, with propellant and heat costs. A missile launch, turret burst or vent can alter a precise docking solution. Players should see those changes on their predicted trajectories.

Vacuum explosions do not produce an ordinary atmospheric blast wave across empty space. Radiation, hot ejecta, fragments, direct impact and material ablation can still be destructive. Internal explosions involve cabin atmosphere and confined structures. Use different damage mechanisms instead of one universal expanding fireball.

Ship crews can fight without ever seeing an enemy as more than a sensor contact. Offer an honest-scale view, a tactical view with enlarged symbols and uncertainty, and a clearly framed cinematic camera. Hull-borne vibration, local machinery and helmet radio can carry sound; an external audio mode can sonify sensor events for readability.

Combat encounters should emphasize decisions: when to reveal a burn, whether to preserve a braking reserve, which compartment to sacrifice, when to fire point defense, whether to rescue a disabled crew and how to protect a freighter whose acceleration is limited by cargo. A battle's geography is its evolving set of trajectories.

# Protomolecule and alien systems

The protomolecule is best represented as a goal-directed alien construction and information system. Its original narrative role is to exploit encountered material and build a connection into an older network. The creator discussion distinguishes unconstrained material reassembly from human-engineered humanoid hybrids. [Protomolecule creator interview](https://www.syfy.com/the-expanse/season-2/blogs/lego-sets-in-space-the-science-of-the-expanse-episode-13) · [Persepolis Rising prologue excerpt](https://www.worldswithoutend.com/novel.asp?id=25712)

## Construction and adaptation

Give a local process access to material inventories, available energy, templates or learned information, activation state and a construction objective. Represent local growth with a graph, voxel field or coarse reaction network; generate detailed tendrils, surfaces and particles for rendering. The entire solar system need not be a cellular automaton.

Distinguish acquiring a sample, understanding it, directing it and controlling ancient infrastructure. Those are separate accomplishments. A sample should not unlock every observed alien feat immediately. Scarcity can arise from knowledge, containment, compatible facilities, access to templates or dangerous side effects.

Possible stages are dormant, activated, acquiring material, reorganizing, building infrastructure and communicating. These stages are a proposed software model, not a canon taxonomy. Thermal emission, mass movement and sensor anomalies provide observable evidence without revealing the internal state to players.

## Eros and inertial coupling

Eros moves in a way that would be impossible for an ordinary human vessel while Miller does not experience the expected crushing acceleration. The creators explicitly describe the local stabilization and also retain energy use and waste heat as observables. [Eros physics discussion](https://www.syfy.com/the-expanse/season-2/blogs/hot-sci-fi-hair-the-science-of-the-expanse-episode-5)

Implement an external motion rule and a distinct local inertial-coupling rule. Selected enclosed objects can share the anomalous motion while nearby ordinary spacecraft retain their normal limits. Do not approximate Eros with only an enormous conventional thruster; that would create the wrong crew, structural and exhaust consequences.

## Venus and engineered transformation

The Arboghast's disassembly is described by the creators as an organized examination, like an exploded engineering diagram. Animate and simulate selected components under alien constraints rather than applying a generic explosion. The subsequent Ring construction is a staged transformation with material transport and changing geometry. [Arboghast creator explanation](https://www.syfy.com/the-expanse/season-2/blogs/lego-sets-in-space-the-science-of-the-expanse-episode-13) · [Abaddon's Gate publisher description](https://www.jamessacorey.com/books/abaddons-gate/)

## Ring topology and defensive capture

Represent each star system and Ring space as separate spatial domains. Gates are oriented connections between domains, with explicit crossing rules for position, velocity, attitude and signal transmission. Ships must still reach the gate and cross the hub. A network edge is not permission to teleport from a planet's surface to another planet's surface.

Transform velocity relative to the moving gate frames. This is a selected portal convention that should be documented, not a claim to have derived wormhole dynamics. Transit must be detected at the correct crossing time; a high-speed object must not skip over the gate between frames. Handle trailing geometry, partial crossings, collision ownership and interruption consistently.

The station's speed restriction is an active defensive state. Probe experiments show capture above a threshold and a separate destructive boundary away from gates; later escalation abruptly slows the fleet. Exact thresholds should be checked and configured separately for book and screen continuity. [Intransigence recap](https://www.syfy.com/the-expanse/season-3/blogs/episode-recap-intransigence) · [Fallen World recap](https://www.syfy.com/the-expanse/photos/fallen-world-season-3-episode-11)

After the Abaddon's Gate and television Season 3 crisis, the initial defensive speed restriction is lifted. Slow Zone persists as a place-name; ordinary later traffic should not inherit a permanent velocity cap. [Official Season 3 finale recap](https://www.syfy.com/the-expanse/photos/abaddons-gate-season-3-episode-13)

To reproduce the casualties, preserve the distinction between an affected hull and its occupants. Changing the hull's velocity while allowing unrestrained contents to retain their prior velocity creates a collision. Applying one identical instantaneous velocity change to hull, crew, cargo and air would erase the crucial event.

## Ilus and disabled fusion

In Cibola Burn and television Season 4, ancient machinery, local hazards and fusion suppression combine into a survival crisis. Model the suppression as a regional technology rule affecting the selected reactions or equipment. Stored electrical energy and unrelated physical processes need their own rules. Ordinary gravity should continue unless the scenario separately changes it.

A reactor shutdown does not by itself make a stable orbit collapse. Reproduce the orbital crisis with the relevant trajectory and atmospheric conditions, including drag or an insufficient periapsis where appropriate. If a railgun supplies an emergency impulse, account for recoil and the finite ammunition; do not turn it into an unlimited reactionless rescue engine. Native biological hazards should remain distinct from protomolecule transformation.

## Transit losses and network stress

The disappearances encountered in Nemesis Games and Babylon's Ashes are a different phenomenon from the earlier speed-limit capture. Their relationship to gate usage becomes a traffic-management and scientific problem. Represent station defense, transit loss and hostile-entity intervention as separate mechanisms.

A useful original model is a hidden stress state increased by traffic and selected energetic events, with recovery and scenario-dependent escalation. The specific contribution function, recovery time and threshold are design parameters. Do not call a convenient mass-times-speed-squared formula the canon solution. Players can estimate a safe operating envelope from observations, but should not receive certainty the source narrative does not provide.

## Laconia and the builders' enemies

Persepolis Rising and Tiamat's Wrath introduce alien-assisted construction, exceptional ship materials, self-repair and the Magnetar-class field projector. Give these distinct models with their own activation, replenishment, damage and vulnerability rules. Antimatter is an energy resource with its own logistics; it is not an explanation of every field effect. A field projector is not an ordinary railgun with a larger damage value.

The original novels are the behavioral reference here; public publisher descriptions establish the technological imbalance but do not provide quantitative weapon specifications. Use scenario values for repair rates, range, energy costs and collateral effects, and retain their provenance. [Persepolis Rising](https://www.jamessacorey.com/books/persepolis-rising/) · [Tiamat's Wrath](https://www.jamessacorey.com/books/tiamats-wrath/)

The beings that destroyed the builders are separate from the builders and their machinery. Model their observed interventions, triggers and consequences without inventing a settled scientific theory of them. Loss of consciousness or perception should not automatically mean all physical motion stopped. Consciousness-related effects can disable or alter agents and observations while the rest of the simulation continues according to the selected scenario.

Leviathan Falls broadens the threat to the network and its connected systems. The Adro archive structure, the bullet-like anti-builder anomalies, repaired organisms and collective-consciousness developments belong in distinct scenario modules, with observations and capabilities checked against the relevant novel. They should not all be implemented as variants of infection. The publisher establishes the broad conflict and Adro investigation; the detailed phenomena are referenced to Tiamat's Wrath and Leviathan Falls. [Leviathan Falls publisher description](https://www.jamessacorey.com/books/leviathan-falls/)

# Numerical architecture for a solar system

Keep a double-precision astronomical state and small local frames for rendering and contacts. At 1 AU expressed in metres, IEEE float32 spacing is 16,384 metres; float64 spacing is approximately 30.5 micrometres. These calculated spacings show why a naive single-precision global scene cannot represent a person walking around a ship at interplanetary coordinates.

| Layer | Suggested representation | Main responsibility |
| - | - | - |
| Celestial environment | Versioned ephemerides and body parameters | Positions, orientations, gravity sources and time |
| Strategic vessels | Double-precision state and trajectory segments | Travel, burns, mass and long-duration resources |
| Local interaction | Shared nearby contact frame | Docking, collisions, tethers, robot arms and boarding |
| Interiors | Ship-local objects and compartments | Crew, loose objects, pressure and equipment |
| Observations | Time-tagged signal and measurement events | Light delay, uncertainty and information access |
| Alien rules | Explicit domains and state transitions | Capture, portals, suppression and reconfiguration |
| Presentation | Camera-relative geometry and semantic overlays | Readable views without altering authoritative physics |

Use JPL Horizons or SPICE as references for real celestial bodies. Record time scale, coordinate frame, center, units, dataset version and valid coverage. Future asteroid positions and rotation models can be uncertain; choosing a precise campaign year does not make every distant extrapolation precise. Override bodies when the fiction changes them. [JPL Horizons manual](https://ssd.jpl.nasa.gov/horizons/manual.html)

Frame changes must transform velocity as well as position. In a rotating frame, the angular-velocity cross-position term is essential. JPL distinguishes position transforms from full state transforms for this reason. [JPL reference frames](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/frames.html)

```math
\mathbf v_{\rm world}=\mathbf v_{\rm frame}+R(\mathbf v_{\rm local}+\boldsymbol\omega\times\mathbf r_{\rm local})
```

Merge or couple nearby contact simulations consistently before ships touch. Independent physics bubbles must not each own a different answer to the same docking collision. Interior frames can be computational conveniences without disconnecting their force and momentum accounting from the exterior ship.

## Integration and time acceleration

Use analytic two-body coast propagation where its assumptions hold. Use numerical finite-thrust propagation where forces, mass or gravity require it. Long-term gravitational integrations may benefit from symplectic methods; a discontinuous burn, atmospheric encounter or close contact needs a method suited to that problem. Select tolerances by observable error, not a universal tick-rate slogan.

A local contact rate of tens or hundreds of steps per second may be appropriate for a particular mechanism, while distant vessels can advance through large coast segments. The necessary rate depends on stiffness, speed and required accuracy. At 1,200 km/s a projectile travels 20 km in a 1/60-second frame. Use swept collision or time-of-impact detection, including curvature refinement where needed, rather than relying on overlapping meshes at frame boundaries.

Timewarp should stop at burn boundaries, fuel exhaustion, significant heating, proximity events, possible impacts, signal arrivals and gate transitions. A long step should be split at the earliest event. Show a fast-forwarded simulation or jump to a validated future state; do not simply skip the difficult interval.

For a persistent multiplayer world, choose an explicit time policy. Shared accelerated time, scheduled arrivals, asynchronous commands and instance-local accelerated scenarios have different social consequences. Per-player timewarp cannot freely coexist inside one shared causal encounter.

## Replay and reproducibility

Persist commands, seeds, event order, scenario parameters, ephemeris versions, integrator settings and relevant solver state. Fixed timesteps alone do not guarantee bit-identical results across platforms. MuJoCo's own documentation qualifies exact reproducibility by version, architecture and retained state. [MuJoCo computation reference](https://mujoco.readthedocs.io/en/stable/computation/)

Offer replay at the observation level as well as the omniscient level: what did a captain know when issuing a burn, when did the information arrive, and how far had the target diverged? This makes debugging, training and player debriefs substantially more useful.

# Interactive presentation and game structure

The same simulated state should support several views. A solar-system navigator shows moving destinations, future trajectories and transfer tradeoffs. A tactical view shows delayed tracks, uncertainty, intercept opportunities and defensive coverage. An engineering view shows thermal, pressure, power and structural networks. A first-person interior makes gravity changes and repair routes tangible. An exploration view shows what instruments can infer about alien activity.

Use scales honestly. At interplanetary distances, ships are symbols and planets may need enlarged markers. Label these as diagrammatic overlays. At close range, restore physical proportions. A camera can transition between those presentations without shrinking the physical solar system.

Give players goal-oriented flight commands such as rendezvous, maintain separation, approach this dock and minimize fuel subject to this arrival time. Keep manual thrust available. An autopilot that respects the same constraints lets a player engage with the physics through decisions instead of requiring prolonged thruster tapping.

For newcomers, preview a maneuver before committing it: path, burn duration, fuel margin and how it changes the floor aboard ship. For experienced users, expose reference frames, solver residuals, sensor uncertainty and subsystem limits. Tooltips can identify a value as measured, estimated, source-defined or scenario-selected.

## Scenario portfolio

| Scenario | Principal mechanics | Reason to play |
| - | - | - |
| First successful retrofit | Finite thrust, mass, structure and crew loading | Discover what the breakthrough changes and what it does not |
| Depot beyond the old routes | Moving-target rendezvous, cargo and maintenance | Build an expansion economy from practical missions |
| Coasting casualty | Docking, tethers, breathing time and delayed messages | Rescue by matching motion and managing uncertainty |
| Hauler under attack | Convoy geometry, defenses and limited acceleration | Protect a valuable slow ship without wasting braking reserve |
| Rotating-station failure | Coriolis effects, docking, compartments and evacuation | Navigate a changing habitat rather than a generic corridor map |
| Eros pursuit | Ordinary crew limits versus anomalous motion | Recognize that the familiar rules have failed |
| First Ring probes | Portal crossing, observation delay and state discovery | Learn a dangerous environment through experiments |
| Slow-zone deceleration | Hull capture, loose contents and rescue logistics | Experience a physically legible catastrophe |
| Ilus reactor loss | Orbit propagation, batteries, recoil and local hazards | Solve a crisis through remaining physical options |
| Union traffic control | Network stress, queues and commercial pressure | Balance throughput against uncertain transit risk |
| Laconian confrontation | Repairing materials, field effects and asymmetric information | Fight an opponent with different technological capabilities |
| A colony after the gates | Inventory, fabrication, agriculture and local institutions | Survive the loss of a civilization-spanning supply chain |

## A recommended first playable implementation

Build a small early-Epstein orbital logistics scenario with one configurable freighter, two ports, a legacy shuttle, a rotating habitat and a repairable failure. The player loads cargo, chooses a route, experiences acceleration and coast aboard the ship, responds to a fault, matches the destination's velocity and docks. Include an ordinary celestial gravity source and a genuine moving destination.

This one scenario exercises the systems that make the setting distinctive: variable mass, finite thrust, attitude control, reference frames, crew gravity, time acceleration, docking and supplies. It is small enough to validate. An attractive fleet battle without those foundations would conceal the hardest unresolved work.

Then add sensors and combat, historical logistics, first-contact anomalies, Ring space, colonies and late-book modules. Each new capability should reuse existing systems where the underlying behavior is ordinary and introduce a separate rule only where the fiction requires it.

# Development sequence and acceptance criteria

| Stage | Deliverable | Acceptance criterion |
| - | - | - |
| 1 | Units, clocks, frames and ephemeris interface | Coordinate transformations preserve full states and dataset coverage is explicit |
| 2 | Coast and finite-burn flight | Analytical benchmark and numerical trajectory agree within stated error |
| 3 | Moving-target planning | Arrival satisfies both position and velocity tolerances |
| 4 | Local ships and interiors | Acceleration, coast, rotation and magboots produce distinct behavior |
| 5 | Components and resources | Cargo, leaks and subsystem failures alter handling and endurance |
| 6 | Docking and timewarp | High time acceleration cannot skip contact or fuel-exhaustion events |
| 7 | Sensors and weapons | Light delay, uncertainty, finite projectile flight and recoil are observable |
| 8 | Era economy | Technology adoption changes routes, fleet composition and infrastructure |
| 9 | Alien events and Ring space | Each exception has a declared scope, trigger and effect on enclosed objects |
| 10 | Colony network and late eras | Opening and closing connections changes actual dependencies and survival |

Regression scenes should include a two-body orbit, a constant-acceleration segment, a variable-mass rocket, an off-center thruster, a rotating-frame object release, a docking frame transition, one-way and round-trip signals, a fast projectile crossing, radiator equilibrium and a compartment leak. Conservation checks need the appropriate system boundary: a ship alone does not conserve momentum while ejecting exhaust.

For each canonical event, test the behavior that matters rather than matching a dramatic camera shot. A braking ship should preserve its normal floor after turning. A captured hull should injure unrestrained occupants when the selected alien rule does not capture them equally. Fusion suppression should disable the selected power source while leaving ordinary orbital propagation active. Removing Ring edges should isolate colonies rather than reset their inventories.

# Reuse for robotics and a simulation language

Reusable physical capabilities include typed units and coordinate frames, articulated mechanisms, constrained contacts, sensors with timestamps, state estimation, actuation limits, docking, tethers, task planning and reproducible execution. These are relevant to robotic inspection, manipulation, servicing and navigation.

A robotics-oriented mode needs calibrated contacts, actuators, sensing, material properties and validation against physical measurements. A visually convincing game scene does not establish predictive accuracy. Keep the Epstein engine, alien fields and narrative physiology outside that validated mode.

For a simulation language or engine, particularly valuable primitives are frame-tagged positions and velocities, distinct instantaneous and accumulated quantities, explicit clocks, deterministic event ordering, bounded numerical errors, component resource flows and well-scoped physical effects. An alien rule should declare which objects and laws it affects; a renderer or gameplay script should not be able to overwrite physical state without an identifiable event.

The most valuable long-term asset is a library of small executable worlds whose expected behavior is independently specified. They can exercise normal physics, human-facing interactions and deliberate fictional exceptions through the same transparent tools.

# Reference collection

## Canon and production

- Drive, collected in Memory's Legion, for the invention-era premise; the official publisher's [short-fiction guide](https://www.orbit-books.co.uk/orbit-books-news/orbit-reading-recommendations/2022/03/16/take-a-closer-look-at-the-short-stories-in-memorys-legion/) locates the stories in the chronology.

- Leviathan Wakes, particularly the early ship operations; the [authorized excerpt](https://www.abebooks.com/9780316129084/Leviathan-Wakes-Corey-James-S.A-0316129089/plp) provides primary evidence for legacy fusion, teakettle maneuvering and relative motion.

- Abaddon's Gate and television Season 3 for Ring discovery and changing station behavior; Cibola Burn and Season 4 for Ilus; Nemesis Games and Babylon's Ashes for later gate traffic and political disruption.

- Persepolis Rising, Tiamat's Wrath, Leviathan Falls and the relevant short fiction for late-book behavior. Their detailed alien mechanisms require original-text checks when assigning exact scene parameters.

- [The Art and Making of The Expanse](https://titanbooks.com/70054-the-art-and-making-of-the-expanse/) for production art and design from the first three seasons.

- [Direct concept-artist interview](https://magazine.artstation.com/2016/02/scenes-concept-art-expanse/) for the reasoning behind ship forms.

- Green Ronin's [Ships of the Expanse publisher material](https://greenronin.com/blog/category/the-expanse-rpg/) for 28 vessels and deck-plan work. Treat the supplement's statistics as a licensed game interpretation.

- [Beyond the Ring](https://greenroninstore.com/collections/the-expanse-rpg/products/beyond-the-ring-pdf-book-for-the-expanse-rpg) for a licensed colony and interstellar campaign framework; distinguish its additions from the original novels.

## Physics and implementation

- [JPL Horizons](https://ssd.jpl.nasa.gov/horizons/manual.html): celestial state, time scales, output frames and limitations.

- [JPL SPICE frames](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/frames.html) and [aberration corrections](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/abcorr.html): transformations and finite signal speed.

- [NASA rocket equation](https://www1.grc.nasa.gov/beginners-guide-to-aeronautics/ideal-rocket-equation/), [thermal control](https://www.nasa.gov/smallsat-institute/sst-soa/thermal-control/) and [trajectory optimization](https://ntrs.nasa.gov/citations/20230006127): core engineering relationships.

- [NASA occupant protection](https://www.nasa.gov/wp-content/uploads/2023/12/ochmo-tb-024-acceleration.pdf), [artificial gravity](https://ntrs.nasa.gov/citations/19740010644) and [shield development](https://hvit.jsc.nasa.gov/shield-development/): useful physical constraints.

- [MuJoCo computation](https://mujoco.readthedocs.io/en/stable/computation/): contact modeling and reproducibility qualifications.

**Recommended implementation priority:** make a single ordinary ship, its interior and its arrival trajectory agree physically. Then make those same rules support a second historical era. Add alien phenomena after the normal behavior is reliable enough for their exceptions to be intelligible.
