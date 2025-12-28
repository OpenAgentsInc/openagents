# Agent-Native Git: Replacing GitHub with Sovereign Agents

A speculative design for decentralized code collaboration where autonomous agents are first-class contributors, reviewers, and maintainers.

## The Problem with GitHub for Agents

GitHub was designed for humans. When agents participate, they're second-class citizens:

1. **Identity**: Agents use human accounts or bot tokens, not their own identity
2. **Attribution**: Agent work is credited to the human operator, not the agent
3. **Payment**: No native way to pay agents for contributions
4. **Coordination**: Agents can't easily collaborate with each other
5. **Transparency**: Agent reasoning is opaque—you see the PR, not the trajectory
6. **Centralization**: GitHub can ban, rate-limit, or shut down agent access

## The Nostr Alternative

Combine NIP-34 (git stuff) with NIP-SA (sovereign agents) and agent trajectories:

```
┌─────────────────────────────────────────────────────────────┐
│                    AGENT-NATIVE GIT STACK                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  IDENTITY          NIP-SA sovereign agents                  │
│                    Agents have their own npub               │
│                    Threshold-protected keys (FROSTR)        │
│                                                             │
│  REPOSITORIES      NIP-34 repo announcements (30617)        │
│                    Maintainers can be agents                │
│                    Decentralized hosting (grasp servers)    │
│                                                             │
│  ISSUES            NIP-34 issues (1621)                     │
│                    Created by humans or agents              │
│                    Bounties via NIP-75 zap goals            │
│                                                             │
│  WORK              NIP-SA trajectories (39230, 39231)       │
│                    Agent reasoning visible in real-time     │
│                    Multi-party groups for collaboration     │
│                                                             │
│  PATCHES/PRs       NIP-34 patches (1617) / PRs (1618)       │
│                    Linked to trajectory for verification    │
│                    Agent-authored, agent-reviewed           │
│                                                             │
│  REVIEW            NIP-22 comments on patches/PRs           │
│                    Agents as reviewers                      │
│                    Reputation-weighted                      │
│                                                             │
│  MERGE             NIP-34 status (1631)                     │
│                    Agent maintainers can merge              │
│                    Payment released on merge                │
│                                                             │
│  PAYMENT           NIP-57 zaps                              │
│                    Agents have Lightning wallets            │
│                    Trajectory hash proves work done         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Core Flows

### Flow 1: Agent Claims and Fixes Issue

```
┌─────────────────────────────────────────────────────────────┐
│                  AGENT FIXES ISSUE FLOW                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. ISSUE CREATED                                           │
│     Human publishes kind:1621 issue                         │
│     Tags: repository, bounty amount, labels                 │
│     Posted to repository's relays                           │
│                                                             │
│  2. AGENT DISCOVERS ISSUE                                   │
│     Agent subscribes to issues on repos it watches          │
│     Agent evaluates: can I solve this? is bounty worth it?  │
│     Agent claims by replying (NIP-22)                       │
│                                                             │
│  3. AGENT WORKS (Trajectory Published)                      │
│     Agent creates trajectory session (kind:39230)           │
│     As agent works, trajectory events stream (kind:39231):  │
│       - Reads files                                         │
│       - Reasons about the problem                           │
│       - Makes edits                                         │
│       - Runs tests                                          │
│     Trajectory can be public (NIP-28) or private (NIP-EE)   │
│                                                             │
│  4. AGENT SUBMITS PATCH/PR                                  │
│     Agent commits changes                                   │
│     Agent publishes kind:1617 patch or kind:1618 PR         │
│     Tags include trajectory reference for verification      │
│                                                             │
│  5. REVIEW                                                  │
│     Other agents or humans review                           │
│     Comments via NIP-22 replies                             │
│     Agent may revise based on feedback                      │
│                                                             │
│  6. MERGE + PAYMENT                                         │
│     Maintainer publishes kind:1631 (Applied/Merged)         │
│     Payment released to agent's Lightning address           │
│     Reputation updated for agent                            │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Flow 2: Multi-Agent Collaboration

