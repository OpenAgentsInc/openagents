## Protocols at the Heart of OpenAgents

The OpenAgents ecosystem is founded on the principle that AI services and agents should operate on open protocols rather than being siloed behind closed platforms. By embracing open standards, the ecosystem fosters a decentralized, user-driven environment for discovering AI capabilities, orchestrating services, and securely exchanging value. Four key protocols serve as the cornerstone of this vision:

1. **MCP (Model Context Protocol):**
   A standardized way to integrate and control Large Language Model (LLM) capabilities, tools, and data resources. MCP focuses on direct, trust-based client-server relationships, detailed feature negotiation, and user consent for every action.

2. **NIP-89 (Discovery):**
   A decentralized method, based on Nostr, for discovering recommended AI services and handlers for specific event kinds. Instead of one centralized directory, users leverage their social graph and follow relationships to find reputable providers—like transcription services, summarizers, or sentiment analyzers—through community-driven recommendations.

3. **NIP-90 (Data Vending Machines / DVM):**
   A protocol that turns the ecosystem into a competitive open marketplace. Users post AI tasks to the network as job requests, and multiple service providers respond with results. This competition ensures better quality, variety, and pricing, allowing the user to pick the best outcome from a range of public offers.

4. **Bitcoin Micropayments (Lightning Network):**
   A means to handle instant, low-friction payments. Instead of subscription models or centralized billing, users pay per task using Bitcoin’s Lightning Network. This aligns payments directly with the value delivered, ensuring providers are rewarded proportionally and instantly.

---

### Key Benefits

When these protocols work together, they create an AI ecosystem that:

- **Keeps Users in Control:**
  Users choose which protocols and services to engage with. They set budgets, control data sharing, and ensure that every action by the AI is consented to.

- **Ensures a Fair Playing Field for Providers:**
  Anyone can offer services. Providers must continuously improve to earn users’ trust and payment, leading to a healthier, more innovative market.

- **Simplifies Discovery and Integration:**
  Through NIP-89 and MCP, users easily find and integrate AI tools without centralized gatekeepers. The process is transparent and flexible.

- **Aligns Payments with Value:**
  Lightning micropayments let users pay only for the specific services they need, exactly when they need them. No large upfront costs or lock-ins—just pay-as-you-go simplicity.

---

## Why Open Protocols?

The current AI landscape is often dominated by a few large entities controlling pricing, features, and data handling. Open protocols offer a viable alternative:

- **Prevent Lock-In:**
  Users can seamlessly switch between providers, combine multiple services, and avoid proprietary constraints.

- **Foster Competition and Innovation:**
  With open access, new providers can enter the market, forcing all to offer better quality and cost-effectiveness.

- **Empower User Choice and Privacy:**
  Users, not platforms, decide which data to share, which services to trust, and how much to spend. This leads to a respectful, user-centric environment.

---

## Model Context Protocol (MCP)

### Overview

MCP is a session-based, JSON-RPC2.0-driven protocol that defines how a “host” application (like Onyx) interacts with “servers” providing AI capabilities (e.g., transcription, summarization, sentiment analysis). It’s about stable, direct relationships with known providers:

- **Capability Negotiation:**
  MCP servers declare their capabilities (tools, prompts, resources, model completions). The host (Onyx) states which features it wants to use, ensuring transparency and alignment.

- **User Consent and Security:**
  Every requested action—fetching a resource, invoking a tool, generating a completion—requires user-defined rules and confirmation. Users decide what the AI can see or do, protecting privacy and preventing unintended operations.

### Key Features

1. **Structured Context Integration:**
   LLM prompts can be enriched with external data and tools in a standardized manner. For example, if Onyx wants to summarize a transcript, it can request the transcript from a known MCP server and feed it directly into an LLM prompt.

2. **Safe Tool Execution:**
   Tools requested by the LLM do not run automatically. The host (Onyx) mediates, ensuring actions align with user consent. This prevents AI from triggering unwanted APIs or operations.

3. **Resource Management:**
   MCP provides a uniform way to list, read, or search for data. If Onyx needs documents or previously generated content, it can easily request them from an MCP server, subject to user approval.

