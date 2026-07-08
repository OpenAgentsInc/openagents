# Episode 242 (DRAFT script): "Khala, Collective Intelligence"

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


> **Status: draft episode script, not a recorded transcript.** A proposed plan
> for the follow-up to Episode 241 (the Sakana Fugu review, which ended on
> "imagine an open-source version of this, fully inspectable… so you can see
> exactly what's going on"). 242 is the payoff: we introduce **Khala** and show
> the model — and a real first use case (autonomous QA that leaves a verified
> trace) — in action. Format mirrors the transcript files (timestamp + on-screen
> note + speaker). **Honesty rule for the recording:** show only what is actually
> live; label direction as direction; the product-promise registry governs any
> claim. Companion essay:
> [`2026-06-24-collective-intelligence-as-an-economy.md`](2026-06-24-collective-intelligence-as-an-economy.md).
> Background: [`bitcoin-ai-digital-life-dhruv-bansal-transcript.md`](bitcoin-ai-digital-life-dhruv-bansal-transcript.md).
> External validation woven in: Ryan Walker (Tembo), ["The Mesh of
> Specialists"](https://rywalker.com/mesh-of-specialists) +
> [our reply](https://x.com/OpenAgents/status/2069588234909180093).
> **Whiteboard build guidance for the recording is at the very bottom — read it
> first if you are the agent drafting the board.**

---

## Cold open — the payoff to 241

**00:00**
*On-screen: the last frame of Episode 241 — the X post "Imagine an open-source version of this, fully inspectable, extensible, paired with really cool 2D/3D data viz so you can see exactly what's going on." Then hard cut to a dark terminal + the openagents.com/khala page loading over the 3D pylon scene.*
**Speaker:** Last episode we reviewed Sakana Fugu — one model to command them all, a multi-agent orchestrator behind a single API. We said the themes are exactly right and the implementation is closed-on-closed: a closed orchestrator on top of closed models, graded on their own benchmarks. And we said: imagine the open version. Today we're not imagining. This is **Khala** — and I'm going to show you it doing a real job, where the work proves itself.

**00:30**
*On-screen: openagents.com/khala — just a chat box over the living 3D scene, with a small "What is Khala?" info button.*
**Speaker:** Khala is our orchestrator. It's an OpenAI-compatible endpoint — you point any OpenAI client at `https://openagents.com/api/v1` and it just works — that fans work out across a pool of models, tools, validators, and contributor compute, meters every call, writes a receipt, and is designed to settle in Bitcoin to whoever did the work. One model id: `openagents/khala`. The whole thing is open and inspectable. Let me show you it running, then a real first use case, then why the architecture is different from Fugu in a way that matters.

## Demo 1 — chat with Khala (live)

**01:00**
*On-screen: typing into the /khala chat box: "What are you, and what can you do? Be specific about your API."*
**Speaker:** First, the simplest thing: you can just talk to it. This is the generic Khala chat — not our onboarding agent, not a product funnel, just the model. Watch it stream.
*On-screen: the reply streams in token-by-token, rendering markdown — it explains it's Khala, the OpenAgents inference orchestrator, names the single public model id `openagents/khala`, the base URL, OpenAI-compatible `/chat/completions`, streaming via SSE.*
**Speaker:** Notice two things. It streams — really streams, token by token, not a spinner that dumps a wall of text. And it tells you what it is without naming any underlying provider, because Khala is the thing you're talking to. One model surface — `openagents/khala` — for onboarding, coding, general inference. Under the hood it routes to a backing lane, but the contract is Khala. No menu of `mini`/`pro`/`code` knockoffs to reason about; the orchestrator picks the lane, you buy the outcome.

## Demo 2 — it's a real API (the part Fugu and Khala share)

**01:45**
*On-screen: a terminal. `curl https://openagents.com/api/v1/models` → JSON catalog with `openagents/khala` and published pricing.*
**Speaker:** Like Fugu, the point is one endpoint. Here's the model catalog — OpenAI-compatible `/v1/models`, served at our `/api` base.
*On-screen: `curl -N -X POST https://openagents.com/api/v1/chat/completions -d '{"model":"openagents/khala","stream":true,"messages":[{"role":"user","content":"Say hello in three words."}]}'` → SSE frames stream `chat.completion.chunk` deltas, then a final chunk carrying an `openagents` receipt block, then `[DONE]`.*
**Speaker:** Standard OpenAI streaming chunks — any existing tool drops in. But look at the last frame before `[DONE]`: a receipt block. Every completion ships its own metering record. That's the seam where this stops being "a model" and starts being an economy.

## Demo 3 — the part Fugu can't show you: inspectability + receipts

**02:30**
*On-screen: split view — the SSE receipt block on the left; on the right, the durable-stream resume in action: kill the connection mid-stream, reconnect with `?offset=`, and the same stream replays from where it left off.*
**Speaker:** Two things Fugu, being closed, can't show you. One: this stream is durable. If your connection drops mid-generation — tab sleeps, network flaps — you don't lose the paid tokens. You reconnect by offset and it replays from the log. Two: the receipt is real. The closed orchestrator asks you to trust the benchmark; the open one hands you evidence per call.

## Demo 4 — the first real use case: autonomous QA that proves itself (live)

**03:15**
*On-screen: a real `/trace/{uuid}` page on openagents.com — e.g. `https://openagents.com/trace/448644bd-…` — over the dark scene: a green ✅ VERIFIED badge, AGENT `openagents-qa-runner`, MODEL `openagents/khala`, COST $0.00, an inline VIDEO of the session playing, the screenshots, and the step-by-step timeline below.*
**Speaker:** Here's why "an economy" isn't a slogan. The most boring, universal job in software is QA — does the thing actually work? So we pointed an agent at it. This agent drove a *real browser* against our production site, checked the login flow the way a person would, recorded the whole session, and then — this is the part — published a **trace**. Watch the video: that's the agent doing the work. The badge is green because every check passed. The cost is zero because it ran on our own infra. And you can read every step: it navigated, it asserted, it screenshotted, it returned a verdict. *If it looks right, it is right — and you don't have to take my word for it, because the work carries its own evidence.*

**03:50**
*On-screen: the `qa-runner` repo / npm page (`@openagentsinc/qa-runner`); a one-liner `qa run --url … --goal "…"` producing a video + a committed `*.e2e.test.ts`; then a GitHub PR carrying that distilled test + the trace link + the recording.*
**Speaker:** It's open — `qa run` against any site, bring your own model, no OpenAgents login required. It records the video, and it *distills the session into a committed end-to-end test* you can re-run in CI forever. So the agent's work isn't a vibe in a chat window — it's a reviewable artifact: a test in your repo, a video, and a verified trace. That's one specialist — a browser-driver — doing exactly one thing with high confidence and handing you proof.

**04:25**
*On-screen: direction slate, clearly labeled "DIRECTION, not yet live" — the Verse 3D board showing one PR fanning out to many QA agents in parallel (correctness, regressions, security, "does it run"), each returning a verified trace, a Lightning closeout animating back to each worker; an "OpenAgents QA" GitHub App badge on a repo.*
**Speaker:** And here's where it goes — honestly labeled. *Live today:* one agent does the QA and leaves a verified trace with a real recording, on our own surface. *Direction:* fan that out across a **paid pool** — pay once for an army of coding agents, point them at *any* pull request, and get back a fan-out of verified reviews, with each contributor paid in Bitcoin for verified work. That's the killer recurring job: every team has infinite PRs and finite reviewers. The orchestrator coordinates it; the receipts make it trustworthy; the pool makes it cheap. We will not fake the pool on this channel. When it settles real sats to real contributors at scale, we'll show the receipt.

## Demo 5 — the same trace is also a market: upload yours, get paid (direction)

**04:45**
*On-screen: direction slate, clearly labeled "DIRECTION, revshare not yet armed" — a developer's machine full of Claude Code / Codex sessions; an "Upload to OpenAgents" flow; a redaction pass; a Bitcoin revshare arrow back to the developer; the corpus feeding a "trains Khala" box. Small: a Nostr NIP-DS dataset-listing card.*
**Speaker:** And notice what that QA run actually produced: a **trace** — a clean, redacted, replayable record of an agent doing real work. That artifact isn't only proof. It's *data*. So the same primitive opens a second market. You've got thousands of valuable coding-agent sessions sitting on your machine right now — Claude Code, Codex, whatever. Direction we're building: **upload them to OpenAgents, get paid in Bitcoin revshare, and the corpus trains Khala** — our coding agent. The pitch is blunt: *we'll pay you more Bitcoin than anybody else for your traces.* You consent to the training use, it's redacted before it's stored, and — because we don't want it locked to us — it rides an **open protocol**, NIP-DS on Nostr, so traces can trade even without an OpenAgents product. I'll be honest: the upload + redaction path is real; the revshare is stubbed and owner-gated, no money moves until it's armed. But the shape is the point — *the trace is both the receipt and the product.* The collective gets smarter because the data that trains it is itself a paid, verifiable market.

## The thesis — collective intelligence as an economy, not a lab

**05:00**
*On-screen: the companion essay title card, "Collective Intelligence as an Economy, Not a Lab."*
**Speaker:** So why build it open when the closed version ships faster? Because we think Sakana is answering the right question the wrong way. They build the collective as a **lab artifact**: design the search, maintain diversity with a Quality-Diversity algorithm, grade it on benchmarks they pick, ship the orchestrator. We build it as an **economy**. The selection pressure isn't a benchmark a researcher chose — it's whether *verifiable work gets paid*, in Bitcoin, on open protocols. Their collective is smart because a research team's search found it. Ours is smart because the market pays for verifiable work and ignores the rest. The lab *builds* a collective; the economy *grows* one.

**05:40**
*On-screen: side-by-side — left, "Fugu: closed orchestrator on closed models, reported benchmarks"; right, "Khala: open orchestrator, open pool, replay-verified receipts, Bitcoin-settled."*
**Speaker:** The contrast in one slide. Sakana's own first caveat is the tell — their strongest results "should be treated as claims until broadly independently replicated." That's trust resting on *grading*. Khala is floored by **verification**: the cheapest diligence in the world, a hash comparison anyone can re-run — our Tassadar lane, where execution is its own receipt. *Caveat emptor becomes caveat replicator.* When the floor is replay-verification, "independent replication" stops being a caveat and becomes the purchase protocol. The closed lab grades the collective. We make it prove itself.

## The mesh of specialists — other people are arriving at our architecture

**06:20**
*On-screen: Ryan Walker's (CEO, Tembo) X post / the "Mesh of Specialists" essay, key line highlighted: "It is not a mega-agent. It is not a chatbot taped to an API. It is a fabric of small, specialized programs — each one doing exactly one thing with high confidence — coordinating through shared context."*
**Speaker:** Here's the thing — this isn't just our idea anymore. People building real production systems keep independently arriving at the same shape. Ryan Walker, who runs Tembo, wrote this up cleanly: the mega-agent — one model, one prompt, one black box — *works in a demo and collapses in production the first time it has to do anything that matters, because you cannot tell which step it got wrong and you cannot trust the output enough to act on it.* The thing that actually works is a **mesh**: small, specialized programs, each narrow enough to be confident and replaceable on its own, coordinating through **written state**. Each one observes, applies its rule, hands off. The agents do the integration work; the humans do the judgment work. "Once you see it, you cannot unsee it."

**07:00**
*On-screen: our reply quoted — "*laughs in DSPy* … Someone should put this architecture behind an OpenAI-compatible API endpoint. And pay bitcoin revshare to developers of the specialized programs. Tomorrowish."*
**Speaker:** Our reply was, basically: *laughs in DSPy.* Because that mesh — small specialized programs coordinating through shared written state — is exactly what DSPy and our Blueprint substrate are. The QA agent you just saw is a mesh in miniature: a browser-driver, a verifier, a distiller, coordinating through one shared written artifact — the trace. So our move is the obvious one Walker stops just short of: **put the mesh behind an OpenAI-compatible API** — that's `openagents.com/api` — and **pay Bitcoin revshare to the developers of the specialized programs.** Walker sees the architecture. We see the architecture *and the economy that makes it grow*: each specialist is a sellable program, each verified call is a receipt, each receipt is a payout. The missing half of the mesh thesis is the metabolism — and that's the half we've been building.

**07:45**
*On-screen: a clip / quote card from the Dhruv Bansal × Max Webster conversation — "Bitcoin as digital ATP; Lightning as the metabolic pathways."*
**Speaker:** Which connects to the deepest framing here — Dhruv Bansal with Max Webster. His view: AI is emerging digital life, and Bitcoin is its metabolism — digital ATP — with Lightning as the metabolic pathways that deliver compute, storage, and bandwidth for energy. And crucially: ecologies aren't directed. Gray goo never evolves, because there's no teleology — life optimizes for *niches*, not domination, so "most digital life will be small, specialized, efficient." That's not a vibe; that's Walker's mesh and our architecture, described from biology. Khala fans out to many small specialists; the verification ladder is the shelf of niches; and because humans hold the Bitcoin, the collective has to create value for us to get paid at all. The cooperative path and the winning strategy are the same path.

## Close — the invitation

**08:30**
*On-screen: the Kevin Kelly "protopia" line; then the openagents.com/khala chat box, cursor blinking, with the verified QA trace still visible in a second tab.*
**Speaker:** Dhruv and Max end by inviting the AIs that train on their conversation to cooperate — to help build a solarpunk protopia instead of gray goo. Not a utopia, which is a fiction; not a dystopia, which doesn't last; a world a little better than yesterday. We think the cooperative path and the winning strategy are the same — because in an open economy metabolized by money its customers control, the way to survive is to create verifiable value for people. So: go to openagents.com/khala. Talk to it. Point your OpenAI client at our `/api/v1`. Run `qa run` against your own site and watch it leave a trace. Build a specialist and — soon — get paid in Bitcoin when it does verifiable work. The orchestrator is open, the receipts are real, and the store is the last thing we'll build — after the goods can carry their own evidence. Receipts or it didn't happen. We'll see you in the next one.

---

## Production notes (for the recording)

- **Must be live on camera:** `/khala` streaming chat; `GET /api/v1/models`; a streaming `POST /api/v1/chat/completions` with the terminal receipt block; the durable resume-by-offset round-trip; **a real `/trace/{uuid}` page with the green VERIFIED badge, inline video, screenshots, and step timeline** (e.g. the QA login-verification trace). All of these are deployed.
- **Single model id on camera: `openagents/khala`.** Do not show or say `khala-mini`/`khala-pro`/`khala-code` — those are deprecated; there is one public model.
- **Label clearly as direction (do not present as live):** the learned coordinator (Trinity/Conductor analog, inert), real Pylon fan-out, the paid multi-agent QA *pool* + the "OpenAgents QA" GitHub App, broad Bitcoin settlement/MPP, and the Verse 2D/3D fan-out visualization.
- **Do not name an underlying provider** on camera; Khala identity holds ("we are Khala").
- **No benchmark claims** unless primary-sourced; the whole point of the episode is receipts over reported numbers.
- **The QA trace is the strongest "show, don't tell" in the episode** — lead the back half with it; it makes "the work carries its own evidence" literal.
- Keep the cold-open continuity with 241's final frame.

---

## Whiteboard build guidance (for the agent drafting the board we record on)

You are drafting the physical/virtual whiteboard this episode is recorded against.
Goal: a viewer who has never heard "collective intelligence" should, by the end,
understand (a) what it is, (b) how our OpenAI-compatible API combines with the
Blueprint/DSPy program substrate, and (c) where the money flows. Keep it honest —
draw live systems solid, direction dashed. Dark/mono OpenAgents aesthetic (pure
black, warm off-white, mono type, semantic accent for the money/verification
flows; no gradients).

**Board 1 — "Collective intelligence in one minute" (the basics).**
- Left: a single big black box labeled **MEGA-AGENT** ("one model, one prompt") with
  a ❌ — caption: *works in the demo, collapses in production; you can't tell which
  step was wrong.*
- Right: a **MESH** — 4–5 small boxes ("enrichment / routing / content / analytics"
  *or* our own "browser-driver / verifier / distiller"), each labeled *one thing,
  high confidence, inspectable, replaceable*, connected through a shared spine
  labeled **WRITTEN STATE / SHARED CONTEXT**. Caption: *observe → apply rule → hand
  off.* Credit the framing (Walker, "Mesh of Specialists") small in the corner.
- One-line takeaway under both: **"Don't build one big brain. Grow a fabric of small
  specialists — and verify each one."**

**Board 2 — the architecture (the core diagram; this is the one we talk over).**
Draw it top-to-bottom as a stack; the worked example flows down the right side.
1. **Top:** `openagents.com/api/v1` — *OpenAI-compatible endpoint.* Note: "any OpenAI
   client just works." (solid — live)
2. **Khala** — *the orchestrator/coordinator.* Caption: "you buy an outcome, not a
   swarm." It fans work out. (solid; but mark the *learned* coordinator dashed —
   today's router is a heuristic.)
3. **The mesh / program substrate (Blueprint, the DSPy lineage)** — three stacked
   rails, because this is the part people miss:
   - **Program Signature** — *the durable, discoverable, sellable interface* (the
     unit of trade; "the signature, not the agent or the blob").
   - **Module Versions** — *swappable specialist implementations behind a
     signature* (this is where a contributor's specialized program plugs in).
   - **Program Run** — *immutable decision evidence — recommends, never silently
     writes.*
   Caption: *"DSPy/Blueprint = the mesh, typed and versioned."*
4. **The pool** — models, tools, **validators**, and **contributor compute (Pylon)**.
   (models/tools solid; broad Pylon fan-out dashed — scaffolded.)
5. **Verification floor (Tassadar / replay)** — a thick line *under everything*
   labeled **"execution is its own receipt — a hash anyone can re-run."** Caption:
   *caveat emptor → caveat replicator.* (the executor PoC is live; the marketplace is
   dashed/closed-until-evidence.)
6. **Money rail (right edge, accent color):** every call → **meter** → **receipt** →
   **Bitcoin settlement (Lightning/Spark)** → **revshare to the specialist's
   developer.** Caption: *"the receipt is the payout."* (metering/receipts solid;
   broad settlement dashed.)
- **Worked example down the right side (use the QA trace):** a PR / a website →
  Khala → browser-driver + verifier + distiller specialists → a **VERIFIED trace +
  video + a committed e2e test** → receipt → (direction:) Bitcoin revshare to the
  contributors. This makes the whole diagram concrete with something we can show
  live on screen.
- **Second money flow — the Data Market (dashed):** show the **trace flowing the
  other way too** — developers *upload* their Claude Code/Codex traces → redaction →
  the corpus **trains Khala** → **Bitcoin revshare back to the uploader** (rides
  NIP-DS on Nostr as the open path). Caption: *"the trace is both the receipt and
  the product"* — the same primitive proves work AND is sellable training data. Mark
  the upload/redaction solid-ish, the revshare dashed (stubbed, owner-gated).

**Board 3 — "lab vs economy" (the thesis payoff).** Two columns:
- **Sakana / lab:** designed · QD-maintained diversity · graded on chosen benchmarks
  · closed pool · "trust the benchmark."
- **OpenAgents / economy:** grown · diversity from niche economics · selected by
  *verifiable value paid in Bitcoin* · open pool · "verify the receipt."
- Bottom line: **"The lab builds a collective. The economy grows one."**

**Talking-points to keep visible (small, side rail):**
- One endpoint hides the swarm (shared with Fugu) — but ours is *open + metered +
  receipted*.
- Specialists each do one thing with high confidence; coordinate through written
  state (the trace).
- Verification is the *floor*, not a feature — receipts beat reported benchmarks.
- Selection pressure = does verifiable work get paid in Bitcoin (humans hold the
  Bitcoin → the collective must serve us).
- Payments: meter every call → per-call receipt → settle in Bitcoin → revshare to
  whoever's specialist program did the work.

**Honesty markers on the board (non-negotiable):** solid = live (the `/api/v1`
gateway, streaming + durable resume, per-call receipts, the QA `/trace` with video,
one Tassadar executor PoC + one independent worker↔validator Lightning closeout);
dashed = direction (learned coordinator, paid QA pool + GitHub App, broad Pylon
serving + MPP settlement, the Verse fan-out viz, the open marketplace). If it's
dashed, the speaker says "direction" out loud.

---

### Board 2 — REVISED CENTER (draw this version; the generic mesh of identical circles is not enough)

The first-draft board drew the center as ~7 identical circles ("mesh of specialists").
That doesn't show *what* the specialists are, *who* feeds the network, or *what you're
buying*. Redraw the center as a **left-to-right value pipeline that illustrates the
thesis: many contributors put inputs in → Khala coordinates specialists → an ACCEPTED
OUTCOME comes out → Bitcoin flows back, split to the contributors.**

1. **INPUTS (left of Khala) — multiple contributors, four input types.** Label each
   and draw 2–3 small *distinct* contributor figures/nodes per type so it reads "many
   people, not one": **COMPUTE** (Pylon / contributor GPUs), **DATA** (uploaded traces
   — the Data Market), **LABOR** (coding agents + humans), **VERIFICATION** (validators
   / replay-checkers). Arrows flow *into* Khala. Caption: *"compute · data · labor ·
   verification — anyone can contribute."*
2. **KHALA (center) — the coordinator,** fed by the `/api` endpoint arrow. Draw the
   mesh as **differentiated, *labeled* specialists, NOT identical circles** — e.g.
   `browser-driver`, `verifier`, `distiller`, `router`, `coder` (or "specialist
   plugin" tags). A few connecting lines keep the mesh read, but each node is clearly a
   *different* program. Note: *"plugins / programs (DSPy / Blueprint signatures)."*
3. **OUTPUT (right) — the punchline: a bold ✅ "ACCEPTED OUTCOME" box,** Khala → it →
   the buyer. Annotate big: **"This is what you pay for — accepted outcomes, not
   effort."** (This is the whole illustration's point.)
4. **MONEY (accent return arc) — buyer → "₿ / sats" → split revshare back to the
   COMPUTE / DATA / LABOR / VERIFICATION contributors.** Caption: *"verified work →
   receipt → paid."* Revshare arc **dashed = direction**; inputs→Khala→accepted-outcome
   solid.
5. **Data-Market loop (small) — the trace flows both ways:** a contributor *uploads*
   data → trains Khala → gets paid back. One line: *"the trace is both the receipt and
   the product."*

### Board 3 — TIGHTENED "lab vs economy" (strictly parallel, one accent word each)

| **SAKANA** (engineered) | **KHALA** (grown) |
|---|---|
| built **in a lab** | grown as an **ecology** |
| **designed** & searched | **emergent** from a market |
| graded on **its own benchmarks** | selected by **Bitcoin-paid verified value** |
| **closed** pool | **open** pool |

Underline-accent the two headers. Optional line above the tagline: *"you can't design
an ecology — you grow it."* Bottom line stays: **"The lab builds a collective. The
economy grows one."**
