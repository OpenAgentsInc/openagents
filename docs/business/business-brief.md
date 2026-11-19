# OpenAgents — Internal Business Brief

**Date:** 2026-11-19  
**Status:** Internal descriptive brief (working document)

> **Disclaimer & Clarification:** This document is an internal descriptive brief prepared solely to articulate OpenAgents' current thesis, product architecture, positioning, and proposed business strategy. It does not constitute forward-looking statements, financial projections, investment or financial advice, legal advice, or an offer or solicitation of any kind. Where monetization mechanics, pricing, or unit economics are described below, they reflect internal proposals and unmeasured working assumptions rather than built features or realized results.

---

## I. Preamble

### Mission: Strengthening Human Agency
OpenAgents exists to **strengthen human agency**. Since November 2023, the project has been built in public around a single conviction: AI agents should enlarge what ordinary people can understand, build, inspect, own, and earn from—operating as durable, user-owned infrastructure grounded in open protocols and Bitcoin rather than rented black boxes.

### Active Direction Over Passive Delegation
Across much of the AI industry, the prevailing trajectory treats step-by-step delegation to frontier models and autonomous systems as a path toward removing people from active control—reducing users to passive consumers of opaque outputs while creative and economic authorship shifts to centralized platforms. Even critiques of that trajectory often treat market forces as naturally favoring human-excluding automation, assume that preserving human primacy requires top-down institutional restraint, or treat accountability, revocable authority, and data sovereignty as voluntary pledges from centralized AI vendors.

OpenAgents rejects those premises at both the architectural and economic levels:

1. **Active human direction, not passive delegation.** Unchecked black-box delegation erodes human capability, context, and oversight over time. OpenAgents designs persistent, collaborative workspaces where humans remain the author and decision-maker—inspecting diffs, reviewing structured state, undoing actions, and compounding their own skill and mastery alongside agents.
2. **Agency enforced by architecture, not vendor promises.** Voluntary vendor pledges around auditable actions, revocable permissions, and data portability are structurally fragile when coupled to a closed, proprietary client harness. When a provider controls the execution contract, retains session history, and meters access unilaterally, user authority exists only at the vendor's discretion. Durable agency requires three architectural guarantees: an inspectable execution contract, portable user-owned state and history, and a fallback the user controls (BYOK and local models).
3. **Aligning economics *with* human agency rather than relying on restraint.** Rather than assuming that market competition inevitably concentrates power in human-free autonomous systems—or relying on top-down restrictions to slow that shift—OpenAgents bets that open protocols, composable creation tools, and open economic rails (including Bitcoin/Lightning) make human-directed, agent-accelerated builders economically stronger, more sovereign, and harder to displace.
4. **Interactive creation and worlds as real agency and economic value.** Rather than treating interactive media as passive entertainment generated for consumers by opaque systems, OpenAgents treats interactive worlds and games as a high-agency creative medium where people build, debug, remix, and own durable digital artifacts and real economic value.

### Core Thesis
Most current AI products couple a frontier model to a closed, proprietary client harness. That architecture institutionalizes user dependency: the individual surrenders execution visibility, state ownership, and switching leverage to a centralized intermediary. By separating open protocols and structured project state from any single model provider, OpenAgents ensures that humans and agents collaborate on long-lived artifacts—codebases, tools, and playable 3D game worlds—while keeping ownership, accountability, and distribution firmly in human hands.

### The Market Shift
Two converging shifts turn strengthening human agency from a philosophical stance into a concrete market opportunity:

1. **From passive one-shot generation to high-agency agentic workspaces.** One-shot prompt-to-output tools encourage passive delegation: users ask a black box for a result they cannot inspect, debug, or extend. In software engineering, value moved rapidly to persistent workspaces that surface context, produce readable diffs, run checks, and keep humans in command across sessions. Interactive media and game creation are entering the same transition: durable, structured workspaces where humans and agents edit the same live project.
2. **From extractive creator platforms to sovereign, agent-accelerated economies.** Legacy user-generated content (UGC) platforms proved that millions of players will participate in creator-built worlds, yet they pair steep take rates with closed tooling and restricted payouts—mirroring the centralized dependency of closed AI labs. Agent-assisted building lowers the skill and time barrier to making playable games, opening the door to an open ecosystem where human creators retain ownership, keep more of what they earn, and settle value natively.

---

## II. Business Analysis

### Product & Platform Architecture
OpenAgents translates human agency into concrete engineering across a layered stack combining open agent infrastructure with a flagship consumer creation surface:

- **OpenAgents Core & Native Apps:** Open-source agent harnesses (including Coder/Microcoder), native desktop and mobile hosts, and the `coax` runtime—built so permissions are intelligible, tool execution is deterministic and inspectable, state stays local and portable, and authority over model routing remains with the user (supporting user-supplied keys, local models, and managed compute).
- **OpenAgents Metaverse Engine:** The durable world and project substrate built in the open (`metaverse-engine/`), exposing explicit, versioned protocols (`world.edit.v1`, `world.scripts.v1`, `world.turn.v1`) so human UIs and autonomous agents operate on the exact same structured, auditable world representation—never behind an opaque proprietary wall.
- **GameBuilder:** The playable interface and persistent game-creation workspace built on the OpenAgents Metaverse Engine. Rather than reducing users to passive consumers of AI-generated novelty, GameBuilder lets creators and agents build, inspect, playtest, and modify levels through atomic, undoable edit batches (`world.edit.v1`), turning game development into an iterative loop that builds real human skill and creative control.

### Ecosystem & Staged Strategy
Internal strategy proposals separate near-term validation from long-term network scale, testing whether agency-first tools drive organic human adoption:

- **Phase One — Prove the Creator-to-Player Loop:** Rather than attempting to launch a mass-market social network on day one, the proposed initial phase focuses on a tight cohort of creators and agent-assisted builders. The core metric is organic propagation ("one game brings half a game"): whether published, playable games naturally inspire players to step into the driver's seat as builders and remixers, governed by explicit pass/stop criteria before scaling spend.
- **Scale Horizon — Tool-Funded Path to a Player Network:** In the proposed progression, creation tools and agent compute generate early revenue while the catalog of playable, remixable worlds compounds. Long-term scale requires transitioning from a creation tool to a self-sustaining network of daily active players and creators who own their work.

### Competitive Positioning
OpenAgents sits at the intersection of two distinct competitive landscapes, differentiating on structural user agency in both:

- **Versus closed AI platform incumbents:** Proprietary agent vendors optimize for lock-in through opaque client harnesses, non-portable session state, and unilateral runtime policies—even while rhetorically endorsing user sovereignty. OpenAgents competes by making transparency, user-owned local/portable state, inspectable execution contracts, revocable agent authority, and provider independence non-negotiable architectural properties.
- **Versus incumbent UGC platforms and AI game generators:**
  - *Incumbent UGC platforms (e.g., Roblox)* have massive player distribution and mature tooling, but extract steep platform fees and lock creators inside proprietary economies.
  - *AI asset and one-shot game generators (e.g., Meshy, Tripo, Rosebud, Ludo)* accelerate isolated assets or prototypes, but reinforce a black-box delegation workflow that breaks down during deep iterative editing, debugging, and playtesting.
  - *GameBuilder + OpenAgents* targets the gap between them: a persistent, agent-native game-creation workspace paired with open protocols and creator-aligned economics that leave users more capable over time.

### Moat & Defensibility
Foundation models are a rapidly commoditizing layer; OpenAgents does not rely on owning a proprietary frontier model—or trapping users inside a closed harness—for defensibility. Instead, durable advantage is designed to compound across four agency-reinforcing layers:

1. **Structured, Agent-Native World Representation:** A durable, open project format and protocol (`world.edit.v1` and sibling specs) where every agent action is structured, diffable, undoable, and testable in-engine—giving humans direct oversight and control.
2. **Closed-Loop Playtest Verification:** Tight coupling between the OpenAgents Metaverse Engine and GameBuilder so agents do not just emit unchecked code or assets—they inspect live world state, apply batched changes, and verify playable results under human direction.
3. **Composable Remix & Asset Graph:** Proposed support for reusable parts ("for a share") and remixable games with automatic attribution and revenue splits creates a compounding human-and-agent creative commons that single-player or closed tools cannot easily replicate.
4. **Aligned, Sovereign Economic Rails:** Combining standard web/app-store payment methods with open Bitcoin/Lightning rails enables rapid, global creator settlement and lower structural overhead where legally and operationally permitted—putting economic agency directly in creators' hands.

### Business Model & Monetization (Proposed)
*Note: None of the monetization features below are yet built in GameBuilder, and production unit costs have not yet been measured. Figures reflect current internal proposals subject to telemetry and owner decisions.*

The proposed model aligns OpenAgents' revenue with expanding human creative capacity rather than capturing user lock-in—combining creation/compute capacity with a modest share of creator commerce:

1. **Creation & Agent Compute Layer:**
   - **Free Tier:** Playing games and core building remain accessible so anyone can move from player to creator without gatekeeping.
   - **Pro Subscription:** Proposed starting price of **$10/month**—intentionally positioned below $20–$25 developer tools to fit gamers, students, and families.
   - **Credits & Metered Agent/Machine Compute:** Prepaid Credit packs and metered usage for managed AI agents and cloud machines, protected by explicit user-agreed spending ceilings. Users who bring their own model keys or run locally retain a clear, unpenalized fallback.
2. **Creator Economy & Marketplace Layer:**
   - **Game Sales, Tips, Parts & Remix Royalties:** Creators earn from games, tips, reusable parts listed for a revenue share, and downstream remixes. The proposed design targets creator-friendly take rates and near-instant settlement (seconds via Bitcoin/Lightning sats rails where compliant, alongside standard fiat/store rails).
   - **Platform Rail Awareness:** Web and native app-store distribution carry different fee and policy structures (e.g., Apple App Store in-app purchase rules for digital goods used inside the app). Pricing, web-to-app parity, and payout flows are designed to respect store rules, card-network policies, and applicable law.

### Key Risks & Execution Realities
A grounded assessment highlights five primary risks that must be managed through disciplined staging:

1. **Unmeasured Unit Economics:** Current cost tables for model inference, cloud machines, and free-tier creator usage are illustrative examples, not measured telemetry. Pricing and margins cannot be finalized until real usage is logged on live builds.
2. **Cold-Start & Distribution Risk:** Turning an agent-assisted creation tool into a large-scale player network is difficult. If early games do not retain players or trigger organic sharing and remixing, the business remains a niche tool rather than a compounding network. Explicit Phase One stop-rules are required to test this cheaply.
3. **Store, Payment & Regulatory Constraints:** Apple App Store rules, card-network requirements, and legal/regulatory boundaries around creator payouts and Bitcoin/sats treasury flows require formal legal counsel and careful product gating before launch.
4. **Upstream Model & Cloud Dependencies:** Relying on third-party frontier models exposes the product to upstream price changes, rate limits, and terms-of-service restrictions (such as provider limits on cloud-hosted authentication). Multi-model routing, BYOK, and local-model support are operational necessities—both for resilience and for preserving user agency.
5. **Execution Scope & Focus:** Building an open-source agent runtime, a game engine, a consumer creation workspace, and a compliant creator marketplace simultaneously carries significant execution risk. Sequencing—proving the core creation and play loop before expanding monetization and network mechanics—is essential.
