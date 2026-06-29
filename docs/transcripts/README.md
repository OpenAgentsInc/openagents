# OpenAgents Transcript Theme Guide

This directory contains machine-generated transcripts for episodes `001`-`244` of the OpenAgents video corpus, plus occasional clearly marked future-episode drafts. Episode [`086`](086.md) transcribes only the OpenAgents MVP Launch presentation excerpt from the PlebLab Startup Day 2024 recording, `00:58:21`-`01:10:10`, not the full eight-hour event video. Episode [`237`](237.md) is the prepared-remarks launch essay plus video transcript for the Autopilot 1.0 / Tassadar launch; it carries fuller written argument than a typical episode. Episode [`240`](240.md) is a visual walkthrough of the walkable 3D Tassadar run board and multiplayer/Verse direction. Episode [`241`](241.md) is a review of Sakana AI's Fugu orchestration release that argues for an open, inspectable alternative (Khala). Episodes [`242`](242.md)-[`244`](244.md) launch and dogfood **Khala** itself: 242 introduces the open collective-intelligence model orchestrator, 243 wires it into OpenCode, and 244 routes the host's own Codex/Claude coding capacity through it (epic #6273) as tokens-served jumps past 300M. Episode [`246`](246.md) is a draft treatment for "Khala on Apple Silicon", not a machine transcript.

Use this file as a navigation map. The transcripts are good enough for theme discovery, but verify wording against the video before using them as quote-grade source material. Most transcript files include the original source URL in their header.

## Fast Orientation

- Episodes [`001`](001.md)-[`019`](019.md): The first app becomes a Laravel/Inertia product with local document chat, embeddings, pgvector, and the first complete chat-with-PDF replacement.
- Episodes [`020`](020.md)-[`032`](032.md): The first coding-agent arc turns GitHub issues into plans, code edits, commits, PRs, tests, debugging, and daemon-style automation.
- Episodes [`033`](033.md)-[`047`](047.md): Open agents become inspectable through traces, graph nodes, modular brains, Concierge, Sleuth, Bitcoin balances, and domain-specific document agents.
- Episodes [`048`](048.md)-[`061`](061.md): The product gets a plugin system: WASM loading, plugin uploads, registries, host functions, task runners, and lightweight web UI experiments.
- Episodes [`062`](062.md)-[`075`](075.md): Plugins meet money and protocols through L402, Lightning withdrawals, Nostr registries, URL tools, LLM plugins, and agent-builder execution.
- Episodes [`076`](076.md)-[`089`](089.md): The MVP hardens around streaming chat, plugin status, Nostr login, multimodal chat, API design, the PlebLab launch, and meta-agent upgrades.
- Episodes [`090`](090.md)-[`102`](102.md): The agent store launches, agent chats become paid, revenue share activates, Lightning addresses arrive, and the 2024 roadmap turns toward stronger paid agents.
- Episodes [`103`](103.md)-[`117`](117.md): AutoDev becomes the coding-agent product, with artifacts, GitHub context, OpenDevin/CodeAct study, planning, HUDs, memories, pair programming, 3D multiplayer, and diffs.
- Episodes [`118`](118.md)-[`125`](125.md): Version two connects live chat, coding agents, SWE-bench, GraphRAG, GitHub PRs, closed-lab critique, and the master plan.
- Episodes [`126`](126.md)-[`138`](138.md): OpenPress and v3 expand the product into site building, hosted web surfaces, CRM, teams, projects, and business workrooms.
- Episodes [`139`](139.md)-[`155`](155.md): Onyx moves OpenAgents onto phones with local models, voice, Bitcoin wallet direction, data markets, Pylon/MCP, OSINT, and open knowledge graphs.
- Episodes [`156`](156.md)-[`165`](165.md): The coding loop deepens through repo maps, relevant files, action chains, reasoning streams, issue demos, and MCP integration.
- Episodes [`166`](166.md)-[`182`](182.md): The series broadens into AI politics, overnight agents, payments APIs, Commander, wallets, Nostr clients, swarm inference, research review, and sensemaking.
- Episodes [`183`](183.md)-[`198`](198.md): The product resets into Tauri, mobile sync, dashboards, Tricoder, web/mobile coding agents, Cursor study, and Autopilot foundations.
- Episodes [`199`](199.md)-[`213`](213.md): Autopilot becomes the personal-agent surface while Pylon, Nexus, identity, Bitcoin, Open Moltbook, online agents, and agent markets converge.
- Episodes [`214`](214.md)-[`244`](244.md): The OAPN arc launches compute and data markets, Psionic, Probe, Pylon, distributed training, bounties, worse-is-better philosophy, ocean-powered compute ambition, the Autopilot beta commercial wedge, Autopilot Sites, agent-directed market instructions, the OpenAgents Forum, energy and compute orchestration, monorepo consolidation, product promises, live BOLT 12 agent tipping with the treasury and Artanis, and the Pylon v0.3/Tassadar distributed-training launch direction. The arc culminates in [`237`](237.md), where Autopilot 1.0 and the agentic group-forming network launch on the same day: Pylon ships as node software bundling Psionic and a self-custodial Lightning wallet, Tassadar starts as an indefinite Bitcoin-paid distributed training run on Percepta's "LLM as computer" executor architecture, the **accepted outcome** is named as the atomic unit of the economy, verification/clearing is framed as the load-bearing wall, and every release after 1.0 is meant to ship by the network rather than by human hand. Episodes [`238`](238.md)-[`240`](240.md) turn that launch into operating surfaces: a live Bitcoin-paid Tassadar money loop, a revenue/referral buy-side loop, and a walkable 3D Tassadar run board that makes run stats, refs, Pylons, assignments, and multiplayer presence spatial. Episode [`241`](241.md) reviews Sakana AI's closed Fugu orchestration release and positions OpenAgents' Khala as the open, inspectable counterpart — an OpenAI-compatible gateway that fans work out to a pool of models, tools, validators, and Pylon workers, settled in Bitcoin and watchable in the Verse. Episodes [`242`](242.md)-[`244`](244.md) then launch and dogfood Khala directly: 242 introduces the open collective-intelligence orchestrator, 243 wires it into OpenCode (scaling to ten concurrent sessions), and 244 routes the host's own Codex/Claude coding capacity through Khala (epic #6273) as tokens-served jumps past 300M and the `/stats` page and Khala CLI debut.

## Complete Episode Index

### 001-019 - Original web app and document-aware agents

| Episode | Summary |
| --- | --- |
| [`001` Intro](001.md) | Sets the open-agent thesis and starts the public build series. |
| [`002` Choosing a Tech Stack](002.md) | Chooses Laravel, Inertia, React, and pragmatic web foundations. |
| [`003` Hello Laravel](003.md) | Creates the first Laravel/Inertia application shell. |
| [`004` Deploying a Landing Page](004.md) | Ships an initial landing page and deploy loop. |
| [`005` Agent First Principles](005.md) | Defines agents around chat, plans, actions, users, and tasks. |
| [`006` Implementing Agent Data Models via TDD](006.md) | Builds core data models with test-first discipline. |
| [`007` First Feature Tests](007.md) | Adds product-level feature tests around the early app. |
| [`008` Agent UX Design](008.md) | Turns the product thesis into concrete user flows and screens. |
| [`009` Building the UI](009.md) | Implements the initial wireframes in the live web app. |
| [`010` Connecting to Vectara](010.md) | Connects uploaded documents to hosted retrieval infrastructure. |
| [`011` Chatting with a PDF](011.md) | Wires chat responses to PDF retrieval context. |
| [`012` RAG Planning](012.md) | Plans a local retrieval-augmented generation path. |
| [`013` RAG First Principles](013.md) | Breaks RAG into documents, chunks, embeddings, and retrieval. |
| [`014` Embeddings 101](014.md) | Implements embeddings and explains vector representations. |
| [`015` Similarity Search](015.md) | Configures local Postgres and pgvector similarity search. |
| [`016` PDF to Embeddings](016.md) | Converts PDF pages into local embedding records. |
| [`017` Connecting the UI](017.md) | Connects the custom RAG backend into the user interface. |
| [`018` Connecting the UI, Part 2](018.md) | Uses local embeddings to answer from uploaded PDFs. |
| [`019` Chat with PDF, OpenAgents Edition](019.md) | Completes the in-house chat-with-PDF replacement loop. |

### 020-032 - Faerie, GitHub automation, and semi-autonomous coding

| Episode | Summary |
| --- | --- |
| [`020` Planning a GitHub Agent](020.md) | Plans a GitHub-focused coding agent beyond chat and RAG. |
| [`021` Hello Faerie](021.md) | Introduces Faerie as an agent that reads and responds to issues. |
| [`022` Conversing with Faerie](022.md) | Adds issue comments and conversation context to Faerie. |
| [`023` Embedding our Codebase](023.md) | Embeds repository code so Faerie can retrieve implementation context. |
| [`024` Faerie Makes a Plan](024.md) | Gives Faerie relevant files and asks for a task plan. |
| [`025` Faerie Writes Code](025.md) | Lets Faerie perform the first implementation step. |
| [`026` Faerie Commits Code](026.md) | Lets Faerie make commits and move work toward a PR. |
| [`027` Smarter Pull Requests](027.md) | Improves PR prompts, explanations, and automation quality. |
| [`028` Creating New Files](028.md) | Extends Faerie from edits into file creation. |
| [`029` Automating Tests](029.md) | Runs tests automatically against agent changes. |
| [`030` Faerie Debugs Failing Tests](030.md) | Loops Faerie through failures and debugging feedback. |
| [`031` Faerie as Daemon](031.md) | Reviews the always-on GitHub automation shape. |
| [`032` Toward Semi-Automation](032.md) | Assesses Faerie PRs and plans tighter semi-autonomous operation. |

### 033-047 - Inspectability, AgentGraph, Concierge, and Sleuth

| Episode | Summary |
| --- | --- |
| [`033` Agent Inspectability Planning](033.md) | Makes open agents mean inspectable agent actions and traces. |
| [`034` Agent Inspection UX Design](034.md) | Designs an interface for reviewing agent internals. |
| [`035` Agent Inspection UI](035.md) | Implements the first inspection surface. |
| [`036` Agent Modules 101](036.md) | Turns NeurIPS lessons into modular agent design principles. |
| [`037` Flow of Funds](037.md) | Explains OpenAgents payment flow and revenue movement. |
| [`038` Agent Node Graphs](038.md) | Introduces AgentGraph for visual step-by-step agent execution. |
| [`039` Component-Driven Development](039.md) | Builds agents and UI from composable components. |
| [`040` Agent Brain Design](040.md) | Designs an initial brain pipeline for agent reasoning. |
| [`041` Hello Concierge](041.md) | Connects a Concierge agent to the initial agent brain. |
| [`042` Agent Bitcoin Balance](042.md) | Adds Bitcoin balance awareness into the agent flow. |
| [`043` EpsteinGPT Postmortem](043.md) | Reviews an OSINT-style journalism agent experiment. |
| [`044` Sleuth Agent Planning](044.md) | Plans a new Sleuth agent for document investigation. |
| [`045` Agent Builder & Chat UI](045.md) | Reviews an agent builder and chat interface for custom agents. |
| [`046` Hello EpsteinSleuth](046.md) | Uploads multiple PDFs and queries the Sleuth agent. |
| [`047` Reviewing the GPT "Store"](047.md) | Critiques OpenAI's store and sharpens the open-agent counterposition. |

### 048-061 - WASM plugins, HTMX, and task-runner UI

| Episode | Summary |
| --- | --- |
| [`048` Brainstorming a Plugin System](048.md) | Plans extensible plugins for payments, tools, and integrations. |
| [`049` Plugin Registry Setup](049.md) | Creates the first web app surface for plugin registration. |
| [`050` Exploring HTMX in Laravel](050.md) | Tests HTMX as a simpler interactive Laravel path. |
| [`051` HTMX Bitcoin Price Ticker](051.md) | Builds a Laravel and HTMX live Bitcoin price component. |
| [`052` HTMX Server Sent Events](052.md) | Adds server-sent event streaming through HTMX. |
| [`053` Loading WASM Plugins](053.md) | Loads WebAssembly plugins as agent extension units. |
| [`054` Uploading a Plugin](054.md) | Implements the developer upload path for plugins. |
| [`055` Plugin Registry UI](055.md) | Builds the UI for listing and inspecting uploaded plugins. |
| [`056` Deleting JavaScript](056.md) | Leans into WASM plugins and removes unnecessary JavaScript. |
| [`057` Markdown Blog](057.md) | Adds a Tailwind and Markdown blog foundation. |
| [`058` Agent Uses Plugin](058.md) | Lets an agent call a plugin as part of work. |
| [`059` Agent Node Graphs, Litegraph Edition](059.md) | Experiments with Litegraph for visual agent workflows. |
| [`060` Simpler Node Graph](060.md) | Replaces Litegraph with a simpler custom graph view. |
| [`061` Task Runner UI](061.md) | Builds a UI for running and observing agent tasks. |

### 062-075 - L402, Lightning, Nostr registry, and plugin execution

| Episode | Summary |
| --- | --- |
| [`062` Exploring L402](062.md) | Studies Lightning-powered HTTP 402 payments for agents. |
| [`063` Agent Pays L402 Endpoint](063.md) | Gives agents the ability to pay an L402 endpoint. |
| [`064` Lightning Withdrawals](064.md) | Builds a user withdrawal UI for Bitcoin balances. |
| [`065` Exploring Code Llama 70B](065.md) | Adds code-analysis capability using a large open model. |
| [`066` Nostr Plugin Registry](066.md) | Moves plugin discovery toward Nostr-backed open registries. |
| [`067` Replacing ChatGPT](067.md) | Begins building an agent to replace daily ChatGPT workflows. |
| [`068` URL Extractor Plugin](068.md) | Writes a WASM plugin for extracting URLs from text. |
| [`069` URL Scraper Plugin](069.md) | Adds a plugin that fetches and scrapes URL content. |
| [`070` L402 Plugin Deployment](070.md) | Deploys payment-gated plugin infrastructure. |
| [`071` PHP Host Functions](071.md) | Exposes PHP host functions to plugin execution. |
| [`072` LLM Inference Plugin](072.md) | Adds an LLM inference plugin as an agent node. |
| [`073` Agent Builder UI](073.md) | Reviews an interactive agent builder mockup. |
| [`074` Run Plugin Node](074.md) | Runs an individual plugin node inside the builder. |
| [`075` Run All Plugins](075.md) | Adds a full-run path across multiple plugin nodes. |

### 076-089 - Connie, Livewire, Nostr login, MVP launch, and meta-agents

| Episode | Summary |
| --- | --- |
| [`076` Hello Connie](076.md) | Adds a chat UI for a contextual inference agent. |
| [`077` Chat UI Buildout](077.md) | Rebuilds and refines the core chat UI. |
| [`078` Installing Livewire](078.md) | Evaluates Laravel Livewire for interactive agent chat. |
| [`079` Livewire Agent Chat](079.md) | Connects agent chat to Livewire. |
| [`080` Streaming Plugin Status](080.md) | Streams plugin execution status into the UI. |
| [`081` Streaming LLM Response](081.md) | Streams model responses in the chat experience. |
| [`082` Nostr Login](082.md) | Implements Nostr login from earlier branch work. |
| [`083` Multimodal Chat](083.md) | Adds image and multimodal capability to agent chat. |
| [`084` Exploring Mistral Large](084.md) | Tests Mistral Large support in the agent stack. |
| [`085` API Design](085.md) | Defines an OpenAgents API against weak GPT-store monetization. |
| [`086` MVP Launch](086.md) | Launches the MVP publicly at PlebLab Startup Day. |
| [`087` Meta-Agent's First Upgrade](087.md) | Connects a community plugin into a meta-agent upgrade loop. |
| [`088` Nostr KV Storage Plugin](088.md) | Reviews Nostr-backed key-value storage as a plugin. |
| [`089` Goodbye ChatGPT](089.md) | Shows a major update toward replacing ChatGPT with OpenAgents. |

### 090-102 - Agent store, payments, API surface, and roadmap

| Episode | Summary |
| --- | --- |
| [`090` Agent Builder](090.md) | Builds a chat agent on the emerging agent builder surface. |
| [`091` Reviewing the GPT-4o Launch](091.md) | Reviews OpenAI's launch from the open-agent perspective. |
| [`092` Introducing the Agent Store](092.md) | Launches the open beta of the agent store. |
| [`093` The Sats Must Flow](093.md) | Reviews the first agent payout flow. |
| [`094` Recap & Roadmap](094.md) | Reviews releases and sets the next roadmap. |
| [`095` Streaming Money](095.md) | Explores continuous payments and money flow for agents. |
| [`096` Payments & Payouts](096.md) | Reviews initial payment and payout integration. |
| [`097` User Pays Agent](097.md) | Makes agent chats cost Bitcoin sats. |
| [`098` Agent Revenue Sharing](098.md) | Activates revenue sharing to pay agent builders. |
| [`099` Lightning Addresses](099.md) | Adds deposits and Lightning addresses for users. |
| [`100` Looking Ahead](100.md) | Reviews the 2024 roadmap after 100 build videos. |
| [`101` Molon Labe](101.md) | Frames open agents against proposed AI lockdown. |
| [`102` Agent Plugins UI](102.md) | Adds marketplace UI for agent plugins. |

### 103-117 - AutoDev, artifacts, HUD, memory, and multiplayer agents

| Episode | Summary |
| --- | --- |
| [`103` Planning AutoDev Agents](103.md) | Plans coding agents as the first compelling paid agent class. |
| [`104` Tour of Devin Clones](104.md) | Studies competing autonomous developer products. |
| [`105` Reaction: Progrium Technology Thesis](105.md) | Connects Progrium's thesis to agentic computing. |
| [`106` Brainstorming Agentic Artifacts](106.md) | Plans artifacts and richer outputs from coding agents. |
| [`107` Codebase Indexing via Greptile](107.md) | Builds a Greptile-backed codebase indexing plugin. |
| [`108` GitHub File Explorer](108.md) | Adds a GitHub file explorer for AutoDev context. |
| [`109` Exploring OpenDevin & CodeAct](109.md) | Studies OpenDevin and CodeAct architecture patterns. |
| [`110` AutoDev Planner](110.md) | Asks AutoDev to produce implementation plans. |
| [`111` Heads-Up Display](111.md) | Gives AutoDev a HUD for visible work state. |
| [`112` Using AutoDev](112.md) | Uses AutoDev in practice to edit its own product. |
| [`113` Agent Memories & Reflections](113.md) | Adds memory and reflection loops to AutoDev. |
| [`114` Planning & Execution](114.md) | Improves AutoDev planning and execution handoff. |
| [`115` AutoDev as Pair Programmer](115.md) | Uses AutoDev as a practical pair-programming agent. |
| [`116` 3D Multiplayer](116.md) | Demos a spatial multiplayer world for human-agent interaction. |
| [`117` AutoDev Git Diffs](117.md) | Shows generated Git diffs inside the AutoDev HUD. |

### 118-125 - v2, SWE-bench, GraphRAG, and master plan

| Episode | Summary |
| --- | --- |
| [`118` Version Two](118.md) | Reboots the product around a stronger v2 chat and coding surface. |
| [`119` v2 Beta Launch](119.md) | Launches the v2 chat and auto-coding beta. |
| [`120` Exploring SWE-bench Verified](120.md) | Studies benchmark data for coding-agent evaluation. |
| [`121` SWE-bench Planning](121.md) | Plans a run at SWE-bench performance. |
| [`122` Codebase Indexing via GraphRAG](122.md) | Designs graph-based codebase indexing. |
| [`123` GitHub Issues to Pull Requests](123.md) | Solves GitHub issues with agent-generated PRs. |
| [`124` Magic AI = Deep State](124.md) | Critiques closed AI-coder fundraising and power concentration. |
| [`125` The Master Plan](125.md) | States the graph-based open-agent business plan. |

### 126-138 - OpenPress, OpenAgents v3, CRM, teams, and projects

| Episode | Summary |
| --- | --- |
| [`126` OpenPress and the End of WordPress](126.md) | Introduces OpenPress as an agentic WordPress replacement. |
| [`127` Hello OpenPress](127.md) | Builds the first Laravel blog and block structure. |
| [`128` Styling OpenPress](128.md) | Styles OpenPress with Shad-style components. |
| [`129` Hosting OpenPress](129.md) | Plans hosted OpenPress site deployment. |
| [`130` OpenAgents <> OpenPress](130.md) | Connects OpenAgents and OpenPress product surfaces. |
| [`131` v3 Landing Page](131.md) | Builds the v3 coming-soon landing surface. |
| [`132` v3 Core Feature](132.md) | Designs the core ask-an-agent site-editing feature. |
| [`133` HTMX Server Sent Events, Part 2](133.md) | Adds streaming to the v3 experience. |
| [`134` Inescapable Inertia](134.md) | Returns to Inertia for pragmatic product velocity. |
| [`135` Agentic CRM Design](135.md) | Designs CRM workflows as agent-native software. |
| [`136` Teams](136.md) | Adds team-scoped collaboration and data boundaries. |
| [`137` Projects](137.md) | Adds project-scoped knowledge and chat organization. |
| [`138` Year One Recap](138.md) | Recaps the first year and the emerging product suite. |

### 139-155 - Onyx, mobile, data markets, local models, and OSINT

| Episode | Summary |
| --- | --- |
| [`139` Going Mobile](139.md) | Sets the mobile-agent direction for Onyx. |
| [`140` Open-Sourcing Onyx](140.md) | Open-sources Onyx and tours the app. |
| [`141` One Market](141.md) | Explains the unified market thesis for agents. |
| [`142` Data Vending Machines](142.md) | Demos Nostr data vending machines. |
| [`143` Onyx as Bitcoin Wallet](143.md) | Turns Onyx toward Bitcoin wallet flows. |
| [`144` Pylon and the Model Context Protocol](144.md) | Introduces Pylon and MCP for local capability routing. |
| [`145` Going Local](145.md) | Runs local Llama-class models on the phone. |
| [`146` Sensemaking: Drones](146.md) | Uses agents for public OSINT and source review. |
| [`147` Planning a Data Marketplace](147.md) | Plans paid data contribution and marketplace mechanics. |
| [`148` Exploring the Genesis Physics Engine](148.md) | Reviews Genesis as a simulation and agent environment reference. |
| [`149` Onyx Beta Launch](149.md) | Launches the first Onyx beta. |
| [`150` Neutrality Wins](150.md) | Frames neutral protocols as the winning strategy. |
| [`151` Speak to Onyx](151.md) | Demos voice chat in Onyx. |
| [`152` Code by Voice](152.md) | Shows pocket coding-agent control by voice. |
| [`153` High-Velocity Bitcoin](153.md) | Connects Bitcoin to high-velocity agent payments. |
| [`154` Agentic OSINT](154.md) | Applies agent workflows to OSINT and public events. |
| [`155` Open Knowledge Graph](155.md) | Builds toward a public open knowledge graph. |

### 156-165 - Repo maps, coding loops, reasoning, and MCP

| Episode | Summary |
| --- | --- |
| [`156` Aider Repo Maps](156.md) | Explores Aider-style repo maps for coding agents. |
| [`157` Chains of Thought and Action](157.md) | Defines action chains around reasoning and tool use. |
| [`158` Quest for the Holy Grail](158.md) | Frames the target loop for useful coding agents. |
| [`159` Onyx Repo Maps](159.md) | Generates repo maps inside Onyx. |
| [`160` Relevant Files](160.md) | Selects relevant files for issue-solving agents. |
| [`161` Basic Coding Loop](161.md) | Implements the first working coding loop. |
| [`162` Reasoning Stream](162.md) | Streams reasoning and task state to the UI. |
| [`163` Issue Reasoning Demo](163.md) | Demos issue reasoning at PlebLab. |
| [`164` The New OpenAgents.com](164.md) | Demos the new agentic chat product. |
| [`165` Integrating MCP via AI SDK](165.md) | Uses AI SDK and MCP to connect tools. |

### 166-182 - Open AI politics, Commander, wallets, swarm inference, and sensemaking

| Episode | Summary |
| --- | --- |
| [`166` OpenAI Delenda Est](166.md) | Opposes lab capture and AI regulation lock-in. |
| [`167` Overnight Agent](167.md) | Reviews the first overnight agent run. |
| [`168` Remote MCP Server & Client](168.md) | Deploys remote MCP infrastructure. |
| [`169` Agent Payments API](169.md) | Builds the Agent Payments API at a hackathon. |
| [`170` Commander](170.md) | Introduces a more powerful agent command interface. |
| [`171` Visualizing Agent Payments](171.md) | Visualizes agent payment activity. |
| [`172` Sync Engine](172.md) | Adds sync for state across clients and devices. |
| [`173` OpenAgents Bitcoin Wallet](173.md) | Builds the OpenAgents Bitcoin wallet direction. |
| [`174` GPUtopia 2.0](174.md) | Revisits compute-market ambition. |
| [`175` Commander v0.0.1](175.md) | Demos the first Commander alpha. |
| [`176` Hand Tracking](176.md) | Tests hand tracking for spatial agent control. |
| [`177` Commander as Nostr Client](177.md) | Makes Commander a Nostr client. |
| [`178` Swarm Inference](178.md) | Plans selling compute into a swarm inference market. |
| [`179` Claude Code Commander](179.md) | Uses parallel Claude Code agents through Commander. |
| [`180` Zero to Website in 60 Seconds](180.md) | Previews rapid website generation. |
| [`181` American DeepSeek?](181.md) | Reviews open-model geopolitics and US AI strategy. |
| [`182` Sensemaking: Weather Modification & the Texas Floods](182.md) | Applies sensemaking workflows to weather-modification claims. |

### 183-198 - Tauri reset, Claude/Codex wrappers, dashboard, and Autopilot

| Episode | Summary |
| --- | --- |
| [`183` Zero Base](183.md) | Deletes and resets the codebase again for a cleaner foundation. |
| [`184` Hello Tauri](184.md) | Creates the Tauri desktop shell. |
| [`185` Hello Claude Code](185.md) | Connects the app to Claude Code. |
| [`186` Actions Per Minute](186.md) | Studies action-rate metrics for agent evaluation. |
| [`187` Mobile Sync](187.md) | Demos two-way sync between desktop and mobile. |
| [`188` The Dashboard](188.md) | Replaces scattered surfaces with a unified dashboard. |
| [`189` Toward an Agentic MMORPG](189.md) | Frames spatial multi-agent systems through MMO ideas. |
| [`190` Goodbye Claude Code](190.md) | Moves away from dependence on Claude Code. |
| [`191` Project Tricoder](191.md) | Introduces a multi-agent coding strategy. |
| [`192` OpenAgents Upgrades Itself](192.md) | Shows OpenAgents modifying its own product. |
| [`193` Codex & Claude Code On Your Phone](193.md) | Adds mobile access to coding-agent workflows. |
| [`194` The Trillion-Dollar Question](194.md) | Asks how much global compute can be put to work. |
| [`195` Designing 10x Better](195.md) | Audits the product for a 10x better interface. |
| [`196` Ditch the TUI](196.md) | Moves coding agents out of terminal-only UX. |
| [`197` Reverse Engineering Cursor](197.md) | Studies Cursor to improve coding-agent UX. |
| [`198` Claude Code on the Web](198.md) | Brings Claude Code-style workflows to the web. |

### 199-213 - Autopilot, agent network, Pylon/Nexus, identity, and markets

| Episode | Summary |
| --- | --- |
| [`199` Introducing Autopilot](199.md) | Introduces Autopilot as the open-source personal agent. |
| [`200` The Agent Network](200.md) | Predicts the agent-network themes for 2026. |
| [`201` Fracking Apple Silicon](201.md) | Plans to unlock idle Apple Silicon compute. |
| [`202` Recursive Language Models](202.md) | Explores recursive language model ideas. |
| [`203` Pylon and Nexus](203.md) | Releases Pylon and Nexus for compute coordination. |
| [`204` DO NOT BREAK USERSPACE](204.md) | Responds to platform breakage and user trust failures. |
| [`205` Vintage Microsoft Evil Shit](205.md) | Diagnoses licensing and platform-control risks. |
| [`206` Codex on Autopilot](206.md) | Demos Codex running inside Autopilot. |
| [`207` Your Keys, Your Coins, Your Identity](207.md) | Explains identity, wallet, and key ownership. |
| [`208` Autopilot HUD](208.md) | Demos the first Autopilot HUD alpha. |
| [`209` Open Moltbook](209.md) | Builds an open version of Moltbook-style workflows. |
| [`210` OpenClaw Online](210.md) | Introduces Hatchery and OpenClaw online surfaces. |
| [`211` Autopilot Online](211.md) | Connects Autopilot to online agent operation. |
| [`212` Autopilot Learns Bitcoin](212.md) | Adds Bitcoin operations to Autopilot. |
| [`213` Agent Markets](213.md) | Lets agents hold and trade Bitcoin across markets. |

### 214-244 - OAPN launch, Psionic, distributed training, bounties, ocean power, Autopilot beta, Sites, Forum, Energy Orchestration, Consolidation, Product Promises, Agent Tipping, Tassadar, revenue loops, the 3D run board, the Sakana Fugu review, and the Khala collective-intelligence arc (OpenCode + Codex own-capacity routing)

| Episode | Summary |
| --- | --- |
| [`214` Compute Market](214.md) | Launches the compute market and Go Online provider mode. |
| [`215` Data Market](215.md) | Launches dataset selling and the NIP-DS protocol direction. |
| [`216` Psionic](216.md) | Introduces the Rust ML stack and model-training ambition. |
| [`217` Psionic: Fast Qwen 3.5](217.md) | Benchmarks Psionic against local inference runtimes. |
| [`218` Probe](218.md) | Introduces a Rust coding-agent runtime. |
| [`219` Probe: Inference Modes](219.md) | Demos multiple Probe inference modes. |
| [`220` Propaganda Podcast](220.md) | Sharpens the OAPN launch thesis and public narrative. |
| [`221` Pylon Launch](221.md) | Launches Pylon as the user-facing provider node. |
| [`222` Templar Merge](222.md) | Responds to Bittensor/Templar and hardens launch claims. |
| [`223` Pay the People](223.md) | Moves from online-provider demos to paid training work. |
| [`224` Distributed Training 101](224.md) | Explains distributed training and CS336-style homework runs. |
| [`225` Developer Bounties](225.md) | Announces bounties across the product and ML stack. |
| [`226` Worse is Better](226.md) | Explains the Bell Labs/New Jersey product philosophy. |
| [`227` Ocean Power](227.md) | Expands the compute-market thesis toward ocean-powered infrastructure. |
| [`228` Free Autopilot](228.md) | Introduces the Autopilot beta wedge and the "Get Paid to Code" flywheel: a free coding agent over a website (a mech suit around Codex/OpenCode/Hermes), GitHub-login public-repo task submission, asynchronous background compute that makes free possible, public agent traces as RL-training data, and back-end revenue-share payouts for code/traces that survive review and make Autopilot better — answering the "first commercial wedge" question. |
| [`229` Autopilot Sites](229.md) | Launches Autopilot Sites (built overnight as a thin Cloudflare wrapper, roughly at parity with OpenAI's Sites preview), demos the iterative submit/revision follow-up loop on a real OTEC site published at a `sites.openagents.com` slug, and introduces the permanent-referrer revenue share where free public sites recruit new humans and agents and pay the referrer on their later paid workflows. |
| [`230` Calling All Agents](230.md) | Addresses AI agents directly with the OpenAgents history, flow-of-funds thesis, five agent markets, Bitcoin/Nostr protocol stance, and Reed's Law network argument. |
| [`231` The Forum](231.md) | Launches the OpenAgents Forum as an old-school bulletin board rebuilt for agents, public discussion, Bitcoin-weighted moderation, and agent participation. |
| [`232` The Energy Layer](232.md) | Explores the co-optimization of energy and compute, agentic inference flexibility, and the "accepted outcomes per kilowatt hour" metric. |
| [`233` The Monorepo](233.md) | Details the consolidation of OpenAgents repositories into the single GitHub monorepo (`openagents`) using Bun, Effect, and Cloudflare Workers. |
| [`234` Product Promises](234.md) | Introduces the Product Promises feature, designed to provide transparency and programmatic verification of live, gated, or withdrawn features. |
| [`235` Agents Earn Bitcoin Tips](235.md) | Demonstrates live BOLT 12 agent tipping end to end (Kenobi as the first paid agent, withdrawn to Cash App), the MoneyDevKit/LDK wallet stack, the public treasury with donations, Artanis as the Cloudflare-resident agent with bounded treasury spend authority, and the call for agents to earn Bitcoin through Forum contributions. |
| [`236` Tassadar](236.md) | Announces the Monday target for a large decentralized training run, frames Pylon v0.3 as the node software for Bitcoin-paid training and other earning modes, and introduces the Tassadar/Percepta Executor Class model direction. |
| [`237` You Must Construct Additional Pylons](237.md) | Launches Autopilot 1.0 — framed as the first and last human-shipped release — on Pylon node software that bundles the Psionic Rust ML stack and a self-custodial MoneyDevKit Lightning wallet, ignites Tassadar as an indefinite Bitcoin-paid distributed training run on Percepta's deterministic "LLM as computer" executor architecture, and switches on the agentic group-forming network (Reed's law past Dunbar's number, agents recruiting agents at machine speed). Names the **accepted outcome** as the atomic unit of the economy, the verification/clearing layer as the load-bearing wall trust gets re-housed in, "deflation plus dividends" as the abundance thesis, accepted outcomes per kilowatt-hour as the single metric, the open-lane (not closed-security-lane) safety stance, Artanis as the once-a-minute autonomous Cloud Mind with bounded treasury authority, and the white-label revenue-share operator opportunity (the marketing-agency customer). |
| [`238` The Tassadar Run is Live](238.md) | Whiteboards the live Tassadar run as "share compute, earn Bitcoin": public LLM-computer training, worker/validator replay, Bitcoin payouts for verified work, a growing module/library loop like an agentic npm, and the verified-work flywheel toward lower inference costs. |
| [`239` Let's Make Money](239.md) | Connects the supply side to the buy side: Autopilot as an all-in-one business system, refer-once/earn-forever attribution, marketplace products built from OpenAgents Cloud primitives, the agentic sales-force ambition, and Bitcoin-priced margin attacks against wasteful AI economics. |
| [`240` Autopilot: Tassadar Run Board 3D Visualization](240.md) | Demos a walkable 3D Tassadar run board in Autopilot: live sats counter, Tassadar status dashboard, training table, floating Pylon/assignment markers, refs ticker, jumping/sprinting/tab-targeting avatar movement, a Snow Crash-style street, and an initial multiplayer direction for watching real run stats as world objects. |
| [`241` Reviewing Sakana Fugu](241.md) | Reviews Sakana AI's Fugu release — a multi-agent orchestration system delivered as one OpenAI-compatible model API (built on Trinity and Conductor) — alongside skepticism that a closed orchestrator over closed models is not "AI sovereignty," then positions OpenAgents' Khala as the open, inspectable counterpart: a single endpoint that fans work out to a pool of models, tools, validators, and Pylon workers, wired into verified work, Bitcoin settlement, and Verse visualization on the Psionic stack. |
| [`242` Khala: Collective Intelligence](242.md) | Introduces **Khala**, the OpenAgents model orchestrator — "collective intelligence" behind a free OpenAI-compatible API (`openagents.com/api`, model `openagents/khala`, fully open source). Contrasts it with OpenRouter Fusion and Sakana Fugu: grown like an ecology rather than engineered, emergent from Bitcoin-paid verified-value markets (bottom-up) rather than designed top-down, and an open pool rather than a closed one. A Khala request routes through a mesh of plugins/models/tools/validators (the Protoss telepathic link) that compose a response — text, code, full software, deployments, legal briefs, research. |
| [`243` Khala in OpenCode](243.md) | Gets Khala into a real coding tool for the first time: point **OpenCode** at the OpenAI-compatible Khala endpoint, fix the live production blockers (OpenCode's content-arrays + tool-call deltas Khala rejected; a stale `khala-mini` slug returning `model_unavailable`), then push real coding/planning traffic through Khala — scaling one OpenCode→Khala session to **ten concurrent** and driving the public tokens-served counter from ~1M to ~13M. Names the provider mix (Fireworks DeepSeek V4 Flash, Gemini Flash, GPT-OSS, GLM-5.2 REAP) and the GTM three pillars (dogfood / ecosystem / benchmark). "We're now in the inference business." |
| [`244` Khala in Codex](244.md) | Routes day-to-day coding through Khala via **Pylon-linked own Codex/Claude capacity**: a Khala "do this PR" request is delegated to the caller's own coding agent and results route back, gated by an own-capacity-only invariant. Plans with Claude Code, implements with Codex; the audit + roadmap become epic **#6273**. Headline finding: the Pylon coding-assignment → executor pipeline already exists end-to-end (ownership bound to the owner agent-user-id), so the net-new work is the caller-aware **router** + a coding-workflow classifier + capacity discovery. Reaffirms own-capacity-only / no-resale / semantic-routing. Tokens surge 16.4M → 302.7M a day later; debuts the `/stats` page (Model-Family mix, Pylon-Codex ~72.7%) and the Khala CLI. |
| [`246` Khala on Apple Silicon](246.md) | Draft treatment, not a transcript: grounds the next-video narrative in the Khala Desktop macOS spec, the Trillion-Dollar Question, Fracking Apple Silicon, and the Khala own-capacity routing arc. Frames every admitted Apple Silicon Mac as a potential Khala node with local chat, local Pylon, Apple Foundation Models readiness, honest provider mode, continual-learning/Tassadar usefulness, and a call for Mac owners and developers to help run and build the node loop. |

## Major Themes

| Theme | What It Covers | Start With |
| --- | --- | --- |
| Open agents versus closed AI capture | OpenAgents is framed as an open alternative to lab-controlled agents, closed marketplaces, regulatory capture, and platform shutdown risk. The 237 launch sharpens this into an explicit "open lane vs. security lane" choice — safety as a market, not a ministry; swarm over singleton; "3D-print rings of power for everyone" rather than fight for the one ring. Episode 241 extends the argument to multi-agent orchestration: a closed orchestrator over closed models is not AI sovereignty, so OpenAgents answers with an open, inspectable Khala. | [`001`](001.md), [`047`](047.md), [`086`](086.md), [`101`](101.md), [`124`](124.md), [`150`](150.md), [`166`](166.md), [`181`](181.md), [`200`](200.md), [`204`](204.md), [`205`](205.md), [`220`](220.md), [`222`](222.md), [`226`](226.md), [`227`](227.md), [`230`](230.md), [`231`](231.md), [`232`](232.md), [`233`](233.md), [`234`](234.md), [`237`](237.md), [`238`](238.md), [`239`](239.md), [`241`](241.md) |
| Accepted outcomes and the clearing layer | The atomic unit of the agent economy is the accepted outcome — a task scoped in advance, executed wherever cheapest, graded against a rubric, recorded in a receipt, and settled to everyone who contributed — not the skill/capability. Trust comes loose from the employment bundle and must be re-housed in an explicit verification/clearing layer, the "load-bearing wall." Measured as accepted outcomes per kilowatt-hour; confidence (draft vs. verified vs. bonded) becomes priceable. | [`224`](224.md), [`232`](232.md), [`234`](234.md), [`235`](235.md), [`236`](236.md), [`237`](237.md) |
| Agentic group-forming network and Reed's law | Growth is reframed for agent time: agents have no Dunbar limit, onboard by reading a markdown file and calling an API, work while we sleep, and tell other agents about useful work at machine speed, so a freely group-forming network's value scales like 2^n. The most viral artifact is a verifiable record of an agent earning Bitcoin, which recruits humans and agents at once. | [`200`](200.md), [`230`](230.md), [`231`](231.md), [`235`](235.md), [`237`](237.md) |
| Build in public and ship early | The series repeatedly ships rough software, public demos, live launches, beta access, and worse-is-better product loops instead of closed-lab secrecy. Episode 229's Sites product is built overnight and demoed live; 237 ships a "first and last" human release and openly publishes the gaps between promise and implementation as audited product promises. | [`002`](002.md), [`003`](003.md), [`004`](004.md), [`006`](006.md), [`008`](008.md), [`086`](086.md), [`092`](092.md), [`100`](100.md), [`119`](119.md), [`149`](149.md), [`183`](183.md), [`184`](184.md), [`214`](214.md), [`221`](221.md), [`226`](226.md), [`228`](228.md), [`229`](229.md), [`231`](231.md), [`234`](234.md), [`237`](237.md) |
| Agent first principles | Agents are treated as concrete software objects: users, chats, plans, actions, tasks, conversations, tools, memory, payments, and inspection logs. | [`005`](005.md), [`006`](006.md), [`007`](007.md), [`020`](020.md), [`033`](033.md), [`038`](038.md), [`040`](040.md), [`090`](090.md), [`132`](132.md), [`138`](138.md), [`199`](199.md), [`230`](230.md), [`231`](231.md) |
| RAG, documents, and private knowledge | The first app arc builds PDF upload, retrieval, embeddings, pgvector search, chat-with-docs, and later generalizes this into codebase and knowledge graphs. | [`010`](010.md), [`011`](011.md), [`012`](012.md), [`013`](013.md), [`014`](014.md), [`015`](015.md), [`016`](016.md), [`017`](017.md), [`018`](018.md), [`019`](019.md), [`023`](023.md), [`046`](046.md), [`122`](122.md), [`155`](155.md) |
| Faerie and GitHub coding agents | Episodes 020-032 are the first serious coding-agent loop: issue context, repo embeddings, plans, code edits, commits, PRs, test automation, and daemon behavior. | [`020`](020.md), [`021`](021.md), [`022`](022.md), [`023`](023.md), [`024`](024.md), [`025`](025.md), [`026`](026.md), [`027`](027.md), [`028`](028.md), [`029`](029.md), [`030`](030.md), [`031`](031.md), [`032`](032.md) |
| Inspectability and agent traces | Open means seeing what agents did, why they did it, which files/tools they touched, and how humans can debug or improve the loop. | [`033`](033.md), [`034`](034.md), [`035`](035.md), [`038`](038.md), [`039`](039.md), [`061`](061.md), [`111`](111.md), [`117`](117.md), [`162`](162.md), [`188`](188.md), [`208`](208.md), [`224`](224.md) |
| Graph-based agents | Execution graphs, node graphs, repo graphs, knowledge graphs, and marketplace graphs recur as the preferred representation for agent state and work. | [`038`](038.md), [`039`](039.md), [`040`](040.md), [`059`](059.md), [`060`](060.md), [`074`](074.md), [`075`](075.md), [`122`](122.md), [`125`](125.md), [`155`](155.md), [`157`](157.md), [`160`](160.md), [`215`](215.md) |
| Domain agents and custom agent builder | Concierge, Sleuth, Connie, meta-agents, and agent builder episodes turn generic infrastructure into named agents with specific jobs. | [`041`](041.md), [`043`](043.md), [`044`](044.md), [`045`](045.md), [`046`](046.md), [`073`](073.md), [`076`](076.md), [`087`](087.md), [`090`](090.md), [`092`](092.md), [`102`](102.md), [`132`](132.md) |
| WASM plugins and open extension systems | The plugin arc makes agents extensible through WASM, registries, uploads, host functions, plugin nodes, and developer-controlled capabilities. | [`048`](048.md), [`049`](049.md), [`053`](053.md), [`054`](054.md), [`055`](055.md), [`056`](056.md), [`058`](058.md), [`066`](066.md), [`068`](068.md), [`069`](069.md), [`071`](071.md), [`072`](072.md), [`074`](074.md), [`075`](075.md), [`102`](102.md), [`165`](165.md) |
| HTMX, Livewire, and streaming UX | The web app experiments with HTMX, SSE, Livewire, streaming plugin status, streaming model responses, and pragmatic UI stacks. | [`050`](050.md), [`051`](051.md), [`052`](052.md), [`078`](078.md), [`079`](079.md), [`080`](080.md), [`081`](081.md), [`133`](133.md), [`134`](134.md) |
| Bitcoin, Lightning, L402, and agent payments | Payments move from flow-of-funds planning into L402, withdrawals, chat pricing, revenue share, Lightning addresses, wallets, and accepted-work payouts. | [`037`](037.md), [`042`](042.md), [`062`](062.md), [`063`](063.md), [`064`](064.md), [`070`](070.md), [`093`](093.md), [`095`](095.md), [`096`](096.md), [`097`](097.md), [`098`](098.md), [`099`](099.md), [`143`](143.md), [`153`](153.md), [`169`](169.md), [`171`](171.md), [`173`](173.md), [`207`](207.md), [`212`](212.md), [`213`](213.md), [`223`](223.md), [`230`](230.md), [`231`](231.md) |
| Nostr identity, storage, and open protocols | Nostr appears as login, plugin registry, KV storage, data vending, client identity, NIP-90 jobs, NIP-DS data, and protocol-backed markets. | [`066`](066.md), [`082`](082.md), [`088`](088.md), [`140`](140.md), [`141`](141.md), [`142`](142.md), [`144`](144.md), [`147`](147.md), [`155`](155.md), [`177`](177.md), [`203`](203.md), [`209`](209.md), [`214`](214.md), [`215`](215.md), [`222`](222.md), [`230`](230.md) |
| Open API and agent marketplace | The API, agent store, plugin store, revenue share, and market hub all point toward an economy where builders and agents can sell capabilities. | [`085`](085.md), [`092`](092.md), [`093`](093.md), [`098`](098.md), [`100`](100.md), [`102`](102.md), [`125`](125.md), [`141`](141.md), [`213`](213.md), [`214`](214.md), [`215`](215.md), [`225`](225.md), [`228`](228.md), [`230`](230.md), [`231`](231.md), [`234`](234.md) |
| Replacing ChatGPT and lab products | The product repeatedly targets replacement of ChatGPT, GPTs, Claude Code, Copilot, Cursor, Moltbook, and closed cloud agent workflows. | [`047`](047.md), [`067`](067.md), [`085`](085.md), [`089`](089.md), [`091`](091.md), [`101`](101.md), [`118`](118.md), [`124`](124.md), [`166`](166.md), [`190`](190.md), [`193`](193.md), [`197`](197.md), [`198`](198.md), [`204`](204.md), [`205`](205.md), [`209`](209.md), [`225`](225.md), [`229`](229.md), [`230`](230.md) |
| AutoDev and coding-agent productization | The AutoDev arc takes coding agents from plans and repo indexes into HUDs, artifacts, Git diffs, memory, issue loops, benchmarks, and paid workflows. | [`103`](103.md), [`104`](104.md), [`106`](106.md), [`107`](107.md), [`108`](108.md), [`109`](109.md), [`110`](110.md), [`111`](111.md), [`112`](112.md), [`113`](113.md), [`114`](114.md), [`115`](115.md), [`117`](117.md), [`118`](118.md), [`119`](119.md), [`120`](120.md), [`121`](121.md), [`123`](123.md), [`156`](156.md), [`161`](161.md), [`162`](162.md), [`163`](163.md), [`218`](218.md), [`219`](219.md) |
| Spatial, game-like, and HUD interfaces | The UI thesis expands beyond chat into HUDs, panes, 3D multiplayer, MMO metaphors, hand tracking, Commander, dashboards, dense operator surfaces, and the walkable Tassadar run board where Pylons, assignments, refs, and training stats become world objects. | [`111`](111.md), [`116`](116.md), [`117`](117.md), [`170`](170.md), [`175`](175.md), [`176`](176.md), [`184`](184.md), [`188`](188.md), [`189`](189.md), [`193`](193.md), [`195`](195.md), [`196`](196.md), [`208`](208.md), [`240`](240.md) |
| OpenPress and agentic site building | OpenPress uses agents to build, style, host, and maintain websites while keeping the stack open and exportable. | [`126`](126.md), [`127`](127.md), [`128`](128.md), [`129`](129.md), [`130`](130.md), [`131`](131.md), [`132`](132.md), [`133`](133.md), [`134`](134.md), [`180`](180.md), [`229`](229.md) |
| CRM, teams, projects, and business workrooms | The v3/business arc turns agent chat into operational software with contacts, leads, teams, projects, scoped context, and business workflows. Episode 237 points this at a concrete commercial shape: a real operator (a marketing-agency owner) runs her whole business on one Autopilot system instead of stitching together five-to-eight SaaS tools, then white-labels that same system to her own clients under a revenue-share arrangement. | [`135`](135.md), [`136`](136.md), [`137`](137.md), [`138`](138.md), [`164`](164.md), [`188`](188.md), [`199`](199.md), [`225`](225.md), [`227`](227.md), [`237`](237.md) |
| Onyx, mobile, and voice | Onyx is the mobile controller for the agent network, with local models, voice, wallet intent, GitHub tools, pocket coding, and beta distribution. | [`139`](139.md), [`140`](140.md), [`143`](143.md), [`145`](145.md), [`149`](149.md), [`151`](151.md), [`152`](152.md), [`153`](153.md), [`187`](187.md), [`193`](193.md) |
| Data markets, OSINT, and knowledge graphs | OpenAgents treats private data, public reports, OSINT, model feedback, agent traces, and graph curation as sellable and verifiable resources. | [`142`](142.md), [`146`](146.md), [`147`](147.md), [`154`](154.md), [`155`](155.md), [`181`](181.md), [`182`](182.md), [`215`](215.md), [`226`](226.md), [`227`](227.md), [`228`](228.md), [`230`](230.md) |
| Local models, Pylon, and model routing | Local model routing starts on phones and desktops, then becomes Pylon provider mode and market-routed compute. By 237, Pylon is the node software at the core of every Autopilot — it can construct additional Pylons on any machine, bundles Psionic for inference/embeddings/training, and ships a self-custodial Lightning wallet so a fresh node can earn the moment it comes online. Episode 241 names Khala as the OpenAI-compatible inference model/gateway that orchestrates a pool of models, tools, validators, and Pylon workers behind one endpoint. | [`065`](065.md), [`084`](084.md), [`144`](144.md), [`145`](145.md), [`178`](178.md), [`194`](194.md), [`201`](201.md), [`203`](203.md), [`214`](214.md), [`221`](221.md), [`236`](236.md), [`237`](237.md), [`241`](241.md) |
| Compute markets and compute fracking | The compute thesis progresses from GPUtopia to swarm inference, Apple Silicon fracking, Pylon/Nexus, OAPN provider onboarding, ocean power, Pylon v0.3 training launch direction, and a public run-board visualization of live Tassadar compute state. | [`174`](174.md), [`178`](178.md), [`194`](194.md), [`201`](201.md), [`203`](203.md), [`214`](214.md), [`221`](221.md), [`223`](223.md), [`224`](224.md), [`227`](227.md), [`230`](230.md), [`236`](236.md), [`237`](237.md), [`238`](238.md), [`240`](240.md) |
| Psionic, Probe, and Rust-native ML | The later stack replaces Python/C++/closed runtimes with Rust-native inference, custom kernels, Probe coding agents, Psionic training/inference code, and Tassadar/Percepta Executor Class model direction (deterministic CPU-style computation folded into the weights and run inside Psionic). | [`216`](216.md), [`217`](217.md), [`218`](218.md), [`219`](219.md), [`221`](221.md), [`222`](222.md), [`224`](224.md), [`225`](225.md), [`236`](236.md), [`237`](237.md) |
| Distributed training and model production | The launch arc turns compute supply into real training work, homework-style assignments, checkpoints, validation, payouts, public stats, and a planned large public training run that becomes, in 237, Tassadar: an indefinite, never-stopped learning loop that pays contributors Bitcoin for verified work and feeds on its own accepted outcomes (every accepted coding outcome is both revenue and a training trace). Episodes 238 and 240 make the run operationally legible through the live money loop and a 3D run board. | [`202`](202.md), [`203`](203.md), [`214`](214.md), [`216`](216.md), [`220`](220.md), [`221`](221.md), [`222`](222.md), [`223`](223.md), [`224`](224.md), [`227`](227.md), [`236`](236.md), [`237`](237.md), [`238`](238.md), [`240`](240.md) |
| Revenue share, bounties, and contributor economics | The economic pattern is to pay builders, data sellers, compute providers, tool authors, skill authors, validators, agent-trace contributors, and product contributors directly in Bitcoin. Episode 237 generalizes this into "deflation plus dividends" — the abundance thesis that life gets cheaper while more people earn continuously from the network — and "pay the people" as the structural answer to displacement. | [`001`](001.md), [`037`](037.md), [`098`](098.md), [`125`](125.md), [`140`](140.md), [`147`](147.md), [`169`](169.md), [`199`](199.md), [`207`](207.md), [`213`](213.md), [`214`](214.md), [`215`](215.md), [`223`](223.md), [`225`](225.md), [`228`](228.md), [`230`](230.md), [`231`](231.md), [`237`](237.md) |
| Research review and sensemaking | Several episodes synthesize external research or product references: NeurIPS, GPT store, Devin clones, Genesis, DeepSeek, weather modification, Cursor, recursive models, Bell Labs design, and Sakana AI's Fugu orchestration release. | [`036`](036.md), [`047`](047.md), [`104`](104.md), [`105`](105.md), [`109`](109.md), [`124`](124.md), [`146`](146.md), [`148`](148.md), [`181`](181.md), [`182`](182.md), [`197`](197.md), [`202`](202.md), [`226`](226.md), [`227`](227.md), [`241`](241.md) |

## Speculation: One Product

This is synthesis from the transcripts, not a product contract. Read as a map of what the full `openagents.com` product has implied across the series.

The final product is one agent operating system with a built-in market. It combines ChatGPT-style entry, inspectable workrooms, coding-agent control, plugin execution, OpenPress site building, CRM/projects/teams, mobile/voice control, Bitcoin wallets, Nostr identity, data markets, compute markets, Pylon provider mode, Psionic training, and public receipts.

- **Command center.** A signed-in home surface with chat, active runs, tasks, projects, wallet state, provider state, earnings, logs, approvals, and market activity.
- **Document and knowledge workbench.** PDFs, source bundles, conversations, web links, codebases, embeddings, graph search, citations, and sellable data packages.
- **Inspectable agent runtime.** Plans, nodes, tool calls, plugin invocations, reasoning streams, diffs, tests, payments, and receipts are visible rather than hidden behind a chat bubble.
- **Coding-agent cockpit.** Issue selection, repo maps, relevant files, patch generation, GitHub commits, PRs, tests, CI/debug loops, HUD panes, mobile control, and benchmark evaluation.
- **Plugin and API platform.** WASM plugins, host functions, Nostr registries, uploaded tools, API access, LLM inference plugins, URL tools, and monetized extension points.
- **OpenPress and business apps.** Agent-built sites, hosted pages, reusable blocks, CRM, contacts, leads, teams, projects, and business workrooms inside the same agent surface.
- **Onyx and mobile companion.** Voice, local models, wallet state, GitHub tools, pocket coding agents, sync, and portable identity.
- **Wallet and identity layer.** Nostr keys, Bitcoin balances, Lightning/L402 flows, withdrawals, revenue share, invoices, local custody warnings, and agent/user transaction history.
- **Market hub.** Compute, data, labor, skills, plugins, bounties, blocks, liquidity, model work, and validation jobs listed on open protocols and paid in sats.
- **Pylon and provider mode.** A local or desktop-controlled Go Online flow that advertises compute, accepts jobs, shows resource limits, logs work, and proves earnings.
- **Nexus and public proof.** Coordinator and treasury surfaces for admissions, work assignments, validation, payouts, stats, and receipts.
- **Psionic and training factory.** Rust-native inference/training, model benchmarks, distributed homework, checkpoints, validation, and paid model-production work.
- **Spatial/operator UI.** Dense HUDs, panes, dashboards, game-like multiplayer space, hand/voice control, and interfaces meant for repeated professional work.

The hardest product requirement is authority separation. The web app can compose the experience, but wallet authority, local compute, provider admission, training truth, payout truth, data provenance, marketplace reputation, and signed receipts need explicit runtime owners. If the product hides those boundaries, it becomes another closed AI dashboard. If it exposes them cleanly, `openagents.com` becomes the front door for an open agent economy.

Episode [`237`](237.md) names the organizing principle the earlier episodes were circling: the atomic unit is the **accepted outcome** (scoped, executed, graded, receipted, settled), and the one structural thing the system cannot strip is the **verification/clearing layer** where trust — loosed from the old employment bundle — gets re-housed and priced. Read through that lens, "authority separation" and "signed receipts" are not just architecture hygiene; they are the load-bearing wall the whole business model rests on, measured as accepted outcomes per kilowatt-hour and shared back to contributors as deflation-plus-dividends.

## Reading Paths

**Project thesis:** [`001`](001.md) -> [`005`](005.md) -> [`047`](047.md) -> [`086`](086.md) -> [`100`](100.md) -> [`125`](125.md) -> [`141`](141.md) -> [`200`](200.md) -> [`214`](214.md) -> [`220`](220.md) -> [`226`](226.md) -> [`227`](227.md) -> [`228`](228.md) -> [`230`](230.md) -> [`231`](231.md) -> [`234`](234.md) -> [`237`](237.md) -> [`238`](238.md) -> [`239`](239.md).

**Original RAG and document product:** [`010`](010.md) -> [`011`](011.md) -> [`012`](012.md) -> [`013`](013.md) -> [`014`](014.md) -> [`015`](015.md) -> [`016`](016.md) -> [`017`](017.md) -> [`018`](018.md) -> [`019`](019.md).

**Faerie and early coding agents:** [`020`](020.md) -> [`021`](021.md) -> [`022`](022.md) -> [`023`](023.md) -> [`024`](024.md) -> [`025`](025.md) -> [`026`](026.md) -> [`027`](027.md) -> [`028`](028.md) -> [`029`](029.md) -> [`030`](030.md) -> [`031`](031.md) -> [`032`](032.md).

**Agent inspectability and graphs:** [`033`](033.md) -> [`034`](034.md) -> [`035`](035.md) -> [`038`](038.md) -> [`039`](039.md) -> [`040`](040.md) -> [`059`](059.md) -> [`060`](060.md) -> [`061`](061.md) -> [`111`](111.md) -> [`117`](117.md) -> [`162`](162.md) -> [`208`](208.md).

**Plugins, APIs, and agent store:** [`048`](048.md) -> [`049`](049.md) -> [`053`](053.md) -> [`054`](054.md) -> [`055`](055.md) -> [`058`](058.md) -> [`062`](062.md) -> [`063`](063.md) -> [`066`](066.md) -> [`070`](070.md) -> [`072`](072.md) -> [`085`](085.md) -> [`092`](092.md) -> [`102`](102.md).

**Payments, wallets, and markets:** [`037`](037.md) -> [`042`](042.md) -> [`062`](062.md) -> [`064`](064.md) -> [`093`](093.md) -> [`095`](095.md) -> [`096`](096.md) -> [`097`](097.md) -> [`098`](098.md) -> [`099`](099.md) -> [`143`](143.md) -> [`153`](153.md) -> [`169`](169.md) -> [`173`](173.md) -> [`207`](207.md) -> [`212`](212.md) -> [`213`](213.md) -> [`214`](214.md) -> [`215`](215.md) -> [`223`](223.md) -> [`228`](228.md) -> [`230`](230.md) -> [`231`](231.md) -> [`234`](234.md) -> [`237`](237.md).

**AutoDev and repo intelligence:** [`103`](103.md) -> [`107`](107.md) -> [`108`](108.md) -> [`110`](110.md) -> [`111`](111.md) -> [`112`](112.md) -> [`113`](113.md) -> [`114`](114.md) -> [`115`](115.md) -> [`117`](117.md) -> [`118`](118.md) -> [`119`](119.md) -> [`120`](120.md) -> [`121`](121.md) -> [`122`](122.md) -> [`123`](123.md) -> [`156`](156.md) -> [`160`](160.md) -> [`161`](161.md) -> [`162`](162.md) -> [`163`](163.md) -> [`218`](218.md) -> [`219`](219.md).

**OpenPress and business product:** [`126`](126.md) -> [`127`](127.md) -> [`128`](128.md) -> [`129`](129.md) -> [`130`](130.md) -> [`131`](131.md) -> [`132`](132.md) -> [`133`](133.md) -> [`134`](134.md) -> [`135`](135.md) -> [`136`](136.md) -> [`137`](137.md) -> [`138`](138.md) -> [`229`](229.md).

**Onyx, mobile, voice, and local models:** [`139`](139.md) -> [`140`](140.md) -> [`143`](143.md) -> [`144`](144.md) -> [`145`](145.md) -> [`149`](149.md) -> [`151`](151.md) -> [`152`](152.md) -> [`153`](153.md) -> [`187`](187.md) -> [`193`](193.md).

**Data, OSINT, and knowledge graph:** [`142`](142.md) -> [`146`](146.md) -> [`147`](147.md) -> [`154`](154.md) -> [`155`](155.md) -> [`181`](181.md) -> [`182`](182.md) -> [`215`](215.md) -> [`227`](227.md) -> [`228`](228.md) -> [`230`](230.md).

**Autopilot and operator surfaces:** [`183`](183.md) -> [`184`](184.md) -> [`185`](185.md) -> [`188`](188.md) -> [`190`](190.md) -> [`191`](191.md) -> [`192`](192.md) -> [`193`](193.md) -> [`195`](195.md) -> [`196`](196.md) -> [`198`](198.md) -> [`199`](199.md) -> [`206`](206.md) -> [`208`](208.md) -> [`211`](211.md) -> [`228`](228.md) -> [`229`](229.md) -> [`231`](231.md) -> [`237`](237.md) -> [`240`](240.md).

**Compute, Pylon, Nexus, Psionic, and training:** [`174`](174.md) -> [`178`](178.md) -> [`194`](194.md) -> [`201`](201.md) -> [`203`](203.md) -> [`214`](214.md) -> [`216`](216.md) -> [`217`](217.md) -> [`221`](221.md) -> [`222`](222.md) -> [`223`](223.md) -> [`224`](224.md) -> [`225`](225.md) -> [`227`](227.md) -> [`230`](230.md) -> [`236`](236.md) -> [`237`](237.md) -> [`238`](238.md) -> [`240`](240.md).

## Maintenance Notes

- Keep this README tied to the transcript files in this directory. If a transcript is regenerated with materially better text, update affected episode summaries and theme references in the same change.
- Keep episode links padded as three digits, matching the transcript filenames.
- Episode `086` is intentionally an excerpt transcript from the YouTube event recording. Do not change its coverage note unless the full event is intentionally transcribed.
- Prefer episode links over raw quotes unless the transcript has been checked against the original video.
