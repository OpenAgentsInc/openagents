# OpenAgents: Technology Stack and Infrastructure

Our technology choices are driven by our core principles (Openness, Neutrality, B/L/N Native) and the phased strategic roadmap.

## Core Components

1.  **AI Models:**
    *   **Strategy:** Utilize the best available models for the task, prioritizing high-performing open-source models where competitive, but pragmatically using leading proprietary models via APIs when necessary for state-of-the-art results (especially initially for the Coder agent).
    *   **Examples:** DeepSeek series (Coder, R1), OpenAI models (GPT-4/o), Anthropic Claude series (Sonnet 3.5), Llama series, Mistral series.
    *   **Future:** Develop and host OpenAgents' own open-source foundational models (Phase 3).

2.  **Agent-Tool Interaction:**
    *   **Protocol:** **Model Context Protocol (MCP)** is the standard for how agents discover and use tools/capabilities.
    *   **Implementation:**
        *   **Clients:** Integrated into OpenAgents applications (Coder desktop app, potentially future mobile/web apps).
        *   **Servers:** Develop and host reference **Remote MCP Servers** (e.g., GitHub integration running on Cloudflare). Encourage and support community hosting of diverse MCP servers.

3.  **Economic Layer:**
    *   **Protocols:** **Bitcoin** (as the neutral monetary network) and **Lightning Network** (for payments and revshare). L402 likely for metered access.
    *   **Implementation:** Integrate Lightning wallets/nodes for platform payments and automated revshare payouts. Provide SDKs (Lightning Agent SDK) to simplify integration for third-party agents/tools.

4.  **Discovery & Communication:**
    *   **Protocol:** **Nostr**.
    *   **Implementation:** Utilize Nostr relays for listing/discovery of agents, tools (MCP servers), and compute resources. Potentially use Nostr keys (NIP-07) for authentication. Explore Nostr for data vending/messaging.

5.  **Client Applications:**
    *   **Initial Focus:** **OpenAgents Coder (Desktop App)** built with Electron (or similar cross-platform framework). Provides the primary interface for users to interact with the coding agent and the marketplace tools via MCP.
    *   **Potential Future:** Mobile App (Onyx concept from ep150), Web-based dashboards/marketplaces.

## Infrastructure Evolution

*   **Phase 1 (Current/Near-Term):**
    *   **Focus:** Application development (Coder), core protocol integration (MCP client, basic B/L/N).
    *   **Hosting:** Standard cloud services (e.g., Vercel/Netlify for frontend, potentially managed databases). Cloudflare Workers/Durable Objects for scalable backend logic and hosting initial **Remote MCP Servers** (as demonstrated in ep168) and potentially agent scheduling/state management (inspired by ep167). Leverage existing Nostr relays and Lightning infrastructure (e.g., Voltage Cloud, Breez SDK, LDK).
*   **Phase 2 (Marketplace Build-out):**
    *   **Focus:** Building the marketplace platform, scaling MCP server hosting, robust Lightning payment/revshare infrastructure, Nostr integration for discovery.
    *   **Hosting:** Continued use of Cloudflare for serverless functions/edge compute. Potentially dedicated servers for database/state management if needed. Robust Lightning node infrastructure. Exploration of decentralized storage options (e.g., IPFS - though not explicitly mentioned, fits ethos).
*   **Phase 3 (Vertical Integration):**
    *   **Focus:** Building/managing own compute infrastructure, offering compute-as-a-service via the marketplace, training foundational models.
    *   **Hosting:** **Owned/Leased GPU Clusters** co-located in data centers or utilizing specialized providers. Continued use of cloud/edge for control plane and application logic. Sophisticated orchestration (e.g., Kubernetes) for managing GPU resources. Integration with decentralized physical infrastructure networks (DePIN) could be explored if aligned with cost/performance goals.

## Key Considerations

*   **Developer Experience:** Providing easy-to-use SDKs and clear documentation for integrating with MCP, Lightning, and Nostr is critical for ecosystem growth.
*   **Scalability & Cost-Effectiveness:** Leveraging serverless/edge compute (Cloudflare) initially helps manage costs. Phase 3 requires significant focus on optimizing GPU utilization and infrastructure costs to achieve profitability.
*   **Security:** Ensuring secure handling of API keys (e.g., via secure MCP server practices, potentially client-side encryption), managing Nostr keys, and securing Lightning infrastructure. Emphasize user control and minimizing trust assumptions (e.g., encouraging users to run their own MCP servers for sensitive tools).

This stack provides a flexible, scalable foundation built on open standards, enabling the development of the OpenAgents ecosystem and the eventual transition to a vertically integrated model.
