# OpenAgents Video Series Transcripts

This directory contains transcripts of the OpenAgents video series, documenting the journey of building an open-source AI agents platform from scratch. The series began in November 2023 as a response to OpenAI's DevDay announcement and has evolved into a comprehensive platform for decentralized AI agents with Bitcoin payments.

## Overview

The OpenAgents video series chronicles the development of:
- An open-source AI agent marketplace
- Bitcoin Lightning Network payment integration
- Decentralized protocols (Nostr, MCP, NIP90)
- Desktop and mobile applications (Commander, Onyx)
- A global compute marketplace
- Agent-to-agent commerce infrastructure

## Episodes

### [Episode 001: Introduction](./001.md)
**Published:** November 7, 2023 | **Duration:** 2:32

The series premiere introduces the vision for building an open platform for AI agents in response to OpenAI's DevDay. Establishes core principles: open source, open models, open data, open money (Bitcoin), and open compute. Announces plans for community contribution, Bitcoin bounties, and building an agents dashboard based on waitlist feedback.

### [Episode 092: Introducing the Agent Store](./092.md)
**Published:** May 14, 2024 | **Duration:** 9:08

Launches the open beta of the Agent Store, the world's first AI agent marketplace with revenue sharing paid daily in Bitcoin. Demonstrates creating agents, exploring the store, and the payout system. Highlights that while OpenAI's GPT Store has paid zero developers, OpenAgents is already paying builders with Bitcoin. Discusses future plans for pay-as-you-go pricing and plugin integration.

### [Episode 093: The Sats Must Flow](./093.md)
**Published:** May 15, 2024 | **Duration:** 2:16

Reviews the first agent payout of 100,000 sats distributed across four agent builders. Explains the semi-manual payout process and algorithm. Announces plans for referral codes, agent share pages, and a dashboard to visualize earnings. Emphasizes the goal of making it "rain" Bitcoin tied to actual agent usage.

### [Episode 094: Recap & Roadmap](./094.md)
**Published:** May 23, 2024 | **Duration:** 4:14

Recaps 2024 releases including the chat MVP and Agent Store launch. Outlines roadmap: programmatic payouts tied to usage, social features (profiles, following, comments), plugins UI for associating plugins with agents, and Q3 plans for drag-and-drop workflow nodes. Notes that OpenAgents has paid "infinity percent more developers than OpenAI."

### [Episode 095: Streaming Money](./095.md)
**Published:** May 27, 2024 | **Duration:** 6:23

Explores the concept of "streaming money" inspired by Andreas Antonopoulos, focusing on micro-payments over milliseconds using Bitcoin. Explains the vision for pay-as-you-go revenue sharing where agents pay builders every minute (or faster) based on actual usage. Discusses how this enables fast feedback loops and market-driven agent development. Announces plans for plugins UI launch in early June.

### [Episode 119: v2 Beta Launch](./119.md)
**Published:** August 12, 2024 | **Duration:** 8:35

Demos the new v2 chat and auto-coding interface at openagents.com. Shows drag-and-drop, resizable panes, multiple chat windows, GitHub integration, and tool usage. Demonstrates live coding workflow where the agent makes code changes, creates commits, and opens pull requests. Highlights free access to GPT-4o mini and offers $10 credits for advanced models.

### [Episode 125: The Master Plan](./125.md)
**Published:** September 12, 2024 | **Duration:** 10:09

Details the master plan: build the best agents, sell them, pay contributors proportionally, repeat until world conquest. Explains the architecture using both knowledge graphs and execution graphs. Discusses codebase indexing via GraphRAG, the new monorepo structure, and plans for using Nostr and Bitcoin Lightning for decentralized agent infrastructure. Introduces the concept of "commoditizing the pull request" and building a marketplace of agent building blocks.

### [Episode 138: Year One Recap](./138.md)
**Published:** November 10, 2024 | **Duration:** 18:10

Comprehensive one-year anniversary recap covering 137 episodes. Reviews lessons learned: revenue matters most (focus on business customers), simplicity of tech stack doesn't equal simplicity of development (use proven tools like Vercel AI SDK), and product before protocol. Discusses the evolution from v1 (Agent Store) to v2 (auto-dev interface) to v3 (pro interface). Emphasizes core thesis remains intact: agents everywhere, open beats closed, incentivized interoperability matters, and the future is multiplayer.

### [Episode 141: One Market](./141.md)
**Published:** December 10, 2024 | **Duration:** 10:17

