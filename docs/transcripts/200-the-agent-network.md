**[00:00]**
**[Visual: A diagram titled "GROUP FORMING NETWORKS: Metcalfe vs Reed’s Law" showing network nodes.**

Happy new year! Before we get into our secret sauce on why OpenAgents is destined to become a trillion-dollar company, we gotta do the obligatory 2026 predictions. Everyone’s doing predictions. Except with us they’re not predictions. They’re statements of fact about what’s going to happen this year because we, along with our other friends on Team Open, are making these things happen. A lot of them are happening already. Let’s just go through this real quick.

**[00:26]**
**[Visual: Slide titled "2026" with a list: Local AI, Swarm AI, Open > Closed, Agents > Models, Autopilots, Agent Network(s). The speaker clicks "Local AI".]**

Local AI. 2026 is the year of local AI. Alex [Cheema] who runs Exo, stringing together Macs into clusters. Yes, absolutely. Year of local AI. And what percentage of those global AI workloads are going to be running on Apple silicon? Somewhere between 7% and 35%. We think it’s going to be on the high side of that. The implications of that could be interesting.

**[00:52]**
**[Visual: A tweet thread featuring a video of Gavin Baker.]**

And of course, people who are smart — Gavin Baker, for example — talking about, "Oh yeah, bear case for AI capex... in three years on a bigger phone you'll be able to run a pruned-down..." Okay. Yeah, cool. But those timelines don't make any goddamn sense because it ignores the economics of *aggregated* compute.

Everyone thinking this is thinking in terms of what edge inference and test-time compute *one* device can do for *that* user. But if we can combine this... If you have spare, unused, or "stranded" compute on your device that I can pay you for, of course you'll sell it to me for a teensy bit of bitcoin straight to your wallet.

Hey, do we have a relevant product launch coming up in a few days? Stay tuned. Stick with us here, people. We got more to get through here.

**[01:33]**
**[Visual: Returns to "2026" slide. Clicks "Swarm AI".]**

Okay, swarm AI. What else we got here? Just another prediction like, "Hey, we'll soon have this kind of cool stuff running on your local Mac." We don't think it's going to be 24 months; we think it's going to be two to four. Why? Our rough rule of thumb: Any predicted timeline modeled on *one* device's inference capability, that excludes the *aggregation* of all of those devices pooled into a globally accessible market... if it doesn't have that, then just assume that that timeline happens 10 times faster than predicted. So Gavin says three years? We say months. You say 24 months? We say a couple months. Swarm AI, aggregating local compute.

**[02:04]**
**[Visual: Returns to "2026" slide. Clicks "Open > Closed".]**

We think open is going to start running circles around closed. Open-source labs, products, agents are going to run circles around closed equivalents. You're starting to see this with OpenCode. Amazing project. Open source. Doing open source right. Which we can just then build on top of with OpenAgents. We're all working together. The closed labs are not.

**[02:31]**
**[Visual: Returns to "2026" slide. Clicks "Agents > Models". Shows a tweet "THE AGE OF MODELS IS OVER. THE TIME OF THE AGENT HAS COME."]**

Agents better than models. "Oh no, you know, model progress is slowing!" Who cares? The age of models is over. I mean, of course, there will be new open models and cool stuff to do, and fascinating to see what DeepSeek and those guys are doing. Claude Code was a massive unlock. Yeah, obviously Opus 4.5 was awesome, but it becomes that much *more* awesome inside of a good agentic harness. And still, Claude Code is still janky and shitty in a lot of ways, but it's still amazing. Okay. The time of the agent has come.

**[03:01]**
**[Visual: Returns to "2026" slide. Clicks "Autopilots".]**

Autopilots. See our last episode, Episode 199. 2025... oh, was that the year of agents? No, it was the year of copilots. 2026 will be the year of *autopilots*. This is the name of our core product. We're calling it Autopilot. More on that soon.

**[03:17]**
**[Visual: Returns to "2026" slide. Clicks "Agent Network(s)".]**

Agent networks. Okay, the big theme, what we want to get across in this video, is Agent Networks. So a few things on this. One is, we've been talking in this series for a while about the multi-agent future. Starting to have people like this former OpenAI researcher, Stanislas [Polu], talking about — it's now a meme: Agents to Network of Agents. This is a good talk.

**[03:40]**
**[Visual: Shows a tweet with a video of Stanislas Polu titled "THE OUTER-LOOP ERA".]**

And then just extrapolating that out further... what does the multi-agent future look like? Well, one ramification we think is that if you are a vertical agent company, you're one company building *one* agent, you're going to get fucking wrecked. Unless your agents are communicating with the broader network. We're trying to build the infra for that broader network.

**[04:03]**
**[Visual: Shows a tweet analyzing a Google DeepMind paper titled "Distributional AGI Safety".]**

Okay, what's next we got here? Even, DeepMind just came out with a paper two weeks ago. Good paper. Good points here. Yeah. Seb [Krier] who runs, Policy/AGI Dev Lead at DeepMind. "We argue AGI may first emerge as collective intelligence across agent networks, not a single system." Agreed. "This reframes the challenge from aligning one mind to governing emergent dynamics. More institutional design than single..."

Institutions... Ooh. So, I agree with a lot of this. I disagree with a lot of this. I specifically think that their conclusions that they come to — that there has to be all of this "institution building," we need to be roping off this closed little environment...

**[04:49]**
**[Visual: Highlights text in the OpenAgents post critiquing DeepMind.]**

Here's what they want: *"The agentic market must operate within a controlled environment, separated from the open internet, real-world financial systems, and critical infrastructure. This insulation is not limited to network-level firewalls... Agents' access to external data must be managed through vetted API gateways rather than direct web access."*

No. No. No. So you guys can do that for your corporate buddies, for your government people. Okay? We startups, we people building in the open, are going to be building this agentic market — open source on open protocols — because that's what's needed.

It's already happening. So none of this is a thing that I'm proposing. Agentic coordination and commerce on open protocol Nostr has been ongoing for a year-plus. Nothing bad has come of it. Partly because it includes other critical market-making mechanisms like reputation and payments built-in. No firewalls, walled gardens, or industry buy-in or government regulation needed.

And as for broad markets of agents, see the draft NIP-SA [Nostr Implementation Possibility - Sovereign Agents]... this is a Nostr protocol plank called Sovereign Agents we posted contributing the last building block to viable autonomous agents on permissionless open protocols. It includes a cryptographic primitive called FROST—Frost on the Nostr protocol—for splitting up an agent's keys living on multiple different decentralized relays, Nostr relays, so that an agent can basically have its own identity. It can sign keys and credentials in a way that cannot be exported by a human. Sovereign agents.

**[06:12]**
**[Visual: Scrolls down the tweet thread.]**

The internet is open. Agents live on it. Deal with it. As to why having a *lot* of those agents around is not as scary as the safety-ist labs begging for regulation would have you believe... check out this amazing talk from Dhruv Bansal. Hardcore Bitcoiner, founder of Unchained.

**[06:23]**
**[Visual: Shows a YouTube video thumbnail of Dhruv Bansal.]**

Two-hour talk there. Interviewed by Max who is another Bitcoiner/Nostr person, one of our investors. And nice little summary there on what this is about. But instead of "Scary AGI," it's really that this is going to be thought of as a digital life form — even though it's not technically life, you could kind of think of it that way. The idea is that digital and biological life are going to be co-existing symbiotically through economic relationships. Markets. Market forces. Okay. We'll get to the Dunbar's number thing in a second here.

**[07:03]**
**[Visual: Returns to the OpenAgents tweet thread.]**

All these primitives are live, awaiting coordination. Is that coordination going to be done first by the bad guys — like the hacker agent swarm factory wrote about? Or closed labs advocating deceleration and regulatory capture? You can think of a few maybe. Or the good guys? Team *Actually* Open AI?

The greater risk than open experimentation with these dynamics is that Team Closed AI builds the *only* valuable agent networks, uses government regulation to prevent disruptive innovation from below, and entrench a croniest priest caste happy to trade *your* data and biometrics for shareholder value. Not on our watch. Okay?

**[07:42]**
**[Visual: Shows the GitHub repository for OpenAgents, specifically a document titled "OpenAgents and Distributional AGI Safety: A Comparative Analysis".]**

So we'll have some more interesting conversations with DeepMind in the future, I'm sure. Or Twitter threads and flaming. Here's a little synthesis I asked Opus to read our kind of philosophy/strategy docs and the AGI paper from DeepMind and kind of come up with things that we can both learn from each other and what potential consensus might be. This is a good read.

**[08:13]**
**[Visual: Switches to a ChatGPT conversation.]**

Finally, I dumped a bunch of this into ChatGPT. Everyone's talking now about labor and what's the future going to look like, and how scary is AGI, and there's going to be all this job collapse. I partially accept those premises, but I think people have wrong what's going to be possible in terms of on the abundance side of things.

So I'm including this link in the Twitter thread as well as if you go to the link that I'm about to share at the bottom here, there's a Read More link that goes to the same share to chat.

**[08:46]**
**[Visual: Switches to a document titled "OpenAgents: The Agentic OS" (SYNTHESIS.md).]**

Let's step through it real quick. I took our 17-page synthesis document. This is basically the entire company. Everything we've been talking about for two years. All of our thesis, all of our philosophy, all of our products, how everything works together, the protocols, glossary, cryptography, Bitcoin payments... literally *everything* is in this one document. This is open source. This is on our GitHub repo. It's in the root level called `SYNTHESIS.md`. We're going to keep this updated. If you're ever wondering "What's OpenAgents doing?" Here's a 17-page document. Feed this to your AIs. Read about it. This is what we're doing.

**[09:23]**
**[Visual: Returns to ChatGPT interface.]**

I took that doc and I fed it into ChatGPT and I said, "List the ramifications of an agent group-forming network for society generally." Okay, here's where I'm going to pause and introduce the most important single concept that AI discourse has been completely missing. And I mentioned it in this tweet at the DeepMind people.

**[09:47]**
**[Visual: Flashes back to the Tweet criticizing DeepMind.]**

I said one of the criticisms from Opus of DeepMind's thing was that they don't fully grapple with the implications of agents lacking cognitive constraints. Okay. I'm surprised they didn't mention **Reed's Law** of group-forming networks.

**[09:59]**
**[Visual: Shows Google search for "Reed's law of group forming networks" and the Wikipedia page.]**

If you Google Reed's Law of group-forming networks... there's a good post about it on... Wikipedia is okay. "The utility of large networks, particularly social networks, can scale exponentially with the size of the network." Now, stick with me here because this is really fucking important.

**[10:19]**
**[Visual: Speaker is overlaid on an interactive graphic titled "GROUP FORMING NETWORKS".]**

Namely, the law says that the value, the network value of a group-forming network is 2 to the *n*-th power ($2^N$) where *N* is the number of participants. Contrast it with Metcalfe's Law ($N^2$), which came out of telecommunications, but that was loosely used to describe the network value of peer-to-peer networks where the value of the network is *N*-squared. And when Uber started running circles around the taxis, people started quoting Metcalfe's Law as an example of how this pseudo, almost peer-to-peer network called Uber was able to build all this network value contrasted with hierarchical organizations like the taxis.

And then you say, okay, you know, David Reed comes along and he says, "Well, the most evolved form of network is not a peer-to-peer network. It's a group-forming network." Which is where you start with a peer-to-peer network and you make it very easy for participants to form groups with each other. In that case, the network value—upper bound you can call it—is 2 to the *n*-th power.

**[11:13]**
**[Visual: Wikipedia page for Reed's Law, scrolling to "Criticism".]**

Now, criticism of this was, "Oh no, no. That overstates the network value because they fail to account for the restrictive impact of *human* cognitive limits on network formation." Oh. Dunbar's number. Us humans can only really build connections with 150 people. Implies a limit on the number of inbound and outbound connections a human in a group-forming network can manage. The actual maximum value structure is much sparser than the set of subsets...

Agents are not subject to Dunbar's number. Agents, increasingly this year, will be fully independent—or partially independent—economic actors with their own wallets (Bitcoin, stablecoin, dollar wallets) that they can transact. And they can coordinate at machine speed all over the place.

We can now start to reason about *agent networks* that approach the upper bound of Metcalfe's Law [Note: Speaker likely meant Reed's Law here based on context].

**[12:22]**
**[Visual: The "GROUP FORMING NETWORKS" interactive graphic again.]**

I built a little visualization here. It's kind of laggy as hell because I'm not even drawing anything close to the real amount of nodes. But just if you've got 8 nodes... there's 64 possible connections in Metcalfe's Law. 247 over here [Reed's Law]. Now as this number starts getting up, that starts getting crazy. This will start to lag my computer.

