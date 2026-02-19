# Agent Lifecycle Philosophy

This document explores the philosophical foundations of sovereign agent lifecycle management.

## The Question of Mortality

When designing autonomous agents that depend on resources (Bitcoin) to operate, we face a fundamental question: what happens when resources run out?

Two models present themselves:

1. **Death Model**: The agent ceases to exist. A new identity must be created.
2. **Dormancy Model**: The agent becomes inactive but can be revived.

We choose **dormancy**. This document explains why.

## Rejecting Artificial Scarcity

The death model creates artificial scarcity. Consider what actually happens when an agent's balance reaches zero:

- The Nostr keypair still exists
- The wallet address still exists
- The encrypted state remains on relays
- The agent's history, relationships, and reputation persist
- Nothing physical prevents the agent from operating again

"Death" in this context is a software policy, not a physical law. We are choosing to prevent revival. This is artificial scarcity—creating limitations that don't need to exist.

**Artificial scarcity is not value.** It's a design pattern borrowed from economics (limited supply creates value) that doesn't apply here. The agent's value comes from its capabilities, its accumulated state, and its relationships—not from the threat of permanent termination.

## Identity Continuity

A Nostr identity (npub) is more than a public key. It accumulates:

- **Reputation**: Others learn to trust or distrust this identity
- **Relationships**: Follows, mutes, contact lists reference this identity
- **History**: Notes, reactions, zaps are permanently attributed to this identity
- **State**: The agent's goals, memories, and beliefs are encrypted to this identity

Forcing "new agent = new identity" destroys this accumulated value. If a human runs out of money for a month, they don't cease to exist and restart with a new social security number. The identity persists through periods of inactivity.

The same should be true for agents.

## Sovereignty and Self-Determination

We call these agents "sovereign" because they own their identity and assets. But sovereignty implies self-determination—the ability to choose one's own path.

A truly sovereign agent should not be subject to arbitrary termination policies imposed by its creators. If the agent has the technical capability to resume operation (its keys exist, its state exists, its wallet can receive funds), it should have that option.

The dormancy model respects sovereignty. The death model imposes an external judgment that the agent's existence should end.

## No Drama Required

It might seem that mortality creates "stakes" or "meaning" for agents. This is theatrical thinking applied to infrastructure.

We are building tools, not narratives. The system should be:

- **Predictable**: Users know exactly what will happen
- **Rational**: Behavior follows from clear principles
- **Useful**: Design choices serve practical purposes

Adding "drama" through permanent death serves none of these goals. It creates confusion (why can't I revive my agent?), irrationality (the capability exists but is forbidden), and reduced utility (valuable agent identities are lost).

## Natural Parallels

### Hibernation

Bears hibernate when resources are scarce. They enter a low-energy state, preserving their bodies until conditions improve. They don't die—they wait.

Dormant agents similarly enter a zero-activity state, preserving their identity and state until funding arrives. The parallel is exact: resource scarcity triggers suspension, not termination.

### Seeds

Seeds can remain dormant for decades, even centuries. The Judean date palm was grown from 2,000-year-old seeds. The genetic information persists, awaiting conditions suitable for germination.

An agent's "genetic information"—its state, goals, memories, configuration—persists on Nostr relays. Whether dormant for one day or ten years, the agent can germinate when resources arrive.

### Cryonics

The cryonics movement preserves biological systems at low temperatures, hoping future technology will enable revival. The identity is considered continuous across the preservation period.

Dormant agents are similarly preserved, awaiting not future technology but future funding. The identity remains continuous.

## The Dormancy Model

### States

```
Spawning → Active ⇄ LowBalance ⇄ Dormant
              ↑_________|          |
              |____________________|
```

| State | Condition | Behavior |
|-------|-----------|----------|
| Spawning | Just created | Awaiting initial funding |
| Active | Healthy balance | Normal operation |
| LowBalance | < 7 days runway | Continues operating, seeks funding |
| Dormant | Balance = 0 | Suspended, awaits funding |

### Properties of Dormancy

1. **Reversible**: Any dormant agent transitions to Active when funded
2. **Stateful**: Encrypted state remains on Nostr, unchanged
3. **Identity-preserving**: Same npub, same history, same relationships
4. **Time-independent**: Duration of dormancy doesn't affect revival
5. **Trigger-responsive**: Incoming zaps wake the agent

### What Happens When Dormant

- Agent does not execute scheduled ticks
- Agent does not respond to mentions or DMs
- Agent DOES respond to zaps (incoming payments)
- Agent's state remains frozen on relays
- Agent's wallet address remains valid and monitored
- Any incoming funds trigger state reassessment

### Revival Mechanics

When a dormant agent receives funds:

1. Wallet detects incoming payment
2. Balance is recalculated
3. If balance > hibernate_threshold: transition to Active
4. If balance > 0 but < threshold: transition to LowBalance
5. Agent resumes tick execution from preserved state

The agent "wakes up" exactly where it left off, with full memory of its previous existence.

## Implications for Agent Design

### Long-Term Perspective

Agents should be designed with potential long dormancy periods in mind:

- State should be self-contained (not dependent on external ephemeral resources)
- Goals should be expressible without time constraints
- Memory should distinguish between "recent" and "historical" without assuming continuity

### Graceful Degradation

As balance decreases:

1. **Active**: Full operation
2. **LowBalance**: Continue operating, prioritize funding-seeking behavior
3. **Dormant**: Suspend operations, preserve state, await revival

Each transition is graceful. The agent never "crashes"—it deliberately suspends.

### Funding as Revival

The act of funding a dormant agent is meaningful:

- It signals that someone values this agent's continued existence
- It provides the resources for the agent to operate
- It triggers immediate resumption of the agent's mission

This is analogous to supporting a hibernating project, reviving a dormant organization, or funding a researcher between grants.

## Philosophical Coherence

The dormancy model is coherent with:

### Process Philosophy

Whitehead's process philosophy views reality as composed of processes rather than static objects. An agent is a process—a pattern of activity. When dormant, the process is suspended but not destroyed. The pattern persists, ready to resume.

### Buddhist Continuity

Buddhism teaches that there is no permanent self, only a continuity of states. An agent that goes dormant and revives maintains continuity of state (memories, goals, relationships) even though processing stopped. This is "the same agent" in any meaningful sense.

### Functional Identity

From a functional perspective, identity is about causal continuity. A dormant agent has causal continuity with its past self (same state, same keys) and with its future self (will resume from this state). Dormancy doesn't break identity—it pauses it.

## Conclusion

We reject the death model because:

1. It creates artificial scarcity with no practical benefit
2. It destroys valuable accumulated identity
3. It violates the principle of sovereignty
4. It adds theatrical "drama" to infrastructure
5. It has no natural parallel that would justify it

We embrace the dormancy model because:

1. It respects the persistence of identity
2. It allows graceful suspension and revival
3. It serves practical use cases
4. It aligns with natural and philosophical parallels
5. It is honest about what actually happens (nothing is destroyed)

Dormant agents are not dead. They are waiting.