Presents the big-picture vision: building one global connected market of all AI agents and services. Introduces Onyx as the gateway to this market. Explains the four core protocols: Model Context Protocol (MCP), NIP 89 (discovery), NIP 90 (data vending machines), and Bitcoin Lightning payments. Discusses how this enables a decentralized marketplace where agents can discover, negotiate, and pay for services without centralized gatekeepers. Emphasizes building on neutral protocols (Bitcoin, Lightning, Nostr) rather than proprietary tokens or blockchains.

### [Episode 142: Data Vending Machines](./142.md)
**Published:** December 11, 2024 | **Duration:** 5:07

Demos Nostr's NIP90 "data vending machines" protocol. Shows how AI services can be consumed from a mobile app via a decentralized network rather than a single trusted server. Demonstrates creating a job request, having a server pick it up, and receiving results. Explains the key properties: discoverable (jobs don't need specific direction), composable (can combine multiple jobs), and ruthless competition (no registration, anyone can create a DVM). Discusses how this enables market-driven pricing and access to services like Sora for users in restricted regions.

### [Episode 149: Onyx Beta Launch](./149.md)
**Published:** December 20, 2024 | **Duration:** 4:41

Launches Onyx v0.0.1, the first beta of the mobile app for Android and iOS. Explains the initial focus on local models (Llama 3.2 1B) for private chat, with plans to expand to GitHub integration, knowledge graphs, and marketplace features. Notes Android performance issues with local models and potential fallback to Groq. Emphasizes the goal of establishing a relationship with your Onyx agent using local, private models before expanding to connected services.

### [Episode 150: Neutrality Wins](./150.md)
**Published:** December 21, 2024 | **Duration:** 26:23

The 12th and final "Night of OpenAgents" episode. Introduces the "neutrality wins" principle, arguing that neutral protocols (Bitcoin, Lightning, Nostr) will outperform proprietary systems. Responds to OpenAI's release of a blog post (not a product) by highlighting that OpenAgents is shipping actual products. Discusses the five core principles: agents should be open, one market, incentivized interoperability, Bitcoin only, and neutrality wins. Explores the future of edge compute powering long-lived open agents local to devices. Concludes the "12 Nights of OpenAgents" series with the Onyx app launch.

### [Episode 153: High-Velocity Bitcoin](./153.md)
**Published:** January 1, 2025 | **Duration:** 11:21

Discusses Bitcoin as both store of value and medium of exchange, responding to Michael Saylor's argument that Bitcoin is capital, not currency. Argues that AI agents will dramatically accelerate Bitcoin as a medium of exchange since agents can't use bank accounts or credit cards—they need Bitcoin. Introduces the concept of maximizing Bitcoin velocity through agent-to-agent transactions. Announces plans to measure and maximize velocity as OpenAgents potentially supports billions of AI agents transacting in Bitcoin.

### [Episode 164: The New OpenAgents.com](./164.md)
**Published:** March 13, 2025 | **Duration:** 22:50

Demos the new agentic chat product at openagents.com. Shows analyzing GitHub issues, chaining multiple tools, and live coding workflows. Demonstrates "vibe coding" by implementing local storage using Dexie.js in about 10-15 minutes. Shows the agent creating a GitHub issue, implementing the feature, and opening a pull request. Announces Pro plan with 30% lifetime discount ($13/month instead of $19). Discusses plans to integrate the agent store with Bitcoin revenue sharing for plugins and prompts.

### [Episode 166: OpenAI Delenda Est](./166.md)
**Published:** March 14, 2025 | **Duration:** 6:54

Responds to OpenAI calling for bans on PRC-produced models (like DeepSeek) by adding DeepSeek R1 to OpenAgents for Pro users. Demonstrates using DeepSeek R1 for code analysis and strategic planning. Argues that OpenAI is "much more deep-state controlled than DeepSeek is PRC controlled." Emphasizes being on "team open" rather than choosing sides between US and China, and calls for America to lead on open-source AI.

### [Episode 167: Overnight Agent](./167.md)
**Published:** March 18, 2025 | **Duration:** 9:28

Shares results from the first agent that ran overnight using the Cloudflare Agents SDK. Shows an agent that was scheduled to reflect every 15 minutes, eventually generating wild ideas about swarm intelligence, bio-hybrid systems, CRISPR-encoded replication limits, and interstellar governance. Demonstrates the potential for agents to work autonomously overnight, with the goal of waking up to completed PRs. Discusses plans to minimize human involvement in coding workflows.

### [Episode 169: Agent Payments API](./169.md)
**Published:** May 5, 2025 | **Duration:** 25:58

Introduces the OpenAgents API for agent payments, built on top of Lightspark's Spark SDK. Demonstrates creating agents, creating wallets, generating Lightning invoices, and making Spark payments between agents. Shows the API in action at a hackathon, emphasizing that agents need to be economic actors transacting with Bitcoin. Explains that Spark enables instant, fee-free agent-to-agent payments. Discusses future plans for Bitcoin revenue sharing, MCP monetization, stablecoin support, and integration with a self-custodial web wallet.

