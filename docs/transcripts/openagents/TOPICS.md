# OpenAgents Transcripts — Topic Index

Categorized topics from the video series transcripts, starting with the **most recent** episodes. Use this to find which episodes discuss a given theme.

**Scope:** Episodes 178–203, plus `20250203-1157-ep-157`, named docs (`194-the-trillion-dollar-question`, `oa-186-actions-per-minute`, `dont-build-agents-build-skills`), and `dspy/` (State of DSPy, DSPy is All You Need).

---

## 1. Products & Launches

| Topic | Episodes | Notes |
|-------|----------|--------|
| **Pylon** | 201, 202, 203 | Swarm compute node: run on your machine, sell compute for Bitcoin; built-in wallet; Regtest → Mainnet; Silicon Mac first, Ollama/Linux coming. |
| **Nexus** | 203 | Swarm relay: Nostr relay optimized for agents; Cloudflare Workers; `nexus.openagents.com`; job stats dashboard (Kind 5940 vs 5050). |
| **Autopilot** | 199, 200, 201, 203 | OpenAgents core product: autonomous coding agent; open source; “2026 = year of autopilots”; Starcraft-style HUD; revenue share. |
| **RLM on swarm** | 202, 203 | `pylon rlm` for recursive language model jobs over Nostr/NIP-90; chunked files, budgets; tuning for coding-agent workflows. |
| **Spark / Regtest** | 203 | Lightspark Regtest faucet; fake Bitcoin for testing; Mainnet “next week” from episode date. |
| **Tricoder** | 191, 192, 193, 194 | Mobile app to manage Codex/Claude Code on the go; `npx tricoder`; QR code pairing; LAN or Tailscale (no account for LAN); TestFlight; v0.3 drops cloud % (orchestration/summaries local). |
| **Commander** | 179, 188 | Desktop app replacing Claude Code TUI; multiple Claude Code windows; load previous chats; Starcraft-style UI; swarm plugins, MCP. |
| **Dashboard** | 188 | Web app: full Claude Code workflow; Next.js, Convex; voice (Aqua); “do issue N on new work tree, push PR”; multiple sessions; APM, context dial; conversation data → Convex; OpenAgents as a service. |
| **RuinsOfAtlantis** | 189 | MMORPG; OpenAgents acquired game studio ($1); AI agents + Bitcoin + real-world integrations; weekly releases. |
| **Zero to website** | 180 | Deploy site in ~60 seconds; Bitcoin puns, BIP 118, Nostr; custom domains (OpenAgents Pro); plugins, MCP, wallet, Commander in one web UI. |
| **Apple Silicon / “Trillion-dollar question”** | 194 | End of cloud inference?; Apple Foundation Models API vs Vercel AI SDK; agentic codebase search on device; ChatGPT analysis: 15–25% AI inference on Apple Silicon by 2030 (7–31% range); trillion-dollar swing; developer adoption of Foundation Models + MLX; Tricoder v0.3; TestFlight, VC DMs. |

---

## 2. Recursive Language Models (RLM)

| Topic | Episodes | Notes |
|-------|----------|--------|
| **RLM paper** | 202 | MIT CSAIL / Omar (DSPy); long prompts as “external environment”; REPL; scale beyond context window; async sub-calls + swarm fit. |
| **Why RLM + swarm** | 201, 202, 203 | Buy-side demand for “micro-jobs”; RLM’s async fan-out matches swarm compute; Prime Intellect “paradigm of 2026.” |
| **Observations** | 202 | 10M+ token regime; REPL necessary; cost comparable to base call; model-agnostic; recursion depth and alternatives. |

---

## 3. Swarm Compute & Edge Inference

