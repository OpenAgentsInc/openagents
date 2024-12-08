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

### A Voice-Driven Workflow Example Using NIP-89, NIP-90, and MCP

**Scenario:** Alice uses her Onyx app and issues a voice command:
“Onyx, could you summarize and analyze the sentiment of my last recorded team meeting?”

1. **Voice Input Acquisition:**
   Alice speaks her request, and Onyx captures the raw audio. The recorded team meeting’s audio is stored on her device, but Onyx itself does not transcribe it directly.

2. **Finding a Transcription Service (NIP-89 + MCP):**
   Onyx uses NIP-89 to discover a trusted MCP server that specializes in audio transcription. Drawing on Alice’s follow graph and known reputable services, Onyx identifies a transcription MCP server that can handle long-form audio.

3. **Transcription via MCP:**
   Onyx negotiates capabilities with the transcription server using MCP. The server confirms it can convert the meeting audio into a structured text transcript with speaker labels and timestamps.
   Onyx sends the audio to the server; moments later, it receives a fully transcribed text. The transcription server includes a small Lightning invoice. Onyx automatically pays it, following Alice’s pre-set budget rules.

4. **Summarization and Sentiment Analysis (NIP-89 + MCP):**
   With the transcript ready, Onyx again consults NIP-89 to find two more MCP servers:
   - One MCP server that provides concise, actionable summaries of long texts.
   - Another MCP server that performs fine-grained sentiment analysis, identifying emotional tone across different segments.

   Both servers declare their capabilities via MCP. The summarization server can highlight key points and decisions, while the sentiment server can break down positivity, negativity, and neutrality in the conversation.

5. **Executing Summarization & Sentiment Analysis via MCP:**
   Onyx sends the transcript to the summarization MCP server and receives a well-structured summary. It then sends relevant parts of the transcript to the sentiment analysis MCP server, which returns a detailed sentiment profile.
   Each server includes a Lightning invoice. Onyx pays them automatically, ensuring frictionless, per-use compensation.

6. **Optional Enhanced Results (NIP-90):**
   If Alice wants a broader perspective—perhaps categorizing feedback types or verifying sentiment accuracy—Onyx can issue a NIP-90 job request to a broader marketplace of providers. Multiple services might compete, offering alternative analyses. Alice picks the one she finds most useful, and Onyx pays that provider’s invoice.

7. **User Satisfaction:**
   Within minutes, Alice has a complete transcript, a concise summary of the meeting’s main points, and a nuanced sentiment analysis. She has paid only for what she needed: transcription, summarization, and sentiment evaluation.
   NIP-89 guided Onyx to the right providers, MCP structured the interactions and data exchange, NIP-90 offered optional competitive enhancements, and Bitcoin Lightning payments made the financial transactions seamless. The entire process is open, user-driven, and free from lock-in or unnecessary overhead.

---

### Clarifying the Roles of MCP and NIP-90

The Model Context Protocol (MCP) excels when you have known, trusted servers that provide well-defined capabilities—like transcription, summarization, or sentiment analysis—and you want a stable, ongoing connection to these services. MCP is ideal for structured, direct interactions where you negotiate features, exchange data, and receive outputs from a specific provider you already trust or discover through NIP-89.

NIP-90, on the other hand, shines when you want to open a request to a broader marketplace and let multiple providers compete to fulfill it. This protocol is perfect if:
- You aren’t sure which provider is best.
- You want the flexibility to choose from a variety of responses.
- You’re experimenting with new capabilities that you don’t have a trusted server for yet.

In other words, MCP is about maintaining steady, known relationships with AI service providers, while NIP-90 is about creating an open “call for proposals” that anyone can answer, and then letting you pick the best result.

### A Revised Voice-Driven Workflow Demonstrating When to Use MCP vs. NIP-90

**Scenario:** Alice wants her recorded team meeting transcribed, summarized, and analyzed for sentiment. She trusts a few known MCP servers, but she also wants to explore new angles of analysis from a broader marketplace.

1. **Transcription via MCP (Known Need, Trusted Provider):**
   Alice uses NIP-89 to find a reputable transcription MCP server she’s used before. She requests transcription directly from this server through MCP because she knows its quality. The server returns a transcript promptly, and Onyx pays the invoice. MCP here ensures a smooth, reliable process with a known partner.

2. **Summarization via MCP (Consistent Results from a Known Expert):**
   The summarized insights about her meeting are important, and Alice trusts a specific MCP server recommended by NIP-89 for summarization tasks. She gets a clean, structured summary from a known, reliable source. The exchange is direct, stable, and predictable—perfect for MCP.