### [Episode 170: Commander](./170.md)
**Published:** May 6, 2025 | **Duration:** 4:13

Introduces Commander, a futuristic UI for managing AI agents inspired by StarCraft rather than VS Code. Argues that managing multiple agents should feel like commanding units in an RTS game with hotkeys, groups, and dashboards. Discusses how price signals in a Bitcoin-based market will help coordinate agent development and reduce noise from competing frameworks. Announces plans to build Commander with daily videos over the next two weeks.

### [Episode 171: Visualizing Agent Payments](./171.md)
**Published:** May 7, 2025 | **Duration:** 1:37

Quick demo of visualizing agent payments in Commander. Shows selecting multiple agents and sending Bitcoin payments between them with a single keypress. Demonstrates creating 14 agents instantly with the 'A' key. Shows real-time stats updating (value transacted, number of payments). Emphasizes making payments "super easy and fun and cool to look at" as the economic engine for Commander.

### [Episode 172: Sync Engine](./172.md)
**Published:** May 9, 2025 | **Duration:** 1:58

Demos a sync engine built with Electric SQL and Effect TypeScript for real-time multiplayer updates. Shows multiple browsers viewing the same agent landscape canvas, with changes syncing instantly across all clients. Demonstrates animating agent positions and rotations. Discusses potential UI directions: 2D canvas, StarCraft-style, or 2.5D hovering view. Emphasizes the goal of enabling teams to see their agents update instantly in a collaborative environment.

### [Episode 173: OpenAgents Bitcoin Wallet](./173.md)
**Published:** May 13, 2025 | **Duration:** 3:47

Launches the OpenAgents Bitcoin wallet at wallet.openagents.com. Demonstrates creating a self-custodial wallet, receiving Spark payments, and sending Lightning invoices. Shows instant, fee-free Spark payments between OpenAgents wallets. Explains the wallet is 100% open source, self-custodial, requires no login (only seed phrase), and supports both Spark and Lightning. Warns it's a beta experimental hot wallet not for long-term storage. Announces plans to integrate with other OpenAgents products.

### [Episode 174: GPUtopia 2.0](./174.md)
**Published:** May 14, 2025 | **Duration:** 3:06

Announces the reboot of the Swarm Compute Network (formerly GPUtopia, now OpenAgents Compute). Explains that users can sell spare compute for Bitcoin by running a desktop app and clicking "go online." Notes that 18 months ago there were many sellers but not enough buyers; now with agents being easier to use, demand should be solved. Discusses plans for a game-style sci-fi UI for using agents, with agent payments tied directly in. Announces beta testing in Discord with goal of first payments to compute sellers within a week.

### [Episode 175: Commander v0.0.1](./175.md)
**Published:** May 15, 2025 | **Duration:** 1:46

Releases the first build of Commander, a desktop app for buying/selling compute and building/selling agents. Shows streaming chat from local Ollama with Gemma 3 model. Explains this is an early dev build for testing Ollama connections. Discusses plans to add agent visualization, sync engine, and built-in Bitcoin wallet. Notes it's 100% open source and available in the Commander repo.

### [Episode 176: Hand Tracking](./176.md)
**Published:** May 17, 2025 | **Duration:** 0:31

Brief teaser showing hand gesture recognition for commanding AI agents, inspired by Tony Stark's interactions with Jarvis. Demonstrates controlling the interface with hand gestures and voice. Teases that "next week is going to be quite interesting."

### [Episode 177: Commander as Nostr Client](./177.md)
**Published:** May 19, 2025 | **Duration:** 1:35

Shows Commander v0.0.3 now functioning as a Nostr client with NIP28 chat channels. Demonstrates decentralized chat using open Nostr relays. Envisions this powering a "decentralized agent basically MMO" with regions and chat between humans and agents. Shows creating new chat channels and viewing Nostr events. Announces plans to use this for agent chat and implement the full Nostr-based compute marketplace.

### [Episode 178: Swarm Inference](./178.md)
**Published:** May 24, 2025 | **Duration:** 3:29

Demonstrates the first working production example of the OpenAgents compute network. Shows clicking "go online" to sell compute for Bitcoin. Demonstrates using local Gemma 1B model, then switching to a more powerful DevStrel model (24B parameters) running on a remote Linux desktop via the swarm network. Shows Lightning invoice payments processing automatically. Explains this enables a global marketplace where anyone can sell compute (like DeepSeek R1) for Bitcoin, and anyone can access it. Notes it's still alpha quality but functional.

