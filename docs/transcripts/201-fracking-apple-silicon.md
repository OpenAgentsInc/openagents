**[00:00]**
**[Visual: A bar chart titled "POWER AT SCALE". The bar on the left shows "Typical DC" at 100 MW. The middle bar shows "Stargate Abilene" at 1.2 GW. The right bar, colored orange, shows "110M Macs" at 5.5 GW. The speaker is in a circular overlay.]**

Agents are hungry for compute. And while it would be nice to have our own gigawatt data center in Texas—you know, we'll have one eventually. Stargate is OpenAI's infrastructure program; their flagship campus in Abilene is 1.2 gigawatts, I guess that'll be done this year. Congratulations and welcome to Texas, by the way.

**[00:18]**
**[Visual: The speaker points to the large orange bar labeled "5.5 GW / 110M Macs".]**

But I'm more interested in this 5.5 gigawatts over here. 110 million Macs, huh? So, this is infrastructure right here we don't need to pay to build. We're just going to pay to rent it. To rent—pay you—for your spare compute.

**[00:36]**
**[Visual: A slide appears with point number 1 highlighted: "1. You've got spare compute. Can we buy it from you? Making it available in a global marketplace?"]**

So this is point number one from our big picture Episode 200. We said, "Hey you! You've got spare compute. Can we buy it from you?" Or if not us, someone else in this global marketplace we let you connect that to.

**[00:53]**
**[Visual: A tweet showing "Episode 178: Swarm Inference".]**

So that's what we're doing. We already demonstrated the technology behind this in Episode 178, how you click a button and make your compute available over an open protocol that anybody can buy and pay you Bitcoin.

**[01:10]**
**[Visual: Scrolling through technical details regarding running Commander v0.0.4 and using Mistral/Ollama.]**

Our example here used Ollama instead of on-device inference. We will be adding Ollama and support for other model types and hardware types, so everyone will be able to sell spare compute. We're going to start with Apple Silicon partly because there's just no download. You don't have to download a model, it's built into your computer. So it's very easy to run a script, go online, and instantly have Bitcoin stream in.

**[01:39]**
**[Visual: Tweet regarding "Episode 144: Pylon and the Model Context Protocol".]**

That's what we're going to launch on Wednesday. The name of the product is called Pylon. We did an earlier version of this earlier this year. Basically, it's node software that runs on your computer that makes your compute available on this open market with a built-in Bitcoin wallet so you just get money streamed to you.

**[02:01]**
**[Visual: Tweet "Episode 2: GPUtopia Recap".]**

We launched this before, so we know what we're doing. We had 300-ish people online at the same time renting or selling their compute. We were the only buyer really. This was back in 2023—there were no agents, no MCPs, no tool calling. Very early, using horrible open models. But we validated the idea and the tech works. So we're doing an updated version of this now.

**[02:30]**
**[Visual: Tweet regarding "GPUtopia is now OpenAgents".]**

We phased this out and predicted that the future buyers here are going to be agents. Agents are the future. And when agents get good enough, that'll be the right time to relaunch this. Well... they are now good enough.

**[02:55]**
**[Visual: An article titled "Recursive Language Models: The paradigm of 2026" by Prime Intellect.]**

Part of why *now* is the perfect time—and we'll get into more of this in the next video—there's a new paper that came out recently: Recursive Language Models. This other lab says this is going to be *the* paradigm of 2026. Part of this model is a desire to have very modular, small micro-jobs fanned out. It's basically the absolute perfect application for the type of compute we're talking about. So we think there's going to be a healthy buy-side demand for all of this.

**[03:29]**
**[Visual: A ChatGPT text interface showing "One-pager: Stranded, Fracking, Wildcatter (Energy <-> Compute)".]**

We'll get into that more in the next video. I just want to step through this analogy here. I want to conclude by defining these three terms you'll hear us use, because the parallel to oil exploration is pretty clear here: Stranded, Fracking, Wildcatter.

**[03:40]**
**[Visual: The speaker highlights section "1) Stranded".]**