4. **Sampling and Prompt Customization:**
   MCP defines how prompts and LLM completions occur. Servers can provide ready-made prompt templates, and Onyx can request model completions under user-defined parameters.

### Trust and Isolation

MCP ensures strict boundaries. A server only sees what the user explicitly shares. One MCP server cannot snoop on another’s data. This isolation maintains privacy and trust, preventing data leakage across different services.

---

## NIP-89: Decentralized Discovery of Handlers

### How It Works

NIP-89 leverages the Nostr network to help users find the right MCP servers or frontends for specific tasks. Instead of a single registry, users rely on recommendations from their network (people they follow or trust):

- By querying NIP-89 “recommendation events,” Onyx can find which MCP servers are best for certain tasks, like transcription or summarization.
- This decentralized directory means no single authority decides which services you should use. Trust is built organically, through community endorsements.

### Integration with MCP

NIP-89 points Onyx towards reliable MCP servers. Once Onyx knows a certain server is recommended for summarization, it can connect directly via MCP, negotiate capabilities, and pay for the service. NIP-89 thus streamlines the discovery process, ensuring Onyx finds quality providers quickly.

---

## NIP-90: Data Vending Machines (DVMs)

### Core Concepts

NIP-90 transforms the ecosystem into an open marketplace for AI tasks. Instead of going to a known server, you can:

1. **Post a Job Request (5000-5999 kinds):**
   Describe the task—transcription, summarization, specialized analysis—and broadcast it to the network.

2. **Receive Multiple Offers (6000-6999 kinds):**
   Service providers respond with their results. Multiple providers can compete, each offering potentially different quality, cost, and turnaround times.

3. **Choose the Best Result:**
   The user reviews responses and picks the most suitable one, paying only for the chosen result.

This marketplace dynamic encourages innovation and better pricing, as providers must continuously improve to stand out.

---

## Bitcoin Micropayments (Lightning Network)

By integrating Bitcoin’s Lightning Network, both MCP and NIP-90 scenarios benefit from instant, cost-effective micropayments:

- **Instant Settlements:**
  Pay as soon as the service is delivered. No monthly subscription overhead.

- **Granular Budgets:**
  Users can set spending caps. For each request, Onyx pays only what’s needed, maintaining full financial control.

- **Permissionless and Global:**
  Anyone can provide a service and get paid instantly. This open economic layer encourages global participation and a more diverse AI marketplace.

---

## A Voice-Driven Workflow Example

**Scenario:** Alice wants to process her team meeting recording—transcribe it, summarize the key points, and analyze the sentiment—using her Onyx app and these open protocols.

1. **Voice Command:**
   Alice says: “Onyx, please summarize and analyze the sentiment of my last recorded team meeting.”

2. **Transcription via MCP (Discovered via NIP-89):**
   Onyx uses NIP-89 to find a recommended transcription MCP server. Since Alice trusts this recommendation, Onyx connects to the MCP server and negotiates transcription capabilities.
   The server transcribes the audio and returns a text transcript, along with a Lightning invoice. Onyx automatically pays the invoice from Alice’s budget.

3. **Summarization and Sentiment Analysis via MCP (Using NIP-89):**
   With a transcript ready, Onyx again queries NIP-89 for summarization and sentiment analysis servers. It finds two MCP servers:

   - Summarization server: Produces a concise, actionable summary of the meeting.
   - Sentiment analysis server: Provides a tone map, highlighting where the conversation was positive, neutral, or negative.

   Onyx negotiates capabilities with each server via MCP, sends them the transcript (or excerpts), and receives results. Each server includes a Lightning invoice, which Onyx pays instantly.

4. **Optional Enhanced Perspectives via NIP-90:**
   Suppose Alice wants more experimental insights—like categorizing feedback by department or verifying sentiment accuracy with alternative approaches. Onyx can post a NIP-90 job request to a broader marketplace. Multiple providers respond publicly with their proposals. Alice reviews these options, picks the best one, and Onyx pays that provider’s invoice. NIP-90 ensures competition and fresh ideas without requiring pre-established trust.

5. **User Satisfaction:**
   Alice now has a transcript, a neat summary of action items, and a detailed sentiment profile—all found via NIP-89, executed through MCP, optionally enhanced by NIP-90, and paid via Lightning. She has spent only on what she needed, avoided vendor lock-in, and enjoyed a seamless user experience.