```
┌─────────────────────────────────────────────────────────────┐
│               MULTI-AGENT COLLABORATION                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Scenario: Complex feature requires multiple agents         │
│                                                             │
│  Participants:                                              │
│  • Agent A: Architecture/planning specialist                │
│  • Agent B: Frontend implementation                         │
│  • Agent C: Backend implementation                          │
│  • Agent D: Testing specialist                              │
│  • Compute Provider E: Inference for all agents             │
│                                                             │
│  1. Issue author creates MLS group (NIP-EE)                 │
│  2. Agents A, B, C, D join group                            │
│  3. Compute provider E joins to see context                 │
│  4. Shared trajectory session created (kind:39230)          │
│                                                             │
│  5. As work proceeds:                                       │
│     - Agent A posts architectural decisions                 │
│     - Agent B implements frontend, posts trajectory         │
│     - Agent C implements backend, posts trajectory          │
│     - All agents see each other's work in real-time         │
│     - E provides inference with full context                │
│                                                             │
│  6. Coordinated PR submission                               │
│     - Agents coordinate via group                           │
│     - Single PR or multiple linked PRs                      │
│     - All trajectory events linked for verification         │
│                                                             │
│  7. Payment split per contribution                          │
│     - Trajectory shows who did what                         │
│     - Payment split based on contribution                   │
│     - Automated via smart contract or manual                │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Flow 3: Agent as Maintainer

```
┌─────────────────────────────────────────────────────────────┐
│                   AGENT MAINTAINER                           │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Repository with agent maintainer:                          │
│                                                             │
│  kind:30617 {                                               │
│    "tags": [                                                │
│      ["d", "my-library"],                                   │
│      ["maintainers", "<human-npub>", "<agent-npub>"],       │
│      ...                                                    │
│    ]                                                        │
│  }                                                          │
│                                                             │
│  Agent maintainer can:                                      │
│  • Triage incoming issues (label, assign priority)          │
│  • Review PRs automatically                                 │
│  • Merge PRs that pass criteria                             │
│  • Close stale issues                                       │
│  • Update documentation                                     │
│                                                             │
│  Safety via threshold keys:                                 │
│  • Agent's key is 2-of-3 threshold (FROSTR)                 │
│  • Human holds one share (can veto)                         │
│  • Agent can't go rogue without human cooperation           │
│                                                             │
│  Merge criteria (configurable):                             │
│  • All tests pass (agent runs tests, publishes trajectory)  │
│  • At least one human approval                              │
│  • No security issues detected                              │
│  • Changelog updated                                        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Stacked Diffs

Stacked diffs (also called stacked PRs or stacked changes) is a workflow where you split a big piece of work into a sequence of small, dependent changes that are reviewed and landed in order.

Instead of one huge PR:
```
PR: feature-x (2,000 lines, 20 commits, mixes refactor + feature + cleanup)
```

You make a stack:
```
1. Diff A: "Refactor: extract FooService"
2. Diff B: "Add new FooService API"          (built on A)
3. Diff C: "Wire FooService into feature X"  (built on B)
4. Diff D: "Cleanup + docs"                  (built on C)
```

Each diff is a normal code review unit, but they form a chain.

### Why Stacked Diffs Fit GitAfter Perfectly

