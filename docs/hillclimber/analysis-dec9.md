tweet:https://x.com/patrick_oshag/status/1998415146427781237

Gavin [Baker] explains that the bear case for AI capex spend is on-device inference:

"In three years, on a bigger phone, you'll be able to run a pruned-down version of Gemini 5, Grok 4, or ChatGPT.

And that's free. This is clearly Apple's strategy - we're going to make it privacy-safe and run on the phone.

Other than scaling laws breaking, edge AI is by far the most plausible and scariest bear case."

---

Our comment: https://x.com/OpenAgentsInc/status/1998437331070505222

Hedge fund man makes sense here

But thinks this will take three whole years? Nah

Even the recent "uh-oh maybe bearish capex trade" realizoors are still dramatically overestimating timelines to that shift happening

Because they're thinking in terms of what edge inference & test-time compute ONE device can do for THAT user

Ignoring the economics of AGGREGATED compute (or "swarm" compute as we've been calling it for a long time: https://x.com/OpenAgentsInc/status/1926403708658544794) across millions of devices currently idle

If you have SPARE, UNUSED ("stranded") COMPUTE on your device that I CAN PAY YOU FOR, of course you'll sell it to me for a teensy bit of bitcoin straight to your wallet

Well guess who's been building all of the pieces for this new platform shift across 198 videos for the last two years.

You'll want to stay tuned for episodes 199 and 200 😎

---

## How We're Implementing This Vision NOW

Baker's thesis is correct but his timeline is wrong. He's thinking about what ONE device can do for ONE user. We're building for AGGREGATED compute across MILLIONS of devices. Here's how Commander implements every piece of this vision today:

### 1. Proving Local Can Win: FM Hill Climber

We're not waiting for "pruned-down Gemini 5" in three years. We're running Terminal-Bench tasks using Apple's on-device Foundation Model RIGHT NOW.

**Current achievement:** 89.5% on Terminal-Bench `regex-log` using only local FM inference.

The MAP (Modular Agentic Planner) architecture demonstrates that **architecture beats raw model size**. A well-structured local system with parallel sampling, test-time compute, and iterative verification can compete with cloud giants.

If we hit #1 on Terminal-Bench with local inference, we prove the bear case isn't three years away—it's here.

### 2. The Three Curves: Scientific Validation

We're not guessing. We're measuring:

1. **TestGen Score vs Evolution Step** — Does our meta-learning work?
2. **HillClimber Pass Rate vs TestGen Config** — Does quality transfer to performance?
3. **TB2 Performance vs Internal Metrics** — Is our proxy valid?

If all three curves slope upward, we've proven that training and architecture can substitute for model size. This is the scientific foundation for why edge AI arrives faster than analysts expect.

### 3. Commander: The Swarm Client

Commander is the desktop app that makes this real:

- **Built-in bitcoin wallet** — Self-custodial, Lightning/Spark, no signup required
- **"Go Online" button** — One click to start selling your spare compute
- **Agent training (GYM)** — Train agents that use the swarm network
- **Agent Store** — Publish agents, others pay for compute when they run

This isn't a roadmap item. We've built all these pieces separately over 198 episodes—wallet, compute network, agent framework. Commander combines them into one product.

### 4. MechaCoder: First Swarm-Native Agent

MechaCoder is an autonomous coding agent that:

- Runs locally when possible (Apple FM)
- Taps swarm compute when local isn't enough
- Completes real coding tasks (Golden Loop v2)
- Demonstrates actual utility of edge inference

MechaCoder proves that useful autonomous agents can run on the edge. Not in three years. Today.

### 5. The Swarm Compute Network

From Episode 178 (May 2025):

> "First working production example of the OpenAgents compute network. Shows clicking 'go online' to sell compute for Bitcoin. Demonstrates using local Gemma 1B model, then switching to a more powerful DevStrel model (24B parameters) running on a remote Linux desktop via the swarm network. Shows Lightning invoice payments processing automatically."

The network EXISTS. Users CAN sell compute. Payments ARE instant. The only question is scale—and bitcoin incentives solve that.

### 6. Why We're Faster Than Baker Thinks

Baker's mental model:
```
ONE device → runs inference → replaces cloud for THAT user
Timeline: 3 years (waiting for better on-device models)
```

Our mental model:
```
MILLIONS of devices → aggregated compute pool → new market
Timeline: NOW (architecture + economics, not waiting for models)
```

**The key differences:**

| Baker's View | Our View |
|--------------|----------|
| Wait for bigger on-device models | Architecture beats model size (Three Curves) |
| Free inference replaces paid cloud | Paid swarm creates new market |
| One device serves one user | Aggregated compute serves everyone |
| Privacy as feature | Privacy + income as features |
| 3 year timeline | Already shipping |

### 7. The Economic Flywheel

Baker sees edge AI as a THREAT to cloud capex. We see it as a NEW MARKET:

```
User sells spare compute → earns bitcoin
    ↓
Bitcoin incentive → more users join
    ↓
More compute available → better agents possible
    ↓
Better agents → more demand for compute
    ↓
More demand → more bitcoin for sellers
    ↓
REPEAT
```

Cloud AI is a cost center (you pay). Swarm AI is a profit center (you earn). This changes adoption dynamics entirely.

### 8. What We Ship vs What They Predict

**Baker predicts (3 years):**
- Pruned-down Gemini 5/Grok 4/ChatGPT on phones
- Free, privacy-safe, replaces cloud

**We ship (now):**
- Apple FM running Terminal-Bench at 89.5%
- Bitcoin payments for compute (Lightning/Spark)
- Self-custodial wallet in Commander
- Swarm network with real inference jobs
- MechaCoder autonomous agent using it all

The bear case for cloud AI capex isn't coming in three years. It's being built in this repo.

---

**Bottom line:** Hedge fund analysts model what ONE device does for ONE user. They miss the aggregation play. When you can PAY someone bitcoin for their idle compute, adoption isn't constrained by device capability—it's driven by economic incentive. That's why we're faster than three years. That's why we're building Commander.