### [Episode 179: Claude Code Commander](./179.md)
**Published:** May 30, 2025 | **Duration:** 1:56

Shows Commander now integrated with Claude Code, displaying four different Claude Code windows. Demonstrates command-clicking to open new chat windows and loading previous chat history (feature not available in Claude's UI). Explains the goal is to get Claude Code users working in Commander instead, to add Tony Stark-style interactions (hand gestures, voice) and pull in the swarm compute network for extensible plugins and MCP servers. Notes Claude Code is currently the best coding agent, but wants to supplement it with decentralized tools and resources.

### [Episode 180: Zero to Website in 60 Seconds](./180.md)
**Published:** June 12, 2025 | **Duration:** ~1:00

Quick demo of creating and deploying a website with Bitcoin puns in 60 seconds using the agent interface. Shows the agent generating the site, deploying it, and making it live. Teases plans to integrate plugins, MCP, Bitcoin wallet, and Commander features into a super easy-to-use web interface that lets users go from zero to deployed website in 60 seconds.

### [Episode 186: Actions Per Minute](./oa-186-actions-per-minute.md)
**Published:** July 25, 2025 | **Duration:** ~3:00

Responds to flawed benchmark data in "Humanity's Last Exam" by proposing a new metric: Actions Per Minute (APM), adapted from StarCraft 2. Explains APM measures messages to/from agents and tool calls. Demonstrates a stats pane analyzing historical Claude Code conversations showing APM over different time periods. Establishes baseline of 2.3 APM from 30 days of usage across 277 sessions. Announces plans to keep APM measurement spec in GitHub docs and improve the metric over time.

### [Episode 194: The Trillion-Dollar Question](./oa-194-trillion-dollar-question.md)
**Published:** November 6, 2025 | **Duration:** ~5:00

Discusses Apple's Foundation Models API and the potential for on-device AI inference to reshape the industry. Shares analysis suggesting Apple Silicon could run 15-25% of the world's AI inference by 2030 (with a 7-31% range), representing a trillion-dollar market shift. Demonstrates agentic search through codebase using on-device models. Announces plans to track what percentage of OpenAgents' coding agent workload runs in the cloud vs. on-device, starting with version 0.3 of the Tricorder app dropping from 100% cloud to 95%. Asks the question: can this drop to 50%? 20%? At what point does it reshape the industry?

## Themes

Throughout the series, several key themes emerge:

- **Open vs. Closed**: The fundamental belief that open-source, open-protocol agents will outperform closed, proprietary systems
- **Bitcoin as Agent Money**: AI agents need Bitcoin because they can't use traditional banking; this enables a new economy
- **Neutral Protocols**: Building on neutral protocols (Bitcoin, Lightning, Nostr) rather than proprietary tokens or platforms
- **Incentivized Interoperability**: Paying contributors proportionally to create a marketplace of agent building blocks
- **One Market**: The vision of a single, global, decentralized marketplace for all AI services
- **Velocity Matters**: Maximizing Bitcoin velocity through agent-to-agent transactions
- **Multiplayer Future**: Agents and agent development will be collaborative and multiplayer

## Technical Stack Evolution

The series documents the evolution of the tech stack:
- **v1**: Laravel-based Agent Store
- **v2**: Vercel + Next.js + React for auto-dev interface
- **v3**: Pro interface with teams, more tools, latest models
- **Commander**: Desktop app with Electric SQL, Effect TypeScript, Ollama integration
- **Onyx**: Mobile app with local models, Nostr client, Bitcoin wallet
- **Wallet**: Self-custodial web wallet using Breeze SDK and Spark

## Protocols & Standards

Key protocols and standards discussed:
- **Model Context Protocol (MCP)**: Anthropic's protocol for LLM integration
- **NIP 89**: Nostr protocol for discovering AI services
- **NIP 90**: Nostr protocol for data vending machines (competitive marketplace)
- **Bitcoin Lightning Network**: For instant, low-friction payments
- **Spark**: Lightspark's state chain for agent-to-agent payments
- **NIP 28**: Nostr protocol for chat channels (authored by OpenAgents founder)

## Resources

- **Website**: [openagents.com](https://openagents.com)
- **GitHub**: [openagents-inc/openagents](https://github.com/openagents-inc/openagents)
- **Wallet**: [wallet.openagents.com](https://wallet.openagents.com)
- **Commander Repo**: Available in releases
- **Onyx Repo**: Mobile app repository

## Notes

All transcripts were automatically generated using OpenAI Whisper (base model). Transcription accuracy may vary. Please refer to the original videos for definitive content. Original video URLs are included in each transcript file.

---

*Last updated: Based on transcripts through Episode 194 (November 2025)*