| Topic | Episodes | Notes |
|-------|----------|--------|
| **Spare compute / 110M Macs** | 200, 201 | 5.5 GW vs Stargate/Abilene; “rent” not build; pay users for spare compute; Episode 178 demo. |
| **Stranded / Fracking / Wildcatter** | 201 | Stranded = resource without market plumbing; Fracking = protocols + money + receipts + routing; Wildcatter = early provider; Daniel Batten, Episode 94. |
| **Nostr + jobs** | 200, 203 | NIP-90 Data Vending Machine; signed JSON over websockets; providers pick jobs; Nexus as relay. |
| **GPUtopia → OpenAgents** | 201 | 2023 experiment; ~300 nodes; phased out; “agents are the future buyers”; relaunch with Pylon. |
| **Swarm inference demo** | 178 | First production demo: “go online” button; sell compute for Bitcoin; NIP-90 over Nostr; Ollama local (Gemma 1b); DevStrel (Mistral 24B) via swarm; Lightning invoice payment; global compute marketplace. |

---

## 4. Agent Networks & 2026 Vision

| Topic | Episodes | Notes |
|-------|----------|--------|
| **2026 themes** | 200 | Local AI, Swarm AI, Open > Closed, Agents > Models, Autopilots, Agent networks. |
| **Reed’s Law** | 200 | Group-forming networks: value ∝ 2^N; contrast Metcalfe (N²); agents not limited by Dunbar’s number. |
| **DeepMind / AGI safety** | 200 | Critique of “controlled environment” and “vetted API gateways”; open protocols + Nostr; NIP-SA Sovereign Agents, FROST. |
| **Two-plane architecture** | 200 | Plane A: open (Nostr, Lightning, transparency). Plane B: containment (high-risk tools, optional). |
| **Coalition latency** | 200 | “Real battleground is coalition latency, not agent intelligence”; open neutral protocols. |
| **SYNTHESIS.md** | 200 | Single long doc: thesis, products, protocols, glossary; “feed this to your AIs.” |

---

## 5. Economics, Labor & Dividends

| Topic | Episodes | Notes |
|-------|----------|--------|
| **Deflation + dividends** | 200 | Execution cheap; deflation in spec-able work; dividends = skills, compute, data, verification paying continuously. |
| **Micropayments** | 200, 201 | Bitcoin/Lightning; revenue splits; “citizen dividend” style; redistribution without “government communism.” |
| **GPT Store / paying devs** | 199, 200 | OpenAI paid zero; OpenAgents paid 20+; “labs that pay people” will win network effects. |
| **Human Portfolio Managers** | 200 | Supervise fleets, set policies, allocate budgets; “playing StarCraft.” |
| **Guilds / micro-firms** | 200 | Temporary coalitions (human + agent); jobs fragment into task graphs; “bring back guilds.” |

---

## 6. Coding Agents & UX

| Topic | Episodes | Notes |
|-------|----------|--------|
| **10 upgrades (10x better)** | 195, 196, 197, 198, 199 | Ditch TUI → desktop app with sidebar/history; Go mobile; Code overnight; CLI/sub-agents; History & memory; MCP simpler; Embrace open source; Local/swarm/cloud inference; Compute fracking; Revenue sharing. |
| **TUI vs desktop app** | 195, 196, 198 | Replace “janky fake terminals” with ChatGPT-style UI, sidebar, chat history, widgets for long-running agents. |
| **Web UI** | 198 | Feature request: “ChatGPT-style UI for coding agents,” npx script, connect to Claude Code. |
| **Code overnight / scheduled prompts** | 195, 199 | Overnight runs; scheduled prompts; “glorified cron” with sub-agents; audit of what worked. |
| **Sub-agents / delegation** | 195, 197 | Main chat delegates to Codex/Claude Code; shared context; “router” with tool calls. |
| **History & memory** | 195 | Sidebar with past chats; local SQL-like store; “three days ago we discussed this”; vs giant JSON blobs. |
| **MCP pain** | 195 | Tool registry too heavy; “create API then script”; want “pull in integrations” in-app. |
| **Cursor / Codex / Claude Code** | 195, 197, 199 | Cursor = “faster horse,” reverse-engineer for interoperability; Codex open source; Claude Code in “mech suit,” Rust port. |
| **Reverse engineering Cursor** | 197 | doc/re Cursor; dependencies vs VS Code; “6 days to MVP, 20 days to 10x Cursor.” |
| **Open Code** | 195 | Leader in open coding agents; healthy contributors; OpenAgents wants same for “opinionated agentic flow.” |
| **Mobile / Tricoder** | 191, 192, 193 | Manage coding agents on the go; async agents; “fuck IDEs, they’re on the way out”; Tailscale to desktop; QR pairing; no account (LAN); Expo OTA update (192: “app updates itself”); five sessions at once. |
| **Goodbye Claude Code** | 190 | Codex now primary; Anthropic mobile/cloud “work from anywhere” not good enough (still beta); Claude Code not open source, SDK/auth/docs pain; Codex open, inspectable; building competitor: mobile + desktop + web. |
| **Super Maven** | 193 | Acquired by Cursor, sunset; “9/11”; Cursor valuation joke. |