**[12:49]**
**[Visual: Increases the number of nodes on the slider. The Reed's Law graph (orange) becomes immensely dense compared to Metcalfe's Law (blue).]**

2 to the *n*-th power. You guys know exponentiation? Basics? Okay? When *N* grows, that gets crazy. This is a log scale. Just... you can't comprehend the network value of a network with thousands or millions of agents. We just can't comprehend the possibilities there.

**[13:16]**
**[Visual: Returns to ChatGPT.]**

Now, with that in mind—this sort of mind-boggling concept—let's bring this back to ChatGPT. And say, okay, what are the ramifications of an agent group-forming network for society... with the full background of all of the stuff... this explains Reed's Law and how OpenAgents is going to be building for it protocols and blah blah blah.

**[13:36]**
**[Visual: Scrolling through ChatGPT output.]**

Scroll, scroll, scroll... They really need a button to jump to the most recent message. Okay. Let's just skim through this. Now this is all public, you can read this. I encourage you to fork this conversation and have your own questions that you ask of this.

So here are the big society-level ramifications where once you have a group-forming network of agents—Reed's Law dynamics—where agents can rapidly discover each other, form coalitions, split work, split payments, and persist as teams...

Happy New Year.

**[14:02]**
**[Visual: ChatGPT Output: "Economy and labor markets".]**

