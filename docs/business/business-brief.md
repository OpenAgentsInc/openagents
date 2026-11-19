# OpenAgents — Internal Business Brief

**Date:** 2026-11-19  
**Status:** Internal descriptive brief (working document)

> **Disclaimer & Clarification:** This document is an internal descriptive brief prepared solely to articulate OpenAgents' current thesis, product architecture, positioning, and proposed business strategy. It does not constitute forward-looking statements, financial projections, investment or financial advice, legal advice, or an offer or solicitation of any kind. Where monetization mechanics, pricing, or unit economics are described below, they reflect internal proposals and unmeasured working assumptions rather than built features or realized results.

---

## I. Preamble

### Mission
OpenAgents builds open agents that people can own, inspect, improve, and earn from—grounded in open protocols and Bitcoin. Since November 2023, the project has been developed in public around a single conviction: AI agents should operate as durable, user-owned infrastructure rather than rented black boxes.

### Core Thesis
Most current AI products couple a frontier model to a closed, proprietary client harness. That architecture creates a structural platform asymmetry: the vendor controls the execution contract, retains the history, meters access unilaterally, and can alter runtime behavior or terms without notice. A capability built on a closed wrapper is not a durable capability. Durability requires three things: an inspectable execution contract, portable state and history, and a fallback the user controls.

OpenAgents applies this thesis across both developer/agent infrastructure and consumer interactive creation. By separating open protocols and structured project state from any single model provider, OpenAgents allows humans and agents to collaborate on long-lived artifacts—codebases, tools, and playable 3D game worlds—without surrendering ownership or distribution.

### The Market Shift
Two converging shifts define the opportunity:

1. **From one-shot generation to persistent agentic workspaces.** In software engineering, value moved rapidly from isolated code completions to persistent workspaces that retrieve project context, produce readable diffs, run checks, and iterate across sessions. Interactive media and game creation are entering the same transition: one-shot prompt-to-game demos break down the moment a creator tries to edit, debug, or extend a project. The winning workflow is a durable, structured workspace where humans and agents edit the same live project.
2. **From closed creator economies to open, agent-accelerated networks.** Legacy user-generated content (UGC) platforms proved that millions of players will participate in creator-built worlds, but they pair high platform take rates with closed tooling and slow, restricted payouts. Agent-assisted building lowers the skill and time barrier to making playable games, opening the door to an ecosystem where creators keep more of what they earn and settle value natively.

---

## II. Business Analysis

### Product & Platform Architecture
OpenAgents is structured as a layered stack combining open agent infrastructure with a flagship consumer creation surface:

- **OpenAgents Core & Native Apps:** Open-source agent harnesses (including Coder/Microcoder), native desktop and mobile hosts, and the `coax` runtime—designed for inspectable state, deterministic tool execution, and multi-provider model routing (supporting both user-supplied keys/local models and managed compute).
- **OpenAgents Metaverse Engine:** The durable world and project substrate built in the open (`metaverse-engine/`), exposing explicit, versioned protocols (`world.edit.v1`, `world.scripts.v1`, `world.turn.v1`) so both human UIs and autonomous agents operate on the same structured world representation.
- **GameBuilder:** The playable interface and persistent game-creation workspace built on the OpenAgents Metaverse Engine. Creators and agents can build, inspect, playtest, and modify levels through atomic, undoable edit batches (`world.edit.v1`), turning game development into an iterative edit-and-play loop rather than a fragile regeneration loop.

### Ecosystem & Staged Strategy
Internal strategy proposals separate near-term validation from long-term network scale:

- **Phase One — Prove the Creator-to-Player Loop:** Rather than attempting to launch a mass-market social network on day one, the proposed initial phase focuses on a tight cohort of creators and agent-assisted builders. The core metric is organic propagation ("one game brings half a game"): whether published, playable games naturally attract new players and remixers into the workspace, governed by explicit pass/stop criteria before scaling spend.
- **Scale Horizon — Tool-Funded Path to a Player Network:** In the proposed progression, creation tools and agent compute generate early revenue while the catalog of playable, remixable worlds compounds. Long-term scale requires transitioning from a creation tool to a self-sustaining network of daily active players and creators.

### Competitive Positioning
OpenAgents sits at the intersection of two distinct competitive landscapes:

