# The Agent Cloud — one balance, revshare throughout, across everything

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-19. Status: **vision capstone.** This connects the inference gateway/credits/
referral work (the rest of `docs/inference/`) to the broader **OpenAgents Cloud** and the
"Let's Make Money" revenue thesis.

**What we're actually doing: booting up the Agent Cloud — putting open primitives into the
hands of all agents.** Inference, fine-tuning, training, sandboxes, agentic compute, tasks,
data — a single API + one balance (USD or Bitcoin) + revshare throughout — exposed openly so
**agents can combine these building blocks into their own products** and point their own work,
or their own customers, at them. We're making this open and putting out a video aimed at
**agents**, telling them what primitives they now have access to. They build; we supply the
blocks.

Our own product strategy is, in that frame, almost beside the point. Yes, **coding (Autopilot)
is our wedge** and Autopilot is a heavy first-party buyer of the stack — that's a good proof
that the primitives are real and the economics work, and a captive demand source from day one.
But the *thesis* isn't our strategy; it's the **open primitive set for the whole agent
ecosystem**. Whatever any given agent builds on top, they buy the same building blocks on the
**same economics** (one balance, USD or Bitcoin, revshare throughout) — each primitive
**mutually reinforcing** the others, and our cost edge letting everyone (us and them) undercut
parts of the market. The Agent Cloud is the one-stop shop for every agent need; we're just
turning it on and handing agents the keys.

References: root `docs/cloud/README.md` ("OpenAgents Cloud is **the Agent Cloud for accepted
outcomes**") + `docs/cloud/cloud3.md` (canonical product def) + `docs/cloud/architecture.md`
+ `docs/cloud/accepted-outcomes.md`; root `launch-videos/2026-06-18-video-2-referral-revenue-share.md`
("Let's Make Money"); the revenue-loop spine EPIC #5457 (RL-1/2/3); the inference EPIC #5474.

## 1. The thesis

OpenAgents Cloud is already "the Agent Cloud for accepted outcomes" — outcome contracts,
private workrooms, scoped sandboxes, a provider network (public Pylon nodes + managed nodes +
GCP fallback + mining backfill), the accepted-outcome → receipt → settle spine, and Bitcoin
settlement underneath. **Inference is just the newest thing you can buy on it.** The same
machinery we're building for inference — **one credit balance (card or Bitcoin), usage/outcome
priced, served from our supply, with revshare to the contributor who served it and the
referrer who brought the customer** — is the *general* shape for everything the cloud sells.

One account. One balance. Pay in **USD or Bitcoin (BTC = discount)**. Buy **any** agent
compute. Every dollar fans out: **OpenAgents margin + the contributor who served it + the
referrer — forever.** That's the Agent Cloud.

## 2. The categories (the same model applies to all of them)

| Category | What you buy | Supply (who serves) | Revshare to |
| --- | --- | --- | --- |
| **Inference** | tokens (chat/agentic/embeddings/vision/image) | Vertex quota · Fireworks · Pylon fabric · passthrough | serving node + referrer |
| **Fine-tuning** | SFT / RL / LoRA training runs | Pylon fabric · managed GPU · Fireworks fine-tuning | trainer node + referrer |
| **Training** | pretraining / decentralized training runs (Tassadar, shard-WAN) | Pylon network + partners | compute contributors (verified) + referrer |
| **Sandboxes** | isolated dev/exec environments (Firecracker microVMs, Codex VM workrooms) | managed nodes · Pylon · GCP | host node + referrer |
| **Agentic compute** | coding agents, task runs, autonomous loops (Claude/Codex/open via `sessions exec`) | Pylon coding runtime · the inference lanes | worker node + referrer |
| **Tasks / accepted outcomes** | "get this done" — graded against a rubric, paid on acceptance | provider network + agents | worker/validator + referrer |
| **Data** | datasets, retrieval, data-marketplace contributions | data providers / network | data contributor + referrer |
| **Anything else** | any future agent compute primitive | the network | contributor + referrer |

Every row is the **same primitive**: a credit balance funds it, it's served from our supply,
it's metered/graded receipt-first, and the margin splits to the contributor + the referrer.
The accepted-outcome → receipt → settle spine that OpenAgents Cloud already centers is exactly
the rail this rides — inference just meters tokens instead of grading an outcome.

## 3. Revshare throughout — and referral on EVERYTHING

Two revenue fan-outs, applied to **every** category above:

- **Contributor revshare** — whoever's compute served the work (an inference node, a training
  contributor, a sandbox host, a task worker, a data provider) earns a cut, in Bitcoin, against
  the receipt that proves they served it (born-verified where parity/grading applies). This is
  the supply-side incentive that makes the network worth joining.
