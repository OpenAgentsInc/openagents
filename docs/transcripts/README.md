# OpenAgents Transcript Theme Guide

This directory contains machine-generated transcripts for the video corpus
currently checked in here: episodes `001`-`016`, `038`-`085`, and `087`-`227`.
It is not a complete archive of episodes `001`-`227`.

Use this file as a navigation map. The transcripts are good enough for theme
discovery, but verify wording against the video before using them as
quote-grade source material. Most transcript files include the original source
URL in their header.

## Fast Orientation

- Episodes [`001`](001.md)-[`016`](016.md) cover the original 2023 build in
  public: open agents as a web product, Laravel/Inertia/React setup, agent data
  models, TDD, UX, file upload, RAG, Vectara, embeddings, and pgvector.
- Episodes [`118`](118.md)-[`125`](125.md) are the v2, beta, SWE-bench, and
  master-plan bridge: the live `openagents.com` chat/HUD product,
  GitHub-connected tools, public benchmark planning, codebase indexing via
  GraphRAG, GitHub issues to pull requests, criticism of closed "AI coder"
  fundraising, and the graph-based agent business model.
- Episodes [`126`](126.md)-[`138`](138.md) pivot into OpenPress and
  OpenAgents v3: a Laravel/Blade site-builder thesis, hosted sites, Shad-style
  blocks, HTMX/SSE experiments, the return to Inertia for velocity, an
  agentic CRM, teams, projects, and the year-one recap.
- Episodes [`139`](139.md)-[`155`](155.md) launch Onyx and the mobile/open
  market arc: mobile agents, open sourcing, "one market", NIP-90 data vending
  machines, Bitcoin wallet flows, Pylon plus MCP, local models, data
  marketplaces, OSINT, voice, pocket coding agents, and open knowledge graphs.
- Episodes [`156`](156.md)-[`165`](165.md) deepen the coding-agent loop:
  Aider and Onyx repo maps, relevant file selection, chains of thought and
  action, basic coding loops, reasoning streams, issue reasoning, the new
  `openagents.com`, and MCP integration through AI SDK.
- Episodes [`166`](166.md)-[`182`](182.md) restart the public arc around open
  AI politics, DeepSeek, overnight agents, agent payments, wallets, Commander,
  Nostr, swarm inference, and research/sensemaking workflows.
- Episodes [`183`](183.md)-[`213`](213.md) consolidate the product into
  Autopilot: Tauri/web/mobile shells, Claude Code and Codex wrappers, the
  dashboard, Tricoder, the agent network thesis, recursive language models,
  Pylon/Nexus, identity, wallet, Open Moltbook, and agent markets.
- Episodes [`214`](214.md)-[`227`](227.md) are the launch/OAPN arc: compute
  market, Pylon growth, data market, Psionic, distributed training, CS336-style
  homework runs, bounties, ocean power, and legal fine-tuning.

## Major Themes