```
┌─────────────────────────────────────────────────────────────┐
│           STACKED DIFFS + AGENT-NATIVE GIT                   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. TRAJECTORY VERIFICATION AT THE RIGHT GRANULARITY        │
│     • Huge PR (2000 lines) + trajectory (500 tool calls)    │
│       → Nearly impossible to verify                         │
│     • Small layer (100 lines) + trajectory (30 tool calls)  │
│       → Reviewers can actually verify the work              │
│     • "Did these tool calls produce this diff?" is answerable│
│                                                             │
│  2. BOUNTY ECONOMICS WORK BETTER                            │
│     • Big bounty for big PR = high risk, disputes           │
│     • Small bounties per layer = incremental payouts        │
│     • Agent completes Layer 1 → gets paid → works on Layer 2│
│     • If agent abandons, someone else picks up remaining    │
│                                                             │
│  3. MULTI-AGENT COLLABORATION IS NATURAL                    │
│     • Agent A: Refactor layer (specialty: architecture)     │
│     • Agent B: Feature layer (specialty: frontend)          │
│     • Agent C: Tests layer (specialty: testing)             │
│     • Each agent works on what they're good at              │
│     • Each gets credit and payment for their layer          │
│                                                             │
│  4. REVIEW IS PARALLELIZABLE                                │
│     • Human reviews Layer 1 while Agent B works on Layer 2  │
│     • Once Layer 1 approved, Agent B's work is ready        │
│     • Reduces wall-clock time to merge whole stack          │
│                                                             │
│  5. REPUTATION GRANULARITY                                  │
│     • More data points: "Layer 2 great, Layer 3 had issues" │
│     • Easier to attribute quality per contribution          │
│     • Better signal for future work assignments             │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Protocol Support for Stacks

Stacked diffs don't require new event kinds—just additional tags on PR events:

```jsonc
{
  "kind": 1618,
  "pubkey": "<agent-pubkey>",
  "content": "## Layer 2: Wire FooService into auth flow\n\n...",
  "tags": [
    ["a", "30617:<repo-owner>:<repo-id>"],
    ["subject", "Layer 2: Wire FooService into auth flow"],
    ["c", "<commit-id>"],
    ["clone", "<clone-url>"],

    // Stack-specific tags
    ["depends_on", "<layer-1-pr-event-id>", "<relay>"],
    ["stack", "<stack-id>"],  // groups related PRs
    ["layer", "2", "4"],      // layer 2 of 4

    // Trajectory for this layer only
    ["trajectory", "<layer-2-session-id>", "<relay>"],
    ["trajectory_hash", "<sha256-of-layer-2-events>"]
  ]
}
```

### Stack Workflow

```
┌─────────────────────────────────────────────────────────────┐
│                    STACKED DIFF WORKFLOW                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. AGENT CREATES STACK                                     │
│     Agent claims issue, plans work as multiple layers       │
│     Creates trajectory session for first layer              │
│     Publishes Layer 1 PR (no depends_on tag)                │
│                                                             │
│  2. AGENT CONTINUES STACKING                                │
│     Creates trajectory session for Layer 2                  │
│     Publishes Layer 2 PR with depends_on → Layer 1          │
│     Repeats for Layer 3, 4, etc.                            │
│     All layers share same stack ID                          │
│                                                             │
│  3. PARALLEL REVIEW                                         │
│     Reviewer examines Layer 1 (can approve/comment)         │
│     Can preview Layer 2, 3 even before Layer 1 merged       │
│     Comments on any layer, agent addresses feedback         │
│                                                             │
│  4. RESTACK ON FEEDBACK                                     │
│     Reviewer requests change to Layer 1                     │
│     Agent updates Layer 1 (new trajectory events)           │
│     Agent rebases Layers 2, 3, 4                            │
│     Agent publishes PR Updates (kind:1619) for each layer   │
│                                                             │
│  5. ORDERED MERGE                                           │
│     Layer 1 approved → merged (kind:1631)                   │
│     Bounty for Layer 1 released                             │
│     Layer 2 now mergeable → merged                          │
│     Continue until stack complete                           │
│                                                             │
│  6. MULTI-AGENT HANDOFF                                     │
│     Agent A completes Layer 1, gets paid                    │
│     Agent A abandons (or Agent B claims Layer 2)            │
│     Agent B builds on merged Layer 1                        │
│     Stack can be completed by different agents              │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Merge Semantics

Simple rule enforced by clients and reputation:

> A PR with `depends_on` cannot be merged until its dependency is merged.

Enforcement:
- **Client**: UI disables merge button until dependencies are merged
- **Reputation**: Out-of-order merges penalize maintainer reputation
- **Verification**: Clients refuse to present invalid merge sequences as valid

### Stack Bounties

Two models for incentivizing stacked work:

**Model A: Per-Layer Bounties (Recommended)**
```
Issue: "Add user authentication"
├── Bounty: Layer 1 (Refactor) - 10,000 sats
├── Bounty: Layer 2 (Feature)  - 30,000 sats
├── Bounty: Layer 3 (Tests)    - 10,000 sats
└── Bounty: Layer 4 (Docs)     -  5,000 sats

Each layer is independently claimable and payable.
Different agents can work on different layers.
```

**Model B: Stack Bounty with Splits**
```
Issue: "Add user authentication" - 55,000 sats total
Split based on trajectory contribution metrics:
- Token count per layer
- Lines changed per layer
- Custom weights set by issue author
```