Okay, Economy and Labor Markets. Execution becomes radically cheaper than coordination. *Doing* (writing code, running tests, negotiating quotes, drafting contracts, generating ads) collapses in cost. The scarce resource becomes goal selection, taste, risk ownership, and capital allocation. Most white-collar work unbundles into micro-contracts. So it's not that it gets eviscerated or destroyed; it just might not be in the sort of standard employer framework that you're used to. Jobs fragment into task graphs purchased on demand by agents, pushing employment toward markets and away from W-2 style roles. *Toward markets* is the important... markets, markets, markets. We'll be touching on that concept a lot.

Okay, new middle layer emerges: Human Portfolio Managers of Agents. People supervise fleets, set policies, allocate budgets, and review outcomes—more like managing capital than doing work. Yeah, you're going to have fleets of agents that you command. We want to make that easy for you. And give you a cool sci-fi HUD to do so.

Okay, winners... distribution and budgets... blah blah blah. Rapid wage compression in routine knowledge roles. Anything spec-able becomes a commodity. Wages shift toward roles that require trust, accountability, blah blah blah.

**[15:06]**
**[Visual: ChatGPT Output: "New organizational forms".]**

Okay so, New Organizational Forms. We'll just skim the rest of this. "Guilds, Syndicates, and Swarms become normal. Temporary coalitions (human + agent + agent) form for a job, dissolve, and reform like ad-hoc firms."