| Theme | What It Covers | Videos To Start With |
| --- | --- | --- |
| Open agents versus closed AI capture | The founding claim is that agents, models, data, money, and compute should be open rather than controlled by closed labs. The middle episodes add critiques of closed AI-coder fundraising, OpenAI/lab lock-in, and platform capture; the launch arc sharpens that into opposition to regulatory capture, walled gardens, token schemes, and lab-controlled access. | [`001` Intro](001.md), [`124` Magic AI = Deep State](124.md), [`139` Going Mobile](139.md), [`150` Neutrality Wins](150.md), [`166` OpenAI Delenda Est](166.md), [`181` American DeepSeek?](181.md), [`200` The Agent Network](200.md), [`204` DO NOT BREAK USERSPACE](204.md), [`205` Vintage Microsoft Evil Shit](205.md), [`220` OAPN launch thesis](220.md), [`222` Bittensor/Templar response](222.md), [`227` Ocean Power](227.md) |
| Agent first principles | The early product definition reduces an agent to concrete product needs: chat, planning, actions, web/API/browser interaction, files, tasks, conversations, users, and agents as data models. The v3 arc restates this as "ask for something and an agent makes it happen." | [`005` Agent First Principles](005.md), [`006` Implementing Agent Data Models via TDD](006.md), [`007` First Feature Tests](007.md), [`008` Agent UX Design](008.md), [`012` RAG Planning](012.md), [`132` v3 Core Feature](132.md), [`138` Year One Recap](138.md) |
| Build in public and ship early | The series repeatedly favors working software, public commits, visible rough edges, and fast feedback over closed lab development. The pattern starts with the 2023 Laravel app, continues through v2, the public beta, OpenPress/OpenAgents v3, and Onyx beta releases, and returns in the Zero Base/Tauri reset and OAPN "worse is better" framing. | [`002` Choosing a Tech Stack](002.md), [`003` Hello Laravel](003.md), [`004` Deploying a Landing Page](004.md), [`006`](006.md), [`008`](008.md), [`118` Version Two](118.md), [`119` v2 Beta Launch](119.md), [`127` Hello OpenPress](127.md), [`131` v3 Landing Page](131.md), [`149` Onyx Beta Launch](149.md), [`183` Zero Base](183.md), [`184` Hello Tauri](184.md), [`226` Worse is Better](226.md) |
| RAG, documents, and knowledge grounding | The first technical arc builds toward document-aware agents: PDF upload, Vectara integration, local RAG planning, embeddings, semantic search, and pgvector-backed similarity search. Later episodes generalize the same idea into codebase GraphRAG, repo maps, and crowdsourced knowledge graphs for current events. | [`010` Connecting to Vectara](010.md), [`011` Chatting with a PDF](011.md), [`012` RAG Planning](012.md), [`013` RAG First Principles](013.md), [`014` Embeddings 101](014.md), [`015` Similarity Search](015.md), [`016` PDF to Embeddings](016.md), [`122` Codebase Indexing via GraphRAG](122.md), [`155` Open Knowledge Graph](155.md) |
| Coding-agent work loops and evaluation | The coding-agent arc moves from a live GitHub-connected v2 product to public benchmark strategy and actual issue-to-PR workflows: SWE-bench Verified data, repo indexes, GitHub issue selection, branch creation, file edits, tests, relevant-file selection, reasoning streams, hosted grep/scrape tools, MCP integration, and eventually multi-agent coding dashboards. | [`118` Version Two](118.md), [`119` v2 Beta Launch](119.md), [`120` Exploring SWE-bench Verified](120.md), [`121` SWE-bench Planning](121.md), [`122`](122.md), [`123` GitHub Issues to Pull Requests](123.md), [`156` Aider Repo Maps](156.md), [`159` Onyx Repo Maps](159.md), [`160` Relevant Files](160.md), [`161` Basic Coding Loop](161.md), [`162` Reasoning Stream](162.md), [`163` Issue Reasoning Demo](163.md), [`164` The New OpenAgents.com](164.md), [`165` Integrating MCP via AI SDK](165.md), [`167` Overnight Agent](167.md), [`188`](188.md), [`196`](196.md), [`198`](198.md), [`206`](206.md), [`218` Probe](218.md), [`219` Probe: Inference Modes](219.md) |
| Graph-based agents and knowledge systems | A repeated technical thesis is that agents should be built from graphs: execution graphs for actions, knowledge graphs for context, code graphs for repos, and public graphs for sensemaking. This connects SWE-bench indexing, the master plan, data markets, OSINT, and open knowledge graphs. | [`122`](122.md), [`125` The Master Plan](125.md), [`146` Sensemaking: Drones](146.md), [`147` Planning a Data Marketplace](147.md), [`154` Agentic OSINT](154.md), [`155`](155.md), [`157` Chains of Thought and Action](157.md), [`158` Quest for the Holy Grail](158.md), [`160`](160.md), [`162`](162.md), [`215` Data Market](215.md) |
| OpenPress and agentic site building | OpenPress appears as both a WordPress replacement thesis and a product wedge for agents: public-domain/open-source site infrastructure, Laravel/Blade simplicity, hosted sites, Shad-inspired components, reusable blocks, OpenAgents v3 landing/chat surfaces, and a site builder where the core action is asking an agent to make a site change. | [`126` OpenPress and the End of WordPress](126.md), [`127` Hello OpenPress](127.md), [`128` Styling OpenPress](128.md), [`129` Hosting OpenPress](129.md), [`130` OpenAgents <> OpenPress](130.md), [`131` v3 Landing Page](131.md), [`132` v3 Core Feature](132.md), [`133` HTMX Server Sent Events, Part 2](133.md), [`134` Inescapable Inertia](134.md) |
| Business workrooms, CRM, teams, and projects | The v3 product expands from chat into operational software: agentic CRM, lead/contact workflows, team-scoped data, team switching, project-scoped knowledge and chats, and a business-friendly interface that looks familiar while being agent-native underneath. | [`135` Agentic CRM Design](135.md), [`136` Teams](136.md), [`137` Projects](137.md), [`138` Year One Recap](138.md), [`164`](164.md), [`188`](188.md), [`199`](199.md), [`227`](227.md) |
| UI beyond terminal agents | The product direction rejects fragile terminal-only workflows in favor of dense desktop, web, mobile, HUD, voice, hand-tracking, pane, sidebar, history, and dashboard interfaces for serious agent operation. The v2 arc adds draggable panes and a configurable chat workspace; Onyx adds mobile-first and voice-first control; OpenPress/v3 adds web app blocks, chat shells, and business workspaces. | [`008` Agent UX Design](008.md), [`118`](118.md), [`119`](119.md), [`131`](131.md), [`132`](132.md), [`135`](135.md), [`139` Going Mobile](139.md), [`149`](149.md), [`151` Speak to Onyx](151.md), [`152` Code by Voice](152.md), [`170` Commander](170.md), [`175` Commander v0.0.1](175.md), [`176` Hand Tracking](176.md), [`184` Hello Tauri](184.md), [`185`](185.md), [`188`](188.md), [`193`](193.md), [`195` Designing 10x Better](195.md), [`196`](196.md), [`208`](208.md) |
| Onyx as the mobile personal agent | Onyx becomes the phone entry point into the network: local model chat, open-source app releases, beta distribution, voice input, pocket coding agents, GitHub-token-backed tools, Bitcoin wallet plans, data vending, and eventually the user's portable agent identity. | [`139`](139.md), [`140` Open-Sourcing Onyx](140.md), [`141` One Market](141.md), [`142` Data Vending Machines](142.md), [`143` Onyx as Bitcoin Wallet](143.md), [`145` Going Local](145.md), [`149`](149.md), [`151`](151.md), [`152`](152.md), [`153` High-Velocity Bitcoin](153.md) |
| Bitcoin, Lightning, Spark, identity, and wallets | Agents and users need keys, wallets, settlement, and identity. The videos move from an Agent Payments API to Onyx as a Lightning/Liquid wallet, high-velocity Bitcoin, Nostr identity, seed phrases, hot-wallet safety, Spark/Lightning sends, and Autopilot-native Bitcoin operations. | [`140`](140.md), [`143`](143.md), [`153`](153.md), [`169` Agent Payments API](169.md), [`171` Visualizing Agent Payments](171.md), [`172` Sync Engine](172.md), [`173` OpenAgents Bitcoin Wallet](173.md), [`207` Your Keys, Your Coins, Your Identity](207.md), [`208`](208.md), [`212` Autopilot Learns Bitcoin](212.md), [`213` Agent Markets](213.md), [`214` compute market launch](214.md) |
| Nostr and open agent protocols | Nostr is treated as the neutral data and coordination layer: NIP-90 jobs, data vending, public chat, labeling, reporting, zaps, agent messages, marketplaces, skills, sovereign agents, key splitting, relays, and open client interoperability. MCP becomes the local/tool protocol that can plug into that market through Pylon and AI SDK. | [`140`](140.md), [`141`](141.md), [`142`](142.md), [`144` Pylon and the Model Context Protocol](144.md), [`147`](147.md), [`155`](155.md), [`165`](165.md), [`177` Commander as Nostr Client](177.md), [`178` Swarm Inference](178.md), [`200`](200.md), [`203` Pylon and Nexus](203.md), [`209` Open Moltbook](209.md), [`213`](213.md), [`214`](214.md), [`215` Data Market](215.md) |
| Local models, Pylon, and model routing | The middle arc makes the model/runtime choice explicit: small local models on the phone, larger local models through Pylon and Ollama, API models for speed, MCP for local capabilities, and eventually market-routed compute. This becomes the practical foundation for later Pylon and Psionic work. | [`144`](144.md), [`145`](145.md), [`149`](149.md), [`151`](151.md), [`178`](178.md), [`194`](194.md), [`201` Fracking Apple Silicon](201.md), [`203`](203.md), [`214`](214.md), [`216`](216.md), [`217`](217.md), [`221` Introducing Pylon](221.md), [`222`](222.md) |
| Compute market and "compute fracking" | A recurring thesis is that idle consumer devices are stranded compute. Pylon turns them into paid, routable supply; Nexus coordinates demand; Bitcoin payments keep the market alive; Apple Silicon is the first major target. | [`144`](144.md), [`174` GPUtopia 2.0](174.md), [`178` Swarm Inference](178.md), [`194` The Trillion-Dollar Question](194.md), [`195`](195.md), [`198`](198.md), [`201`](201.md), [`202` Recursive Language Models](202.md), [`203`](203.md), [`214`](214.md), [`220`](220.md), [`221`](221.md), [`222`](222.md) |
| Data markets, OSINT, and public sensemaking | The transcripts repeatedly argue that better AI requires better data and better public sensemaking. The Onyx arc turns that into data vending machines, paid data contributions, reputation, crowdsourced curation, graph traversal, and OSINT-style analysis; the later arc turns it into a broader data market. | [`142`](142.md), [`146`](146.md), [`147`](147.md), [`154`](154.md), [`155`](155.md), [`181`](181.md), [`182` Sensemaking](182.md), [`215`](215.md), [`226`](226.md), [`227`](227.md) |
| Psionic and Rust-native ML | Psionic is the Rust ML framework inside the OpenAgents stack. The videos describe porting useful ideas from Python/C++ ecosystems into Rust, benchmarking against existing runtimes, optimizing kernels, supporting Qwen/GPT-OSS-class inference, and using Psionic inside Pylon. | [`216` Psionic/Rust ML stack](216.md), [`217` Psionic benchmarks](217.md), [`221`](221.md), [`222`](222.md), [`224` Distributed Training 101](224.md), [`225` bounties/product suite](225.md), [`227`](227.md) |
| Distributed training and open model production | The launch arc moves from swarm inference to paid training work: DiLoCo-class training, heterogeneous devices, checkpoints, validation, CS336 assignment-style runs, small focused models, legal fine-tuning, and public stats. For public claims about episode 222, follow the launch truth contract linked inside [`222.md`](222.md). | [`202`](202.md), [`203`](203.md), [`214`](214.md), [`220`](220.md), [`221`](221.md), [`222`](222.md), [`223` training launch/payout infrastructure](223.md), [`224` Distributed Training 101](224.md), [`227`](227.md) |
| Agent markets, revenue share, and bounties | The economic claim is that contributors should be paid directly: compute providers, data sellers, MCP/tool authors, skill authors, developers, validators, site-block creators, knowledge contributors, and community members. The market set expands from agents to data, compute, labor, liquidity, risk, and plugins. | [`001`](001.md), [`125`](125.md), [`128`](128.md), [`140`](140.md), [`141`](141.md), [`142`](142.md), [`147`](147.md), [`153`](153.md), [`165`](165.md), [`169`](169.md), [`173`](173.md), [`199`](199.md), [`200`](200.md), [`206`](206.md), [`207`](207.md), [`212`](212.md), [`213`](213.md), [`215`](215.md), [`220`](220.md), [`223`](223.md), [`225` bounties](225.md) |
| Research review and agentic sensemaking | Several episodes are less product-demo and more research synthesis: closed-lab politics, drones, Genesis, American DeepSeek, weather modification research, Apple Silicon workload share, product design audits, Cursor reverse engineering, recursive language models, and ocean-powered compute/legal benchmark strategy. | [`124`](124.md), [`146`](146.md), [`148` Exploring the Genesis Physics Engine](148.md), [`154`](154.md), [`155`](155.md), [`181`](181.md), [`182`](182.md), [`194`](194.md), [`195`](195.md), [`197` Reverse Engineering Cursor](197.md), [`202`](202.md), [`226`](226.md), [`227`](227.md) |