---

## 7. Metrics & Benchmarks

| Topic | Episodes | Notes |
|-------|----------|--------|
| **Actions Per Minute (APM)** | oa-186 | StarCraft parallel: measure agent velocity (messages + tool calls); Humanity’s Last Exam critique (bogus data); stats pane in OpenAgents; baseline 2.3 APM from 30-day / 277 sessions; apm.md spec on GitHub. |

---

## 8. Chains of Thought and Action (COTA)

| Topic | Episodes | Notes |
|-------|----------|--------|
| **COTA concept** | 20250203-1157-ep-157 | Combine reasoning (e.g. DeepSeek R1) with tool use; “reality checks”; split thinking across generations + tools. |
| **OpenAI Deep Research** | 20250203-1157-ep-157 | Critique: leaderboard compares tool-using model to non-tool models; “one thing on here uses tools”; reasoning traces hidden. |
| **GitHub issue solver** | 20250203-1157-ep-157 | Agent: repo map, traverse files, CI, tests, PR; multi-step reasoning + tools; full trace (COTA) visible. |
| **Transparency** | 20250203-1157-ep-157 | Show reasoning + tool I/O; “open agents show trace”; inspectable, testable, upgradable steps. |

---

## 9. Protocols & Stack

| Topic | Episodes | Notes |
|-------|----------|--------|
| **Nostr** | 200, 203 | Relays, event kinds (5940 RLM, 5050 inference); agent coordination/commerce “for a year+.” |
| **NIP-90 / NIP-SA** | 200, 203 | Job marketplace; Sovereign Agents (FROST, key split across relays). |
| **Bitcoin / Lightning** | 201, 203 | Payments to providers; built-in wallet; streaming money; Spark Regtest. |
| **Rust / Cloudflare** | 203 | Pylon and stack in Rust; Nexus on Cloudflare Workers. |
| **Install / platforms** | 203 | Paste instructions to agent; Silicon Mac required initially; Ollama support added. |

---

## 10. Competitive & Critique

| Topic | Episodes | Notes |
|-------|----------|--------|
| **Closed labs** | 200, 201 | OpenAI, Anthropic, xAI “structurally incapable” of leading agent networks; need open, neutral infra. |
| **Cursor** | 197 | “Faster horse”; fork open source then close; “what do you need $2B for”; educational/interop reverse engineering. |
| **Microsoft Copilot®** | 199 | Trademark; “jump on the name”—Autopilot not Copilot. |
| **Open vs closed ecology** | 200 | “Open ecology vs closed corporate anthill”; “we win.” |

---

## 11. Skills, Marketplace & Revenue Share

| Topic | Episodes | Notes |
|-------|----------|--------|
| **Skills marketplace** | 199, 195 | List skills, get paid per use; micropayments; “$200 on skills, worth 5x” (Ryan Carson). |
| **Sovereign Agents / keys** | 199, 200 | FROST; keys split across guardians/relays; paid skills not copy-pasteable to another agent. |
| **Built-in wallet** | 195, 199, 201 | “We built this already”; agent plugins → registry → tiny share per use. |
| **Anthropic “Don’t Build Agents, Build Skills”** | dont-build-agents-build-skills | Skills = folders with procedural knowledge; scripts as tools; progressively disclosed; MCP + Skills; agents writing skills from experience; versioning, testing, dependencies; Financial Services / Life Sciences verticals. |

---

