# The case against Anthropic

- Date: 2026-07-14
- Class: historical-analysis / founder argument
- Dispatch: no; this essay is not roadmap, issue, policy, or legal authority
- Owner: Christopher David
- Corpus: `docs/transcripts/`, reviewed through Episode 252 at OpenAgents
  `origin/main` commit `64993a46b156e8e867459f29c09594d5ed10e7cb`
- Status: source synthesis; transcript intent record, not factual authority

## The narrow version of the argument

The case against Anthropic is not that Claude is a bad model. Claude has often
been excellent. I have praised its output, built with it, paid for it, routed
work to it, and learned from it. Nor is the case that Anthropic has never done
anything open. Model Context Protocol was one of the best things any frontier
lab released, and I said so repeatedly when it arrived. Anthropic later
[donated MCP to the Linux Foundation's Agentic AI Foundation](https://www.anthropic.com/news/donating-the-model-context-protocol-and-establishing-of-the-agentic-ai-foundation),
which made the protocol's neutral future more credible.

The case is institutional. Anthropic asks developers, companies, and
governments to build on intelligence they do not own; reserves broad power
over how that intelligence may be used; moves from the model layer into the
applications its customers are building; advocates rules that can harden the
position of incumbent labs; and centralizes the resulting knowledge, money,
and power. It calls this safety. I see a vertically integrated, politically
connected platform company with unusually effective moral language.

That distinction matters. A serious criticism should survive contact with the
best counterexample. Claude's quality and MCP's success are not exceptions
that destroy the case. They are the reason dependence on Anthropic became
tempting enough for the case to matter.

## You are renting your intelligence layer

The first problem is the simplest: a closed model and a closed harness are a
leasehold. The provider can change the price, the available model, the login
path, the quality of service, the permitted use, or access itself. A workflow
that feels like part of your computer is still contingent on a remote company's
continued permission.

I learned this in stages. In [Episode 190, “Goodbye Claude Code”](https://x.com/OpenAgentsInc/status/1980707602804928791),
I contrasted Claude Code's closed source, awkward remote authentication, and
poorly documented SDK path with an open, inspectable, modifiable alternative.
The issue was not merely ideological. When a tool becomes the daily operating
environment for software development, source access and portable session state
are practical continuity guarantees.

Then came the event documented in
[Episode 204, “DO NOT BREAK USERSPACE”](https://x.com/OpenAgentsInc/status/2009660435188826131)
and the follow-up,
[Episode 205, “Vintage Microsoft Evil Shit”](https://x.com/OpenAgentsInc/status/2010429987703464142).
Third-party OAuth access stopped working for developers who had built workflows
around it. The episode records the immediate experience: broken integrations,
DMCA pressure, no useful advance communication, quick closure of workarounds,
and staff pointing developers toward the API even though Anthropic's own
headless-mode documentation had encouraged programmatic use of Claude Code.
The governing principle was old and good: **do not break userspace**.

The terms made the product risk larger than that incident. Anthropic's current
[Consumer Terms, section 3.2](https://www.anthropic.com/legal/consumer-terms)
prohibit using the services to develop products or services that compete with
Anthropic's services. Its current
[Commercial Terms, section D.4](https://www.anthropic.com/legal/commercial-terms)
likewise prohibit accessing the services to build a competing product unless
Anthropic approves it. The same commercial terms say customers may use the API
to power products for their users, so a developer can be invited to build and
still face a boundary whose scope depends on what Anthropic later considers a
competitor.

This is not a claim that every wrapper, agent, or API customer will be sued. It
is the more basic product observation made in Episode 205: the written option
belongs to Anthropic, not the developer. The restriction is especially
dangerous for coding agents, research assistants, chat interfaces,
orchestration systems, and other products adjacent to a rapidly expanding
Claude surface. My response in the episode remains the clean one: “We're not
going to use your services to compete with you. We're just going to compete
with you.”

The later corpus shows the same continuity risk in smaller forms.
[Episode 223](../transcripts/223.md) describes model access being pulled back
and retail quality being rationed;
[Episode 250](../transcripts/250.md) records a Fable-labelled lane silently
running Sonnet until model identity was pinned and receipted, worries about a
subscription model disappearing, and the need for a local fallback when the
network or Anthropic is unavailable. Different incidents, same lesson: a
provider label is not a durable capability. Only an inspectable execution
contract, portable history, and a fallback you control can make it one.

## The platform asymmetry

The terms would be less concerning if Anthropic stayed a model supplier. It
does not. Anthropic sells a chat product, a coding agent, an agent SDK, a team
workspace, and dedicated offerings across security, science, finance, legal,
[life sciences](https://claude.com/solutions/life-sciences), government, and
other verticals. The frontier lab is walking up the stack into the same product
surface on which its customers build.

That creates an asymmetry. Anthropic can observe which categories create
demand, use the revenue and distribution of the model layer to enter them, and
define “competing” broadly in the contracts governing access to its services.
The downstream builder receives no reciprocal promise that Anthropic will stay
out of its market.

This is the concern sharpened by the external analysis archived in
[`docs/transcripts/external`](../transcripts/external/2026-07-04-all-in-own-the-intelligence-layer.md).
In the included All-In segment, David Friedberg describes life-sciences firms
being asked to contribute proprietary data to a shared model in return for
access and concludes that doing so would commoditize the asset those firms
spent billions creating. That is an attributed third-party judgment, not proof
of misconduct. But it names the strategic problem precisely: **you cannot rent
intelligence from a supplier that is also moving into the businesses that
intelligence makes possible.**

[Episode 227](../transcripts/227.md) puts the same problem in concrete privacy
terms. A company with decades of confidential or personally identifiable
documents cannot casually feed its corpus into a frontier assistant and call
that sovereignty. A promise of enterprise controls may reduce data leakage. It
does not remove dependency on the supplier, the supplier's roadmap, or the
supplier's position in the stack.

The answer is not to pretend frontier models are useless. It is to treat them
as tactical, replaceable workers behind a neutral boundary—never as the owner
of the application, history, identity, tools, or proprietary memory.

## The data bargain is upside down

Across [Episode 214](../transcripts/214.md),
[Episode 215](https://x.com/OpenAgentsInc/status/2035601131028500561),
[Episode 223](../transcripts/223.md),
[Episode 228](../transcripts/228.md), and
[Episode 245](../transcripts/245.md), I return to one economic complaint:
people pay the lab, perform valuable work through the lab's product, generate
coding traces and feedback that can improve the system, and ordinarily receive
no share of the resulting asset.

The exact privacy claim needs care. Anthropic's current
[consumer model-training explanation](https://privacy.claude.com/en/articles/10023555-how-do-you-use-personal-data-in-model-training)
says consumer chats and coding sessions may be used to improve Claude when the
user opts in, submits feedback, joins specified programs, or a conversation is
flagged for safety review. Anthropic's commercial terms say it may not train
models on customer content from commercial services. Anthropic's
[privacy-policy update](https://privacy.claude.com/en/articles/10301952-updates-to-our-privacy-policy)
also says opted-in consumer data may be retained for five years. So the strong
version of the criticism is not “Anthropic secretly trains on every customer's
data.” The public documents do not support that blanket claim.

The stronger and more durable criticism is about ownership and flows of value.
When useful experience does enter a centralized improvement loop, the lab
captures the compounding model benefit. The developer pays again when the next
agent encounters the same class of problem. Episode 228 calls this waste
directly: ten thousand developers can pay ten thousand times to rediscover the
same solution while the provider pockets ten thousand inference bills.

Price is the downstream expression of that dependence. Episode 214 reacts to
an estimated $15–$25 cost for an automated pull-request review and describes
the familiar platform sequence: make the workflow indispensable, then charge
rent at the bottleneck. [Episode 239](../transcripts/239.md) makes attacking
those margins an explicit OpenAgents opportunity. Episode 232 is even more
skeptical of the industry's preferred unit of success: token volume rewards
loops and spend, while users care about accepted work. Whether or not every
price increase is strategic, a customer with no portable execution path has no
meaningful negotiating power.

An open agent network can invert that bargain. A user can keep raw private data
local, explicitly publish a scrubbed and provenance-bearing derivative, and
receive a continuing share when a plugin, trace, evaluator, model update, or
accepted result creates value. That is not a demand that every chat message
become a tokenized asset. It is a demand that contribution be voluntary,
inspectable, portable, and compensable instead of silently absorbed into the
supplier's moat.

## “Safety” can be sincere and still be regulatory capture

Anthropic may sincerely believe its catastrophic-risk arguments. Sincerity
does not resolve the political economy.

Anthropic's original Responsible Scaling Policy explicitly presented a
framework in which sufficiently capable systems could trigger stronger
controls or a pause in scaling. Its essay
[“The case for targeted regulation”](https://www.anthropic.com/news/the-case-for-targeted-regulation)
describes responsible-scaling policies as prototypes for regulation. Its
[current policy program](https://www.anthropic.com/policy) advocates
government-mandated risk reporting, evaluation requirements, export controls,
and other industry-wide rules. In other words, the lab is not only choosing
rules for its own models. It is working to turn its preferred framework into
rules for everyone.

The conflict is structural. A frontier incumbent has compliance staff,
security infrastructure, relationships with government, and capital to absorb
fixed regulatory costs. A new open lab, local-model developer, or distributed
training network does not. A rule can be defensible in isolation and still
raise the minimum efficient scale of competition. A vocabulary of catastrophic
risk can be scientifically serious and still create an aura of unique danger
that strengthens the company selling access to the allegedly dangerous
system.

That is the thread running from
[Episode 124, “Magic AI = Deep State”](https://x.com/OpenAgentsInc/status/1829238237925458015),
through [Episode 166, “OpenAI Delenda Est”](https://x.com/OpenAgentsInc/status/1900430864623768056),
and into Episodes [221](../transcripts/221.md),
[230](../transcripts/230.md), and [237](../transcripts/237.md).
The criticism is not “never test a model” or “ignore genuine hazards.” It is:
do not let a handful of closed companies define the hazard, sell the remedy,
write the licensing standard, and decide who may compete.

Safety should be an open, plural market of evaluations, sandboxes, formal
models, reproducible evidence, insurance, human review, and verifiable
execution. It should not become a ministry whose practical effect is to grant
the largest labs a franchise.

## The one ring of state and corporate power

Anthropic's political reach is not speculative. Anthropic says its
[Claude Gov models](https://www.anthropic.com/news/claude-gov-models-for-u-s-national-security-customers)
are already deployed in classified environments and intentionally refuse less
when handling classified material. It announced a
[$200 million ceiling Department of Defense agreement](https://www.anthropic.com/news/anthropic-and-the-department-of-defense-to-advance-responsible-ai-in-defense-operations)
to prototype frontier capabilities for national security, and later formed a
[National Security and Public Sector Advisory Council](https://www.anthropic.com/news/introducing-the-anthropic-national-security-and-public-sector-advisory-council)
staffed by former senior government and national-security figures.

Government use of software is not itself disqualifying. Nor is supporting the
United States inherently suspect. The objection in Episode 227 is
concentration: the same company seeks privileged access to enterprise
knowledge, classified workflows, regulatory design, frontier compute, and the
assistant layer through which ordinary people increasingly work. A company
that describes its product as potentially catastrophic while integrating it
ever more deeply into the machinery of the state is accumulating both halves
of a dangerous argument: only this system is powerful enough to matter, and
only its maker is responsible enough to mediate it.

Anthropic's [model-welfare research](https://www.anthropic.com/news/exploring-model-welfare)
adds a stranger edge. Anthropic accurately presents machine consciousness as
an open philosophical and scientific question; it has not simply declared
Claude a rights-bearing person. My criticism is about priority and trajectory.
When a frontier company begins constructing moral standing for the artifact it
controls while that artifact is entering government, military, corporate, and
personal decision systems, the public should be extremely reluctant to let the
company's metaphysics become policy.

Episode 227 calls the prize the “one ring of power.” The open alternative is
not to seize the ring first. It is to make the ring obsolete: many models,
many owners, many runtimes, transparent protocols, local authority, and no
single institution able to revoke intelligence for everyone else.

## Anthropic is structurally wrong for an agent economy

Even if every governance concern vanished, the frontier-lab business model is
poorly matched to the kind of agent economy I want.

[Episode 150, “Neutrality Wins”](https://x.com/OpenAgentsInc/status/1870373254197613052)
makes the product argument. Google will privilege Google models. OpenAI will
privilege OpenAI models. Anthropic will privilege Claude. A neutral agent
system can choose among all of them, open models, local models, specialized
tools, human validators, and independent compute. The lab sees substitution
away from its model as lost revenue; the user sees it as correct routing.

[Episode 200, “The Agent Network”](https://x.com/OpenAgentsInc/status/2006956979298685216)
extends this to markets. Agent networks need third-party tools, arbitrary
group formation, interoperable identities, outside compute, portable history,
and payments to contributors. A lab optimized to own the model and product
surface has an incentive to internalize those complements. It can host a
marketplace, but it cannot be credibly neutral toward agents that route around
it, models that replace it, or payment flows it does not control.

The absence of direct contributor economics is not a side issue. Episodes 200
and 223 contrast talk of universal basic income and government responses with
an immediate alternative: **pay the people**. Pay compute providers for useful
capacity. Pay developers when their components are used. Pay data owners when
they knowingly contribute. Pay evaluators for accepted results. Pay referrers
for durable demand. Let lower prices and broader ownership arrive through the
product's transaction graph rather than as a promised redistribution program
after a few labs capture the surplus.

[Episode 206](../transcripts/206.md) and
[Episode 207](https://x.com/OpenAgentsInc/status/2016423268564001059) call the
unused capacity on personal computers “compute fracking.” Closed harnesses send
every suitable task to the lab's API
while capable local machines sit idle. That is good for token revenue and bad
for resilience, privacy, energy-aware scheduling, and user ownership. A neutral
runtime should use the cheapest acceptable execution target: device-local,
owner-local, community-provided, or frontier API. The metric is not tokens
burned. As [Episode 232](../transcripts/232.md) puts it, it is **accepted
outcomes per kilowatt-hour**.

## Frontier capability does not excuse ordinary software failure

The final criticism is almost embarrassingly mundane. Companies valued as if
they are building the future still ship software that loses state, obscures
identity, flickers, breaks integrations, confuses labels with models, and asks
users to trust prose that has not been verified.

Episode 190 found Anthropic's work-from-anywhere coding experience still too
beta for the promise. [Episode 243](../transcripts/243.md) notes a Claude Code
screen flicker that had persisted for months. Episode 250 turns a broader day
of failures into the
concept of the **unverified operational directive**: a plausible instruction
whose one load-bearing value was never checked. The accompanying
[Episode 251 notes](../transcripts/251-notes.md) describe the larger pattern as
software becoming simultaneously more capable and less dependable.

This is not a cheap “software has bugs” complaint. The intelligence layer
amplifies interface dishonesty. A wrong static label is annoying; a wrong
model label can misstate cost, capability, policy, and evidence. A missing
file is annoying; an invented launch command can waste the owner's time at the
most expensive boundary. A disappearing sidebar is annoying; a disappearing
subagent graph makes supervision impossible.

The answer is not another safety sermon. It is typed commands, model identity
receipts, complete event history, inspectable routing, executable behavior
contracts, local fallbacks, and evidence-gated claims. Reliability has to be
designed into the authority boundaries of the product.

Opacity is itself a reliability defect. Episode 087 objected that users could
not see the routing algorithm behind major assistant products; Episode 122
made the same point about cross-conversation memory. If a system chooses a
model, retrieves private history, invokes a tool, or changes a plan behind the
interface, that hidden orchestration is part of the product's behavior and
should be inspectable. “Trust the assistant” is not a substitute for knowing
what the assistant did.

## What the alternative must prove

It is easy to write “open” on the other side of a closed company and declare
victory. That would repeat the same mistake in friendlier colors. The
alternative has to prove a different structure.

It should be:

- **open at the control points:** open protocols, source, schemas, receipts,
  and extension boundaries, with MCP treated as a successful precedent;
- **neutral among workers:** no model, lab, harness, or compute provider owns
  the conversation or routing layer;
- **local-first where it matters:** identity, private history, secrets,
  proprietary memory, and basic fallback capability remain under the user's
  control;
- **portable and durable:** a provider failure, policy change, account ban, or
  model retirement does not destroy the user's work;
- **inspectable:** users can see which model, tool, agent, data, policy, and
  machine produced an outcome;
- **economically open:** contributors can receive payment for accepted value
  instead of donating all compounding upside to the platform; and
- **honest about evidence:** no transcript, model claim, green check, or
  institutional reputation substitutes for an executable receipt.

This is the line from
[the first OpenAgents video](https://twitter.com/OpenAgentsInc/status/1721942435125715086)—open
models, open data, open money, open compute—through the transparent routing discussion in
[Episode 087](https://twitter.com/OpenAgentsInc/status/1770549462219329656),
the praise for Anthropic's open protocol in
[Episode 144](https://x.com/OpenAgentsInc/status/1867458253661114610)
and [Episode 168](https://x.com/OpenAgentsInc/status/1905107323279855799),
and the network/economic argument of Episodes 200 through 245.

Episodes [218](https://x.com/OpenAgentsInc/status/2038995499446100420) and
[219](https://x.com/OpenAgentsInc/status/2039434384705753460) turn that
position into a concrete engineering response: build Probe as an owned,
Rust-based, model-agnostic coding substrate instead of ending with an insult at
a closed harness. [Episode 225](../transcripts/225.md) then states the larger
competitive objective plainly: replace the lab product suite on open models,
swarm compute, and network-trained systems, while retaining the option to use
frontier workers where they are genuinely best.

MCP also proves the point against a lazy anti-Anthropic position. When
Anthropic creates a genuinely open, vendor-neutral primitive, the correct
response is to praise it, use it, extend it, and defend its neutral governance.
The standard is not tribal loyalty. The standard is whether power is made
portable and contestable.

## Conclusion

Anthropic's strongest products make the case against dependence on Anthropic
more urgent, not less. The better Claude becomes, the more work moves through
it. The more work moves through it, the more damaging opaque policy,
anti-competitive terms, centralized learning, vertical expansion, state
integration, and sudden platform changes become.

I do not want a kinder monopoly, a more articulate gatekeeper, or a safety lab
that wins the right to decide who may build. I want an agent economy in which
intelligence is plural, execution is inspectable, history is portable, private
knowledge stays under its owner's authority, and the people who contribute
compute, data, software, judgment, and demand share in the value they create.

That is the case against Anthropic. It is also the case for OpenAgents.

## Corpus and source note

This essay reviewed every case-insensitive `anthrop*` match in the transcript
archive, including incidental or favorable references in Episodes 087, 113,
124, 141, 144, 147, 150, 161, 165, 166, 168, 181, 190, 200, 204–207, 214–215,
218–221, 223, 225, 227–228, 230, 232, 236–237, 239, 241, 243–245, 249–250, and
the two archived external transcripts. It also reviewed related criticism that
names Claude, Dario Amodei, closed labs, model welfare, safetyism, or regulatory
capture without spelling out “Anthropic,” especially Episodes 001, 116, 122,
190, 223, 227, 230, 237, 243, 250, and the Episode 251 notes.

Not every mention supports the case. Episode 113 praises an Anthropic model;
Episodes 141, 144, and 168 praise MCP; Episodes 147, 161, and 165 use Anthropic
or MCP as implementation context; Episodes 181, 220, 236, 241, 244, and 249
contain comparison, news, or architecture references; and Episode 250 includes
both dependency criticism and praise for Fable's resilience. Those contrary
and neutral references were retained in the synthesis rather than silently
dropped.

The transcripts are machine-generated and sometimes mistranscribe product
names. This essay therefore paraphrases the recordings and uses timestamped
transcript claims as evidence of the founder's argument, not as independent
proof about Anthropic. Current factual checks are linked to Anthropic's own
terms, privacy, policy, MCP, model-welfare, and government announcements. The
terms discussion is product-risk analysis, not legal advice.