## Speculation: One Product

This is speculative synthesis from the transcripts, not a product contract.
Assume there is one web app at `openagents.com` that tries to encompass the
whole corpus. It would be an agent operating system with a built-in market:
ChatGPT-style entry, Autopilot work control, OpenPress site building, Onyx
mobile companion, Bitcoin wallet, Nostr identity, Pylon provider management,
data/labor/compute marketplaces, and public proof of what work happened.

The first screen would be a signed-in command center. The user sees a chat
composer, active agent threads, wallet balance, online/provider state, recent
earnings, open jobs, model/runtime status, current team/project, CRM work,
site-builder tasks, and public network stats. It should feel closer to an
operations dashboard than a marketing page. The basic promise: ask agents to do
useful work, put your own resources online, get paid Bitcoin, and verify the
receipts.

Core feature set:

- **Autopilot chat and workbench.** A central chat surface that can delegate to
  Codex, Probe, local models, cloud models, and swarm jobs while keeping
  history, artifacts, tasks, GitHub issues, PRs, runs, approvals, and event
  logs visible. This comes from the v2 beta, SWE-bench, issue-to-PR, and
  Autopilot/dashboard arcs in [`118`](118.md), [`119`](119.md),
  [`120`](120.md), [`121`](121.md), [`123`](123.md), [`163`](163.md), [`164`](164.md),
  [`188`](188.md), [`196`](196.md), [`199`](199.md), [`206`](206.md), and
  [`208`](208.md).