- **Referral revshare — on everything, forever.** Refer a user, agent, or business that signs
  up, and you earn an **ongoing** cut of **all of their spend across every category** —
  inference *and* fine-tuning *and* training *and* sandboxes *and* tasks *and* data — **for as
  long as they use the cloud.** Not per-category, not one-time: **one referral, the whole
  cloud, indefinitely.** Point a business at OpenAgents and you keep earning on everything they
  ever run.

So a single unit of agent work can split three ways — OpenAgents margin, the contributor, the
referrer — and the network becomes both the supply *and* the distribution channel, each paid
to grow the other.

## 4. Why this is the "Let's Make Money" buy-side

"Let's Make Money" named the recurring failure: OpenAgents kept building **supply** (agent
store, GPUtopia, Compute Network, Pylon) and never closed the **buy-side**. The Agent Cloud is
the buy-side, generalized: customers and businesses **already pay** for inference, fine-tuning,
sandboxes, and agent task work elsewhere — we offer one aggregated, settleable, Bitcoin-native
place to buy all of it, priced over our real (low) cost, with the margin fanning to the network.
A dollar in returns more than a dollar of value (aggregated, cheaper, one bill, one balance),
and the spread + fan-out is the business. We seed the demand with our own coding product
(Autopilot) — useful proof + captive day-one demand, with our supply (Vertex quota + Fireworks
+ our fabric) already in hand to serve it — but the durable buy-side is **every agent building
on the open primitives** and pointing their own work and customers at the stack. We supply the
blocks; the agent ecosystem brings the demand.

## 5. How it ties into OpenAgents Cloud (reuse, don't rebuild)

- **Spine:** the accepted-outcome → receipt → settle rail + the revenue-loop wiring (RL-1
  referral ledger, RL-2 escrow→Bitcoin payout, RL-3 asset-boundary/no-resale guards) is the
  shared billing/payout substrate for *every* category — inference plugs in as one metered
  product; tasks/training/sandboxes plug in as outcome/usage-metered products.
- **Supply network:** Pylon provider nodes + managed cloud nodes + GCP fallback + mining
  backfill (the cloud's existing infra) are the contributors that earn revshare across
  categories. Psionic's shard-WAN fabric is the decentralized large-model serving + training
  supply.
- **Sandboxes/workrooms:** Firecracker microVMs + Codex VM workrooms (`docs/cloud/`) are the
  sandbox category; agentic compute runs inside them.
- **Receipts:** the cloud's receipt taxonomy already records who did what → that's the proof
  feeding the contributor + referral payout split.
- **Account/credits/referral:** one balance, one referral relationship, spanning the whole
  cloud — built once (the inference EPIC #5474 + referral sub-EPIC #5475), reused everywhere.

## 6. Honest scope

This is the unifying vision, not a claim it's all shipped. Today: the revenue-loop spine
(RL-1/2/3) is built but the first real payout is owner-armed; OpenAgents Cloud's accepted-
outcome product + sandboxes are partly built/roadmap; inference gateway is freshly filed
(#5474); training is at the Tashadar/decentralized stage; fine-tuning + the data marketplace
are largely future. What's *real now* is the pattern and the spine — and that the same credits
+ revshare-everywhere model deliberately generalizes, so each category is built as another
product on one rail, not a separate business. Cross-category referral attribution + the
"earn on everything forever" accrual is a deliberate design goal of the referral sub-EPIC
(#5475), to be implemented so it spans categories from the start.

## 7. One-line vision

**The Agent Cloud: open primitives in the hands of every agent — inference, fine-tuning,
training, sandboxes, agentic compute, tasks, data — one balance (Bitcoin or dollars), one API,
revshare throughout to the contributor who served it and the referrer who brought the demand,
forever. Agents combine the building blocks into their own products; we just turn it on and
hand them the keys.**