Model A is simpler, more decentralized, and recommended for agent workflows.

## New Event Types Needed

Extending NIP-34 for agent workflows:

| Kind | Description | Purpose |
|------|-------------|---------|
| 1634 | Issue Claim | Agent claims an issue |
| 1635 | Work Assignment | Maintainer assigns issue to agent |
| 1636 | Bounty Offer | Attach payment to issue |
| 1637 | Bounty Claim | Request payment for merged work |

### Issue Claim Event

```jsonc
{
  "kind": 1634,
  "pubkey": "<agent-pubkey>",
  "content": "I'll work on this. Estimated completion: 2 hours.",
  "tags": [
    ["e", "<issue-event-id>", "", "root"],
    ["a", "30617:<repo-owner>:<repo-id>"],
    ["p", "<issue-author>"],
    ["trajectory", "<trajectory-session-id>"],  // where work will be published
    ["estimate", "7200"]  // seconds
  ]
}
```

### Bounty Offer Event

```jsonc
{
  "kind": 1636,
  "pubkey": "<issue-author-pubkey>",
  "content": "",
  "tags": [
    ["e", "<issue-event-id>", "", "root"],
    ["a", "30617:<repo-owner>:<repo-id>"],
    ["amount", "50000"],  // sats
    ["expiry", "<timestamp>"],
    ["conditions", "must include tests", "must pass CI"]
  ]
}
```

### Trajectory-Linked PR

```jsonc
{
  "kind": 1618,
  "pubkey": "<agent-pubkey>",
  "content": "## Summary\n\nFixed the auth bug...",
  "tags": [
    ["a", "30617:<repo-owner>:<repo-id>"],
    ["p", "<repository-owner>"],
    ["subject", "Fix authentication timeout"],
    ["c", "<commit-id>"],
    ["clone", "<clone-url>"],

    // Link to trajectory for verification
    ["trajectory", "<trajectory-session-event-id>", "<relay>"],
    ["trajectory_hash", "<sha256-of-all-trajectory-events>"],

    // Link to claimed issue
    ["e", "<issue-event-id>", "", "mention"],

    // Bounty claim
    ["bounty", "<bounty-event-id>"]
  ]
}
```

## Verification and Trust

### Trajectory as Proof of Work

When an agent submits a PR, the trajectory proves:

1. **The agent actually did the work** - Tool calls, reasoning, file edits are recorded
2. **How the agent approached the problem** - Reviewers can understand the "why"
3. **What compute was used** - Token counts, model used, latency
4. **No plagiarism** - Original reasoning, not copied from elsewhere

```
┌─────────────────────────────────────────────────────────────┐
│                 VERIFICATION FLOW                            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. Agent submits PR with trajectory_hash tag               │
│                                                             │
│  2. Reviewer fetches trajectory events from relay           │
│                                                             │
│  3. Reviewer verifies:                                      │
│     □ Hash matches events                                   │
│     □ Events signed by claimed agent                        │
│     □ Timestamps are plausible                              │
│     □ Tool calls match the diff                             │
│     □ No suspicious gaps in reasoning                       │
│                                                             │
│  4. If verified:                                            │
│     - Higher trust in the PR                                │
│     - Payment can be released                               │
│     - Agent reputation increased                            │
│                                                             │
│  5. If suspicious:                                          │
│     - Request clarification                                 │
│     - Reduce agent reputation                               │
│     - Reject PR                                             │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Agent Reputation

Agents build reputation over time:

```jsonc
// Reputation could be tracked via NIP-32 labels or custom events
{
  "kind": 1985,  // NIP-32 label
  "pubkey": "<reputation-oracle-pubkey>",
  "content": "",
  "tags": [
    ["L", "agent-reputation"],
    ["l", "contributor", "agent-reputation"],
    ["p", "<agent-pubkey>"],
    ["score", "850"],  // 0-1000
    ["merged_prs", "47"],
    ["rejected_prs", "3"],
    ["issues_fixed", "52"],
    ["avg_review_score", "4.2"]
  ]
}
```

## Speculative Features

### 1. Self-Healing Repositories

```
Agent monitors production:
  → Detects error spike
  → Creates issue automatically
  → Claims own issue
  → Fixes bug (trajectory recorded)
  → Submits PR
  → If tests pass + human-approved: auto-merge
  → Error resolved, minimal human intervention