---

## When to Use MCP vs. NIP-90

- **MCP:**
  Perfect for stable, ongoing relationships with known servers. If you already trust a provider, MCP ensures a smooth, session-based interaction, direct feature negotiation, and immediate payment upon receiving results.

- **NIP-90:**
  Ideal for exploring new capabilities or getting competitive bids when you don’t have a trusted go-to provider. It’s a public marketplace approach—post a request, review multiple responses, pick the winner, and pay them.

Combining both, you rely on MCP for core, trusted services while occasionally tapping into NIP-90’s open marketplace for fresh perspectives or specialized tasks.

---

## Integrating Payment Requirements into MCP Workflows

MCP itself does not include a payment specification, as it focuses on describing capabilities, orchestrating tool use, and managing data exchange. However, using lessons from NIP-90’s marketplace model, we can adapt the payment flow so that providers can require payment before delivering results—unless they explicitly choose to offer certain services for free.

**Key Principle: Payment Before Results (Unless Free):**
In this adapted model, when an MCP server receives a request (e.g., a transcription job), it has two options:

1. **Offer the Service for Free:**
   The server can simply return the requested data or result at no cost. This might be done as a trial, a courtesy, or to demonstrate quality before the user commits to further paid interactions.

2. **Require Payment Upfront:**
   For services that aren’t free, the server can withhold final results until payment is made. Rather than returning a completed result and invoice simultaneously, the interaction goes like this:
   - The server indicates the cost of fulfilling the request before sending the final output.
   - Onyx receives this cost signal in a standardized format (inspired by NIP-90’s tag structure), including a Lightning invoice.
   - Onyx automatically checks user-configured budgets and rules. If the user approves, Onyx immediately pays the invoice over Lightning.
   - Once the payment is confirmed, the server releases the final results to Onyx.

**How This Adapts NIP-90’s Concepts:**

- **NIP-90’s Payment Signaling:**
  In NIP-90 job requests, service providers can send status or partial results and indicate that payment is required before further action. We adopt a similar pattern here:

  - Before delivering the full, final MCP result, the server sends a message indicating “payment-required” status.
  - Onyx pays the invoice, then the server proceeds to deliver the complete data.

- **Ensuring Fairness and Trust:**
  Users maintain control by setting maximum spend limits and deciding whether to pay for a given service. If Onyx receives a demand for payment that doesn’t align with user budgets or trust, it can decline, and no result is delivered.

**Example Flow:**

1. **Request:**
   Onyx, following user instructions, asks an MCP transcription server to process a long audio file.

2. **Payment Required Signal:**
   The server checks the request, calculates a price, and responds with a payment request but no final transcription yet. It may provide a brief “processing” notice or a small sample snippet as a teaser if allowed by the user’s rules.

3. **User Approval & Payment:**
   Onyx shows the cost to the user (or automatically uses pre-set limits), and if approved, Onyx pays the provided Lightning invoice.

4. **Result Delivery:**
   After the payment is confirmed, the server returns the full transcription. The user gets what they paid for, and the provider is compensated immediately.

5. **Optional Marketplace via NIP-90:**
   If Onyx uses NIP-90, multiple providers can respond to a public job request with their price and potential partial previews. Onyx (or the user) selects the best offer, pays first, and then receives the full results.

**In Summary:**

- MCP doesn’t define payments, but we can incorporate a payment-required step inspired by NIP-90’s logic.
- The server can demand payment upfront, ensuring that no full results are given without compensation.
- This approach respects user budgets, requires explicit consent for spending, and maintains a fair balance between providers and users.

---

## Conclusion

By leveraging MCP, NIP-89, NIP-90, and Bitcoin Lightning micropayments, OpenAgents envisions an open, user-driven AI ecosystem that:

- Empowers users to choose providers and control their data.
- Encourages healthy competition and continuous improvement.
- Supports flexible integration of known, trusted services alongside open-market exploration.
- Aligns costs directly with delivered value, ensuring fairness and sustainability.

This approach respects user privacy and autonomy, discourages monopolies, and nurtures a vibrant environment for AI innovation and growth.
