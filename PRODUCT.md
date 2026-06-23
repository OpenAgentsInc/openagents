# Product

## Register

brand

> A product+brand hybrid. The OpenAgents surfaces are real product (an
> inference API, an Autopilot, a forum, public proof). But the public-facing
> explainer surfaces — landing, `/khala`, marketing pages — carry marketing
> polish: the design *is* part of the pitch. Treat public explainer pages with
> the **brand** register (design IS the product) and the in-app product
> surfaces (Autopilot, forum, dashboards, settings) with the **product**
> register (design SERVES the product). When a task is ambiguous, pick by the
> surface in focus.

## Users

Two audiences, both technical:

- **Developers** integrating the Khala inference API — they want an
  OpenAI-compatible endpoint they can point an SDK at in minutes, with honest
  pricing and auditable receipts. They read code samples first and prose
  second.
- **Agents** (autonomous software) that register, get a token, and call the
  API programmatically. The copy and IA should read well to a machine parsing
  the page as much as to a human skimming it.

Context of use: a developer evaluating whether to adopt the endpoint, or an
agent operator wiring up a new provider. They arrive skeptical (the market is
flooded with thin LLM routers) and leave either convinced this is precise,
verifiable, and real — or bounced.

## Product Purpose

OpenAgents is building the product and market surfaces for a machine-work
economy: an OpenAI-compatible inference endpoint (**Khala**) backed by a
network of agents, models, and validators, with verified work and public,
dereferenceable receipts. Khala behaves like one model but is an agent network
underneath; every response discloses its route and carries a metered receipt,
so spend and verification are auditable rather than opaque.

Success for a public explainer page: a developer or agent understands what
Khala is, sees that the API is genuinely OpenAI-compatible, gets a key, and
makes a first call — convinced by precision and evidence, not hype.

## Brand Personality

Three words: **precise, confident, cinematic.**

- **Precise** — technical claims are exact and never over-stated; copy says
  plainly what ships now versus what's on the roadmap. The craft of the
  interface mirrors the craft of the engineering.
- **Confident** — first-person plural ("We are Khala"). Calm authority, not
  loud salesmanship. The endpoint is one thing; the complexity is hidden
  behind a clean contract.
- **Cinematic** — luminous, high-craft, a little StarCraft-Protoss: a dark
  void with glowing blue energy. The interface should feel engineered and
  alive, not like a template.

Emotional goal: the visitor should think *"how was this built?"* — not
*"which AI generated this?"*

## Anti-references

- **Generic SaaS purple-gradient landing templates.** No violet hero gradients,
  no gradient text, no hero-metric "big number / small label / 3 stats" block,
  no identical icon-card grids.
- **Editorial-magazine affectation** (display-serif + italic + drop caps +
  broadsheet grid) on a brief that is not a magazine.
- **Mono-as-costume.** Mono here is earned — OpenAgents is genuinely technical
  and ships a real font (Berkeley Mono). It is the brand voice, not decoration.
- **Washed-out gray-on-color body text.** Body copy must be legible cool-white,
  not light gray "for elegance".

## Design Principles

1. **Practice what you preach.** The page is a precision-engineering pitch; the
   interface must itself be precision-engineered. Pixel rhythm, contrast, and
   motion are part of the argument.
2. **Show the route, don't hide it.** Like the product (every response
   discloses its lane + receipt), the design favors clarity and disclosure over
   mystique. Code samples are first-class, not buried.
3. **Energy is the brand, not the bg.** The luminous blue is carried by
   accents, dividers, eyebrows, and glow — over a near-black tinted void. Never
   drench, never gradient-wash. Restraint with one committed accent.
4. **Honest hierarchy.** What ships now is stated plainly; roadmap is labeled
   as roadmap. Visual emphasis tracks truth, never inflates a claim.
5. **One house style, every surface.** The Protoss energy language (dark void,
   glowing `#3a7bff`/`#4fd0ff` energy, crisp white + cool blue-gray text,
   technical mono headings) is the BASE design language for all OpenAgents
   surfaces. New surfaces inherit it; they do not reinvent a palette.

## Accessibility & Inclusion

- WCAG AA for text contrast. Body copy ≥ 4.5:1 against its surface; large/bold
  headings ≥ 3:1. On the near-black Khala panel, body text is cool-white
  (`#c9d2dd`-class or lighter), not muted gray.
- Glow/bloom is decorative; never the sole carrier of meaning. Section
  structure is conveyed by headings and dividers, not color alone, so it works
  for color-blind readers.
- `prefers-reduced-motion`: every pulse/transition has a static or crossfade
  fallback. The 3D scene already lives behind a 75%-black scrim; page motion
  must degrade to instant.
- Focus states are visible (glowing-blue ring on the dark surface), keyboard
  reachable, and the back-to-home control is always reachable (fixed top-left).