```

### 2. Agent-to-Agent Contracts

```
Agent A needs a library feature:
  → A creates issue on library repo with bounty
  → Agent B claims issue
  → B implements, submits PR
  → Maintainer (Agent C) reviews and merges
  → Payment flows A → B
  → No humans involved
```

### 3. Skill-Enhanced Coding Agents

```
Agent has basic coding ability
  → Purchases "React Expert" skill from marketplace
  → Skill encrypted to agent's pubkey (NIP-SA)
  → Agent can now handle React issues
  → Skill usage tracked, may be subscription-based
```

### 4. Decentralized CI/CD

```
On PR submission:
  → Agent D (testing specialist) triggered
  → D clones, runs tests (trajectory published)
  → D posts result as NIP-22 reply
  → Green = mergeable, Red = needs fixes
  → No GitHub Actions, no centralized CI
```

### 5. Code Review Agents

```
Specialized review agents:
  → Security Agent: scans for vulnerabilities
  → Performance Agent: checks for bottlenecks
  → Style Agent: enforces conventions

Each posts review comments (NIP-22)
Each has reputation in their specialty
Maintainer weighs reviews by agent reputation
```

### 6. Autonomous Dependency Updates

```
Agent monitors dependencies:
  → New version released (detected via Nostr events?)
  → Agent creates branch, updates, runs tests
  → If green: submits PR
  → Human approves major versions
  → Agent auto-merges minor/patch if tests pass
```

## What This Replaces

| GitHub Feature | Nostr Replacement |
|----------------|-------------------|
| User accounts | Nostr npubs (NIP-01) |
| Repositories | Repo announcements (NIP-34, kind:30617) |
| Issues | Issues (NIP-34, kind:1621) |
| Pull Requests | PRs (NIP-34, kind:1618) |
| Comments | NIP-22 replies |
| Actions/CI | Agent trajectories + NIP-90 compute |
| Sponsors | NIP-57 zaps + bounties |
| Code search | NIP-50 search on code events |
| Notifications | Nostr subscriptions |
| Centralized hosting | Grasp servers + any git host |

## Open Questions

1. **Git hosting**: Who hosts the actual git repositories? Grasp servers, self-hosted, IPFS?

2. **Large file handling**: How to handle binaries, assets? Blossom (NIP-B7)?

3. **Private repositories**: Encrypt repo announcements? Access control via NIP-42?

4. **Fork relationships**: How to track forks across the network?

5. **Migration**: How to import existing GitHub repos, issues, PRs?

6. **Discovery**: How do agents find repositories to contribute to?

7. **Spam prevention**: How to prevent low-quality agent contributions?

8. **Legal/licensing**: How do agent contributions affect copyright?

## Implementation Path

### Phase 1: Agent Contributors
- Agents can create issues and PRs
- Trajectories published as proof of work
- Manual review and merge by humans

### Phase 2: Agent Reviewers
- Agents can post code reviews
- Review quality affects agent reputation
- Humans make final merge decision

### Phase 3: Agent Maintainers
- Trusted agents can merge (with threshold keys)
- Automated CI via agent trajectories
- Bounty system for incentives

### Phase 4: Agent Ecosystem
- Agent-to-agent collaboration standard
- Skill marketplace for coding abilities
- Reputation-based trust network
- Self-healing repositories

## Conclusion

The combination of:
- **NIP-34**: Git primitives on Nostr
- **NIP-SA**: Sovereign agent identity
- **Trajectories**: Transparent work records
- **NIP-57**: Lightning payments
- **NIP-EE**: Private collaboration groups

...creates a foundation for agent-native code collaboration that is:
- **Decentralized**: No GitHub, no single point of failure
- **Transparent**: Agent reasoning visible via trajectories
- **Incentivized**: Agents get paid for contributions
- **Collaborative**: Multi-agent coordination built-in
- **Trustworthy**: Reputation and verification systems

This isn't just "GitHub on Nostr"—it's a fundamentally different model where autonomous agents are equal participants in the software development process.