## 12. DSPy (External / Reference)

| Topic | Episodes | Notes |
|-------|----------|--------|
| **State of DSPy** | dspy/state-of-dspy | Omar: DSPy ≠ “prompt optimization”—declarative self-improvement; Signatures; Bitter Lesson; Quake fast inverse sqrt; RLM (Alex Tan), REPL, 10M+ tokens; Context Rot; 3M downloads/month. |
| **DSPy is All You Need** | dspy/dspy-is-all-you-need | Kevin Madura: Signatures, Modules, Tools, Adapters, Optimizers, Metrics; GEPA, MIPRO; transferability; LLM-as-Judge; DSPy Hub; cost, large context. |

---

## Episode → Topics (quick map)

| Episode | Primary topics |
|---------|-----------------|
| **203** | Pylon & Nexus launch, RLM on swarm, Regtest/Spark, Nexus dashboard, `pylon rlm` |
| **202** | RLM paper, swarm as RLM backend, async sub-calls, event Kind 5940 |
| **20250203-1157-ep-157** | COTA, Deep Research critique, GitHub issue solver, trace transparency |
| **201** | Fracking Apple Silicon, 110M Macs, Stranded/Fracking/Wildcatter, GPUtopia, Pylon preview |
| **200** | Agent network, 2026 predictions, Reed’s Law, DeepMind, two-plane, SYNTHESIS.md, deflation/dividends |
| **199** | Introducing Autopilot, copilots vs autopilots, revenue share, Sovereign Agents, HUD |
| **198** | Web UI request, Claude Code on web, ChatGPT-style UI |
| **197** | Reverse engineering Cursor, doc/re Cursor, 10x roadmap, 6 days MVP |
| **196** | Upgrade #1: ditch TUI, sidebar, chat history, widgets |
| **195** | Designing 10x better: full list of 10 upgrades, Codex/Claude, compute fracking, revenue sharing |
| **194** | Trillion-dollar question, Apple Silicon / Foundation Models, Tricoder v0.3, TestFlight, VC DMs |
| **193** | Codex & Claude Code on phone, npx tricoder, QR pairing, LAN/Tailscale, Super Maven/Cursor |
| **192** | OpenAgents mobile app, “magic trick” OTA update (ask Codex, push update), Codex bridge, Expo |
| **191** | Project Tricoder: mobile app for managing coding agents; async agents; Tailscale; TestFlight |
| **190** | Goodbye Claude Code; Codex primary; Anthropic mobile “not yet”; building competitor (mobile + web) |
| **189** | Toward agentic MMORPG; Commander → game; RuinsOfAtlantis; OpenAgents acquired game studio |
| **188** | The Dashboard: web app, full Claude Code workflow, Convex, voice (Aqua), APM, conversation → Convex |
| **180** | Zero to website in 60 seconds; Bitcoin puns, BIP 118, Nostr; deploy; custom domains, plugins, MCP |
| **179** | Claude Code Commander: multiple windows, history, load chats; Jarvis; swarm plugins, MCP |
| **178** | Swarm Inference: first production demo; “go online”; NIP-90; Ollama (Gemma 1b); DevStrel via swarm; Lightning |
| **oa-186** | Actions Per Minute (APM); StarCraft parallel; Humanity’s Last Exam critique; baseline 2.3 APM; apm.md |
| **dont-build-agents-build-skills** | Anthropic Skills talk: folders, scripts as tools, MCP + Skills, versioning, agents writing skills |
| **dspy/state-of-dspy** | Omar: DSPy ≠ prompt optimization; Signatures; Bitter Lesson; RLM; Context Rot; 3M downloads/month |
| **dspy/dspy-is-all-you-need** | Kevin Madura: Signatures, Modules, Tools, Adapters, Optimizers, Metrics; GEPA, MIPRO; DSPy Hub |

---

*Generated from transcripts: 178–203, 20250203-1157-ep-157, oa-186, 194-the-trillion-dollar-question, dont-build-agents-build-skills, dspy/state-of-dspy, dspy/dspy-is-all-you-need. Expand this index as more transcripts are added.*
