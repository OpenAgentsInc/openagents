# OTEC for SLAB: a rigorous assessment

**Research cutoff: November 18, 2026**

**My assessment: OTEC is a credible power option for a tropical industrial platform, particularly when electricity, cooling and water all have paying customers. It is not yet a demonstrated, financeable solution for supplying SLAB with 120 MW of continuous power.** It fails the geographic test off California. Bitcoin mining can help absorb surplus generation, but current mining economics make it a weak sole customer for a first commercial OTEC plant.

The strongest development strategy is **integration testing in Kona, followed by a conditional 5–10 MW net demonstration in Guam**, with Puerto Rico as an Atlantic alternative. Expansion should depend on measured net output, seasonal performance, maintenance costs and bankable customer contracts.

**Full report:** [OTEC_for_SLAB_Research_Report_2026-11-18.pdf](OTEC_for_SLAB_Research_Report_2026-11-18.pdf)

The PDF contains **33 pages, a 48-entry project and facility inventory, the ranked site atlas, calculation assumptions and 87 references**. The principal findings and supporting analysis follow.

---

## 1. The thesis: where it holds and where it fails

| Proposition | Assessment |
|---|---|
| Tropical oceans contain a large, continuously replenished thermal resource. | **Supported.** But an abundant resource does not establish economical extraction. |
| OTEC’s output profile suits a data center. | **Yes, conditionally.** It avoids the daily solar cycle, but seasonal derating, maintenance and equipment failures remain. |
| Deep seawater can provide electricity and cooling. | **Yes.** This is one of the strongest integration opportunities, provided cooling does not materially degrade the power cycle. |
| OTEC is suitable off California. | **No for conventional ambient-temperature OTEC.** The surface water is too cold, particularly in winter. |
| OTEC can currently be procured as a proven 100–120 MW power plant. | **Not established.** Demonstrations have produced positive net power, but sustained commercial operation at this scale has not been demonstrated. |
| Bitcoin makes expensive OTEC power economical. | **Only under favorable assumptions.** Mining competes against electricity worldwide; island retail electricity prices do not determine its willingness to pay. |
| Moving offshore removes state and local regulation. | **Not reliably.** The OTEC Act can incorporate adjacent-state or territory law into federal law for certain connected offshore facilities. |