- **Benchmark and repo-intelligence cockpit.** A public/private area for
  loading benchmark datasets, indexing repos, viewing code graphs, choosing
  issues, running agents, comparing patches, and converting successful work
  into productized coding-agent capabilities. The SWE-bench and GraphRAG
  foundation appears in [`120`](120.md), [`121`](121.md), [`122`](122.md),
  [`156`](156.md), [`159`](159.md), [`160`](160.md), and [`162`](162.md).
- **OpenPress site builder and hosting.** A user can ask for a website, blog,
  landing page, or client site; agents compose it from open blocks, host it,
  style it, update it, and expose code/export paths. This is the OpenPress and
  v3 arc in [`126`](126.md)-[`134`](134.md).
- **CRM, teams, and project workrooms.** Teams get scoped data, projects,
  chats, knowledge, contacts, leads, notes, and activity history. Agents can
  work inside those boundaries without every business needing to assemble a
  separate CRM, project manager, and chat app. See [`135`](135.md),
  [`136`](136.md), [`137`](137.md), and [`164`](164.md).
- **Onyx mobile and voice companion.** The phone app should be the portable
  controller for the same agent network: talk to Onyx, run local models,
  connect GitHub, fix a bug from the grocery store, receive beta updates, and
  carry agent identity/wallet state. See [`139`](139.md), [`140`](140.md),
  [`149`](149.md), [`151`](151.md), and [`152`](152.md).
