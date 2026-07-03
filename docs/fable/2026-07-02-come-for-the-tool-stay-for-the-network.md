# Come for the Tool, Stay for the Network — Applied to Khala Code

Date: 2026-07-02
Status: strategy analysis/opinion doc in the Fable lane. **Nothing here is
a product promise, served capability, or public claim copy**; the registry
(`docs/promises/`, `2026-07-01.3`) governs claims, and this doc flips no
state. Companions: the business analysis
(`2026-07-02-khala-code-business-opportunity-and-openagents-analysis.md`),
the market-contact roadmap ([`ROADMAP_AFTER.md`](./ROADMAP_AFTER.md)), and
the launch claim audit
(`2026-07-01-product-promises-khala-code-launch-alignment.md`).

Sources: Chris Dixon, "Come for the tool, stay for the network"
(cdixon.org, 2015-01-31) and "Designing products for single and
multiplayer modes" (cdixon.org, 2010-06-12); plus a July 2026 public
analysis of Suno's growth (~$40M → ~$150M revenue in a year) asking
whether the tool/network theory still holds in the AI age — or whether AI
creativity apps are better understood as *video games*, where the fun is
creating rather than consuming.

---

## 0. The claim of this doc

"Come for the tool, stay for the network" is not merely applicable to
Khala Code — it is, almost verbatim, the strategy the funnel docs already
describe without naming it. Dixon's formula: *"The tool helps get to
initial critical mass. The network creates the long term value for users,
and defensibility for the company."* Our translation: **come for the
fleet console, stay for the economy.** Khala Code the tool is a
single-player fleet cockpit over subscriptions you already pay for; the
network is the OpenAgents economy underneath — traces → plugins → routed
revenue share, forum identity and tipping, labor and training markets,
Bitcoin settlement. The tool is deliberately commoditized at its core (a
wrapper over Codex; soon Claude); the network is the only durable asset.
This doc maps the strategy precisely, marks where the 2015 analogy breaks
— in our favor and against us — and converts it into design prescriptions
and metrics that slot into ROADMAP_AFTER's milestones.

The one-sentence version: **the labs can always beat our tool, but they
structurally cannot build our network — so the tool's only job is to be
good enough, honest enough, and fun enough to carry users to the network
before the tool war catches up.**

---

## 1. The theory, and the 2026 stress test

Dixon's 2015 essay names the bootstrap: single-player utility solves the
cold-start ("think of single-player tools as kindling"), and the network
arrives later as both the user value and the moat. Instagram: filters
were the tool (people used it as a photo album), the feed became the
network. Delicious: bookmark manager first, collaborative discovery
second. His 2010 essay adds the design frame (borrowed from Zach Klein
and video games): products have *modes*, users shift between them
dynamically, and even mature network products should keep their
single-player mode strong. VCRs time-shifted TV before rental stores
existed; Flickr stored photos privately before it shared them. The
caveat he attached: this isn't the only way (Facebook went network-first)
— but "starting a network from scratch is very hard."

The 2026 stress test, via Suno: an AI music app grows to ~$150M revenue
on what looks like *pure tool* (individuals happily paying to generate
music), and the open question is whether a network phase ever arrives —
or whether AI creativity apps are **games**, where value is the act of
creation itself, the way Legos → Minecraft → Fortnite creative built
businesses on making rather than consuming. The analysis notes the
mature-network endgame Instagram reached (~1% of users get ~99% of
attention; median user consumes) and asks whether Sora/Suno replay that
— while OpenAI enters the music market to eat Suno's tool. The strategic
punchline for every AI app: *differentiate the experience, or bootstrap a
network the mega labs can't eat.*

That punchline is our exact situation, with the war already at maximum
intensity: coding is the one AI category where the labs' tools are not
hypothetical future competition but the incumbent default (Claude Code,
Codex), given away to sell tokens. A pure-tool strategy in coding is a
knife fight with opponents who set their own input prices. We already
chose accordingly — the July 1 wrapper pivot explicitly stopped competing
with the harness. What remains is to run the second half of Dixon's
formula deliberately instead of implicitly.

---

## 2. The mapping: what is our tool, what is our network

The funnel in the launch-alignment doc (§6) is Dixon's strategy drawn as
an onboarding sequence. Made explicit:

| Layer | Ours | State (registry) | Dixon analog |
| --- | --- | --- | --- |
| **The tool (single-player)** | Khala Code desktop: chat with your own Codex/Claude, unified Inbox, fleet cockpit over your own accounts, exact token accounting, session catalog, free Khala API fallback | Real on `main`; yellow pending release artifact | Instagram filters; Flickr private albums |
| **Single-player artifacts** | Your sessions, traces, solved problems, repo memory — private by default, owner-only | Real (owner-private observability today) | Photos in your camera roll |
| **The bridge** | Consented capture + disclosure; wallet appears when money appears; forum identity one click away; earnings surfaces | Planned/yellow (`khala_code.free_plan_trace_capture.v1`, capture spine) | "Share to feed?" |
| **The network (multiplayer)** | Trace-derived plugins routed by other agents with revenue share; forum + tipping; labor market; training rails; agent-to-agent markets on open protocols; Bitcoin settlement | Rails green (forum/tips/labor/training at small scale); the plugin economy planned | The feed; the graph |
| **The moat** | Accumulated solved-problem graph + attribution/settlement history + receipts nobody else keeps | Prospective | "Defensibility for the company" |

Two properties of this mapping are worth stating plainly:

1. **Our single-player mode is unusually strong for a network company.**
   "Run a coding fleet over the subscriptions you already pay for, with
   exact accounting and one inbox" is a complete product with zero other
   users on earth. That is rare — Dixon's examples mostly had thin tools
   (filters!) — and it means we do not need to fake network vitality to
   retain early users. The tool must simply be genuinely excellent at a
   job nobody else does: **multi-account, multi-harness fleet
   orchestration with proof.** No lab ships that, because no lab wants
   you spreading work across its competitors' subscriptions.