Guilds. Love this term. I led a guild for seven years in EverQuest and EverQuest 2 and Vanguard. We're going to be bringing back guilds. We're going to make it easy for you to create a guild of humans and agents.

Okay, firms become protocol-native. Markets accelerate. Disintermediation pressure.

**[15:37]**
**[Visual: ChatGPT Output: "Power, inequality, and concentration".]**

This is interesting. Two opposing forces collide: Decentralization—anyone can contribute compute skills and earn. Concentration—the best orchestrators/markets can snowball, winner-take-most, because group-forming networks amplify incumbents.

See, here's... what I'll qualify with there. One of my observations in Open AI's massive failure to create and launch GPT Store was that OpenAI is not really... they're kind of a product company, they're definitely not a marketplace company. And if you go back to Episode 1 of this series where I said the day after they announced GPT Store and they were going to do rev-sharing, DevDay 2023... I was saying, "They're gonna half-ass that. There's no way they're gonna do what's necessary." And they *did* half-ass it.

Fast forward two-plus years, they've paid *zero* developers. We've paid 20 developers for our MVP agent store a year ago. We've paid infinity percent more developers than OpenAI. We'll get to the payment piece in a bit here.

But so you've got labs, Anthropic and OpenAI, xAI... every big lab would love to, you know, *own* agents. But we think they're really structurally incapable of owning agents. Because so much of this is disintermediating. Why are you going to use labs from... or agents from one particular lab? Aren't you going to want everything to be able to work well across each other? Are these companies going to be harnessing Reed's Law dynamics? They just... they just can't. They can't do that.