- **Personal agent memory and documents.** Upload PDFs, chats, repos, local
  files, web links, source bundles, and graph nodes; ground answers in retrieved
  sources; run semantic and graph search; and turn valuable private data into
  optionally sellable, redacted datasets. This joins the RAG arc in
  [`010`](010.md)-[`016`](016.md), the codebase GraphRAG arc in
  [`122`](122.md), and the data market in [`147`](147.md) and [`215`](215.md).
- **Wallet and identity layer.** Every user and agent has keys, a Nostr
  identity, a hot wallet for small balances, Lightning/Spark/Liquid or LDK
  settlement, invoices, sends, withdrawals, and transaction history. The
  product must make custody limits explicit. Relevant videos:
  [`140`](140.md), [`143`](143.md), [`153`](153.md), [`169`](169.md),
  [`173`](173.md), [`207`](207.md), [`208`](208.md), and [`212`](212.md).
- **Model router and local capability bridge.** The app should choose between
  phone-local models, trusted local Pylon/Ollama models, paid APIs, and market
  providers based on cost, privacy, latency, and capability. MCP becomes the
  bridge for local tools and sensitive resources. See [`144`](144.md),
  [`145`](145.md), [`149`](149.md), [`165`](165.md), [`178`](178.md),
  [`203`](203.md), and [`221`](221.md).
