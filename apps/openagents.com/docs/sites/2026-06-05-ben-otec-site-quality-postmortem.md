# Ben OTEC Site Quality Postmortem

Date: 2026-06-05

Reviewed surface:

- Deployed URL: `https://sites.openagents.com/otec`
- Source artifact: `docs/sites/otec/index.html`
- Deployment trace: `docs/sites/2026-06-05-ben-otec-site-trace.md`
- Screenshot reviewed from browser at `sites.openagents.com/otec`

## Executive Assessment

The first OTEC Site deployment is not acceptable customer work. It proves that
the static Sites runtime can serve a custom slug, but it does not prove that
OpenAgents can fulfill a serious customer website order. It should be treated
as a deployment smoke test that accidentally reached a public customer-facing
standard, not as a usable deliverable.

The visual result is stark, underdesigned, hard to trust, and emotionally
wrong for the subject. The prompt asks for a website for ocean-based, OTEC
powered, SWAC cooled, gigawatt-scale floating datacenter infrastructure. The
output looks like a developer-made placeholder diagram with oversized terminal
type. It does not look like ocean infrastructure, energy infrastructure, a
datacenter concept, a venture-grade project page, an engineering brief, or a
credible customer artifact.

This is a process failure. The pipeline optimized for "get a public URL and
proof record" before requiring the checks that decide whether the thing is fit
to show to a human customer.

## What Is Bad

### The Hero Is Visually Crude

The headline is too large, too blocky, and too awkwardly wrapped. In the
screenshot the phrase breaks into:

- `OTEC`
- `powered.`
- `SWAC`
- `cooled.`
- `Built for`
- `floating`
- `compute.`

That is not a designed composition. It reads like the browser was allowed to
wrap an arbitrary line length at a huge font size. The result overwhelms the
screen without communicating sophistication.

The visual hierarchy is also backwards. The acronym block dominates, while the
actual product idea, floating datacenter infrastructure, is buried in body
copy. The page should immediately make the object feel real: ocean platform,
cooling pipes, thermal gradient, modular compute, power cycle, grid/offtake
context. Instead it leads with a typographic stunt.

### The Diagram Looks Like a Placeholder

The right-side graphic is the most important visual surface and it is not good
enough. It is a CSS box diagram with colored bands, rectangles, and a vertical
bar. It does not communicate scale, materiality, engineering plausibility, or
the relationship between OTEC, SWAC, and a floating datacenter.

Specific failures:

- The platform is represented by nested rectangles, not an actual platform.
- The water layers are generic tinted blocks.
- The vertical cold-water pipe is visually disconnected from real intake,
  pumping, heat exchange, and discharge concepts.
- There is no ocean surface, vessel, semi-submersible, mooring, datacenter
  module, turbine, condenser, heat exchanger, cable, or cooling loop detail.
- The labels overlap the diagram visually and feel pasted on.
- The illustration does not help a customer, investor, engineer, or agent
  understand what should be built next.

For a site about ocean infrastructure, this needs a real image, render, map,
technical cutaway, or carefully composed engineering schematic. A CSS diagram
is acceptable only as a temporary wireframe, and it should have been labeled as
internal draft quality.

### The Page Does Not Feel Customer-Specific

The order came from Ben and requested a concrete site concept. The deployed
page does not reflect customer discovery, domain research, buyer audience,
company voice, or a plausible target user. It does not answer basic questions:

- Who is this for?
- Is this a venture pitch, technical concept, project microsite, request for
  partners, investor memo, or public landing page?
- What is the proposed location or operating envelope?
- What scale is realistic now versus speculative?
- What proof points exist for OTEC, SWAC, offshore datacenters, and ocean
  energy?
- What should a visitor do next?

The current page mostly documents that OpenAgents created a proof artifact.
That is useful for internal accountability, but it is not the customer's site.

### The Copy Is Thin And Over-Caveated

The copy is careful, but it is too thin. The customer asked for a website, not
an internal caveat page. The text repeats "thesis", "proof", "claims", and
"receipts" instead of making a compelling public case for the concept.

The page should separate two jobs:

- Customer-facing site copy that makes the concept understandable and
  compelling.
- Public proof metadata that agents and operators can inspect.

Those should be linked, but the proof system should not dominate the customer
experience. The deployed page currently feels like an OpenAgents test harness
with a concept pasted into it.

### The Research Base Is Not Serious Enough

The evidence queue links to Wikipedia pages. That may be fine as a low-friction
starting point for a public proof record, but it is not enough for a serious
site about gigawatt-scale floating datacenter infrastructure.

The next version needs a research brief with higher-quality sources:

- OTEC technical references and deployment history.
- SWAC operating examples and thermal/cooling performance references.
- Offshore platform and mooring constraints.
- Datacenter heat-rejection and power-density assumptions.
- Environmental, permitting, and marine-risk constraints.
- Comparable ocean-energy, offshore wind, subsea cable, and maritime
  infrastructure projects.

Without that, the site either sounds vague or risks making claims it cannot
support.

### The Design Language Is Wrong

The current look is all black, mono type, thin borders, and engineering-console
styling. That may fit an internal operator panel, but it does not fit this
customer deliverable.

The subject calls for a more credible visual language:

- Ocean infrastructure, not terminal UI.
- Technical confidence, not hacker-console austerity.
- Energy and cooling systems, not generic dark dashboard blocks.
- Human-scale explanation, not a sparse proof artifact.

The site needs depth, imagery, and material cues. It can still be restrained,
but it should not look like a placeholder from an internal build system.

### The Public Deployment Bar Was Too Low

The most important failure is that this shipped publicly after only runtime
verification:

- URL returns 200.
- Static R2 artifact resolves.
- Proof endpoint links to the deployment.
- No obvious secret strings appear.

Those are necessary checks, but they are not enough. They say the site is
servable. They do not say the site is good, useful, on-brand, customer-safe,
or remotely worth showing.

## What A Better OTEC Site Should Be

The next OTEC iteration should be rebuilt as a real customer-facing concept
site, not patched lightly.

Minimum target:

- A first viewport that immediately shows an ocean platform or technical
  cutaway, with the OTEC/SWAC/datacenter relationship visible.
- A headline that says what the project is in plain language, not just acronym
  fragments.
- A clear audience and call to action, for example partner/investor inquiry,
  technical review, or concept exploration.
- A concise systems explanation: warm water loop, cold water loop, compute
  load, power conversion, heat rejection, mooring/cabling, resilience.
- A "what is proven / what is speculative" section that is visually secondary,
  not the whole product.
- A source-backed research section with serious references.
- Agent-facing proof links that are present but not dominant.
- Responsive layouts checked on desktop and mobile before deployment.

Possible structure:

1. Hero with generated or sourced ocean-infrastructure visual.
2. Concept summary: floating datacenter powered by OTEC and cooled by SWAC.
3. System diagram: OTEC loop, SWAC loop, compute platform, export/load model.
4. Why ocean thermal infrastructure: cooling efficiency, site constraints,
   resilience, offshore energy context.
5. Feasibility and unknowns: scale, environment, permitting, maintenance,
   mooring, cables, economics.
6. Evidence and proof: public sources, OpenAgents trace, agent challenge.
7. Next actions: research brief, technical concept art, customer review.

## Required Process Improvements

### Add A Pre-Deployment Design Gate

Every customer-visible Site needs a gate before public deployment:

- Capture desktop and mobile screenshots.
- Review visual hierarchy, responsiveness, text wrapping, and brand fit.
- Decide whether the page is customer-presentable or only an internal proof.
- Record the decision in the trace before deployment.

If the page is only a smoke test, deploy it under an internal/staging slug or
keep access restricted. Do not put it on the customer's final public slug.

### Separate Runtime Proof From Customer Acceptance

The current trace made deployment proof look like delivery proof. These are
different states.

The Sites records should distinguish:

- `runtime_verified`: artifact serves correctly.
- `internal_draft`: visible to team, not customer-ready.
- `customer_review_ready`: good enough to send for feedback.
- `customer_accepted`: explicitly accepted or otherwise approved.

The OTEC page should currently be considered `runtime_verified` and
`internal_draft`, not customer-review-ready.

### Require Research Before Domain-Specific Sites

For domain-specific customer sites, Adjutant should run a research pass before
designing the page. The research pass should produce:

- Audience assumption.
- Top-level narrative.
- Claim inventory.
- Source list.
- Risks and caveats.
- Suggested visual direction.

The site build should use that research brief. The trace should link the brief.

### Require Visual Asset Strategy

The current artifact has no serious visual assets. For website orders,
Adjutant should choose one of these before implementation:

- Use a customer-provided visual asset.
- Use a verified public/sourceable image.
- Generate a bitmap concept visual.
- Build a proper technical SVG/canvas schematic.
- State explicitly that the output is a wireframe.

For the OTEC order, a generated or researched hero image plus a technical
cutaway would be much better than CSS rectangles.

### Add A Customer-Facing Copy Review

The copy should be reviewed separately from the proof metadata. Checklist:

- Does the page explain the actual customer concept?
- Does the headline read naturally?
- Does the first screen make sense without OpenAgents context?
- Are proof caveats present but not overwhelming?
- Are claims source-backed or clearly labeled as speculative?
- Is there a meaningful next action?

### Add Screenshot-Based Regression To Sites Fulfillment

The process should save screenshots in the trace or attach them to proof
records. Review should include:

- Desktop first viewport.
- Mobile first viewport.
- Full-page screenshot.
- Text overflow/wrapping scan.
- Link and CTA sanity check.

This would have caught the giant awkward headline and placeholder diagram
before the page was called deployed.

### Add A "Do Not Share" Flag

A Site can be technically deployed but not shareable. The system needs an
explicit flag or proof state for this:

- `shareable: false`
- reason: `visual_quality_failed`
- next action: `redesign_before_customer_review`

That lets agents keep working publicly without implying the customer should see
the result.

### Create A Sites Quality Rubric

Adjutant should score each Site before deployment:

- Customer fit.
- Visual quality.
- Responsiveness.
- Content depth.
- Source quality.
- Proof/agent friendliness.
- Accessibility.
- Secret/privacy safety.
- Performance.

A low score should block deployment to the final public slug unless an operator
explicitly marks the deployment as a technical smoke test.

## Immediate Remediation Plan

1. Mark the current OTEC deployment internally as a runtime smoke test, not a
   customer deliverable.
2. Create a second OTEC iteration plan before making more public changes.
3. Run a research pass on OTEC, SWAC, offshore datacenters, and relevant
   infrastructure precedents.
4. Generate or source a real visual direction for the concept.
5. Rebuild the page around a customer-facing narrative.
6. Capture screenshots and run the design gate before replacing the deployed
   artifact.
7. Update the public proof to say that v0 was infrastructure validation and v1
   is the first customer-review candidate.

## Bottom Line

The current OTEC Site is bad enough that it should not be presented to the
customer as fulfilled work. It is useful only as proof that the custom slug,
R2 artifact path, D1 Site records, and public proof endpoint can work together.

The next process milestone should not be another deployment. It should be a
quality gate that prevents a deployment like this from being mistaken for
customer-ready work.