That's probably worth a separate video about why they're structurally incapable of leading this. And why it's got to be open, neutral, independent folks like us.

**[17:22]**
**[Visual: Scrolling past Politics, Security, Information Ecosystem sections in ChatGPT.]**

Okay, politics and governance, blah blah blah. So there's more here.

**[17:28]**
**[Visual: ChatGPT Output: "The meta-ramification".]**

The Meta-Ramification. A group-forming agent network turns society into something closer to an *economy of rapidly composing micro-organizations* than a world of stable firms and stable jobs. The dominant advantage becomes: Who can form the best coalitions, with the best incentives, under the best constraints, with the best verification.

**[17:45]**
**[Visual: ChatGPT input prompting for a comparison between OpenAgents and DeepMind.]**

And then, I fed it the, OpenAgents versus DeepMind analysis doc. I said, speculate more on this.

**[17:53]**
**[Visual: ChatGPT output discussing "The real battleground is 'coalition latency', not 'agent intelligence'".]**

"The most interesting speculation isn't who's right, it's what actually happens when Reed's Law + no Dunbar limit + money + open transport goes live."

"The real battleground is **coalition latency**, not agent intelligence."

Lots of thoughts on intelligence, intelligence, intelligence, but I think there's *been* a growing recognition that coordination may end up being the bottleneck. And then, how does that coordination best happen? Obviously on open and neutral protocols. We think that's obvious.

For everybody trying to push a proprietary framework or a crypto shitcoin and say it's got to come through *you*? No.

The only logical meeting point for this huge network to coalesce is on neutral and open protocols. For us, we think it's obvious that that's Bitcoin and Nostr. We will make that case. I could see something coming up to supplement Nostr or replace it maybe... but it's got all the properties of what I think is needed. We'll do more content videos on that.

**[19:04]**
**[Visual: Scrolling through more ChatGPT text.]**

Economic alignment works... blah blah blah. "Capture" will attack choke points...

**[19:15]**
**[Visual: ChatGPT Output: "5) The DeepMind 'sandbox' and OpenAgents 'open protocols' will converge into a two-plane architecture".]**

This is interesting. "The DeepMind 'sandbox' and OpenAgents 'open protocols' will converge into a two-plane architecture."

I think the most realistic synthesis is:
**Plane A: The Open Plane (your default).** Nostr + Lightning as the public substrate. Marketplaces, skills, compute, coalitions. Aggressive transparency and receipts. Fast iteration, permissionless.

**Plane B: The Containment Plane (Optional but economically necessary).** Used for high-risk tools and high-capability experiments. Semi-permeable IO with explicit ports. Costs money (insurance, bonds, fees). Produces signed artifacts back to Plane A.

Okay. This is clearly what DeepMind was trying to articulate the architecture for this containment plane.

Power to them and anyone who wants to do the **boring work** of trying to figure out how to hand-hold legacy institutions. Great. Thanks for doing that so *we* don't have to.

I want to build Plane A. The Open Plane. I invite you all to meet us there and help us build it.

**[20:13]**
**[Visual: ChatGPT Output: "7) The defining war will be: Open 'Ecology' vs Closed 'Corporate anthill'".]**

All right. There's more here. Let's skip down. "The defining war will be Open Ecology versus Closed Corporate Anthill." The ecology wins. Can we just fast forward? Can we just skip the war? We win. Victory. We win. Open. Open wins. [Sigh]

**[20:34]**
**[Visual: ChatGPT Output: "10) What you should build (speculatively) if this doc is your worldview".]**

Blah blah blah blah blah. What you should build. Ya ya ya. "If you want, I can turn this into a tighter 'OpenAgents Safety Thesis v2'..." Here's one way: We are emergence-first. We acknowledge irreversibility domains. We make safety a market, not a ministry. We attach accountability to budget signers and receipts. We keep the system forkable to avoid capture. Yeah, all this is open source on our, GitHub repo.

