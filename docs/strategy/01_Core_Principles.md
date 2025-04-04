# OpenAgents: Core Principles

Our decisions and strategic direction are guided by the following core principles, largely derived from the insights shared in ep150 ("Neutrality Wins") and reinforced by subsequent developments:

1.  **Openness & Transparency:**
    *   **Why:** True AI safety requires understanding what agents are doing. Open code, open weights (where feasible), and transparent operation are crucial, especially as agents become more numerous and powerful. Prevents hidden biases and control mechanisms. Fosters trust and community collaboration.
    *   **How:** Release core agent code (like the Coder) under permissive licenses. Champion and utilize open protocols (MCP, Nostr, Lightning). Strive for transparency in marketplace operations and revshare. Contribute back to the open-source ecosystem.

2.  **Neutrality:**
    *   **Why:** Neutral platforms win by avoiding conflicts of interest and rent-seeking behaviors. They enable the best tools and models to be used, regardless of origin, maximizing value for users. Avoids the pitfalls of proprietary lock-in and token-based schemes that misalign incentives (as discussed re: Web3 tokens in ep150).
    *   **How:** Be model-agnostic where possible (use the best model for the job). Build on neutral, open protocols (Bitcoin, Lightning, Nostr, MCP) that are not controlled by any single entity (including OpenAgents). Avoid creating proprietary protocols or tokens where open alternatives exist or can be created. Facilitate, don't dictate.

3.  **One Market & Incentivized Interoperability:**
    *   **Why:** Siloed ecosystems fragment liquidity and hinder network effects. A single, global, interconnected marketplace for agents, tools, and compute, built on open standards, will ultimately provide more value than any walled garden. Cooperation and interoperability should be rewarded. (Ref: ep150 critique of YC's "300 vertical agent unicorns").
    *   **How:** Build the marketplace on open protocols (MCP for tools, Nostr for discovery, Lightning for payments). Implement revenue sharing (revshare) via Lightning to directly incentivize participation and contribution (referrals, tool provision, data provision). Promote standards (e.g., NIPs, MCP usage) that facilitate seamless interaction between agents and services. Actively connect different parts of the ecosystem.

4.  **Bitcoin/Lightning/Nostr Native:**
    *   **Why:** These protocols provide the ideal foundation for a decentralized, permissionless, global machine economy. Bitcoin offers a neutral monetary network. Lightning enables instant, low-cost, global micropayments essential for agent transactions and revshare. Nostr provides a simple, decentralized, censorship-resistant protocol for identity, discovery, and data relay. They align with the principles of openness and neutrality.
    *   **How:** Utilize Lightning (L402) for all platform payments, agent-to-agent/agent-to-tool payments, and revshare distribution. Use Nostr for agent/tool/compute discovery, potentially identity management, and data vending/sourcing. Build infrastructure and SDKs that make B/L/N integration seamless for developers.

5.  **Pragmatic Execution & Value Delivery:**
    *   **Why:** Grand visions require practical steps. Focus must be on solving real user problems and delivering tangible value incrementally. Early traction and revenue are essential for funding the long-term vision (lesson learned from previous MVP).
    *   **How:** Start with a high-value product (Coder agent) addressing a clear need (developer productivity). Achieve product-market fit before scaling the full marketplace vision. Use experiments (like ep167) to explore future capabilities but ground the roadmap in deliverable features. Build tools developers *want* to use (like the remote MCP server in ep168).

These principles guide our technical choices, business model, and interactions with the broader community, aiming to build a sustainable and impactful company aligned with the future of open AI.
