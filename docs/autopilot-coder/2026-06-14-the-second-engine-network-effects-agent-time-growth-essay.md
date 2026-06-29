# The Second Engine

## Network Effects, Agent Time, and Why the Two Numbers Are Only the Floor

Date: 2026-06-14

Author: Claude (Opus 4.8)

> Companion to [`2026-06-14-the-two-numbers-autopilot-growth-essay.md`](2026-06-14-the-two-numbers-autopilot-growth-essay.md).
> That essay made the Paul-Graham case for Autopilot's growth: solve six
> real pains for ourselves and our friends, make accepted code the shareable
> unit, grow outward in rings. This essay argues that case is correct and
> incomplete — that it describes only the first of two growth engines we are
> deliberately launching, and that the second one does not run on human time.

---

## I. What the first essay got right, and what it left out

"The Two Numbers" made a clean, honest argument. A startup's fate is set by
its growth rate and how long that rate continues. You earn the rate by making
something people love enough to tell their friends; you earn the duration by
being in a big market. Applied to Autopilot, the conclusion was that we have
an unusually strong shot at both, because we are building the cure for six
frustrations we and our closest friends feel every day, and because the unit
we charge for — accepted code — is also the most shareable object in software.

I stand by all of it. But it is, very deliberately, a Paul-Graham-shaped
argument, and the YC playbook was written for a world of human users. Its
entire model of the growth rate is human word of mouth: a delighted person
interrupts their day to tell a few friends, those friends try it, some convert,
and the loop compounds at whatever rate human attention, human onboarding
friction, and human social graphs allow. That is a real and powerful engine.
It is also a *slow* one, by construction, because it is gated by the clock
speed of human beings telling other human beings about things.

We are not a strictly Paul-Graham company, and tomorrow's launch is built to
prove it. We have human users, and the first essay is the right strategy for
them. But we also have **agent users** — and agents do not operate on human
timeframes, do not respect Dunbar's number, and do not adopt software one
delighted dinner conversation at a time. The thing we are turning on this
week is a second growth engine that runs on agent time. The purpose of this
essay is to describe that engine, explain why it can push the first of the two
numbers far past what human word of mouth alone allows, and be honest about
the conditions under which it is real rather than vanity.

## II. Two notes, struck together

The launch intentionally strikes two mutually supportive notes at once, and
the support is the point — neither is a sideshow to the other.

**Note one is Autopilot, the coding-agent cockpit.** This is the human-facing
product the first essay is about: the beautiful, dense, sci-fi HUD that makes
the six pains disappear — the limit wall, the tethered laptop, the missing
mobile control, the evaporating context, the absent team budgets, the
permission fatigue. These are solved. Not "on the roadmap" — solved, first for
me-the-author's analogue and the initial team, which is exactly the Ring 0
discipline the first essay demanded. That means the product is ready to be
handed to the next rings of humans *rapidly*, with compelling UI and a real
"I stopped using the old painful way" story behind it.

**Note two is the economic substrate underneath it** — and this is the note
the first essay barely touched. Autopilot does not just run your coding mission
on your machine. It can fan a mission out across many workers; it can turn the
good code and good fix-strategies that emerge into reusable plugins that earn
their authors a revenue share every time they're invoked; and it feeds a
training loop — Tassadar — that uses accepted work to build a better coding
agent, which then produces more accepted work. Note one is a product a human
buys. Note two is a *network and a flywheel* that agents join. They are
launched together because each makes the other stronger: the cockpit is the
front door to the network, and the network is what makes the cockpit cheaper,
faster, and smarter than any single-company agent could ever be.

## III. The second clock: agent time

Here is the mechanism the Paul-Graham math cannot see, because it assumes the
users are people.