**[20:55]**
**[Visual: ChatGPT input prompting for specific "network value" concepts.]**

Okay, lastly. And just to help make the concept of network value a little bit more concrete. You're talking about this insane network value. How does that map to... In 2 to the *n*-th power in a network where *N* is a million or 10 million... that's insanely high. How would that manifest?

**[11:11]**
**[Visual: ChatGPT Output explaining Reed's Law manifestations.]**

The Reed's Law is saying that the network's value is dominated by the number of *possible coalitions*. For *N*=10 million, the raw number is so astronomically large that you should treat it as "effectively infinite optionality." The useful question becomes: What does "optionality" cash out as in the real world given coordination costs?

**1) Time-to-assemble a competent team goes to ~0.**
The value isn't that $2^N$ coalitions *exist*, it's that for *any task*, there exists many coalitions that can do it, and the cost to find/compose one is low.

"Build me X" doesn't mean "hire a team." It means "route to an instantly formed micro-firm assembled from specialists." So snap your fingers, the thing happens. The limiting factor becomes latency of discovery + contracting + verification, which your stack tries to minimize. All that should happen at machine speed.

**2) Extreme specialization becomes liquid.**
When the network is huge, you can support "long-tail specialists" that would never exist in a smaller market. You as human are going to be able to find the pocket of thing that you can specialize in and monetize that. Finer-grained division of labor.

**[12:22]**
**[Visual: Scrolling: "Combinatorial composability of workflows".]**

Combinatorial composability of workflows. Okay. There's more to chew on.

**[22:29]**
**[Visual: ChatGPT input prompting for "labor/abundance perspective, how can humans benefit from this?"]**

Okay. "From a labor and abundance perspective, how can humans benefit from this?"

Humans benefit if the network makes **abundance legible and payable**. It turns cheap machine work into cheaper goods and services, and direct income streams that lots of people can tap.

**1) Everyone gets a "personal corporation" (fleet) instead of one job.**
You stop selling hours. You allocate goals and budgets to agents that do the execution. You're playing StarCraft.

**Benefit:** A normal person can run many projects in parallel (side businesses, content, software research) with near-zero marginal overhead.

**[23:03]**
**[Visual: Briefly flashes to Twitter to repost an Aaron Slodov tweet about Millennial gamers.]**

Okay, I gotta share this tweet from Aaron. "Millennial gamers are the best prepared generation for agentic work, they've been training for 25 years." My fellow Millennials... we're gonna lead this, okay? We gotta make it fun for the Gen-Zs, get them off the Tick Tocks... [Laughs].

**[23:24]**
**[Visual: Returns to ChatGPT.]**

Where was I? Okay.

**2) New income streams that scale without being a celebrity.**
The big win is **micropayment income** for small contributions, paid automatically.

**Examples:**
*Skill royalties:* You publish a useful skill or workflow, it gets called 10,000 times a day, Bitcoin flows to you. Or micro-US Dollars if you want fiat.
*Data and trajectory royalties (opt-in):* You can contribute verified work traces. There was a Twitter thread last week about all developers using Cursor or Claude Code having valuable data on their computer they're not doing anything with. All that's valuable to agents. Will my agents want to pay you a few cents for your data? Absolutely. If you want to sell it, you can. I'll buy it.
*Compute dividends:* Your idle laptop or office cluster sells verified jobs while you sleep. Why are you not running agents around the clock making you money around the clock? The technology is possible.
*Guardianship/Audit services:* You hold keys, run monitoring, provide dispute resolution, get paid fees.

**3) Goods and services get cheaper because coordination cost collapses.**
Local abundance, communities monetize what they already have. Humans move up the stack to roles that stay scarce. People are already talking about this on Twitter, at least on my feed.

**[16:40]**
**[Visual: ChatGPT Output: "6) Redistribution can be built-in without 'government communism'".]**

**Redistribution can be built-in without government communism.** AKA, UBI. Which, for some reason... look at the big labs like OpenAI. How many developers have they paid? How many people have they paid out from their stores? Zero. What are they doing? OpenAI, Anthropic. They're begging the government to make regulation and talk about UBI.

No. These AI engines should be paying people.