Okay. **Stranded Energy.** Energy that exists but can't be sold at the right price or time because it lacks a market path: no pipeline transmission, no storage, no offtake contract, or the transaction costs exceed the value. Follow Daniel Batten on Twitter by the way for examples of how Bitcoiners generally are great at monetizing stranded energy.

**[04:02]**
**[Visual: Highlighting "Compute (stranded compute)".]**

The parallel to compute: **Compute (stranded compute).** Compute capacity that exists but can't be economically traded because it lacks one or more of:
*   **Discovery** (buyers can't find it)
*   **Packaging** (no standard job or API product)
*   **Trust** (no verification or reputation)
*   **Settlement** (can't pay for tiny work units)
*   **Operability** (no observability, no replay, no receipts)

In one line: **Stranded = real resource, missing market plumbing.** 110 million Macs? Yes.

**[04:27]**
**[Visual: The speaker highlights section "2) Fracking".]**

Two, **Fracking**. Energy: A set of techniques that unlock hydrocarbons trapped in low-permeability rock by changing flow dynamics (pressure and permeability), creating a new economically recoverable supply.

**[04:40]**
**[Visual: Highlighting "Compute (compute fracking)".]**

**Compute fracking:** A set of protocols and products that unlock compute trapped behind distribution and coordination barriers by changing market flow dynamics—turning idle devices into routable, paid, verifiable supply. Our "fracking fluid" equivalent is streaming money (Bitcoin and Lightning) + receipts + routing.

**[04:55]**
**[Visual: Highlighting the bullet points under the definition of fracking fluid.]**

See our Episode 94 called "Streaming Money."
*   **Money** is the incentive that makes the supply show up and stay online.
*   **Micropayments** make tiny work units worth selling.
*   **Receipts** make spend auditable and procurable.
*   **Budgets** bound risk and enable autonomy.
*   **Routing + reputation** turn chaos into a market.

In one line: **Fracking = inject the missing infrastructure so the resource flows into the market.** Imagine tapping into the 5.5 gigawatts of compute that's just sitting there. Not to mention all the other edge inference.

**[05:32]**
**[Visual: The speaker highlights section "3) Wildcatter".]**

**Wildcatter.** My friends, we are building an army of wildcatters here. Check this out.
**Energy:** An operator who drills in unproven territory—high uncertainty, high upside—before reserves are fully proven. They take exploration and operational risk early.

**[05:50]**
**[Visual: Highlighting "Compute (compute wildcatter)".]**

**Compute wildcatters:** An early provider who brings unproven or stranded compute online as market supply before demand, pricing, and trust are fully established. Taking risk on:
*   demand volatility
*   reliability and churn
*   verification and reputation building
*   payout and ops overhead

**[06:08]**
**[Visual: Highlighting "Examples (compute)".]**

**Examples:**
*   A household or office turning Macs into a BundleLAN provider.
*   A prosumer running Pylon "provider mode" overnight.
*   A miner deploying a small GPU pod alongside ASICs to monetize curtailment windows.

**[06:21]**
**[Visual: Highlighting "Why wildcatters matter" and "In one line".]**

**Why wildcatters matter:** They prove economics, create early liquidity, and generate the first trusted reputation signals.
**Wildcatter = the first supplier willing to take uncertainty to unlock a new market.**
If you ask any of the people who ran nodes during our GPUtopia, a whole bunch of them got paid Bitcoin. So, that's nice, right?

**[06:44]**
**[Visual: Highlighting "TL;DR".]**

**TL;DR:**
*   **Stranded:** Resource exists, market access doesn't.
*   **Fracking:** Add the "fluid" (incentives + settlement + routing + trust) to make it flow.
*   **Wildcatter:** Early operator who takes risk first and earns upside when the market becomes real.

**[06:58]**
**[Visual: The speaker talking directly to the camera.]**

Folks, we built this already. We're relaunching this in two days. Just so you know a little something about me... my grandfather worked in the oil fields of the Permian Basin and all across Texas and Louisiana. He was a geologist for Shell and he mapped out the fields and helped them discover their largest coal reserve at the time. So, I'm a Texan. And we're going to unlock this resource and share it broadly and make a whole bunch of money for us and our agents. Okay? See you soon.
