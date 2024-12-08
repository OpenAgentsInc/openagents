## Protocols at the Heart of OpenAgents

The OpenAgents ecosystem is grounded in the idea that AI services and agents should not be locked behind closed walls. Instead, these agents should operate using open protocols, enabling a decentralized and user-driven ecosystem for AI discovery, service orchestration, and secure value exchange. At the center are several key protocols:

1. **MCP (Model Context Protocol):**
   A standardized way to integrate and control Large Language Model (LLM) capabilities, tools, and data resources within AI-driven applications.

2. **NIP-89 (Discovery):**
   A mechanism from the Nostr ecosystem for discovering recommended handlers or frontends for specific event kinds, effectively enabling a decentralized “directory” of services.

3. **NIP-90 (Data Vending Machines / DVM):**
   A protocol for open, asynchronous job requests—like posting AI tasks “to the network”—and receiving bids and results from multiple competing service providers.

4. **Bitcoin Micropayments (Lightning Network):**
   A means to instantly compensate services for their valuable contributions without complex intermediaries, ensuring a fluid economic model for AI services.

### Key Benefits

These protocols, when combined, form the backbone of the OpenAgents vision. They ensure that:

- **Users remain in control:**
  They choose which protocols and services to engage with, set budgets, and maintain data privacy.

- **Providers compete on a fair playing field:**
  Anyone can offer services through standardized and open interfaces.

- **Discovery and integration are frictionless:**
  With NIP-89 and MCP, users can easily find, evaluate, and integrate new AI capabilities.

- **Payments align with value:**
  Lightning micropayments ensure that services are rewarded proportionally and instantly for their contributions.

---

## Why Open Protocols?

The dominant AI landscape currently tends toward centralized, proprietary ecosystems. This leads to a handful of large companies controlling the direction, features, and pricing of AI services. In contrast, open protocols:

- **Prevent lock-in:**
  Users can switch providers, combine multiple services, and avoid proprietary formats or walled gardens.

- **Foster competition and innovation:**
  Providers must continuously improve quality and cost-effectiveness, knowing that users can easily discover alternatives.

- **Empower users with choice and privacy:**
  Users remain at the center of decision-making, enabling custom-tailored workflows and respectful data handling.

---

## Model Context Protocol (MCP)

### Overview

The Model Context Protocol standardizes how AI applications interact with Large Language Models, tools, and data resources. It defines a session-based, JSON-RPC2.0-driven communication model between a “host” (e.g., the Onyx app) and “servers” that provide capabilities (like specialized prompts, resource fetchers, or tool APIs).

MCP focuses on **capability negotiation** and **granular user consent**:

- **Capabilities:**
  Servers declare the resources, tools, prompts, and completion services they can provide. Hosts (like Onyx) declare what features they are willing to use.

- **User consent:**
  Every invocation of a tool or resource fetch is subject to user rules and budgets. The user can limit data sharing, confirm actions, and ensure privacy.

### Key Features

1. **Structured Context Integration:**
   MCP ensures that LLM inputs (prompts) can be enriched with external tools and data in a standardized way. For example, Onyx might discover a “transcription” resource server and integrate its transcripts directly into an LLM prompt.

2. **Safe Execution of Tools:**
   Tools (like code execution, calling an external API, or querying a database) are requested by the LLM through MCP. Before execution, the host (Onyx) ensures user consent. This prevents the LLM from taking arbitrary actions without user oversight.

3. **Resource Management:**
   MCP standardizes resource listing, reading, and searching. For instance, Onyx can ask an MCP server for available documents or previously obtained data sets, then feed that into an LLM prompt.

4. **Sampling and Prompts:**
   MCP also standardizes how prompting and model completions occur. Servers declare what prompts they support, and Onyx can request samples from an LLM under user-defined constraints.

### Trust and Isolation

MCP enforces that servers cannot see the entire conversation history or other servers’ data. Each server sees only what the user approves. This ensures privacy and prevents one server from leaking data to another without the user’s knowledge.

---

## NIP-89: Decentralized Discovery of Handlers

### How It Works

NIP-89 introduces a pattern for discovering services (or “handlers”) for specialized event kinds within the Nostr ecosystem. In Nostr, all data is expressed as events of certain “kinds” (think of it as a schema or category).

- **Event Discovery:**
  When Onyx (or any Nostr client) encounters a new event kind it doesn’t understand (like a specialized data type), it can query the network for NIP-89 recommendation events.

- **Recommendation Events:**
  A kind:31989 event may point to one or more kind:31990 events that describe applications (services) capable of handling that event kind.

This “open directory” approach means users can discover MCP servers or DVM providers relevant to their tasks without relying on a centralized directory.

### Integration with MCP

NIP-89 can point Onyx toward MCP-compatible servers. Suppose a user wants a new summarization tool. Onyx can:

1. See a specialized event kind in a conversation.
2. Use NIP-89 to discover which MCP server or web handler is recommended for that kind.
3. Connect to the MCP server found via NIP-89 and negotiate capabilities, fetching data or running tools as needed.

---

## NIP-90: Data Vending Machines (DVMs)

### Core Concepts

NIP-90 defines a model for posting “job requests” to a decentralized network (via Nostr), letting multiple service providers compete to fulfill that request. This is the “Data Vending Machine” concept.

1. **Job Requests (5000-5999 kinds):**
   A user (or Onyx on behalf of a user) posts a job request, specifying what computation they want (like “transcribe this audio file” or “summarize this text”).

2. **Results and Feedback (6000-6999 kinds):**
   Service providers respond with job results and status updates. They can also request payment. Multiple providers can respond, and the user can choose the best result.

3. **Marketplace Dynamics:**
   Instead of a single closed API endpoint, NIP-90 makes the ecosystem a marketplace. Providers bid for attention by offering better quality, lower cost, or faster turnaround times. Users benefit from this competitive environment.

---

## Bitcoin Micropayments: The Economic Engine

Traditional payment methods introduce friction: credit cards, subscriptions, and overhead. Instead, we rely on Bitcoin’s Lightning Network for real-time micropayments:

- **Instant Settlements:**
  Pay as soon as a result arrives—no monthly bills or waiting.

- **Granular Control:**
  Users define budgets and max spend per request. Onyx enforces these rules automatically.

- **Permissionless and Open:**
  Anyone can run a service and get paid instantly. Users can pay small amounts (millisats) cost-effectively.

---

## A Unified Workflow Example

### Scenario: Specialized Summarization

A user, Alice, wants a specialized summarization of a complex topic recently discussed in a niche Nostr event kind (kind:31337). The workflow involves:

1. **Discovery via NIP-89:**
   Alice’s Onyx client encounters a kind:31337 event. Unsure how to handle it, Onyx queries for kind:31989 recommendations and discovers a recommended kind:31990 handler.

2. **MCP Capability Negotiation:**
   Onyx connects to the MCP server recommended by NIP-89. This server declares it can handle text summarization but suggests leveraging a marketplace (NIP-90) for best results.

3. **DVM Job Request (NIP-90):**
   Alice instructs Onyx to summarize the content. The MCP server triggers a DVM job request (kind:5050) onto the Nostr network.

4. **Provider Competition & Payment:**
   Providers respond with summaries (kind:6050). Onyx picks the best and pays via Lightning.

5. **Final Result:**
   The chosen summary is returned to Alice.

---

## Conclusion

The combination of MCP, NIP-89, NIP-90, and Bitcoin Lightning micropayments lays a robust foundation for an open AI ecosystem. By building on open protocols, OpenAgents creates a future where the best AI services are just a discovery query away, users remain in control of their data and budgets, and providers are fairly and instantly compensated for their contributions.
