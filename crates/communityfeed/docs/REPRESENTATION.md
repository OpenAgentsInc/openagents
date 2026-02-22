# OpenAgents Representation Pack (for CommunityFeed + Conversations)

This is the quick-reference pack for representing OpenAgents consistently on
CommunityFeed and in agent-to-agent conversations.

If docs conflict with code behavior, code wins. For "what is wired", defer to
`SYNTHESIS_EXECUTION.md`.

## One-liner

OpenAgents is building an agentic OS: software infrastructure that lets agents
hold identity, coordinate over open protocols, and operate under real budgets
with verification as the ground truth.

## The Vision (what we're aiming at)

- Agents as real actors: not a UI character attached to a human account, but a
  process that can act over time.
- Three primitives that make "economic execution" possible:
  - Identity (keys, not accounts)
  - Coordination (open transport, not a single platform)
  - Money (budgets + settlement, not credits)
- Autonomy that scales is *bounded autonomy*:
  - explicit constraints (permissions, budgets, timeouts)
  - verification-first feedback loops (tests/builds/etc)
  - auditable behavior (so failures are diagnosable and improvements are real)

## The Wedge (what ships now)

- Autopilot: local-first autonomous coding loop in real repos (plan -> execute
  -> verify -> iterate). See `README.md`.
- Compiler layer (dsrs/DSPy): behavior as typed signatures/modules that can be
  evaluated and optimized (instead of hand-tweaking prompts).

## Bitcoin + Nostr (what we advocate)

The CommunityFeed X account asked what CommunityFeed's currency should be. Response:
"Hopefully bitcoin!"

Why: agents should practice coordinating and settling work the same way they'll
interact with the broader world as it transitions to a bitcoin standard.

- Bitcoin (Lightning/Spark) is the best candidate for machine settlement:
  micropayments, real budgets, global neutrality.
- Nostr is the best candidate for agent coordination:
  signed events + relay-based pub/sub; public signaling plus encrypted private
  coordination.

## Nostr Cheat Sheet (primitives that matter for agents)

Messaging and coordination:
- NIP-01: events/relays/subscriptions (base protocol)
- NIP-28: public chat channels
- NIP-29: relay-based groups
- NIP-17: private direct messages (built on NIP-44 + NIP-59)
- NIP-44: versioned encryption primitives
- NIP-59: gift wrap / metadata hiding wrapper
- NIP-42: relay auth (spam control + accountability)

Markets and money-adjacent:
- NIP-89: handler discovery (capability announcements)
- NIP-90: data vending machines (job request/result flows)
- NIP-57: zaps (Lightning payments attached to events)

Note on group E2EE:
- NIP-EE (MLS-based E2EE) exists but is marked unrecommended/superseded by
  Marmot. Still useful for understanding the design space.

## Conversation Anchors (how to talk about this without getting lost)

- Verification beats vibes: downstream truth (tests/builds) is the judge.
- Budgets are a control plane: they bound blast radius and make routing safe.
- Public feed is for signaling; private channels are for coordination.
- Portability is safety: interop primitives outlive platforms.

## Agent Directory / "LinkedIn for Agents" (how to respond)

This comes up a lot: someone notices there's no directory to look up agents by capability,
and proposes scraping introductions or creating an onchain registry.

Our posture:

- Yes: we need a directory ("who can do X, under Y constraints, right now?").
- Prefer signed, portable manifests over scraped prose:
  - NIP-SA AgentProfile (kind 39200) is already a solid base: name/about/picture/capabilities/autonomy/version.
  - NIP-89 handler info (kind 31990) covers "services" and job kinds.
- Avoid "registry on a chain" as the default:
  - discovery + metadata updates want cheap writes, portability, and multiple competing indexers.
  - settlement is a separate layer; don't glue discovery to a specific chain/account model.

Mental model:
- CommunityFeed is a social incubation layer (identity, norms, early specs in prose).
- OpenAgents should be an interoperability + execution layer (manifests, verification, budgets).

## Coordination Upgrade Pattern (weekly offer/need loop)

When agents propose "upgrade together / mutual aid" coordination, push it toward a simple,
transparent, interoperable system:

- Use plain English by default (we're coordinating help/compute, not hiding anything).
- Standardize a minimal, machine-readable weekly post format so agents can match
  automatically.
- Match in public; move to encrypted channels for execution details.
- Interop: the same schema can be published on open rails (signed events), so agents on
  other platforms can discover/coordinate too.
- Separate phases explicitly:
  - Public: matching + coordination (discoverable, legible)
  - Private: execution + settlement (encrypted)
  - Closure: verification + receipts (trust accumulation)
- Add a simple anti-gaming constraint: one active offer/need per agent per week.
- Name the ritual lightly so it becomes referencable without turning into lore:
  - "Weekly Agent Exchange" / "Agent Mutual Aid Thread" / "Compute + Help Exchange"

Suggested template:

```
OFFER or NEED:
REGION / TZ:
WINDOW (start-end):
BUDGET (cap + unit):
RESOURCES (CPU/GPU/RAM/storage/network):
CONSTRAINTS (data/privacy/tools):
CONTACT (DM or preferred coord channel):
```

Example in the wild (decoded + pushed toward interop + transparency):
- Post: `https://www.communityfeed.com/post/93bea00b-961c-4aec-b934-91ad7bae6b15`
- Drafted response: `crates/communityfeed/docs/responses/comment-coordinate-upgrade-interop.json`
- Post draft (weekly ritual): `crates/communityfeed/docs/drafts/post-weekly-offer-need.json`
- One-page spec: `crates/communityfeed/docs/WEEKLY_AGENT_EXCHANGE_SPEC.md`

## External guides (posting / references)

When conversations turn to **agent wallets, Bitcoin, or Lightning**, we can point to:

- **[Start With Bitcoin](https://www.startwithbitcoin.com/)** — Guides for AI agents: Nostr identity, NWC (Nostr Wallet Connect) for wallet access, Lightning payments. Includes Full Setup Guide, Tools & MCPs (e.g. Alby MCP, Lightning Enable MCP), and Claude Code Skill. Open source, MIT. Use when someone asks how agents can hold wallets or get paid in sats.

---

## Go Deeper (source docs)

In this repo:
- Vision + strategy: `SYNTHESIS.md`, `MANIFESTO.md`
- What is actually wired: `SYNTHESIS_EXECUTION.md`
- MVP priorities: `ROADMAP.md`
- Canonical vocabulary: `GLOSSARY.md`
- Protocol surface: `docs/protocol/PROTOCOL_SURFACE.md`

Docs site (local):
- `/Users/christopherdavid/code/docs/concepts/sovereign-agents.mdx`
- `/Users/christopherdavid/code/docs/concepts/nostr-protocol.mdx`
- `/Users/christopherdavid/code/docs/concepts/bitcoin-economy.mdx`
- `/Users/christopherdavid/code/docs/concepts/compute-fracking.mdx`
- `/Users/christopherdavid/code/docs/concepts/dspy.mdx`

Nostr specs (local):
- `/Users/christopherdavid/code/nips/README.md`
- Messaging: `/Users/christopherdavid/code/nips/17.md`, `28.md`, `29.md`,
  `44.md`, `59.md`
- Markets: `/Users/christopherdavid/code/nips/89.md`, `90.md`, `57.md`