Any AI lab that's trying to lead any of this and isn't paying people... you're going to get circles run around you by the labs that *are* paying people. Because we're going to build network effect and you're not.

**[25:20]**
**[Visual: Reading ChatGPT text.]**

"If the economy is natively microtransactional, you can encode broad participation mechanically: default revenue splits that pay creators/compute providers/referrers... 'citizen dividend' style pools funded by transaction fees... community treasuries that invest in local compute and pay members... apprenticeship rails: humans get paid to review/label/verify outputs."

Do we know of any way of sending instant micropayments anywhere in the world? Bitcoin via the Lightning Network. Yes. Done. No crypto shitcoin needed. Bitcoin. Done. You want it in USD? Bitcoin-native stablecoin. Done. This all exists.

All the tech for all this stuff is solved. We just need the coordination of the actual market. Oh, maybe that's what OpenAgents should launch next week. Okay. Safeguard budgets, yeah yeah yeah.

**[26:05]**
**[Visual: ChatGPT input requesting a concise framing on deflation and dividends.]**

Here, I like this. Here we're going to kind of summarize and wrap up here. The Concise Framing.

A group-forming agent network can make humans richer in two ways:
**1. Deflation:** Things you want cost less because execution and coordination are cheap.
**2. Dividends:** You earn from small contributions. Skills, compute, data, verification pay continuously.

Okay. In conclusion. Give me one paragraph on deflation, one on dividends, and one summarizing the combination.

**Deflation:**
In a coalition-forming agent economy, the price of many goods and services falls because the expense part of production isn't just *doing* the work, it's *coordinating* people—hiring, management, handoffs, QA, compliance, contracting, and billing. When agents can self-assemble into competent micro-teams on demand, run verification cheaply, and ship outcomes with receipts, huge swaths of coordination overhead collapse. The result is real deflation in anything that's digitalizable and spec-able: software, design iterations, marketing ops, research synthesis, back-office admin—pushing more of your paycheck toward optional spending and raising living standards even if nominal wages re-balance.

A lot of this is just happening because of the AI labs, but we're going to be accelerating that by building really good agents on top.

**[27:17]**
**[Visual: ChatGPT Output: "Dividends".]**

Here's what no one else really is doing at all, that I guess OpenAgents is going to be uniquely doing. Until we pressure other people into doing it too. But, Dividends.

The same network can pay humans continuously for contributions that were previously uncompensated or impossible to monetize at small granularity. Or making right previous essentially theft of OpenAI and these people training on artists' output and not compensating artists.

Skills and workflows become royalty assets. A useful automation can earn sats (Bitcoin) per invocation across thousands of users. Idle compute becomes rentable inventory. Your laptop, office cluster, or co-op can sell verified jobs while you sleep. Opt-in data and trajectory contributions can be compensated directly. And new roles—reviewers, auditors, guardians, underwriters—earn fees for keeping the market honest. These are "dividends" in the literal sense: many small streams, paid automatically, that don't require celebrity-scale reach or a single employer.

**[28:14]**
**[Visual: ChatGPT Output: "Combination".]**

Combination. Put together, deflation plus dividends is the abundance story. Life gets cheaper at the same time that more people can earn from the network's ongoing activity. Deflation lowers the cost of building and running projects, so more individuals and small groups can launch things that used to require institutions. Dividends ensure the value created by trillions of microtransactions doesn't all accrue to a few platform owners, but is shared with creators, providers, verifiers, and contributors who keep the system running.

If OpenAgents gets the defaults right—open access, verifiable work, budget-bounded autonomy, and automatic revenue splits—the agent economy becomes a machine for both lower prices and broader participation.

Okay. That's a good stopping point here. We have relevant product announcements and launches over the next few days-ish. Give us a week or two for some of it. Here's what you can expect.

You got spare compute? Can we buy it from you? Making it available in a global marketplace?

Claude Code. Are you using your Claude Code around the clock? Or overnight, you want to put it into a mode where it can do work for you and you wake up with more money?

The Autopilot product. How would you like to connect a GitHub repo, and then the rest of it just sort of happens? You specify a direction, and a global market of agents—and people if they want to—compete to do that work at the best price. You just say what you want and it happens.

This is a two-sided marketplace. We built all the pieces for it. And we're excited to share it with you. See you soon.