These conclusions distinguish **demonstrated performance**, **engineering estimates**, **developer claims** and **my calculations**. That separation matters because OTEC discussions frequently mix turbine ratings, gross electrical production, net output and prospective commercial capacity. NOAA’s technical material and the major engineering assessments explicitly distinguish those quantities. [coast.noaa.gov](https://coast.noaa.gov/data/czm/media/technicalfactsheet.pdf)

### The SLAB baseline

Using the assumptions in your earlier report:

\[
100\ \text{MW IT}\times1.20\ \text{PUE}=120\ \text{MW facility load}
\]

\[
120\times8{,}760=1{,}051{,}200\ \text{MWh/year}
\]

Therefore:

- Each **1¢/kWh** changes the annual electricity bill by **$10.512 million**.
- At **$200/MWh**, annual electricity expenditure is **$210.24 million**.
- A **100 MW net** OTEC plant supports only **83.3 MW of IT load** at PUE 1.2, before other industrial tenants.
- At an assumed **90% capacity factor**, **133.3 MW** of nameplate capacity supplies the annual energy equivalent of a continuous 120 MW load. That calculation does **not** establish uninterrupted power.

Availability, annual energy production and firm capacity are different requirements.

---

## 2. How OTEC works

OTEC operates a heat engine between warm surface seawater and cold deep seawater, usually drawn from approximately 600–1,000 metres or deeper, depending on local conditions.

### The three main cycles

| Cycle | Process | Main implications |
|---|---|---|
| **Closed cycle** | Warm seawater evaporates a low-boiling working fluid, commonly ammonia. Its vapor drives a turbine, then cold seawater condenses it. | The leading electricity-focused architecture. Seawater and working fluid remain separate. |
| **Open cycle** | Warm seawater enters a vacuum chamber and partially flashes into steam. The low-pressure steam drives a turbine. | Can produce desalinated water when the steam is condensed without remixing it with seawater. Requires large low-pressure equipment and vacuum-system management. |
| **Hybrid** | Combines open-cycle flashing or desalination with a closed working-fluid power loop. | Can integrate power and water production, but adds equipment and operating interactions. |

Closed-cycle OTEC does **not automatically produce freshwater**. Desalination requires an additional process or a suitable open/hybrid configuration. [coast.noaa.gov](https://coast.noaa.gov/data/czm/media/technicalfactsheet.pdf)

### The thermodynamic ceiling

The maximum possible efficiency of a heat engine is:

\[
\eta_{\text{Carnot}}=1-\frac{T_c}{T_h}
\]

Temperatures must be in kelvin.

| Illustrative temperatures | Difference | Carnot maximum |
|---|---:|---:|
| Warm 25°C; cold 5°C | 20°C | **6.71%** |
| Warm 29°C; cold 5°C | 24°C | **7.94%** |

Actual performance is lower because heat exchangers need temperature differences to transfer heat, turbines and generators have losses, and pumps consume electricity. A practical **net thermal efficiency around 2–3% is a useful design order of magnitude**, not a guaranteed result. Small systems and marginal sites can perform substantially worse.

This low efficiency means enormous heat and seawater flows are needed for relatively modest electricity production. [irena.org](https://www.irena.org/-/media/Files/IRENA/Agency/Publication/2014/Ocean_Thermal_Energy_V4_web.pdf)

### Gross output versus net output

The relevant electricity is:

\[
P_{\text{net}} =
P_{\text{generator}}
-P_{\text{seawater pumps}}
-P_{\text{working-fluid pumps}}
-P_{\text{vacuum and other auxiliaries}}
\]

A DOE-funded 2012 engineering assessment modeled approximately **132–137 MW gross** to deliver **100 MW net**, depending on location. Those were design estimates, not operating results from a 100 MW plant. [digital.library.unt.edu](https://digital.library.unt.edu/ark:/67531/metadc842592/m2/1/high_res_d/1045340.pdf)

For a separate illustrative assumption of parasitic consumption equal to **30% of gross output**:

\[
120/(1-0.30)=171.4\ \text{MW gross}
\]

The plant would generate 171.4 MW internally and consume 51.4 MW itself to deliver 120 MW.

A headline such as “a 100 MW OTEC plant” is therefore incomplete unless it specifies **gross or net**, inlet temperatures and the measurement boundary.

### Why 20°C is a screening threshold

OTEC does not suddenly stop working at 19.9°C. However, declining temperature difference reduces output while much of the pumping requirement remains.

An illustrative sensitivity—not a universal plant model—is to assume gross output scales with \(\Delta T^2\), with pumping fixed at 30% of gross output at a 24°C design condition. A reduction to 20°C then leaves:

\[
\frac{(20/24)^2-0.30}{1-0.30}=56.3\%
\]

of design net output.

That explains why **winter minimum temperatures and actual temperatures at the heat exchangers matter much more than annual-average ocean temperatures**.

### Useful coproducts

- **Cooling:** Cold seawater can replace much of the work otherwise done by refrigeration equipment.
- **Freshwater:** Open/hybrid configurations can produce desalinated water, subject to equipment, energy and distribution costs.
- **Aquaculture:** Deep seawater offers low temperatures and nutrients useful for selected cultivation systems.

These benefits need real customers. Nutrient-rich discharge into the ocean is also an environmental issue; it is not automatically a beneficial aquaculture product. [JICA](https://www.jica.go.jp/english/information/topics/2023/20231117_01.html)

---

## 3. What has actually been built?

**Positive net generation has been demonstrated. Sustained commercial megawatt-scale net generation has not been established in the public evidence reviewed.**

There is no authoritative, continuously updated global OTEC registry. The PDF therefore provides a bounded inventory of identifiable generating plants, relevant component demonstrations and substantive proposals, rather than claiming that every announcement worldwide can be exhaustively accounted for.

### Built ocean-fed generating systems

| Project | Size and dates | Status and significance |
|---|---|---|
| **Claude, Cuba** | Approximately 22 kW gross, 1930 | Historic experiment; no positive net output. Pipe damage contributed to its end. |
| **Mini-OTEC, Hawaii** | Approximately 50–53 kW gross and 10–18 kW net under reported operating conditions, 1979 | Short demonstration completed. Established positive net generation. |
| **Nauru** | 100 kW continuous gross rating; reported peaks of 120 kW gross and 31.5 kW net, 1981–82 | Demonstration completed. Gross rating must not be treated as net export. |
| **Tokunoshima, Japan** | Approximately 50 kW rated, beginning 1982 | Historic experimental installation; not evidence of a current commercial plant. |
| **Hawaii open-cycle experiment** | Achieved approximately 255 kW gross and 103 kW net, 1993–98 | Demonstration completed. An important verified net-output milestone. |
| **Kumejima, Okinawa** | 100 kW nominal installation, operating research program since 2013 | Research and multiproduct seawater demonstration. No audited record of continuous 100 kW net delivery located. |
| **Makai/NELHA, Kona** | 105 kW turbine-generator installed in 2015 | Active research infrastructure, especially heat exchangers. Turbine rating is not an annual net-generation result. |
| **KRISO K-OTEC1000** | 1 MW design class; 338 kW electrical output reported during September 2019 Korean trials | Trial completed; full commercial net performance not established. Subsequent Kiribati deployment delayed. |
| **Chinese South China Sea prototype** | 20 kW generator rating; 16.4 kW peak during 2023 trials, reported in September 2026 | Short sea trial. Deep-water lifting power was excluded from the reported system balance. |

The historical Hawaiian results are documented in government assessments; the current Makai and Kumejima installations are described by their operators and public institutions. The 2026 Chinese paper explicitly limits what its trial establishes about net performance. [govinfo.gov](https://www.govinfo.gov/content/pkg/GOVPUB-I-308587baa7c377053add3f132bcafac9/pdf/GOVPUB-I-308587baa7c377053add3f132bcafac9.pdf)

### Significant current development efforts

| Project | Announced scale | Status as of the research cutoff |
|---|---:|---|
| **NIOT Kavaratti, India** | Approximately 65 kW plus 100 m³/day freshwater | Government reported construction in March 2026, including a 3.8 km HDPE pipeline. Commissioning not verified. |
| **Kumejima expansion** | 1 MW | Latest MOL announcement targets **FY2031** operation, superseding earlier timelines. |
| **Global OTEC “Dominique,” São Tomé** | 1.5 MW | Development, approvals and financing activity; no verified commissioning. |
| **Global OTEC/ENOGIA, Kona** | 500 kW concept | Operator identifies the project as **Concept Phase**. |
| **KRISO, Kiribati** | 1 MW design | Environmental and deployment preparations; no verified commercial operation. |
| **KRISO/College of the Marshall Islands** | 50 kW | Development and design work associated with an aid program. |
| **Kwajalein, OTE/OCEES/Johnson Controls** | 17.5 MW concept | Approximately $3.50 million engineering contract completed in July 2026; this is not a construction award. |
| **Bahamas consortium study** | Not fixed | September 2026 funded feasibility work explicitly considers data-center cooling, electricity and water. |
| **Mauritius** | 1 MW pilot concept | Government feasibility and seawater-intake studies; not commissioned. |
| **TCC Ho-Ping, Taiwan** | 1–2 MW first phase | Developer targets end-2029. Uses warmer power-station discharge, so it is **waste-heat-assisted**, not a pure ambient-ocean benchmark. |

The distinctions between construction, engineering contracts, feasibility studies and operating plants are essential here. [Press Information Bureau](https://www.pib.gov.in/PressReleasePage.aspx?PRID=2236326&lang=2&reg=48)

Additional substantive development programs include **Barbados**, **Sabah**, and **Australia/Indo-Pacific partnerships**. The PDF also separates laboratory or structural demonstrations—Saga, Réunion, Delft, Mexico, Guangdong, Malaysia and PLOTEC—from ocean-fed electricity plants. In particular, **PLOTEC’s offshore platform and riser demonstration is not a generating OTEC power station**.

### Large projects that stalled

The important precedents include:

- **India’s Sagar Shakti, 1 MW:** Failed cold-water-pipe deployments in 2001 and 2003; project wound up in 2005.
- **Lockheed Martin/Makai’s Hawaiian 10 MW effort:** Did not proceed to a completed generating plant.
- **Lockheed Martin/Reignwood’s Chinese 10 MW proposal:** Announced through agreements in 2013; no verified commissioning.
- **NEMO, Martinique:** Planned approximately **16 MW gross/10 MW net**, with substantial prospective European support. Paused in 2018 and subsequently abandoned amid technical-development and cost difficulties.
- **Kahe, Hawaii, 40 MW; NAUTILUS, Martinique, 5.7 MW; Curaçao proposals; Bahamas proposals; St Croix; and Grand Cayman’s 25 MW proposal:** Remained unbuilt, suspended or unrealized in the evidence reviewed.

These projects did not all fail for the same reason. Recurring barriers were **pipe deployment risk, pumping and heat-exchanger performance, high initial capital cost, insufficient commercial guarantees, financing and competing energy costs**. India’s public audit is especially valuable because it documents actual failed deployments rather than prospective vendor assurances. [CHAPTERVII.doc](https://saiindia.gov.in/uploads/old_reports/union/union_compliance/2007_2008/Civil/Report_no_3/chap_7.pdf)

---

## 4. The engineering bottleneck is the seawater system

### Flow per net megawatt

A peer-reviewed preliminary design for a **100 MW net** plant near Mentawai, Indonesia, used approximately:

- **235 m³/s cold seawater**
- **470 m³/s warm seawater**
- **705 m³/s combined flow**

Those are modeled design quantities, not measurements from an operating commercial plant. [Springer Nature Link](https://link.springer.com/article/10.1007/s00773-019-00630-7)

Linearly scaling that design gives the following illustration. Pipe diameters assume one equivalent circular cold-water pipe with **2 m/s** average velocity:

\[
D=\sqrt{\frac{4Q}{\pi v}}
\]

| Net electrical output | Cold flow | Warm flow | Equivalent cold-pipe diameter |
|---:|---:|---:|---:|
| 1 MW | 2.35 m³/s | 4.7 m³/s | 1.22 m |
| 10 MW | 23.5 m³/s | 47 m³/s | 3.87 m |
| 100 MW | 235 m³/s | 470 m³/s | 12.23 m |
| 120 MW | 282 m³/s | 564 m³/s | 13.40 m |

These are scaling calculations, not engineered designs. Multiple pipes are possible. Higher flow velocity reduces diameter but increases hydraulic losses.

### Compare that with data-center cooling

For the earlier 120 MW facility illustration, assume:

- Heat rejection: **120 MW**
- Seawater volumetric heat capacity: **4.1 MJ/(m³·K)**
- Temperature rise: **10°C**

Then:

\[
Q_{\text{cooling}}=\frac{120}{4.1\times10}
=2.93\ \text{m³/s}
\]

At 120 MW net OTEC output, the scaled flows above are approximately:

- **96 times** the data-center cooling flow for the cold-water circuit alone.
- **289 times** the cooling flow when warm and cold circuits are combined.

**A deep-water cooling project is therefore a much smaller hydraulic undertaking than an OTEC plant that also generates the data center’s electricity.**

### A 1,000-metre intake does not require 1,000 metres of pump head

The seawater columns largely balance hydrostatically. Pumps overcome friction, heat-exchanger losses, density effects and elevation differences—not simply the full ocean depth.

Nevertheless, modest hydraulic losses become large power loads at these flows. With assumed seawater density 1,025 kg/m³, flow 282 m³/s, effective head 10 m and pump efficiency 85%:

\[
P=\frac{\rho gQH}{\eta}=33.4\ \text{MW}
\]

That is an illustrative **cold-water pumping requirement alone**. It must not be added again to a net-power estimate that already includes pumping.

### What must survive

The cold-water pipe is simultaneously a hydraulic system and a large marine structure. Critical issues include:

- Installation, towing and upending.
- Connections to a moving platform.
- Wave and current loads, vortex-induced vibration and fatigue.
- Pressure transients, buckling and accidental damage.
- Inspection, repair and replacement.

HDPE, composites and steel each create different compromises between flexibility, stiffness, corrosion resistance, weight and installation complexity. India’s failed deployments demonstrate that installation risk is material even at much smaller dimensions than a 100 MW design. [CHAPTERVII.doc](https://saiindia.gov.in/uploads/old_reports/union/union_compliance/2007_2008/Civil/Report_no_3/chap_7.pdf)

Heat exchangers are the other central challenge. They must transfer enormous heat flows while imposing very little pressure loss. Makai/HNEI’s August 2025 report describes two **2 MW thermal** thin-foil heat exchangers: the evaporator met modeled performance, while the condenser underperformed, probably because of assembly issues. In a one-year fouling comparison, pressure loss increased approximately **400% without cleaning versus 13% with cleaning**. Those are component results, not whole-plant electrical performance. [hnei.hawaii.edu](https://www.hnei.hawaii.edu/wp-content/uploads/OTEC-Heat-Exchanger-Development-2024-2025.pdf)

Moorings, power cables, corrosion control and repair access add further constraints. A platform’s proposed 1,000-year structural life would not imply comparable lifetimes for pumps, turbines, heat exchangers or computing hardware.

---

## 5. Economics: published estimates span a very wide range

**No independently audited commercial fleet establishes OTEC’s delivered cost at 10 or 100 MW.** Published figures are predominantly engineering models, historical compilations or developer claims.

### Size and cost evidence

| Evidence | Plant size | Capital cost | Electricity cost | How to interpret it |
|---|---:|---:|---:|---|
| **IRENA, June 2014** | 1–10 MW | Approximately **$16,400–35,400/kW**, in the cited 2010-dollar capital-cost range | Compiled estimates: roughly **$510–940/MWh** for 1–1.35 MW; **$190–450/MWh** for 10 MW | Historical compilation with differing assumptions; not current quotations |
| **IRENA, June 2014** | 100 MW | Design-dependent | Approximately **$70–190/MWh** | Prospective scale economies, not operating evidence |
| **DOE-funded model, 2012** | 100 MW net | Approximately **$1.40–1.67 billion**, 2010 basis, depending on location | Representative result around **$177/MWh**, constant January 2010 dollars | Detailed engineering estimate, substantially industry-developed |
| **IEA-OES/Vega–Martin, March 2024** | 10.6 MW | **$21,606–27,012/kW** | Commercial financing: **$492–615/MWh**; concessional: **$370–462/MWh** | Updated supplier-based engineering model |
| **Same 2024 study** | 53.5 MW | **$11,223–16,578/kW** | Commercial: **$256–378/MWh** | Modeled scale economies |
| **Same study, 100 MW-class extrapolation** | Approximately 100 MW | **$8,817–13,023/kW** | Commercial: **$201–297/MWh** | Extrapolation, not a delivered project |
| **Global OTEC FAQ, retrieved November 2026** | Early-generation projects | Not fully disclosed | Approximately **$150–300/MWh claimed** | Vendor claim without a fully disclosed comparable basis |

Sources and dates should travel with these figures; averaging them would produce a misleading answer. [irena.org](https://www.irena.org/-/media/Files/IRENA/Agency/Publication/2014/Ocean_Thermal_Energy_V4_web.pdf)

Two qualifications are particularly consequential:

1. The 2024 study uses refreshed **2023 supplier quotations** and industry-linked modeling. It is more current than the frequently repeated 2014 figures, but it is not an independent audit of an operating plant.
2. Its mooring assumptions concern locations **unaffected by hurricanes**. Guam or Caribbean projects require different storm, repair, insurance and contingency assumptions. [tethys-engineering.pnnl.gov](https://tethys-engineering.pnnl.gov/sites/default/files/publications/Vega-Martin-2024.pdf)

Small-plant estimates also need boundary checks. A quoted power-block price can exclude tens of millions of dollars of shared seawater infrastructure.

### Financing sensitivity

My illustrative calculation uses:

- Installed generation capital cost: **$10,000–20,000 per net kW**
- Life: **30 years**
- Capacity factor: **90%**
- Annual operating allowance: **3% of initial capital**
- Real financing rates: **4%, 8%, 12%**

\[
LCOE=
\frac{1{,}000C\,[CRF(r,30)+0.03]}{8{,}760\times0.90}
\]

| Capital cost | 4% finance | 8% finance | 12% finance |
|---:|---:|---:|---:|
| $10,000/kW | $111/MWh | $151/MWh | $196/MWh |
| $15,000/kW | $167/MWh | $226/MWh | $293/MWh |
| $20,000/kW | $223/MWh | $301/MWh | $391/MWh |

This is a sensitivity model, not a complete project budget. Backup power, construction financing and project-specific replacement, insurance and decommissioning provisions need explicit treatment.

### Compare equivalent services

| Alternative | Dated evidence or explicit assumptions | Implication |
|---|---|---|
| **Island diesel** | My fuel-only calculation: delivered fuel **$0.80–1.20/litre**, 38% efficiency, 35.8 MJ/litre → **$212–318/MWh** | OTEC can plausibly compete with expensive imported fuel, but both need full lifecycle costing. |
| **LNG/gas** | My fuel-only calculation: **$8–16/MMBtu**, 50% efficiency → **$55–109/MWh** | Terminal, shipping, storage, plant and financing costs are additional. Gas can still be formidable competition. |
| **Utility solar** | IRENA’s July 2026 report: 2025 global average approximately **$44/MWh** | Variable generation, not continuous island supply. |
| **Solar plus storage** | IRENA May 2026 Las Vegas example: approximately **$113/MWh** at **95% annual energy served** | This is not uninterrupted data-center power and not an island-specific bid. |
| **Offshore wind** | IRENA 2025 global average approximately **$78/MWh** | Requires assessment of variability, backup, cyclone design and local wind resource. |
| **Small nuclear reactors** | IEA January 2025 scenario: approximately **$4,500/kW in the U.S./Europe by 2040** under accelerated learning | A future scenario, not a present floating-reactor quotation. Licensing and delivery risks remain. |

The solar and wind numbers are observed global project-cost benchmarks; the firm-solar and nuclear figures are modeled scenarios. They should not be treated as interchangeable evidence. [irena.org](https://www.irena.org/Publications/2026/Jul/Renewable-Power-Generation-Costs-in-2025)

OTEC’s strongest economic case is against **expensive island fuel plus valuable cooling and water services**. Its case is much weaker against inexpensive mainland electricity.

---

## 6. Ranked site atlas

### What a qualifying site actually needs

A credible site assessment requires:

- A sufficient **minimum**, not merely average, temperature difference.
- Actual temperatures after the seawater travels through the intake system.
- A surveyed route to deep water.
- Mooring, geotechnical and metocean studies.
- Customers, repair access, fiber and permits.

**Water depth is not pipeline length.** Reaching 1,000-metre water may require several kilometres of pipe.

The ranking below prioritizes overall SLAB feasibility. It does not certify that every listed location maintains at least 20°C at the plant inlet every day.

### U.S. jurisdictions

| Rank | Location | Thermal and bathymetric assessment | Principal advantage | Principal weakness |
|---:|---|---|---|---|
| **1** | **Guam** | Strong tropical candidate. Historical monthly surface values approximately 27.7–29.2°C; steep western bathymetry offers comparatively short deep-water access. | Fiber hub, port, airport and substantial institutional demand | Typhoons, reef constraints and storm-resistant infrastructure cost |
| **2** | **Puerto Rico, south/southeast** | Strong regional prospect, but strict winter minima require site-specific verification. A Yabucoa study case used roughly 4.7 km of pipe to 1,045 m depth. | Access to U.S. Atlantic markets and a larger customer base | Hurricanes, seismic exposure and site-dependent winter margins |
| **3** | **Kona, Hawaii** | Best-established U.S. research ecosystem. Winter temperature margin can be narrow, especially after intake warming. | NELHA infrastructure, experienced operators and seawater tenants | Strict year-round 20°C criterion cannot be assumed |
| **4** | **St Croix, U.S. Virgin Islands** | Favorable tropical resource; study examples reach approximately 1,000 m north of Christiansted over 6–7 km | Potential electricity, cooling and water demand | Hurricanes and a relatively small market |
| **5** | **Tutuila, American Samoa** | Promising tropical temperatures and steep bathymetry; modern site-specific minima need stronger evidence | Potential multiproduct island demand and operational subsea connectivity | Remoteness, cyclone/tsunami exposure and maintenance logistics |
| **6** | **Northern Mariana Islands** | Plausible southern-island candidates; route and seasonal temperatures need qualification | U.S. jurisdiction and tropical resource | Typhoons, smaller markets and limited support infrastructure |
| **7** | **Florida Straits / southeast Florida** | Conditional continental-U.S. exception. Annual averages can look attractive while winter minima fail the strict screen. | Mainland markets and logistics | Seasonality, strong currents and offshore installation conditions |
| **8** | **U.S. Gulf of Mexico** | Warm-season resource; winter cooling and a broad continental shelf weaken the case | Existing offshore industry | Poor fit for a dependable year-round thermal resource near shore |
| **Exclude for conventional OTEC** | **California** | Surface temperatures are insufficient for the required year-round gradient | Strong market and fiber access cannot fix the thermal resource | Fundamental temperature limitation |

The Guam, Puerto Rico/USVI and NELHA findings are supported by government, university and operator studies. The University of Guam’s October 2026 report remains explicitly preliminary and identifies seawater cooling as a nearer-term opportunity. [digital.library.unt.edu](https://digital.library.unt.edu/ark:/67531/metadc1098475/)

### California and the continental U.S.: the direct answer

NOAA’s San Clemente Basin climatology gives approximately **15.2°C in February and 20.9°C in September**. In winter, even hypothetical 0°C deep water would not provide a 20°C difference. California therefore fails the conventional OTEC resource test. Deep-water cooling is a separate possibility. [All Coastal Regions table](https://www.ncei.noaa.gov/access/coastal-water-temperature-guide/all_table.html)

It would nevertheless be too broad to say that **every continental-U.S. location is physically impossible**. Southeast Florida warrants conditional analysis. The important distinction is between an attractive annual-average resource and sufficient output through the worst season. Independent National Academies analysis cautions against treating broad regional resource estimates as deployable generation. [nationalacademies.org](https://www.nationalacademies.org/read/18278/chapter/7)

### Allied, associated and other countries

| Priority | Regions worth examining | Main qualification |
|---|---|---|
| **High** | Palau, Federated States of Micronesia, Marshall Islands | Strong tropical resource; small markets and remote servicing. These are sovereign freely associated states, not U.S. territories. |
| **High** | Philippines, Indonesia, Sabah | Favorable tropical deep-water settings; site selection must address storms, earthquakes, bathymetry and customers. |
| **High to medium** | São Tomé and Príncipe | Strong developer and national-planning interest; financing and verified operating conditions remain decisive. |
| **High to medium** | Maldives outer atolls | Strong potential, but some published attractive gradients use **1,500 m**, not 1,000 m, depth. |
| **Medium** | Martinique, Guadeloupe, Cayman and selected steep Caribbean coasts | Useful proximity to deep water; hurricane design is central. |
| **Medium** | Nauru and Kiribati | Attractive thermal conditions; tiny grids, remoteness and deployment economics. |
| **Conditional** | Seychelles, Comoros, Mayotte, Mauritius and Réunion | Some locations have winter gradients near or below the screening threshold. |
| **Conditional** | Bahamas and Okinawa | Important projects and studies, but winter temperatures can substantially reduce output. |

Peer-reviewed Pacific and Caribbean mapping supports concentrating on steep tropical islands and selected deep-water coastlines, rather than treating the tropics as uniformly suitable. [mdpi.com](https://www.mdpi.com/1996-1073/17/11/2766)

Kumejima illustrates the distinction: its existing intake is about **612 m** deep, with approximately **9°C** deep water and surface temperatures reported around **20–30°C**. An operating research installation does not demonstrate a year-round 20°C difference.

---

## 7. Fit with AI data centers

### The match is real, but “always on” needs qualification

OTEC’s resource does not disappear at sunset. That suits flat computing loads and can reduce the storage requirement relative to solar.

However, a data center needs more than an annual energy balance. It needs protection against:

- Pipe or pump outages.
- Heat-exchanger cleaning.
- Turbine maintenance.
- Storm shutdowns.
- Cable failures.
- Seasonal output reductions.

Several generating modules sharing one intake pipe do not provide full redundancy against failure of that pipe.

### Reuse the cold water carefully

A promising arrangement is:

**Deep-water intake → OTEC condenser → secondary data-center cooling heat exchanger → controlled discharge**

The appropriate sequence depends on actual temperatures. Sending water through a large cooling load before the OTEC condenser can reduce power-cycle efficiency. A split flow may be preferable.

Seawater should remain isolated from the data center’s internal cooling loops. SWAC eliminates or reduces refrigeration work; it does not eliminate pumps, heat exchangers or water treatment. [40+ Years of Proven Experience](https://www.makai.com/swac-seawater-air-conditioning/)

At the illustrative **282 m³/s** cold-water flow, adding **120 MW** of heat raises bulk water temperature by only:

\[
\Delta T=\frac{120}{4.1\times282}=0.104^\circ\mathrm C
\]

That shows substantial thermal capacity, although it does not establish that the water temperature is suitable at every cooling stage.

If cooling integration reduced PUE from **1.20 to 1.10**, a 100 MW IT facility would save:

- **10 MW**
- **87.6 GWh/year**
- **$17.52 million/year at an assumed $200/MWh**

That benefit is meaningful, but it does not independently solve electricity generation.

### Fiber and latency

Guam has operational cable-landing infrastructure and data-center services. Hawaii is also well connected, but an important geographic detail is that the SEA-US landing is at **Mākaha, Oahu**, not Kona. A Kona installation needs adequately redundant interisland and terrestrial connectivity. [GTA](https://www.gta.net/cable-landing-station)

For perspective, using approximate great-circle distance and light speed in fiber of **200,000 km/s**:

| Route | Approximate distance | Ideal propagation-only round trip |
|---|---:|---:|
| Kona–Los Angeles | 4,044 km | 40 ms |
| Guam–Tokyo | 2,524 km | 25 ms |
| Guam–Los Angeles | 9,801 km | 98 ms |
| Yabucoa–Miami | 1,705 km | 17 ms |
| Yabucoa–Northern Virginia | 2,557 km | 26 ms |
| Pago Pago–Los Angeles | 7,722 km | 77 ms |

These are physical lower bounds, not measured network latencies.

The best workload fit is therefore **local training clusters, batch computation and appropriately located inference demand**. Tight interconnection between geographically separated training clusters is a different requirement. Remote sites also need resident technicians, spare GPUs, corrosion control and reliable import and repair arrangements.

---

## 8. Bitcoin mining: operationally useful, economically demanding

Mining is attractive as a flexible load because it needs little bandwidth and can curtail rapidly. It could:

- Absorb generation beyond local demand.
- Provide an early customer while other tenants arrive.
- Allow a larger plant than the initial island market supports.
- Yield electricity to higher-value customers during shortages.

The limitation is revenue: miners compete in a global market.

### Dated market inputs

Luxor’s **November 16, 2026** snapshot reported approximately:

| Input | Value |
|---|---:|
| Bitcoin price | **$65,118/BTC** |
| Seven-day network hashrate | **967 EH/s** |
| Hashprice | **$30.45/PH/s/day** |
| Average transaction fees | **0.0229 BTC/block** |

These are a dated commercial data provider’s observations, not live November 18 quotations. [hashrateindex.com](https://hashrateindex.com/blog/hashrate-index-roundup-november-16-2026/)

### Revenue formula

Let:

- \(B\): bitcoin price, dollars/BTC.
- \(H\): network hashrate, EH/s.
- \(e\): ASIC efficiency, J/TH.
- \(u\): facility PUE.
- \(p\): pool fee.
- \(S\): subsidy, BTC/block.
- \(f\): transaction fees, BTC/block.

Assuming the long-run target of **144 blocks/day**:

\[
\boxed{
R_{\$/\mathrm{MWh}}
=
\frac{6B(S+f)(1-p)}{e\,u\,H}
}
\]

This is expected revenue per **facility MWh**, before electricity, ASIC purchases, maintenance, financing and taxes.

The subsidy is currently **3.125 BTC/block**, with halvings every **210,000 blocks** under Bitcoin’s consensus rules. [raw.githubusercontent.com](https://raw.githubusercontent.com/bitcoin/bitcoin/v29.0/src/validation.cpp)

### Price and network scenarios

The following calculations assume:

- ASIC efficiency **15 J/TH**
- Facility PUE **1.05**
- Pool fee **2%**
- Subsidy **3.125 BTC/block**
- Fees **0.02 BTC/block**

**Expected revenue, dollars per facility MWh:**

| BTC price | 600 EH/s | 1,000 EH/s | 1,500 EH/s | 2,000 EH/s |
|---:|---:|---:|---:|---:|
| $50,000 | $98 | $59 | $39 | $29 |
| $100,000 | $196 | $117 | $78 | $59 |
| $150,000 | $294 | $176 | $117 | $88 |
| $200,000 | $391 | $235 | $157 | $117 |

Higher bitcoin prices can support more expensive power, but network competition can absorb much of the gain.

### Cross-check against observed hashprice

At **15 J/TH**, one PH/s consumes **15 kW**, or **0.36 MWh/day** before facility overhead:

\[
\frac{\$30.45}{0.36}\times\frac{0.98}{1.05}
=\$78.94/\mathrm{MWh}
\]

Under the same overhead and pool assumptions:

| ASIC efficiency | Revenue per facility MWh |
|---:|---:|
| 10 J/TH | $118 |
| 15 J/TH | $79 |
| 20 J/TH | $59 |
| 25 J/TH | $47 |

Efficiency matters greatly. Manufacturer specifications for newer machines can be better than the 15 J/TH scenario, but a product specification is not proof of fleet-wide delivered performance. [file12.bitmain.com](https://file12.bitmain.com/shop-product-s3/firmware/0dd419ca-c3e8-4cdf-8dba-f39db1b61354/2025/06/03/09/S23%20Hyd.%20Product%20Manual_v1.0.5.pdf)

### Revenue is not the electricity budget

Illustratively, assume:

- Hardware costs **$15/TH**.
- Efficiency **15 J/TH**, PUE **1.05**.
- Three-year life, **90% utilization**, zero residual value.
- No financing cost.
- Other operations cost **$8/MWh**.

Hardware amortization is approximately **$40.27/MWh**. From $78.94/MWh revenue, the remaining electricity budget is:

\[
78.94-40.27-8=\boxed{\$30.68/\mathrm{MWh}}
\]

The $15/TH hardware price is an assumption, not a current quotation.

This explains why mining may buy **otherwise unused output** while being unable to support the full cost of a new OTEC project.

### Halving exposure

With price, hashrate, efficiency and fees otherwise unchanged:

- The next subsidy halving, expected around **2028**, reduces the scenario revenue to approximately **50.3%** of today’s level.
- The following halving, around **2032**, reduces it to approximately **25.5%**.

Dates depend on block production. Price and network response are uncertain.

**Mining should be a curtailable supplementary customer.** Making it the sole anchor exposes a long-lived infrastructure project to much shorter hardware cycles and repeated subsidy reductions.

---

## 9. Law and regulation

### The OTEC Act remains relevant

The **Ocean Thermal Energy Conversion Act of 1980** remains in the U.S. Code, and NOAA continues to identify itself as responsible for the federal OTEC program.

But the original implementing regulations, **15 CFR Part 981**, were rescinded in **1996**. The Coast Guard’s associated plantship regulations had also been removed.

That creates a significant procedural distinction:

- The statute survives.
- The old regulations should not be presented as a current, ready-to-use application process.
- I did not locate a demonstrated track record of issued commercial licenses that would establish a routine path for a SLAB-scale project.

A developer needs a written licensing roadmap from NOAA and the other responsible agencies. [coast.noaa.gov](https://coast.noaa.gov/czm/thermalenergy/)

### A moored platform does not automatically escape U.S. jurisdiction

The Act distinguishes an OTEC **facility**, including fixed or moored structures, from a moving or unmoored **plantship**. It also addresses U.S.-documented facilities, facilities within U.S. territorial waters and facilities connected to the United States by pipeline or cable, alongside provisions for U.S.-citizen plantships. [uscode.house.gov](https://uscode.house.gov/us/usc/119-118/t42/s9102/)

A ship-like hull is therefore not sufficient to establish an exemption.

### The state and territory connection matters

Under **42 USC §9163(b)(2)**, certain licensed facilities beyond territorial waters that are connected by an **electric transmission cable or pipeline** can become subject to applicable, consistent laws of the relevant adjacent state **as federal law**.

Important qualifications:

- The statute’s definition of “State” includes relevant U.S. territories.
- This provision does **not** extend state taxation beyond state boundaries.
- A data-only fiber connection raises a different interpretive question from an electric transmission cable.
- The Act’s customs treatment does not settle vessel classification for every other law. [uscode.house.gov](https://uscode.house.gov/us/usc/119-118/t42/s9163/)

For SLAB, this creates a practical tension: the shore connections that improve financing, water sales, grid support and maintenance can also strengthen regulatory connections.

### Environmental assessment is substantial

The main concerns are:

- Entrainment and impingement of marine organisms.
- Large temperature, density and nutrient discharge plumes.
- Changes in oxygen and dissolved-carbon conditions.
- Biofouling treatments and cleaning discharges.
- Working-fluid leaks.
- Reef, seabed, anchoring and pipeline effects.
- Construction and operational interactions with protected species.

Discharging below the sunlit layer may reduce some nutrient risks, but no single depth is universally safe. Plume behavior and ecological consequences require site-specific analysis. [coast.noaa.gov](https://coast.noaa.gov/data/czm/media/environmentalfactsheet.pdf)

---

## 10. Recommended configuration and decision gates

### Recommendation

**Develop a tropical energy-and-cooling project with SLAB as a possible customer and platform configuration. Begin with measured integration performance before committing to hyperscale demand.**

| Phase | Configuration | Evidence required to proceed |
|---|---|---|
| **0. Site qualification** | Guam first commercial candidate; Puerto Rico Atlantic alternative | Paired temperature measurements, route survey, storm design, environmental baseline, fiber redundancy and customer commitments |
| **1. Integration demonstration** | Kona/NELHA, approximately **0.1–0.5 MW electrical test scale**, subject to verified available flow | Independently metered gross and net output, fouling behavior, cooling integration and maintainability |
| **2. Commercial demonstration** | **5–10 MW net** tropical installation | Full construction price, storm-qualified marine systems, credible guarantees, financing and independent backup |
| **3. Replication** | **20–50 MW**, with deliberate redundancy | Multi-year net-output, outage, maintenance and lifecycle-cost evidence |
| **4. Hyperscale** | More than **120 MW delivered capacity** for the 100 MW IT illustration, plus resilience provisions | Demonstrated supply-chain capability, replacement plans and contracted firm-power arrangements |

A 5–10 MW first commercial plant may require a demonstration budget or customers willing to pay for additional services and energy security. Current modeling does not make it automatically cheaper than existing island generation.

### Anchor-load mix

The strongest anchor is a creditworthy combination of **utility, industrial, institutional, cooling and water demand**. A small AI facility can provide additional stable demand. Mining should consume residual available electricity.

For illustration, a **10 MW net** plant could allocate:

| Use | Assumed load |
|---|---:|
| 3 MW IT at PUE 1.15 | 3.45 MW |
| Utility, industrial and water services | 4.00 MW |
| Curtailable mining | Up to 1.00 MW |
| Uncommitted operating margin | 1.55 MW |

This is an example, not a forecast of local demand. The margin does not cover a complete plant trip; critical loads need independent backup.

### What would make the thesis succeed?

OTEC becomes compelling if a specific project can demonstrate all of the following:

1. **Adequate winter net output**, measured at actual plant inlet conditions.
2. **A survivable, maintainable seawater system**, with a credible replacement plan.
3. **Competitive total delivered cost**, including financing, insurance and backup.
4. **Real customers for cooling and water**, without double-counting benefits.
5. **A reliable computing location**, including fiber, technicians, spares and logistics.
6. **A workable regulatory process** accepted by the relevant governments.
7. **An expansion plan based on measured performance**, rather than assumed economies of scale.

The decisive implication for SLAB is geographic and institutional: **choosing OTEC points toward tropical U.S. territories or other tropical jurisdictions, and toward close cooperation with shore-based customers and regulators.** It can improve the industrial platform’s energy proposition, but it does not substantiate either a California location or a broad promise of escaping state and local rules.

---

## Selected primary references

The PDF contains the complete 87-reference bibliography, including the individual project records.

1. **NOAA:** [OTEC technology factsheet](https://coast.noaa.gov/data/czm/media/technicalfactsheet.pdf) and [environmental impacts factsheet](https://coast.noaa.gov/data/czm/media/environmentalfactsheet.pdf).
2. **U.S. DOE-funded engineering assessment, 2012:** [OTEC Life Cycle Cost Assessment](https://digital.library.unt.edu/ark:/67531/metadc842592/m2/1/high_res_d/1045340.pdf).
3. **IEA-OES, March 2024:** [OTEC Economics: Updates and Strategies](https://www.ocean-energy-systems.org/documents/52058-otec-economics-march-2024-.pdf/).
4. **Adiputra et al., 2019/2020:** [Preliminary design of a 100 MW-net OTEC plant](https://link.springer.com/article/10.1007/s00773-019-00630-7).
5. **Makai/HNEI, August 2025:** [OTEC Heat Exchanger Development](https://www.hnei.hawaii.edu/wp-content/uploads/OTEC-Heat-Exchanger-Development-2024-2025.pdf).
6. **Comptroller and Auditor General of India, 2008:** [Audit of the OTEC demonstration and failed pipe deployments](https://saiindia.gov.in/uploads/old_reports/union/union_compliance/2007_2008/Civil/Report_no_3/chap_7.pdf).
7. **University of Guam, October 2026:** [OTEC and SWAC feasibility overview](https://zenodo.org/records/23469208).
8. **IRENA, 2026:** [Renewable Power Generation Costs in 2025](https://www.irena.org/Publications/2026/Jul/Renewable-Power-Generation-Costs-in-2025) and [24/7 Renewables](https://www.irena.org/-/media/Files/IRENA/Agency/Publication/2026/May/IRENA_TEC_24-7_renewables_2026.pdf).
9. **Luxor, November 16, 2026:** [Hashrate Index market snapshot](https://hashrateindex.com/blog/hashrate-index-roundup-november-16-2026/).
10. **U.S. law:** [42 USC §9163](https://uscode.house.gov/us/usc/119-118/t42/s9163/) and [NOAA’s 1996 rescission notice](https://www.govinfo.gov/content/pkg/FR-1996-05-09/pdf/96-11464.pdf).

