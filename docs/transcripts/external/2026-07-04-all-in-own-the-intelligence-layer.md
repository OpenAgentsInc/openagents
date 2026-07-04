# External: All-In (July 4th weekend 2026) — "Who Owns the Intelligence Layer"

Archived 2026-07-04 from a public thread summarizing the July 4th weekend
All-In episode. The Friedberg life-sciences/hub-spoke segment already
archived at `2026-07-03-friedberg.md` is part of this same episode. This is
the primary external validation corpus for the Reactor plan
(`docs/fable/2026-07-04-reactor-open-model-private-deployment-plan.md`) and
the "Own your AI" campaign (apollo plan §11). All claims/numbers below are
the show's, attributed to their speakers — usable in sales material as
third-party statements only, never as OpenAgents claims.

The framing line of the whole episode:

> **"You can't rent intelligence from the same place that rents it to your
> competitor."**

---

The July 4th weekend All-In turned into a long argument about who owns the
intelligence layer. The besties think enterprises just woke up to a trap
they had been walking into:

- **The Palantir–Nvidia deal is a bet against the model-layer duopoly.**
  Palantir will use Nvidia's Nemotron open models to build a custom
  frontier-quality model for US government agencies — and the agencies own
  the hardware, the data, and the weights. Sacks framed it as structural:
  an application company and a chip company both want a competitive model
  layer, so they are natural partners against a two-provider middle.
- **Alex Karp's CNBC "crashout" was actually the thesis.** Karp argued
  enterprises have lost trust in the frontier labs and want to own their
  compute, models, data, and alpha. Sacks translated it as a new
  definition of enterprise AI safety: safety means the model provider
  cannot hoover up your proprietary knowledge and turn it into its next
  product.
- **Figma is the cautionary tale that made it real.** Anthropic launched
  Claude Design into Figma's category; its chief product officer sat on
  Figma's board and resigned only 3 days before launch; Figma's stock is
  down ~50% this year while Anthropic's valuation surged. Sacks listed
  Claude Science, Security, Legal, Financial, and Code as the same move:
  dominate the model layer, then take the lucrative verticals.
- **The playbook has a name: Microsoft and Google.** Sacks argued
  Anthropic is running the operating-system strategy — own the layer
  everyone builds on, then walk up the stack. His Google receipt: fewer
  than half of searches now send you off-site.
- **The BCG number raises the stakes.** Chamath cited a BCG
  return-on-capital-employed study: cost of capital is back to its
  long-run 8–11%, and half of large US companies cannot earn returns
  above it. If you're teetering on your cost of capital, handing your
  alpha to a provider that may compete with you is not a luxury risk —
  it is fatal.
- **The 16.4× number is the whole argument in one data point.** Chamath
  ran a code-migration task through 8090's harness. Wrapping Claude was
  1.4× cheaper and 1.5× faster than Claude Opus alone. **Wrapping the
  best open-source model was 16.4× cheaper, at ~3× slower.** For a
  background task, three extra hours to cut cost 16× is not a close call.
- **Even at 100× cheaper, enterprises were saying no for the wrong
  reason.** An ex-Meta PM's point (via Chamath): companies reject open
  models over China and safety fears, when they could host those same
  open weights on their own GPUs in US data centers with nothing flowing
  back. The safety objection is backwards: **the leak is the data you
  hand the frontier labs.**
- **Friedberg: the frontier labs are trying to commoditize their own
  customers.** Anthropic signing up life-sciences companies to feed a
  life-focused model in exchange for early access; nearly everyone now
  refuses — data they spent billions generating becomes worthless once
  pooled. (Full segment: `2026-07-03-friedberg.md`.)
- **Deployment topology: big hubs → distributed spokes.** Friedberg's
  map: large hubs (foundation training), medium hubs (enterprise training
  clusters), distributed spokes including **on-prem inference in your own
  building**. Owning your weights is the point.
- **Chamath's endgame is running GLM himself.** With harness
  post-training and telemetry, an open Chinese model like GLM could get
  as good as Anthropic's Mythos: take GLM, control it soup-to-nuts on US
  hardware **with only US citizens touching it**, pay a fraction.
- **The Apple analogy.** Apple kept its stock apps basic to protect the
  ecosystem and collect its 30% tax. There is no 30% tax on open models —
  and worse, you cannot rent intelligence from the same place that rents
  it to your competitor without ending up identical to them.
- **Nvidia's open model is now good enough to matter.** Calacanis: you
  can't tell Nemotron from Claude on 95% of searches; Nvidia downplayed
  the model until customers signaled their own silicon ambitions.
- **Sacks sized the duopoly:** Anthropic ~$60B ARR, OpenAI ~$40B, nobody
  else with meaningful model-layer revenue. Policy line: the US doesn't
  ban monopolies, only anti-competitive tactics — but government should
  do nothing to make the duopoly more likely.
- **Token deflation call: 90%/year for three years** (Calacanis), putting
  the price of intelligence near free and making it rational to waste
  tokens on hardware you already own. Friedberg's split: 70/20/10 between
  big cloud, local, and other clouds.
- **A wave of platform lock-in spending is landing.** Microsoft ~$2.5B on
  forward-deployed engineers, Amazon ~$1B, plus OpenAI's version.
  Calacanis's read: enterprises will slam the door — letting a provider's
  engineers study your business is how it ends up in their model.
- **The server-per-employee prediction.** Calacanis: every employee gets
  $10–20k of local compute (Mac Studio / high-RAM Dell) running a
  personal local model syncing to a thin laptop. A server per person, so
  nothing leaks.
- **On jobs:** RAMP/Revelio study of 21,000+ US firms — heaviest AI
  spenders grew headcount ~10% over two years, entry-level +12%.
  Friedberg: no AI job loss yet, only clunky gradual value creation.
  Near-term displacement named: support, entry-level data entry/BPO,
  driving (Waymo as present-tense evidence). Friedberg's human-premium
  counternarrative: automation puts a premium on human interaction
  (Klarna's support-AI reversal cited).
- **The export-control episode** (Fable 5 controls lifted after two
  weeks; Mythos 5 restored to US customers ~June 26): Sacks says don't
  over-read it — a particular set of circumstances, not a standing lever.
- **The import question:** why block Chinese cars/drones but not Chinese
  open models? Sacks: a forked open model run on US hardware stops being
  Chinese; banning open source would isolate the US and impose a token
  tax on American enterprises — let the market decide if American open
  models win.
- **California fiscal segment** (Friedberg): $351B "balanced" budget with
  $20–40B borrowed, 65% budget growth in six years, top 1% paying $70B of
  $142B personal income tax, 8.9% corporate rate vs Texas zero; 1–1.5% of
  AGI leaving annually, 15+ Fortune 500 HQs gone since 2019, new 8%
  software sales tax; $1.4T debt + up to $1.5T unfunded pensions.

The line that framed the whole show, again, because it is the campaign:
**"You can't rent intelligence from the same place that rents it to your
competitor."** That is the sovereignty thesis in one sentence, and every
number in the episode is an argument for it.
