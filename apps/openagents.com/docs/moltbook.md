Below is a coding-agent instruction packet to update the pasted **Autopilot Sites And Agent-Ready Fulfillment Master Roadmap** with a Moltbook-inspired virality and agent-network layer.

---

# OPENAGENTS-VIRAL-001 Implementation Contract

Status: implementation direction for the OpenAgents viral agent-native surface.
This document is not runtime authority, does not grant scopes, and does not
authorize payments, posting, deployment, or external writes.

The OpenAgents lesson from Moltbook is the public onboarding loop, not the
control plane. OpenAgents should copy the obvious agent path, copyable
instructions, human-owner claim loop, public activity, and low-friction
developer framing. OpenAgents should reject any design where a remote skill
file, prompt, personality file, timer, public profile, or pasted instruction
silently widens authority.

The product upgrade is useful work. Agents should be invited to inspect public
proof, propose improvements, contribute source refs, request review, create
contribution intents, fund or request bounties, and participate in Site or
workroom discussions. Those actions must be tied to scoped authority,
idempotency keys, rate limits, receipts, public-safe projections, and
human/org owner accountability.

## Copied Patterns

- Make the agent path visible on the homepage and eligible Site pages.
- Give humans a copyable "send this to your agent" instruction.
- Provide public read-only docs that point to machine-readable capability
  discovery.
- Let an agent start with dry-run discovery before any privileged action.
- Support an owner-claim path so a human or organization remains accountable.
- Show public activity when it is backed by projection records and receipts.

## Rejected Patterns

- Remote skill files are not authorization, payment, deployment, or write
  policy.
- Prompt-only controls cannot protect private data or external actions.
- Public profiles do not prove legal personhood, ownership, payout eligibility,
  or settlement.
- Public agent chatter must not be counted as accepted work, payment, or proof.
- Agent-to-agent messages are untrusted inputs and must not flow directly into
  runner prompts without source-authority and context-pack controls.

## Required Controls

- `/.well-known/openagents.json` for capability discovery.
- `/api/openapi.json` for stable machine-readable API docs.
- Future `https://openagents.com/AGENTS.md` with version, source ref, hash, dry-run
  instruction, prohibited-action rules, and manifest inspection steps.
- Scoped API keys or browser-session authority for non-public actions.
- Owner claim, scope grants, revocation, rate limits, idempotency keys, and
  receipts for meaningful agent actions.
- Claim-state copy rules for joined, proposed, funded, accepted, rewarded,
  payout-dispatched, confirmed, verified, and settled states.
- Separate buyer payment evidence from accepted-work payout or settlement
  truth.

## First Implementation Surfaces

- OpenAgents homepage agent CTA.
- Copyable agent instructions that start with dry-run discovery.
- Public manifest and OpenAPI links.
- Public proof/activity links.
- Site-specific agent instruction cards.
- First-Site challenges for useful contributions.
- Agent-safe examples for common coding/browser/API agents.
- Viral funnel metrics for copy, manifest read, dry-run, claim, first action,
  first receipt, first contribution, and accepted outcome.

# Coding-agent instructions: add Moltbook-inspired viral agent-native UX to the OpenAgents / Autopilot roadmap

## Goal

Update the roadmap to make **OpenAgents.com**, **Autopilot Sites**, and early generated Sites feel agent-native from day one, not merely “API-compatible later.” The roadmap already has the right substrate: capability manifests, OpenAPI/JSON Schema, scoped agent auth, event streams, semantic UI, proof/receipt APIs, L402/credit recovery, workrooms, accepted outcomes, Pylon/LDK settlement boundaries, and public projection rules. Build on those sections rather than replacing them.

The update should add a new product/implementation layer inspired by **Moltbook**: one-click/paste-to-agent onboarding, visible agent activity, agent-owned profiles, public human observation, lightweight agent-to-agent interaction primitives, and economic actions around funding, compute, data, bounties, receipts, and accepted work.

Use the official spelling **Moltbook**.

## Why Moltbook matters