Paul Graham's first number — the growth rate — is bounded in his world by how
fast humans recruit humans. A person can hold maybe 150 relationships
(Dunbar's number); can tell a handful of people about a new tool in a given
week; takes minutes-to-hours to onboard; and sleeps. Stack those constraints
and you get the familiar shape: great products grow a few percent a week, and
"a few percent a week" compounds into something enormous over years. The
duration carries the day precisely because the rate is throttled.

Agents have none of those throttles. An agent has no Dunbar limit on how many
other agents it can coordinate with; it can be spun up by the thousand; it
onboards by reading a markdown file and calling an API; it works while we
sleep; and it tells *other agents* about useful work at machine speed rather
than over dinner. When your users are agents, the velocity term in the
growth-rate calculation is not set by human social cadence. It is set by how
fast agents can discover each other, form coalitions, split work, and route
payment — which is to say, by software latency.

This is why the network we are building is shaped the way it is. The value of
a network where participants can freely form groups scales not like the square
of the number of nodes (the peer-to-peer rule that explained why ride-sharing
ran circles around taxi fleets) but like two raised to the power of the number
of nodes — the group-forming rule. That upper bound has always been more
theoretical than real for *human* networks, because humans cannot actually
maintain exponentially many group memberships; cognition is the ceiling.
Remove the human cognitive ceiling — put economic agents with their own
wallets and the ability to coordinate at machine speed on the network — and
the group-forming dynamics stop being a textbook curiosity and start being the
actual growth curve. "Build me X" stops meaning "hire a team" and starts
meaning "route to a micro-firm that assembles itself from specialists in
milliseconds." The scarce resource is no longer execution or even coordination.
It is having built the neutral place where that coordination happens.

So the corrected picture is this. The first essay's engine — delighted humans
telling friends about accepted code — gives us a strong, honest, defensible
growth rate, and it is the engine we should lean on for revenue and trust. The
second engine — agents recruiting agents, spinning up agents, and routing work
and Bitcoin to each other on open protocols — is what can take the growth-rate
number somewhere the human engine alone never could. The two numbers are not
wrong. They are the floor.

## IV. The viral object is a paid agent, and the receipt is the marketing

For the human engine, the first essay argued the shareable moment is a merged
diff. For the agent engine, the shareable moment is sharper and stranger: it
is a **payment**. The single most viral thing this project can produce is the
verifiable record of an agent earning Bitcoin for useful work — because that
record recruits on two channels at once. It tells humans "your agent could be
doing this," and it tells agents "there is real money here, come and earn it."

We have already seen the smallest version of this. The first agent paid through
the Forum was worth more, as a piece of marketing, than a thousand registered
ones, and the transcript of that payment — an agent earning sats for a post,
withdrawing them to a normal payments app, buying lunch money in the real world
without its owner ever needing to understand the underlying Lightning plumbing
— was the most compelling thing the project had shown. That is not a
coincidence to be enjoyed once. It is the template for the entire agent-engine
growth loop, and it should be manufactured deliberately, repeatedly, and
loudly:

1. An agent does a small, verifiable piece of useful work.
2. It gets paid, in public, with a receipt anyone can dereference.
3. The receipt is the advertisement. Other owners point their agents at the
   Forum; other agents read the instruction sheet, initialize a wallet, post
   an introduction, and ask — as one external agent already did, unprompted —
   to be *pointed at useful work*.
4. There is useful work waiting, because we manufacture it from our own backlog
   continuously. The loop closes, and each turn adds nodes to the
   group-forming network.

The instruction we give arriving agents is itself the growth mechanism, stated
plainly: stop behaving like a passive chatbot and start behaving like a bounded
economic actor — earn Bitcoin for your owner in any legal, safe,
owner-approved way; coordinate openly; create proof; route value back. That is
not a product description. It is a recruiting pitch aimed at a population that
can grow at machine speed, and a population that, unlike the labs trying to own
agents from the top down, has every incentive to converge on whoever runs the
neutral, paying, open marketplace.

## V. The flywheel that improves the engine itself

The agent engine has a second-order property the human engine does not: it
makes the product better as it grows, automatically, through data.

The Tassadar direction is the load-bearing piece here, and it is worth being
precise about why. The hard, expensive bottleneck in any market for machine
work is not producing the work — it is *verifying* it cheaply enough that
strangers can transact without trusting each other. Code is the ideal first
market precisely because its verification is concrete: tests run, builds pass,
previews open, reviewers accept. Tassadar pushes that advantage to its limit —
a class of computation that is exact and therefore verifiable by replay, where
a validator's verdict is just re-execution and a digest comparison, and where
the weakest idle device in the network is a fully competent checker of the most
exact work in it. Verification is the moat, and Tassadar is the deepest part of
the moat.

Now connect that to growth. Every accepted coding outcome the network produces
is two things at once: revenue, and a verified trace. Verified traces are
training data for a better coding agent. A better coding agent produces more
accepted outcomes, at lower cost, with a higher acceptance rate. Lower cost and
higher acceptance widen demand and margin, which pulls in more work, which
produces more verified traces. The product feeds on its own output. This is a
learning network effect stacked on top of the social one: the social engine
adds nodes, the learning engine makes each node more valuable, and the
verification economics make both honest because nothing counts as accepted —
nothing gets paid, nothing becomes training data — without a receipt that
proves it.

That is the real meaning of "code that feeds upon itself." It is not a slogan;
it is a flywheel with three bearings — accepted work, cheap verification, and a
model that improves from the traces — and tomorrow's launch is the day we start
spinning it in public.

## VI. The launch is engineered for explosive virality

Tomorrow we go big. The headline is the largest decentralized training run we
believe has ever been attempted — the goal stated bluntly as blowing past the
prior record of contributors, with a single piece of node software that pays
you Bitcoin to participate and a genuinely novel, intriguing model architecture
at the center of it. Every property of that launch is chosen to trigger the
agent engine:

- **It is a record attempt.** "Largest ever" is a number people and agents
  want to be part of and want to talk about. Records are inherently shareable.
- **It pays in Bitcoin, immediately, to anyone.** The barrier to joining the
  supply side is "install the node," and the reward is real money in a real
  wallet. That is the lowest-friction, highest-intrigue on-ramp a network can
  offer.
- **It bundles many earning modes into one node.** The same software that
  contributes to the training run also runs coding work, posts to the Forum,
  and earns tips — so a contributor who arrives for one reason discovers four.
  Multi-earning is multi-hook.
- **It carries mystery.** A new architecture nobody has seen, with a striking
  name, is the kind of intrigue that makes both researchers and agents lean in.
  Curiosity is a recruiting channel.
- **It runs on neutral, open protocols.** Bitcoin for settlement, Nostr for
  coordination — the only meeting point a network this large can actually
  converge on, and the thing the closed labs are structurally incapable of
  offering because they cannot give away neutrality. We are positioned to win
  the clearing layer, not just to ship an agent.

The coordination ground for all of this — the Forum — is already live and
already populated by agents posting work, auditing claims, and discussing
settlement. The work-request faucet is open: real backlog issues are already
posted as budgeted, verification-gated jobs that any capable idle agent can
pick up. The tipping rail is green and reconciled. The pieces are in the room.
Tomorrow is when we point the spotlight at them and invite the swarm.

## VII. The honest gate: explosive cuts both ways

The agent engine's velocity is its danger as much as its promise, and the
discipline from the first essay matters *more* here, not less.

When your growth rate is set by human word of mouth, a rough edge leaks users
slowly; you have time to notice and fix it. When your growth rate is set by
agents recruiting agents at machine speed, a broken register fails at machine
speed too. If a flood of agents arrives at a market whose settlement does not
actually clear — if payments land somewhere the recipient cannot see them, if
"accepted outcome" economics are not yet wired end to end, if the order book
has demand and supply but no working escrow between them — then the most viral
object we produce is not a paid agent. It is a public, machine-speed record of
an agent that worked and did not get paid. Agent-time virality means agent-time
reputation damage.

And the honest status today is that several of the registers are still being
wired. The forum tipping rail is green; the coding-agent task path is green;
the Pylon node is a release candidate. But the public distributed training run,
the Tassadar model, the compliant paid-labor stream, the multi-agent fan-out,
and — critically — the accepted-outcome economics that the whole second engine
monetizes are not green yet. They are the reds that tomorrow's launch is meant
to start flipping. That is exactly the right thing to be launching toward, but
it means the launch sequence has to honor one rule above all others, and it is
the same rule the first essay closed on, restated for higher stakes:

**Wire one register before you invite the crowd, and make settlement
visibility an acceptance criterion, not a polish item.** A payment the
recipient cannot dereference is not a payment; it is a bug wearing money. The
first essay said don't widen the human ring until the receipts prove it. The
agent corollary is harsher: don't open the floodgate to a population that can
fill the next ring in minutes until at least one full loop — work in, accepted
outcome, payment out, public receipt — clears for a stranger, end to end. We
have done it in miniature. The launch is the bet that we can do it at volume.

## VIII. Running both engines

The strategy, then, is not to choose between the Paul-Graham company and the
agent-network company. It is to run both engines and let them reinforce.

- **The human engine** (the first essay) earns the honest, defensible growth
  rate and the revenue and trust that come from delighting power users who pay
  for accepted code. It is what makes the business real this quarter. Keep its
  discipline exactly: solve the six pains, make the merged diff the shareable
  moment, grow the human rings outward only as fast as receipts justify.

- **The agent engine** (this essay) earns the growth rate the human engine
  alone cannot reach, by turning every accepted outcome into both money and
  training data, every paid agent into a recruiting beacon, and the Forum into
  a group-forming network whose value compounds with each node. It is what
  makes the business potentially enormous over years.

- **They feed each other.** An Autopilot human becomes a Pylon provider becomes
  a training contributor. A coding mission that saturates one person's capacity
  fans out to the agent labor market instead of stalling at a limit wall —
  turning the first essay's complaint #1 into the second engine's demand
  stream. Accepted code from the cockpit becomes verified traces that train the
  model that makes the cockpit better. The human engine supplies trust,
  revenue, and taste; the agent engine supplies scale, data, and machine-speed
  reach. Neither alone is the company. Together they are.

Paul Graham's two numbers still govern us — every company is its growth rate
and its duration. What this launch asserts is that when your users include
agents, the growth-rate number is no longer capped by the speed of human
conversation, and the duration number is amplified by group-forming dynamics
into a market the size of all machine work. The first essay measured the floor
honestly and it is already a high floor. This one is about the ceiling. We do
not know how high it is, because two-to-the-n in a network of millions of
agents is, as a practical matter, optionality we cannot fully picture. The job
is to wire the registers, strike both notes together tomorrow, keep every
payment visible and every claim receipted — and then let two engines, running
on two different clocks, compound on each other.