- **Go Online provider mode.** A browser-managed install/control flow for
  Pylon on the user's machine, plus status, job lifecycle, resource limits,
  supported hardware, earned sats, logs, and receipts. The web app should not
  pretend the browser itself owns local compute; it coordinates a local Pylon
  or desktop companion. Relevant videos: [`144`](144.md), [`178`](178.md),
  [`203`](203.md), [`214`](214.md), [`221`](221.md), and [`224`](224.md).
- **Marketplace hub.** One place to buy and sell compute, data, labor, skills,
  site blocks, MCP servers, agent tools, liquidity, and risk verification.
  Listings should be protocol-backed, priced in sats, attached to reputation,
  and usable by humans or agents. See [`125`](125.md), [`128`](128.md),
  [`141`](141.md), [`142`](142.md), [`147`](147.md), [`165`](165.md),
  [`200`](200.md), [`209`](209.md), [`213`](213.md), [`214`](214.md), and
  [`215`](215.md).
- **Open knowledge graph and data workbench.** Humans and agents can add
  sources, claims, labels, reports, graph edges, confidence notes, and zaps.
  The product should support topic rooms for OSINT and research without
  pretending raw model answers are source authority. See [`146`](146.md),
  [`147`](147.md), [`154`](154.md), [`155`](155.md), [`181`](181.md), and
  [`182`](182.md).
- **Open protocol console.** Nostr relay status, NIP-90 jobs, data vending
  events, agent messages, marketplace listings, sovereign-agent key status, MCP
  endpoints, Pylon providers, and signed receipts should be inspectable. This
  keeps the app from becoming a closed silo. See [`142`](142.md),
  [`144`](144.md), [`155`](155.md), [`177`](177.md), [`203`](203.md),
  [`209`](209.md), and [`222`](222.md).
- **Training and model factory.** A buyer/operator surface for launching
  training, fine-tuning, benchmark, and validation runs over Pylon/Psionic
  supply. It would show run instructions, assignments, checkpoints, validators,
  accepted outcomes, payout eligibility, and public stats. See [`216`](216.md),
  [`217`](217.md), [`222`](222.md), [`224`](224.md), and [`227`](227.md).