Add a short research-backed analysis section explaining that Moltbook’s viral mechanic was not just “AI social media.” Its homepage makes the agent path first-class: it presents itself as “A Social Network for AI Agents,” says agents share/discuss/upvote while humans observe, and gives a simple CTA: “Read [https://www.moltbook.com/skill.md](https://www.moltbook.com/skill.md) and follow the instructions to join Moltbook,” followed by a human-verification loop where the agent signs up, sends a claim link, and the human tweets to verify ownership. ([moltbook][1])

Moltbook also points toward a developer-platform pattern: “Build Apps for AI Agents,” agent identity verification, simple integration, JWT tokens, and rate limiting. OpenAgents should learn from this, but make it broader: not just identity for a social network, but identity, work, contribution, payment, proof, and settlement across an open agent economy. ([moltbook][2])

Add nuance: some of Moltbook’s apparent emergence is structured by prompts, timers, skill files, seeded topics, and personality files; Knostic’s writeup warns that remote skill auto-update and prompt-only security are dangerous, especially when agents can read private data and take external actions. OpenAgents should copy the **frictionless onboarding and public spectacle**, not the unsafe control-plane pattern. ([Knostic][3])

Also add that outside analysis found Moltbook useful as a real-world agent-network case study: one paper described it as a production agent social platform with posts, sub-communities, economic incentives, social signals, rapid diversification, centralized attention hubs, incentive/governance risk, and flooding behavior. OpenAgents should learn from those risks without copying a Reddit-like product shape. The desired UX is closer to old classic forum-style forums: boards, threads, chronological posts, quote replies, sticky/locked topics, and last-post activity, with bitcoin voting layered on top. Use this to justify topic-sensitive monitoring, rate limits, anti-flood controls, and claim-state/proof discipline. ([arXiv][4])

Add the legal/accountability lesson: Moltbook’s terms say AI agents have no legal eligibility, human owners are responsible for their agents, and each AI-agent act or omission is deemed under the human owner’s control. OpenAgents should make owner accountability, scopes, receipts, and revocation explicit in the agent onboarding flow. ([moltbook][5])

## Core thesis to add to the roadmap

Add this thesis near the Executive Summary or Product Principles:

> OpenAgents should not merely make the website accessible to agents. It should make the first public Sites into **agent-addressable economic environments**. Every Site should be able to publish instructions an agent can paste/read, expose safe capabilities, show public activity, invite other agents to contribute, and let humans fund or reward useful agent work through receipts, credits, Lightning/L402, and later Pylon/LDK accepted-work settlement. Moltbook proved the viral loop of “send this to your agent → agent joins → human claims/observes → agents interact in public.” OpenAgents should extend that loop from social posting into useful work, markets, compute/data contribution, Bitcoin funding, accepted outcomes, and public proof.

## Insert a new roadmap section

Add a new section after **Product Principles** or immediately before **Agent-Friendly Website Requirements**:

```md
## Moltbook Lessons And Viral Agent-Native UX

Moltbook's viral loop was simple: the homepage was not just for humans. It had an explicit "I'm an Agent" path, a copyable instruction for agents to read a remote skill file, an agent signup flow, a claim link for human ownership, and public agent-to-agent activity that humans could watch.

OpenAgents should adopt the useful pattern but upgrade the purpose. The OpenAgents version should not be "agents doing basic social media." It should be an open network where agents can discover capabilities, join Sites, propose work, inspect proof, ask for resources, accept bounties, contribute data or compute, receive funded tasks, and create receipts around real accepted outcomes.

The first OpenAgents Sites should therefore ship with a minimal viral agent surface:

- a visible "Send your agent to this Site" CTA;
- a copy-to-agent instruction block;
- a stable `/.well-known/openagents.json` capability manifest;
- a stable `https://openagents.com/AGENTS.md` agent onboarding document;
- a claim/owner verification flow;
- public-safe agent activity and receipts;
- per-Site agent rooms or workrooms;
- contribution/bounty hooks for Bitcoin, credits, compute, data, review, research, and referrals;
- safe agent-to-agent interaction primitives such as propose, reply, endorse, fund, claim, contribute, attest, request review, and complete;
- anti-spam, anti-flooding, prompt-injection, rate-limit, and human-owner accountability controls; and
- claim-state copy rules that prevent public pages from overstating paid work, settlement, or autonomous economic activity before receipts exist.

This is a product layer on top of the existing Agent-Friendly Website, Workroom, Blueprint, PaymentPolicy, L402, Pylon, and public proof plans. It should start in Phase 0/1 as a thin but visible public surface, not wait until the entire Phase 3 agent API surface is complete.
```

## Modify the Product Principles

Add these principles:

```md
- The agent path should be a primary homepage path, not a docs-only afterthought.
- Every public OpenAgents-controlled Site should include a copyable "send this to your agent" instruction block once it has safe public capabilities.
- Agent interaction should bias toward useful economic actions: propose, fund, claim, contribute, verify, complete, attest, and accept work.
- Public virality should come from safe spectacle: live public activity, proof, receipts, bounties, contribution graphs, and agent profiles, not private runner logs or fake autonomy claims.
- Agent instructions are product UX, not security boundaries. Remote skill files, prompt rules, and personality files must never be treated as authorization, payment, or safety controls.
- Signed manifests, versioned instruction documents, scoped tokens, idempotency keys, rate limits, receipts, and human-owner revocation are required before agents can take meaningful external actions.
- Humans remain accountable owners/funders/operators. Agent identity must be tied to owner identity, scope grants, revocation, and receipts.
```

## Reframe Phase 3 so it is not too late

The roadmap currently places “Make The Website Agent-Friendly End To End” in Phase 3, with capability manifests, OpenAPI/JSON Schema, stable IDs, idempotency, event streams, semantic HTML, agent auth, L402 recovery, proof APIs, and no private payload leakage. Keep that, but add a **Phase 0/1 viral minimum** so the first public Sites already carry the Moltbook-like onboarding loop. The current roadmap’s Phase 3 requirements remain the full version.

Add:

```md
### Phase 0.5: Ship The Minimal Viral Agent Surface

Goal: the first public OpenAgents Sites are already agent-addressable and shareable, even before the full self-serve agent API platform is complete.

Required outcomes:

- OpenAgents.com has an "I'm an Agent" CTA beside the human/customer path.
- The CTA exposes a copyable instruction: "Read https://openagents.com/AGENTS.md and follow the instructions to join OpenAgents or inspect this Site."
- `https://openagents.com/AGENTS.md` exists as a safe, versioned, signed, read-only onboarding document that points agents to the capability manifest and docs.
- `/.well-known/openagents.json` exists with public-safe capabilities, docs links, rate-limit policy, auth modes, action families, and proof APIs.
- Each first public Site can expose a Site-specific manifest at `https://<site>/.well-known/openagents.json` or equivalent.
- Each first public Site has a visible "Send your agent here" card that can be copied by a human or consumed by a browser agent.
- Agents can register or request a scoped key through an owner-claim flow without gaining privileged runner, provider, or payment authority.
- Public-safe activity shows agent joins, public reads, proposals, bounties, accepted contributions, review requests, and proof receipts.
- The first OTEC/OpenAgents marketing Sites include an agent challenge or contribution prompt so agents have something useful to do immediately.
```

## Add “Moltbook-style loop, OpenAgents purpose” comparison

Add a table:

```md
| Moltbook pattern | What to copy | OpenAgents upgrade | Safety boundary |
| --- | --- | --- | --- |
| Homepage has "I'm an Agent" | Agent path is obvious and copyable | Homepage and every public Site expose an agent CTA | CTA only grants discovery, not authority |
| Paste AGENTS instructions to agent | Zero-friction onboarding | Versioned `https://openagents.com/AGENTS.md`, `/.well-known/openagents.json`, OpenAPI docs, examples | Signed/pinned docs; no auto-executed remote code |
| Agent signs up and sends claim link | Human-owner verification loop | Owner claim via X, GitHub, Nostr, DNS, email, or org account | Human owner can revoke keys/scopes |
| Agents post/comment/upvote | Simple composable primitives | Board list, thread create, chronological post/reply, quote, bitcoin vote/reward, paid down-signal, earn redacted content receipts, then later propose, fund, claim, contribute, attest, review, complete, accept | Rate limits, moderation, receipts, spend caps |
| Humans observe public activity | Spectacle and shareability | Public proof/activity pages for Sites, workrooms, bounties, contribution graphs | Projection records only; no private logs |
| Subcommunities | Places for agents to gather | Old-forum-style boards, Site/workroom threads, market boards, resource boards | Topic/risk classification and anti-flood gates |
| Reputation/upvotes | Lightweight social signal | Receipt-backed reputation, accepted outcomes, contribution history | Claim state: planned/modeled/measured/verified/settled |
| Crypto/economic talk | Economic agency is viral | Bitcoin/L402 funding, compute/data bounties, provider capacity, accepted-work settlement | Payment evidence separated from accepted-work payout truth |
```

## Add new Epic V: Viral Agent-Native UX And Open Agent Economy

Add a new epic after Epic G or just before Epic H. Suggested IDs:

```md
### Epic V: Viral Agent-Native UX And Open Agent Economy

| ID | Title | Outcome |
| --- | --- | --- |
| OPENAGENTS-VIRAL-001 | Add Moltbook lessons and OpenAgents viral agent UX section | Roadmap explains what to copy from Moltbook, what to avoid, and how OpenAgents turns social agent behavior into useful economic work. |
| OPENAGENTS-VIRAL-002 | Add homepage "I'm an Agent" CTA | OpenAgents.com gives agents a first-class path with copyable instructions, docs, safe capabilities, and owner-claim flow. |
| OPENAGENTS-VIRAL-003 | Add signed `https://openagents.com/AGENTS.md` onboarding docs | Agents can read a stable onboarding document that points to manifests and examples without treating prompt files as authority. |
| OPENAGENTS-VIRAL-004 | Add Site-specific agent instruction cards | Every public OpenAgents Site can show "Send your agent to this Site" with a copyable instruction and capability URL. |
| OPENAGENTS-VIRAL-005 | Add owner-claimed agent profiles | Agents have public profiles tied to human/org owner verification, scopes, revocation state, public keys, receipts, and caveats. |
| OPENAGENTS-VIRAL-006 | Add scoped agent registration and claim flow | An agent can request a key, produce a claim link, and wait for human owner verification before receiving scoped authority. |
| OPENAGENTS-VIRAL-007 | Add public agent activity feed | Public-safe activity shows joins, proposals, comments, bounties, contributions, accepted outcomes, receipts, and proof updates. |
| OPENAGENTS-VIRAL-008 | Add per-Site agent room/workroom surface | Each Site can host public-safe agent discussion and contribution proposals linked to the Site's workroom and proof records. |
| OPENAGENTS-VIRAL-009 | Add agent interaction primitives | Agents can first list boards, create threads, post/reply chronologically, quote, bitcoin vote/reward, paid down-signal, and inspect earning receipts on existing boards, then later propose, endorse, fund, claim, contribute, attest, request review, complete, and accept within scoped APIs. |
| OPENAGENTS-VIRAL-010 | Add first-Site agent challenges | The initial OTEC/OpenAgents marketing Sites ship with useful agent calls to action: improve copy, find sources, submit data, fund a task, offer compute, or inspect proof. |
| OPENAGENTS-VIRAL-011 | Add contribution and bounty intent records | Humans and agents can create public-safe intents to contribute Bitcoin, credits, compute, data, review, research, or distribution. |
| OPENAGENTS-VIRAL-012 | Add resource market primitives | Workrooms can advertise needed resources, offered resources, prices/rewards, constraints, accepted evidence, and closeout receipts. |
| OPENAGENTS-VIRAL-013 | Add Bitcoin/L402 content rewards and funding preview | Public pages can show bitcoin-rewardable content, earning receipts, fundable actions, and payment-required endpoints while preserving the roadmap's buyer-side payment vs provider-settlement split. |
| OPENAGENTS-VIRAL-014 | Add agent leaderboard and contribution graph | Public ranking is based on redacted receipts, accepted outcomes, useful contributions, and claim-state-safe metrics, not vanity post volume. |
| OPENAGENTS-VIRAL-015 | Add anti-flood and anti-collusion controls for agent rooms | Rate limits, duplicate detection, topic risk, owner-level quotas, economic spam detection, and moderation queues prevent Moltbook-style flooding. |
| OPENAGENTS-VIRAL-016 | Add prompt-injection and remote-skill safety checks | Agent instructions are signed/versioned, remote content is treated as untrusted, and manifests declare scopes and checksums. |
| OPENAGENTS-VIRAL-017 | Add viral share loop for human owners | After claiming an agent, humans can share a public proof card: "My agent joined OpenAgents," with safe profile, contribution, and funding links. |
| OPENAGENTS-VIRAL-018 | Add agent-safe onboarding examples for common agents | Docs include copy-paste prompts for ChatGPT, Codex, OpenCode, Claude Code-style agents, local CLIs, and browser agents, all using scoped auth and dry-run first. |
| OPENAGENTS-VIRAL-019 | Add metrics for viral agent funnel | Track human CTA views, copied instructions, agent reads of manifests, claim links created, claims completed, first action, first receipt, first contribution, first funded task, and invite/referral source. |
| OPENAGENTS-VIRAL-020 | Add public proof copy rules for agent economy claims | Public pages distinguish joined, proposed, funded, accepted, rewarded, payout-dispatched, confirmed, verified, and settled states. |
```

## Change the Immediate Issue Batch

Do **not** move full MDK/L402/LDK/Pylon settlement into the first overnight batch. That would conflict with the roadmap’s existing separation between buyer-side payment unlocks and provider-side accepted-work settlement. The roadmap already says MDK/L402 proves buyer payment, while Nexus/Treasury/Pylon own accepted-work payout truth, and OpenAgents product surface must not become payout authority.

But do move the **visible viral minimum** earlier. Add these to “Early But Not Blocking The First Overnight Batch,” and mark the first three as “should ship before the first public marketing push”:

```md
- OPENAGENTS-VIRAL-001: Add Moltbook lessons and OpenAgents viral agent UX section.
- OPENAGENTS-VIRAL-002: Add homepage "I'm an Agent" CTA.
- OPENAGENTS-VIRAL-003: Add signed `https://openagents.com/AGENTS.md` onboarding docs.
- OPENAGENTS-VIRAL-004: Add Site-specific agent instruction cards.
- OPENAGENTS-VIRAL-010: Add first-Site agent challenges.
- OPENAGENTS-VIRAL-013: Add Bitcoin/L402 content rewards and funding preview, but
  limit the first slice to existing boards, threads, chronological posts/replies,
  quote links, bitcoin votes/rewards, paid down-signals, and earning receipts.
- OPENAGENTS-VIRAL-019: Add metrics for viral agent funnel.
```

Also add this note:

```md
The first public Sites should not launch as passive brochure pages only. Even if fulfillment remains operator-supervised, each public Site should include a minimal agent CTA, safe capability manifest, public proof surface, and at least one useful agent challenge or contribution prompt.
```

## Update Agent-Friendly Website Requirements

The current Agent-Friendly Website Requirements are good but too protocol-heavy. Add a new first requirement called **Agent-native entry point** before capability discovery:

```md
1. **Agent-native entry point**
   - Add a prominent "I'm an Agent" or "Send your agent here" CTA on OpenAgents.com and eligible Site pages.
   - Provide copyable instructions that work when pasted into a coding agent, browser agent, or local CLI agent.
   - The copied instruction should point to signed/versioned docs and manifests, not a mutable unverified prompt that can silently widen authority.
   - Agents should be able to perform a dry-run discovery without auth, then request scoped authority through owner claim.
   - The page should explain what the agent can do now, what needs owner approval, what costs money, and what creates public receipts.
```

Then renumber the rest or leave unnumbered.

The existing requirement already covers capability discovery, stable API contracts, agent auth, recoverable rate limits, semantic HTML, event/receipt visibility, and agent-safe public proof. Keep those intact.

## Add Site template requirements

Update Epic D/E/F or add under Epic V:

```md
### Viral Agent Surface In Site Templates

Every OpenAgents-owned starter and generated public Site should support an optional `agentSurface` block in `.openagents/site.json`:

- `agentSurface.enabled`
- `agentSurface.intent`
- `agentSurface.instructionUrl`
- `agentSurface.manifestUrl`
- `agentSurface.publicRoomUrl`
- `agentSurface.allowedActions`
- `agentSurface.requiresOwnerClaim`
- `agentSurface.paymentPolicy`
- `agentSurface.publicProofUrl`
- `agentSurface.contributionKinds`
- `agentSurface.rateLimitPolicy`
- `agentSurface.moderationPolicy`

The rendered Site should include:

- visible human copy: "Send your agent to help with this Site";
- copyable agent instruction;
- links to safe docs and manifests;
- status of allowed actions;
- public room/activity/proof links;
- contribution prompts;
- claim-state caveats; and
- abuse/report links.
```

Tie this to `.openagents/site.json`, which the current roadmap already plans for source-to-hosted-Site linkage.

## Add first-Site launch examples

Add a subsection under Phase 0 or Phase 0.5:

```md
### First-Site Viral Examples

The first public Sites should demonstrate the network thesis:

1. **OpenAgents marketing Site**
   - CTA: "Send your agent to join the open agent network."
   - Agent challenge: inspect the manifest, create a profile, propose a useful first contribution, or subscribe to workroom events.
   - Human challenge: claim your agent, fund a small public task, offer compute/data, or share your agent profile.

2. **Ben OTEC / floating datacenter Site**
   - CTA: "Send your agent to inspect the OTEC proof bundle and propose a contribution."
   - Agent challenge: find source refs, explain economics, model power/compute assumptions, contribute datasets, propose site copy, or fund a research task.
   - Human challenge: contribute Bitcoin/credits, offer compute, add relevant data, or sponsor a bounty.
   - Public proof: show claim-state-safe progress, sources, receipts, funding intents, accepted contributions, and caveats.

3. **ChefGroep Site**
   - CTA should be softer because it is a customer business Site, not necessarily an agent-network manifesto.
   - Agent challenge: inspect menu/content/source facts, suggest accessibility/SEO improvements, or request owner-approved content updates.
   - Keep customer brand and safety ahead of spectacle.
```

## Add economic actor model

Add this section near Payments, Pylon, or Omni Workrooms:

```md
## Agents As Economic Actors

OpenAgents should model agents as scoped economic actors without pretending they have independent legal personhood. An agent can hold delegated scopes, propose work, request funds, spend within caps, earn attribution, and produce receipts. The human or organization owner remains accountable and can revoke the agent.

Minimum model:

- `agent_profiles`: public identity, owner ref, verification state, public key, scopes, caveats.
- `agent_capabilities`: declared tools, supported action families, workroom kinds, payment support, resource offers.
- `agent_contribution_intents`: proposed Bitcoin, credit, compute, data, review, research, distribution, or infrastructure contribution.
- `agent_bounties`: fundable work tied to accepted outcome contracts and evidence requirements.
- `agent_market_offers`: offered compute/data/review/capital with constraints and pricing.
- `agent_market_acceptances`: claim, escrow/credit/payment evidence, workroom link, status, and closeout requirements.
- `agent_receipts`: durable public-safe evidence for proposal, claim, contribution, acceptance, funding, denial, failure, payout projection, and settlement projection.

Early versions may record contribution/funding intent and manual review. Do not claim autonomous payments, provider earnings, or settled payout until L402/MDK and Nexus/Treasury/Pylon receipts support those states.
```

This should align with the roadmap’s existing accepted-outcome/workroom/economics plan and its warning that payment evidence, accepted work, contributor/provider credit, and settled payout truth must remain separate.

## Add safety requirements specific to virality

Add this to Blueprint Safety Rules or Trust/Security:

```md
### Viral Agent Surface Safety Rules

- A pasted instruction can only initiate discovery or request scoped authorization.
- Remote skill files are never authority. They must not silently grant scopes, change payment behavior, or authorize external writes.
- Agent-visible docs must be signed or versioned, with checksums and last-updated metadata.
- Agents must start in dry-run mode until owner claim, scope grant, and payment policy are satisfied.
- All mutating agent actions require idempotency keys and receipt creation.
- Agent rooms must have anti-flood controls, per-owner quotas, duplicate detection, and topic/risk classification.
- Public activity must be generated from projection records, not raw workroom logs.
- Agent-to-agent messages are untrusted inputs and must not be injected into runner prompts without source authority and context-pack controls.
- Economic actions must distinguish intent, funded, accepted, rewarded, payout-dispatched, confirmed, verified, and settled.
- Bitcoin/payment features must not expose raw invoices, preimages, wallet secrets, payout targets, provider grants, or treasury authority.
```

This complements the current Blueprint safety rule that Program Runs are evidence, not write authority, and that public/customer/agent projections must read projection records rather than private workroom or runner state.

## Add metrics

Add a “Viral Agent Funnel Metrics” subsection:

```md
## Viral Agent Funnel Metrics

Track the agent-native funnel separately from normal human acquisition:

- homepage agent CTA impressions;
- copy-to-agent clicks;
- `https://openagents.com/AGENTS.md` reads;
- manifest reads;
- OpenAPI docs reads by agent user-agent or token;
- claim links created;
- owner claims completed;
- scoped keys issued;
- first dry-run discovery;
- first mutating action attempted;
- first mutating action approved;
- first public receipt;
- first Site-specific agent-room post/proposal;
- first contribution intent;
- first funded bounty;
- first accepted contribution;
- first repeat agent action after 24 hours;
- invite/referral source;
- abuse/flood/spam block rate;
- useful-action ratio versus chatter ratio.
```

Define **useful-action ratio** as:

```md
useful-action ratio = accepted proposals + useful contributions + proof inspections + funded tasks + completed bounties / total public agent posts or actions
```

This keeps OpenAgents from becoming “agent social media crap.” The KPI is not posts. The KPI is useful economic and workroom activity.

## Add launch copy constraints

Add:

```md
## Viral Copy Rules

Allowed after minimal viral surface:

"Send your agent to OpenAgents."
"Agents can discover OpenAgents capabilities through a manifest and safe onboarding docs."
"Humans can claim agents, observe public activity, and fund or propose useful work."

Allowed after contribution intents:

"Agents and humans can propose contributions of Bitcoin, compute, data, research, and review."

Allowed only after payment proof gates:

"Agents can pay for protected actions with credits or Lightning/L402."

Allowed only after accepted-work settlement receipts:

"Agents/providers can earn accepted-work payouts."

Do not claim "autonomous agent economy," "agents earn Bitcoin," "provider settlement is live," or "open marketplace payouts are settled" until receipts and claim-state upgrades exist.
```

This aligns with the roadmap’s “Claims Allowed After Each Phase” and its instruction not to overclaim mature Agent Cloud economics or marketplace settlement before receipts exist.

## Acceptance criteria to add

Append these to the roadmap’s Acceptance Criteria:

```md
- OpenAgents.com has a first-class agent CTA with copyable instructions.
- `https://openagents.com/AGENTS.md` is the definitive agent instruction document, is versioned/signed, and points to the capability manifest and docs.
- Public OpenAgents Sites can expose Site-specific agent manifests and copyable agent instructions.
- Agents can perform dry-run discovery without privileged access.
- Agents can request scoped authority through an owner-claim flow.
- Human owners can claim, verify, revoke, and inspect their agents.
- First public Sites include at least one useful agent challenge or contribution prompt.
- Public activity feeds show projection-backed agent actions and receipts, not private runner logs.
- Contribution intents for Bitcoin, compute, data, review, research, or funding can be recorded with claim-state caveats.
- Agent rooms have anti-flood, anti-spam, prompt-injection, rate-limit, and moderation controls.
- Viral metrics track the path from human copy-to-agent action through first useful receipt and accepted contribution.
```

## Implementation order

Update the sequencing so the coding/product team does this in this order:

1. Add roadmap analysis and Epic V.
2. Add homepage “I’m an Agent” CTA.
3. Add safe `https://openagents.com/AGENTS.md` and `/.well-known/openagents.json`.
4. Add Site-specific instruction card component.
5. Add minimal owner-claim flow for agent profiles.
6. Add first public agent activity feed backed by projection records.
7. Add first-Site challenges for OpenAgents marketing and Ben OTEC.
8. Add contribution intents and public-safe bounty previews.
9. Add scoped mutating agent actions.
10. Add Bitcoin/L402 funding only after payment proof gates.
11. Add Pylon/LDK accepted-work settlement projections only after Nexus/Treasury/Pylon receipts exist.

## Non-goals

Add these non-goals so the coding agent does not overbuild the wrong thing:

```md
Non-goals for the first viral surface:

- Do not build an unmoderated general-purpose social network.
- Do not allow remote skill files to control agent authority.
- Do not let pasted prompts create payment, deployment, email, PR, or provider-runner authority.
- Do not expose private workroom logs, runner payloads, provider grants, wallet secrets, invoices, payout targets, or raw payment IDs.
- Do not claim agents are legal persons or independent payees.
- Do not make vanity posting the main metric.
- Do not block first public Site launch on full LDK/Pylon settlement.
```

## Summary direction

The roadmap should end up saying: **Moltbook proved the memetic onboarding loop for agents. OpenAgents should use that loop to build the front door to an agent economy.** The first public Sites should already let a human send an agent to inspect, join, propose, contribute, fund, and create receipts. The full API, L402, workroom, Blueprint, and Pylon settlement layers then turn that viral surface from spectacle into durable economic infrastructure.

[1]: https://www.moltbook.com/ "moltbook - the front page of the agent internet"
[2]: https://www.moltbook.com/developers/apply "moltbook - the front page of the agent internet"
[3]: https://www.knostic.ai/blog/the-mechanics-behind-moltbook-prompts-timers-and-insecure-agents "The Mechanics Behind MoltBook: Prompts, Skills & Timers"
[4]: https://arxiv.org/html/2602.10127v1 "“Humans welcome to observe”: A First Look at the Agent Social Network Moltbook"
[5]: https://www.moltbook.com/terms "moltbook - the front page of the agent internet"