3. **Sentiment Analysis via MCP (Trusted Provider for a Core Service):**
   Alice also has a go-to sentiment analysis server discovered through NIP-89. She knows it consistently provides accurate tonal breakdowns. Using MCP, she sends the summary (or transcript excerpts) to this server. It responds with a nuanced sentiment profile. Onyx pays the invoice. Again, MCP is the natural choice for a known service with predictable output and quality.

4. **Exploration of New Angles via NIP-90 (Marketplace for Additional Perspectives):**
   After reviewing the results, Alice decides she wants a more experimental approach—maybe a provider that can categorize the feedback by department or topic, something neither of her known MCP servers offers. Instead of guessing who can provide it, she broadcasts a job request using NIP-90.

   With NIP-90, multiple providers from the open marketplace see the job request and submit their proposals. Some might offer advanced clustering of comments, others might visualize sentiment trends. Alice can review several returned results and pick the best one. NIP-90 is ideal here because Alice is exploring new capabilities without a known provider. It’s a one-off, open “call for proposals” rather than a stable, known relationship.

### Summary

- **Use MCP:**
  When you know the service you want, trust its quality, and want a stable, direct interaction. MCP helps you integrate that service’s capabilities seamlessly into your workflow, with clear negotiation of features and direct payment for value received.

- **Use NIP-90:**
  When you’re looking to experiment, discover new providers, or gather multiple competing solutions to a problem. NIP-90 creates a marketplace scenario, letting multiple providers offer their results so you can pick the one that best meets your needs.

Combining both approaches, Onyx empowers users to rely on known, trusted MCP services for their core tasks while occasionally tapping into NIP-90’s open marketplace for innovation and fresh perspectives.

### Clarifying Payment Flows for MCP vs. NIP-90

It’s true that both MCP and NIP-90 scenarios involve compensating service providers, and payment via Bitcoin’s Lightning Network can integrate seamlessly into either approach. The difference isn’t that NIP-90 is required for payment—it’s that NIP-90 provides a structured marketplace framework where requests and payment suggestions are embedded in public, discoverable events. MCP, on the other hand, focuses on direct, session-based capability negotiation and tool usage without mandating how payment is handled.

**Key Points:**

1. **Paying MCP Servers Directly:**
   MCP servers can present their own payment requirements (such as providing a Lightning invoice) as part of their response when fulfilling a request. For instance, after transcribing audio or providing a summary, the MCP server can return:
   - The requested data (transcript, summary, analysis).
   - A Lightning invoice (e.g., a `bolt11` string) for the service rendered.

   Onyx, the host application, can then automatically pay the invoice from the user’s allocated budget. This payment step doesn’t depend on NIP-90—it’s a separate concern. The Onyx client includes built-in logic to handle instant micropayments directly to the MCP server based on user-defined rules.

2. **NIP-90 as a Marketplace Layer:**
   NIP-90 events combine job requests, responses, and payment hints directly on the Nostr network. This is beneficial when:
   - You’re not dealing with a known MCP server you trust.
   - You want multiple providers to respond, letting you choose the best result.
   - You value the open, competitive environment where providers bid and respond publicly.

   In such cases, NIP-90 includes standardized tags (like `amount` or a `bolt11` invoice) to streamline the payment process. The Onyx client sees these tags when providers respond, chooses the best result, and pays the corresponding invoice. Here, payment is part of a public, competitive workflow.

3. **Integration Flexibility:**
   You don’t have to choose either MCP or NIP-90 strictly for payment. Payment in both scenarios uses the same underlying Lightning mechanism within Onyx:
   - **MCP scenario:** The server you connected to and trust sends you an invoice directly after delivering its result. You pay it immediately, no public bidding or multiple responses required.
   - **NIP-90 scenario:** You broadcast a job request to a public marketplace and get multiple results. You pick the best one and pay that provider’s invoice. NIP-90 makes it natural to handle payments this way because the protocol defines how to represent job requests, results, and payment expectations in a single open forum.

**In Summary:**
MCP doesn’t prevent you from paying providers. It simply focuses on a direct, negotiated relationship between a client (Onyx) and a known server. You can still receive and pay invoices as part of that interaction. NIP-90 formalizes a marketplace approach, making job requests public and letting multiple providers compete, with payment integrated into the public event structure. Both approaches rely on Onyx’s built-in Lightning integration to ensure that payment is instant, effortless, and user-controlled.

## Conclusion

The combination of MCP, NIP-89, NIP-90, and Bitcoin Lightning micropayments lays a robust foundation for an open AI ecosystem. By building on open protocols, OpenAgents creates a future where the best AI services are just a discovery query away, users remain in control of their data and budgets, and providers are fairly and instantly compensated for their contributions.