- **Versus closed AI platform incumbents:** Proprietary agent vendors optimize for lock-in through opaque client harnesses, non-portable session state, and restrictive runtime policies. OpenAgents competes on transparency, user-owned local/portable state, inspectable execution contracts, and provider independence.
- **Versus incumbent UGC platforms and AI game generators:**
  - *Incumbent UGC platforms (e.g., Roblox)* have massive player distribution and mature tooling, but extract steep platform fees and lock creators inside proprietary economies.
  - *AI asset and one-shot game generators (e.g., Meshy, Tripo, Rosebud, Ludo)* accelerate isolated assets or prototypes, but lack a persistent, structured, multi-session engine workspace capable of deep iterative editing and playtesting.
  - *GameBuilder + OpenAgents* targets the gap between them: a persistent, agent-native game-creation workspace paired with open protocols and creator-aligned economics.

### Moat & Defensibility
Foundation models are a rapidly commoditizing layer; OpenAgents does not rely on owning a proprietary frontier model for defensibility. Instead, durable advantage is designed to compound across four layers:

1. **Structured, Agent-Native World Representation:** A durable project format and protocol (`world.edit.v1` and sibling specs) where edits are structured, diffable, undoable, and testable in-engine by both humans and agents.
2. **Closed-Loop Playtest Verification:** Tight coupling between the OpenAgents Metaverse Engine and GameBuilder so agents do not just emit code or assets—they inspect live world state, apply batched changes, and verify playable results.
3. **Composable Remix & Asset Graph:** Proposed support for reusable parts ("for a share") and remixable games with automatic attribution and revenue splits creates a compounding library that single-player tools cannot easily replicate.
4. **Aligned, Low-Friction Economic Rails:** Combining standard web/app-store payment methods with open Bitcoin/Lightning rails enables rapid, global creator settlement and lower structural overhead where legally and operationally permitted.

### Business Model & Monetization (Proposed)
*Note: None of the monetization features below are yet built in GameBuilder, and production unit costs have not yet been measured. Figures reflect current internal proposals subject to telemetry and owner decisions.*

The proposed model combines two revenue layers—charging for creation/compute capacity while taking a modest share of creator commerce:

1. **Creation & Agent Compute Layer:**
   - **Free Tier:** Playing games and core building remain accessible to drive adoption and sharing.
   - **Pro Subscription:** Proposed starting price of **$10/month**—intentionally positioned below $20–$25 developer tools to fit gamers and families.
   - **Credits & Metered Agent/Machine Compute:** Prepaid Credit packs and metered usage for managed AI agents and cloud machines, protected by explicit user-agreed spending ceilings. Users who bring their own model keys or run locally retain a clear fallback.
2. **Creator Economy & Marketplace Layer:**
   - **Game Sales, Tips, Parts & Remix Royalties:** Creators earn from games, tips, reusable parts listed for a revenue share, and downstream remixes. The proposed design targets creator-friendly take rates and near-instant settlement (seconds via Bitcoin/Lightning sats rails where compliant, alongside standard fiat/store rails).
   - **Platform Rail Awareness:** Web and native app-store distribution carry different fee and policy structures (e.g., Apple App Store in-app purchase rules for digital goods used inside the app). Pricing, web-to-app parity, and payout flows are designed to respect store rules, card-network policies, and applicable law.

### Key Risks & Execution Realities
A grounded assessment highlights five primary risks that must be managed through disciplined staging:

1. **Unmeasured Unit Economics:** Current cost tables for model inference, cloud machines, and free-tier creator usage are illustrative examples, not measured telemetry. Pricing and margins cannot be finalized until real usage is logged on live builds.
2. **Cold-Start & Distribution Risk:** Turning an agent-assisted creation tool into a large-scale player network is difficult. If early games do not retain players or trigger organic sharing and remixing, the business remains a niche tool rather than a compounding network. Explicit Phase One stop-rules are required to test this cheaply.
3. **Store, Payment & Regulatory Constraints:** Apple App Store rules, card-network requirements, and legal/regulatory boundaries around creator payouts and Bitcoin/sats treasury flows require formal legal counsel and careful product gating before launch.
4. **Upstream Model & Cloud Dependencies:** Relying on third-party frontier models exposes the product to upstream price changes, rate limits, and terms-of-service restrictions (such as provider limits on cloud-hosted authentication). Multi-model routing, BYOK, and local-model support are operational necessities, not optional features.
5. **Execution Scope & Focus:** Building an open-source agent runtime, a game engine, a consumer creation workspace, and a compliant creator marketplace simultaneously carries significant execution risk. Sequencing—proving the core creation and play loop before expanding monetization and network mechanics—is essential.