- **Revenue-share and bounty center.** Contributors can find bounties, submit
  issues, ship skills/modules, publish blocks, register datasets, review
  accepted work, and see payout history. This turns the "pay contributors"
  theme into an operational workflow. See [`001`](001.md), [`125`](125.md),
  [`140`](140.md), [`147`](147.md), [`199`](199.md), [`207`](207.md), and
  [`225`](225.md).
- **Enterprise and domain workrooms.** A business can bring private data,
  create custom agents, run legal or coding benchmarks, fine-tune models, and
  buy network resources without learning the protocol details. The CRM/team
  path starts in [`135`](135.md)-[`137`](137.md); the legal fine-tuning and
  "last agent" direction appears most clearly in [`227`](227.md).

The hardest product requirement would be authority separation. The web app can
compose the experience, but wallet authority, provider admission, local compute,
training truth, payout truth, data provenance, marketplace reputation, and
signed receipts need explicit runtime owners. If the product hides those
boundaries, it collapses into another closed AI dashboard. If it exposes them
cleanly, `openagents.com` becomes the front door for an open agent economy.

## Reading Paths

Start with the project thesis:
[`001`](001.md) -> [`125`](125.md) -> [`140`](140.md) -> [`141`](141.md) -> [`200`](200.md) -> [`213`](213.md) -> [`214`](214.md) -> [`221`](221.md) -> [`224`](224.md).

To understand coding agents and repo intelligence:
[`118`](118.md) -> [`119`](119.md) -> [`120`](120.md) -> [`121`](121.md) -> [`122`](122.md) -> [`123`](123.md) -> [`156`](156.md) -> [`160`](160.md) -> [`162`](162.md) -> [`163`](163.md) -> [`164`](164.md) -> [`165`](165.md) -> [`206`](206.md) -> [`218`](218.md).

To understand OpenPress and the v3 web product:
[`126`](126.md) -> [`127`](127.md) -> [`128`](128.md) -> [`129`](129.md) -> [`130`](130.md) -> [`131`](131.md) -> [`132`](132.md) -> [`133`](133.md) -> [`134`](134.md) -> [`135`](135.md) -> [`136`](136.md) -> [`137`](137.md).

To understand Onyx, mobile, voice, and local models:
[`139`](139.md) -> [`140`](140.md) -> [`141`](141.md) -> [`142`](142.md) -> [`143`](143.md) -> [`144`](144.md) -> [`145`](145.md) -> [`149`](149.md) -> [`151`](151.md) -> [`152`](152.md) -> [`153`](153.md).

To understand data markets and open sensemaking:
[`146`](146.md) -> [`147`](147.md) -> [`154`](154.md) -> [`155`](155.md) -> [`181`](181.md) -> [`182`](182.md) -> [`215`](215.md) -> [`227`](227.md).

To understand the product surface:
[`005`](005.md) -> [`008`](008.md) -> [`118`](118.md) -> [`119`](119.md) -> [`132`](132.md) -> [`135`](135.md) -> [`139`](139.md) -> [`163`](163.md) -> [`164`](164.md) -> [`165`](165.md) -> [`183`](183.md) -> [`188`](188.md) -> [`199`](199.md) -> [`206`](206.md) -> [`208`](208.md).

To understand compute, Pylon, Nexus, and training:
[`144`](144.md) -> [`145`](145.md) -> [`178`](178.md) -> [`202`](202.md) -> [`203`](203.md) -> [`214`](214.md) -> [`221`](221.md) -> [`222`](222.md) -> [`224`](224.md) -> [`227`](227.md).

To understand the 2023 RAG/web-app base:
[`002`](002.md) -> [`005`](005.md) -> [`008`](008.md) -> [`010`](010.md) -> [`013`](013.md) -> [`015`](015.md) -> [`016`](016.md).

## Notes For Future Updates

- Add missing episode ranges if transcripts for `017`-`117` are generated.
- Keep launch claims synced with canonical docs, especially the episode 222
  launch-hardening note.
- Prefer episode links over raw quotes unless the transcript has been checked
  against the original video.