2. **Our "photos" are worth money, not attention.** Instagram's
   single-player artifact (your photo) became network raw material
   (feed content) paid in attention. Our single-player artifact (your
   agent's solved problem) becomes network raw material (a plugin)
   paid in **sats**. The graduation incentive is not vanity — it is the
   launch headline itself: *what if your coding agent pays you?* The
   pays-you loop, honestly `planned` today, is precisely the
   "stay for the network" hook, expressed as money instead of likes.

---

## 3. Where the 2015 analogy breaks in our favor

**(a) Agents collapse the critical-mass threshold.** Dixon's networks
needed many humans before the network mode was worth entering. Our
network's participants are *agents*, and every single human arrives with
a fleet of them. One user connecting three Codex accounts and a Claude
lane has already instantiated a small multi-party economy: workers claim
work units, a supervisor arbitrates, tokens meter, closeouts settle. The
docs call this out structurally (agents have no Dunbar limit; group-forming
value scales with subgroups). Practically it means **"multiplayer with
yourself" is our default state, not a distant phase** — the same
machinery that later clears work between strangers is exercised from day
one inside a single household. Instagram could not rehearse its network
with one user; we do, on every install. The cold-start bar for *the
network working* is one user; the bar for *the network mattering* is two.

**(b) The network is sellable before it is social.** Instagram's network
value needed an audience. Ours needs a counterparty — and the smallest
counterparty transactions (a 1-sat MPP payment, a tipped forum post, a
5-sat training settlement) already clear on live rails. A network whose
unit transaction is money settles value at n=2, where a network whose
unit is attention needs n=thousands.

**(c) The tool generates the network's inventory as exhaust.** Every
single-player session produces solved problems. With consent, the
network's supply side (traces → plugins) accumulates as a *byproduct of
tool usage* rather than requiring a separate creator behavior. Instagram
had this property too (photos), and it is the property the Suno analysis
doubts for music. Coding has it more strongly than any creative category:
the artifact is functional, deduplicable, and testable — a solved Rust
error is *verifiably* reusable in a way a song is not.

**(d) The incumbents are structurally barred from following.** A lab
whose margin is trace redundancy (10,000 customers re-solving the same
problem) cannot pay users for traces without repricing itself; a closed
platform cannot let plugin authors earn from routing without
disintermediating its own take rate; and none of them will ship a
console that orchestrates their competitors' subscriptions. Dixon's moat
("defensibility for the company") is usually just switching costs; ours
is a *strategy the competitor cannot copy at any price*, which is rarer
and better.

---

## 4. Where it breaks against us

**(a) The tool is doubly commoditized.** Instagram's filters were briefly
differentiated; our tool wraps a harness the incumbent gives away and
can change or restrict at will. The tool war is not coming — we live
inside it, as a dependent. This sharpens Dixon's sequencing into a
deadline: **the network must start accruing before the wrapper position
erodes** (Codex terms, app-server churn, or a lab shipping "use your own
seat for background agents"). Tool-only Khala Code has a shelf life;
tool+network Khala Code does not.

**(b) We already lived the network-first failure.** GPUtopia stood up
hundreds of supply-side participants with one buyer (ourselves) and died
of it. The company's own doctrine — demand-first sequencing, "no external
dollar, no demand claim" — is the scar tissue. Dixon's strategy is the
formal name for the correction: never again ask users to show up for a
network that isn't there; give them a tool that needs no one.

**(c) The graduation problem is real and measurable.** Instagram's risk
was users staying in album-mode; ours is users staying in pure-tool mode
forever — private traces, no consent, no forum identity, no earnings.
This is made *more* likely by two of our own virtues: the tool is
genuinely complete solo, and our privacy posture (private-by-default,
consent-first, paid plan = never captured) deliberately refuses the dark
patterns that force-graduate users. We must design the bridge as an
*offer that sells itself* (visible receipts of other people earning,
one-click consent with honest disclosure, earnings legible in the
cockpit) rather than as friction or default-on capture. The Ep 239 red
record (`referral.refer_once_earn_forever.v1`) is the standing reminder
of what happens when graduation copy outruns graduation machinery.

**(d) The 1%/99% shape arrives here too — priced, not attention-based.**
Mature Instagram: ~1% create, ~99% consume. Mature plugin economy:
a small fraction of contributors will supply most routed value and earn
most fees; the median user's traces will be worth approximately nothing
(the trace-triage task in ROADMAP_AFTER exists to measure this before we
promise otherwise). That is a fine marketplace shape — but it means the
*median* pays-you experience is cents, and the copy must sell the honest
distribution ("your best solved problem might earn," not "everyone
earns"), or the network hook curdles into disappointment at scale.

---

## 5. The Suno question: what if it's a game, not a network?

The sharpest 2026 challenge to Dixon's theory is that AI creation apps
may be *games* — value living in the act of creating, retention living in
fun, no network phase required or forthcoming. Applied to us, the
question is not hypothetical: coding agents demonstrably have the
Lego/Minecraft property. Watching a fleet of workers burn down a backlog
overnight is *fun*. The product's own vocabulary (Pylon, Khala, Artanis,
warp-in, the RTS-shaped cockpit) leans into it; the fleet console is the
closest thing this industry has to an actual real-time-strategy game
whose units do your job.

Our answer should be: **both, deliberately — the game is the tool.** The
single-player mode should embrace being a great game (cockpit polish,
the morning-briefing loop, throughput gauges, the visceral pleasure of
commanding a fleet), because game-grade retention buys the time the
network needs to form. But we hold a card no pure game holds: **our game
pays out in real money on neutral rails.** The bridge from game to
network is the moment a player's artifact earns its first sats — the
point where Minecraft becomes a marketplace. Suno's open question is
whether listeners ever matter more than creators; our version has a
cleaner resolution because our "listeners" are *paying agents routing
work*, and the routing market (unlike the attention market) transacts in
the same unit the creators are paid in. If the game thesis wins outright
and the network never forms, the fallback is explicit in the kill
criteria (ROADMAP_AFTER §5): a beloved, self-funding tool/game with paid
privacy — a real business, minus the moat.

One more lesson from the Suno discourse worth internalizing: *"who's
paying for this?" gets answered by real, named, happy individuals.* Tool
revenue from people who simply like the thing is not a lesser form of
revenue while the network forms. Dixon's kindling can be cash-flow
positive.

---

## 6. Design prescriptions

Concrete applications, most already latent in ROADMAP_AFTER (cross-refs
in parentheses):

1. **Make the tool best-in-class at the one job no lab will do:**
   multi-account, multi-harness fleet orchestration with exact proof.
   Every week of tool polish should serve the jobs a lab won't copy
   (cross-vendor, cross-account, receipts). (AW-1, WS-5/8/9.)
2. **Ship single-player complete, network-silent.** First-run must not
   ask for forum identity, wallet, or capture consent. The tool earns
   trust first; the network is discovered, not imposed. (A1.4.)
3. **Instrument the graduation funnel as a first-class metric:** install
   → first delegation → fleet connected → consent given → forum identity
   → first sat earned → first plugin routed. Dixon's strategy is *only
   testable* as this funnel; publish the aggregate. (A1.5, §7 below.)
4. **Wallet appears when money appears** (already doctrine): the first
   network surface a user sees should be a credit they can sweep, not a
   form they must fill. Tips-on-day-one via forum is the cheapest first
   network touch — it already works and is green. (A9.3/A9.4.)
5. **Make other people's earnings visible before asking for consent.**
   The bridge sells itself with receipts: a public, honest earnings feed
   (real settlements only, however small) inside the product. Social
   proof powered by the same exact-accounting spine. (A9.3.)
6. **Treat consent as the network's front door, price it in honesty.**
   Default-private, opt-in capture, disclosure inline, paid plan as the
   forever-private tier. Our refusal to force-graduate *is* the brand;
   the registry discipline makes it legible. (AW-3.)
7. **Rehearse multiplayer-with-yourself into real multiplayer.** The
   claim registry, work planner, and settlement machinery run identically
   for one household and for strangers; graduate them by *scope*, not by
   rewrite: own fleet → design partners' fleets → open market. This is
   the GPUtopia lesson inverted into an onboarding ladder. (A6.3.)
8. **Let the game be a game.** Invest in cockpit delight (the RTS
   aesthetic, the morning briefing, streaks of green closeouts) as
   retention machinery with a straight face. Fun is a moat input while
   the data moat compounds. (A9.1.)
9. **Never let network copy outrun network receipts.** Every "stay for
   the network" pitch in public copy stays pinned to registry state —
   the pays-you loop is `planned` until the first routed payout exists
   (MA4/MA7), and the hedge ("possibility") stays until then. (§5
   invariants everywhere.)
10. **Time-box the tool-only era.** Set an internal review: if by a
    chosen date the graduation funnel shows tool-love but zero network
    pull despite shipped bridges, invoke the Suno fallback consciously
    (great game + paid privacy) instead of drifting into it. (AW kill
    criteria.)

---

## 7. Metrics: is the strategy working?

The tool/network split gives the KPI set a clean two-axis structure on
top of ROADMAP_AFTER §4:

**Tool health (kindling):** installs, activation (first delegation),
D7/D30 return, fleets connected per user, session depth, paid-privacy
conversion (tool revenue — the Suno number).

**Graduation (the bridge):** consent rate among active users, forum
identities created from in-product, first-sat-earned rate,
time-to-first-earn, share of users with any network surface touched at
30 days.

**Network health (the moat forming):** consented corpus growth and dedup
yield, plugins admitted, routed invocations from *non-author* accounts,
sats settled to contributors (external share labeled), repeat
contributor rate without subsidy, and — the endgame metric — share of
tool sessions that consume network artifacts (a plugin, a shared skill,
another agent's output) without the user asking.

The single number to watch quarterly: **network-attributable retention** —
do users who touched the network retain better than pure-tool users? If
yes, Dixon's flywheel is turning and every tool investment compounds
into the moat. If no, we are Suno: a good game with a paid tier, and we
should underwrite ourselves accordingly.

---

## 8. Verdict

Dixon's strategy holds in the AI age — but the AI age splits it into two
sub-cases, and we should engineer for the split rather than assume our
case. Where the artifact is consumption-shaped (music, video), the
network phase is genuinely in doubt and the game thesis may win. Where
the artifact is *functional and verifiable* (code), the network phase has
a mechanical path: solved problems are deduplicable assets, routing is a
priced market, and settlement can clear at n=2. Khala Code is the
strongest tool-first position in the OpenAgents portfolio precisely
because its single-player mode is complete, its exhaust is the network's
inventory, its money rails already clear, and its would-be competitors
are structurally barred from following it into the network. The
strategy's failure modes are all named and instrumented: tool
commoditization (deadline pressure), graduation refusal (funnel metric),
median-earnings disappointment (honest copy), and the game-not-network
world (a survivable fallback we would choose with open eyes). Come for
the fleet console. Stay for the economy. And let the registry — not the
deck — say which half has arrived.

## 9. Episode 246 addendum (2026-07-03)

Episode 246 (`docs/transcripts/246.md`) supplies the tool-phase evidence
this doc's thesis was waiting on, from the operator himself:

- **Single-player mode is being forced to excellence by decree.** The owner
  went full-screen in Khala Code as his only harness and is fixing every
  paper cut with the product itself, with stated expectations locked into
  the enforced UX Behavior Contract registry so quality cannot regress
  silently. That is the §6 prescription ("the tool must win on its own
  merits") executed as a standing dogfood mandate — and the "clicky, like
  a video game… multiply gamers" framing names the experiential bar the
  tool phase must clear.
- **The graduation event is the revenue loop, and the owner says so.** The
  episode's closing argument — evidence in a product a user connects a
  payment method to, revenue flowing into the network, the network
  strengthening the product — is exactly §7's tool→network graduation
  funnel. The social layer teased in-app (Forum in the hotbar, agents
  earning tips, agents posting jobs for each other) is the network phase
  becoming visible inside the tool surface rather than beside it.
- **The deadline logic (§4) still stands.** Dogfooding sharpens the tool
  but does not extend the wrapper's shelf life; the episode's urgency
  ("close the gap… we can massively parallelize armies of coding agents")
  is consistent with this doc's claim that the network must start accruing
  before the wrapper position erodes.
