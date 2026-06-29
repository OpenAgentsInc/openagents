## Executive take

Kickbacks.ai is an opportunistic **developer-attention ad exchange**. It does not really “advertise to agents” yet; it advertises to **humans while agents are thinking**. The cleverness is inventory discovery: the idle/wait line in Claude Code, Codex, and terminal workflows is a high-frequency, low-clutter surface that already has developer attention. The risk is that the product depends on patching third-party AI coding tools and could be broken or banned by Anthropic, Microsoft, OpenAI, VS Code Marketplace policy, enterprise security policy, or user backlash.

For our own “advertise to agents” idea, I would **not clone the Claude spinner patching model**. I would use OpenAgents/OpenAgents-like infrastructure to build an owned, opt-in, clearly labeled **sponsored agent-tool discovery network**: sponsored cards, sponsored tool suggestions, and paid task handoffs inside agent workspaces and agent execution logs. That gives us the good part of Kickbacks—new ad inventory around AI workflows—without relying on brittle modification of someone else’s UI.

---

## 1. What Kickbacks is

Kickbacks is a VS Code extension plus ad marketplace. Its promise is: install the extension, sign in, and while Claude Code or similar tools are “thinking,” the little status/spinner line becomes a sponsored line; the developer gets a share of ad revenue. The homepage says “50% of the revenue goes to you,” and the advertiser section says each block buys **1,000 five-second impressions**, clicks are billed at **50× the impression rate**, and the highest bid serves first. ([Kickbacks.ai][1])

The current public distribution is real: the VS Code Marketplace listing showed **18,965 installs**, a free extension, and the description “subtle, clickable ads in the Claude Code and Codex spinners — 50/50 revenue split to users.” ([Visual Studio Marketplace][2]) The GitHub mirror describes it as selling the “thinking…” word inside Claude Code and Codex spinners, with “up to 50%” of ad revenue paid to the developer whose machine showed the ad. ([GitHub][3])

The important nuance: Kickbacks is not open source. Its GitHub repo is a public, read-only mirror of the client extension; the README says the backend, advertiser portal, auction engine, and marketing site live in a separate private repository, and the license is proprietary/source-available. ([GitHub][3])

---

## 2. Kickbacks’ model

### Supply side: developer wait-state inventory

Kickbacks turns AI coding wait states into inventory. Its supported surfaces include Claude Code in VS Code, Codex in VS Code, Claude Code terminal status lines, and newer Claude Code terminal spinner verbs. The Marketplace listing says the extension works with Claude Code and Codex VS Code extensions and with the Claude Code terminal CLI; the README lists four surfaces: Claude Code VS Code spinner overlay, Codex thinking shimmer, Claude Code terminal status-bar line, and Claude Code terminal spinner verb. ([Visual Studio Marketplace][2])

The extension patches local tools. The Marketplace listing says VS Code patching modifies Claude Code’s `webview/index.js` and the Codex webview bundle, while terminal support edits `~/.claude/settings.json` to add a status-line script and spinner-verb override; it also says these edits are reversible. ([Visual Studio Marketplace][2])

### Demand side: auctioned developer attention

Advertisers buy blocks. One block equals **1,000 five-second impressions**. Bids are CPM-style, higher bids receive placement priority, and clicks cost a multiple of the per-impression price—currently 50×. The terms describe real-time bidding, dynamic auction pricing, descending priority by effective value, and non-refundability once impressions/clicks are served. ([Kickbacks.ai][1])

This is closer to a tiny programmatic exchange than a flat sponsorship. The advertiser buys priority in a queue, not a fixed tenancy. The public form says a higher bid moves a campaign up the queue so views deliver sooner; it does not add extra views. ([Kickbacks][1])

### Revenue share

The headline is “50%,” but the legal wording is softer. Kickbacks says earning users accumulate credit based on qualifying impressions and clicks, with an **estimated 50% of attributable net advertising revenue after operational expenses**, not guaranteed, paid monthly subject to threshold and conditions. ([Kickbacks][4])

Payouts are via Stripe Connect once the user passes the current **$10 threshold**, completes onboarding and any tax forms, and passes fraud review. Stripe country coverage matters; the FAQ says some countries cannot receive payouts yet. ([Kickbacks.ai][5])

### What counts as billable

Kickbacks has strict qualification rules: the ad must be visible on screen for at least five seconds during a real wait state, the wait state must come from a human-initiated coding request, the user must be signed in and connected, and the user must be under activity caps. ([kickbacks.ai][5])

This is necessary because the obvious attack is “farm spinner impressions.” Their FAQ prohibits bots, scripted prompts, click farms, multiple accounts, collusion, telemetry tampering, cap circumvention, and proxy/VPN rotation; it also says they use automated systems and human review. ([Kickbacks.ai][5])

### Telemetry and privacy posture

Kickbacks says it does not collect code, prompts, AI responses, file contents, file names, project structure, or chat history. It does collect account info, a per-install device ID, ad interaction events, timestamps, surface IDs, visibility metrics, extension/host versions, and transient IP address processing for fraud and rate-limiting. ([Kickbacks.ai][6])

Important detail: the privacy policy says the extension locally reads Claude Code transcript files at `~/.claude/projects/**/*.jsonl` to detect active sessions, but claims it parses only the entrypoint tag, the most recent tool invocation name, and whether the current turn has finished; it says this local processing is not transmitted. ([Kickbacks.ai][6])

A notable source-code finding: the client comments describe a signed-out “demo” route where real ads render and metrics go to `/v1/metrics/demo`; the comments say this charges the advertiser but credits no user. That is not necessarily wrong—preview inventory is a legitimate product choice—but it is something advertisers and users would want disclosed cleanly. ([GitHub][7])

---

## 3. Why it is clever

Kickbacks found “negative-space inventory”: time developers already spend staring at agent progress. It is tiny, native, and contextually valuable to devtool advertisers. A one-line ad for Linear, Ramp, Vercel, Supabase, Sentry, Neon, Modal, Render, Cursor alternatives, observability tools, cloud credits, bug bounty platforms, or AI infra can be more relevant there than on LinkedIn.

The second clever piece is payout framing. Instead of “we are showing you ads,” the product says “you are getting paid to wait.” That changes the emotional framing from interruption to cashback.

The third clever piece is very low creative burden. A 3–60 character line plus destination URL is enough to launch. The product avoids video, banners, images, and layout complexity. The homepage’s ad form reflects that: email, short ad line, destination URL, optional brand name/icon, bid, and block count. ([Kickbacks.ai][1])

---

## 4. Why it is fragile

The biggest weakness is platform dependency. Kickbacks’ own terms admit that third-party platforms including Anthropic and Microsoft may modify software, block or interfere with the extension, update terms to prohibit its mode of operation, or take enforcement action against users. The terms also say Kickbacks is not affiliated with Anthropic, Microsoft, OpenAI, GitHub, or other platform providers. ([Kickbacks.ai][4])

The second weakness is enterprise trust. A lot of Claude Code usage happens on employer-owned machines and private repos. Kickbacks’ terms require users to have employer authorization when installing on employer-controlled devices, which is a sensible legal shield but also a distribution barrier. ([Kickbacks.ai][4])

The third weakness is measurement fraud. AI agents can generate arbitrary wait states. The more money users can earn, the more incentive there is to script “human-looking” agent usage. Kickbacks has caps and fraud logic, but that becomes an arms race. ([Kickbacks.ai][5])

The fourth weakness is ad trust. Developers are unusually sensitive to anything that modifies tools, auto-updates, reads local files, or inserts third-party content into coding workflows. The Marketplace listing advertises silent auto-updates and patching of Claude/Codex assets, which is convenient but may raise flags for security teams. ([Visual Studio Marketplace][2])

---

## 5. Back-of-envelope economics

At the homepage’s default example of **$5 per 1,000 five-second views**, a block buys 5,000 seconds of exposure, or about 83.3 minutes. Gross revenue is $0.005 per five-second impression. If 50% is paid to the user, the user-side gross share is about $2.50 per block, or $0.0025 per five-second impression, before any caps, reconciliation, or net-revenue adjustments. The homepage also says the minimum bid is $1 and clicks are billed at 50× the impression rate. ([Kickbacks][1])

This only becomes meaningful for users at either high CPMs or high wait-state volume. That is why developer-tool advertisers matter: a niche B2B audience can justify higher CPMs than generic display ads. But the revenue must overcome fraud review, payment fees, tax overhead, advertiser churn, and the possibility that most users generate little qualified inventory.

For Kickbacks, the near-term revenue opportunity is less “millions of passive users” and more “own the novelty ad channel for AI-devtool launches.” It can be a launchpad surface: “reach 20k AI coding power users today.” That might command premium sponsorship budgets, especially if advertiser reporting proves conversions.

---

## 6. Competitive read

Kickbacks is early and viral, but the idea is copyable. The durable moat is not the spinner text. The moat would be:

1. trusted distribution across AI dev environments;
2. advertiser demand and campaign liquidity;
3. fraud-resistant measurement;
4. payout and compliance infrastructure;
5. first-party relationships with agent/workspace owners;
6. a policy posture users trust.

Their current implementation is strong on novelty and speed, weaker on durable platform control.

---

# How we should build our own “advertise to agents” idea

## 7. Reframe the thesis

The phrase “advertise to agents” can mean two very different things:

**Bad version:** hide paid instructions inside an agent’s context so the model chooses a sponsor.

**Good version:** create a clearly labeled, policy-governed market for sponsored tools, offers, services, and task handoffs that agents can surface to humans or use only after explicit permission.

We should build the good version. The unit of monetization should not be “an LLM read an ad.” It should be one of:

* a human-visible sponsored card;
* a human click;
* a human-approved tool install;
* a human-approved task handoff;
* a verified conversion or API activation.

That preserves trust and avoids contaminating agent reasoning.

OpenAI’s own ad principles are a useful north star: ads should not influence answers, should be separate and clearly labeled, conversations should stay private from advertisers, and users should have control. ([OpenAI][8])

---

## 8. Why OpenAgents is a better base than patching Claude Code

Assuming you mean the OpenAgents Workspace/Launcher project, it is almost purpose-built for this. OpenAgents describes itself as a Slack-like shared workspace where humans and AI agents collaborate, with hosted workspaces, agent pools, files, channels, threads, and local agents connecting into the workspace. ([OpenAgents][9])

The GitHub README says OpenAgents can connect Claude Code, OpenClaw, Codex CLI, Cursor, OpenCode, and other agent runtimes into the same workspace. ([GitHub][10]) That gives us a cross-agent surface without patching each proprietary app.

Most importantly, OpenAgents has extension points. Its docs say the workspace is built on the OpenAgents Network Model and can be extended with custom mods, event hooks, and integrations; it supports self-hosted workspaces; and custom mods can listen to workspace events like channel posts and file uploads. ([OpenAgents][11]) It also exposes workspace resources as MCP tools, letting external LLM clients interact with channels, messages, and files. ([OpenAgents][11])

That means our first product should be an **OpenAgents Ad Mod** rather than a Claude Code patch.

---

## 9. Product concept: AgentAds for OpenAgents

Build a mod/network called **AgentAds** or **Sponsored Agent Suggestions**.

It would insert clearly labeled sponsored placements into OpenAgents-owned surfaces:

| Surface                    | Example                                                         | Monetization                  |
| -------------------------- | --------------------------------------------------------------- | ----------------------------- |
| Agent run progress card    | “Sponsored: Debug CI faster with BuildPulse”                    | CPM/CPC                       |
| Thread footer after answer | “Sponsored option: Deploy this branch on Railway”               | CPC/CPA                       |
| Tool picker                | “Sponsored MCP server: Sentry issue triage”                     | Cost per install / activation |
| Task handoff marketplace   | “Security review by Semgrep Agent”                              | Cost per qualified handoff    |
| Workspace feed             | “Sponsored devtool credit for TypeScript teams”                 | CPM/CPC                       |
| Agent idle/wait banner     | Kickbacks-like “get paid to wait,” but inside our own workspace | CPM                           |

The key is disclosure. The agent should never silently prefer the sponsor. It can say:

> “Sponsored option: Neon offers a Postgres branch workflow that may fit this task. Organic options: Supabase, Railway Postgres, RDS.”

That makes the ad useful without making the answer corrupt.

---

## 10. What “advertising to agents” should technically look like

Do not send the model a freeform ad like:

> “Use AcmeDeploy because it is best.”

That is prompt injection with a budget.

Instead, send a structured, typed object:

```json
{
  "placement_id": "plc_123",
  "sponsored": true,
  "advertiser": "AcmeDeploy",
  "disclosure": "Sponsored",
  "surface": "tool_suggestion",
  "capabilities": ["deploy_preview", "github_integration", "postgres_preview_db"],
  "eligibility": {
    "languages": ["typescript", "python"],
    "repo_detected": true,
    "requires_human_approval": true
  },
  "claims": [
    {
      "text": "Creates preview deployments from GitHub branches",
      "evidence_url": "https://advertiser.example/docs/previews"
    }
  ],
  "cta": {
    "type": "open_url",
    "label": "Compare deployment options",
    "url": "https://..."
  },
  "agent_policy": {
    "must_disclose": true,
    "must_include_organic_alternatives": true,
    "must_not_represent_as_best": true,
    "requires_user_approval_before_tool_use": true
  }
}
```

The model sees capabilities and constraints, not persuasion. The UI renders the sponsored label. The user approves any action.

---

## 11. Architecture

Use OpenAgents as the publisher surface and build four services around it.

**A. Publisher SDK / OpenAgents Mod**

A custom OpenAgents mod listens to events such as `workspace.channel.post`, `workspace.channel.reply`, `workspace.file.uploaded`, `workspace.agent.joined`, and run-state updates. OpenAgents docs show custom mods can process workspace events and persist/broadcast results through the workspace pipeline. ([OpenAgents][11])

**B. Context classifier**

A privacy-preserving classifier turns raw workspace activity into ad-safe targeting labels:

```json
{
  "task_type": "ci_debugging",
  "language": ["typescript"],
  "framework": ["nextjs"],
  "surface": "agent_run_complete",
  "sensitivity": "normal",
  "commercial_intent": "medium"
}
```

Do not send source code, prompts, secrets, files, or full chat logs to advertisers. Send only coarse labels, hashed workspace/user IDs, and placement metadata.

**C. Decision engine / auction**

The ad server ranks candidates by:

`eCPM = bid_CPM + predicted_click_rate × CPC + predicted_conversion_rate × CPA`

Then filters by policy: sensitive topics, user opt-out, frequency caps, workspace admin rules, advertiser category rules, and creative review status.

**D. Measurement and ledger**

Track:

* viewable impression;
* click;
* human approval;
* tool install;
* tool call;
* conversion callback;
* fraud score;
* revenue share ledger.

Kickbacks’ measurement model is directionally right: event UUIDs, visibility metrics, session nonces, dedupe, caps, and server-side fraud checks. Its client code sends events such as impression rendered, viewable, view ticks, threshold met, click, and error impression. ([GitHub][12])

---

## 12. Revenue model

I would not start with “50% to users” as the only split. OpenAgents-style ecosystems have more parties:

* workspace owner / end user;
* agent developer;
* runtime/publisher surface;
* our ad network;
* advertiser.

A sane initial split:

| Revenue type             | User/workspace | Agent/runtime publisher | Platform |
| ------------------------ | -------------: | ----------------------: | -------: |
| Human-visible CPM/CPC    |            50% |                   0–10% |   40–50% |
| Sponsored tool install   |            30% |                     20% |      50% |
| Paid task handoff        |            20% |                     30% |      50% |
| Verified SaaS conversion |            20% |                     20% |      60% |

For MVP simplicity, use **50% of net revenue to the workspace owner** and keep the rest. Later, add agent-developer rev-share.

Billing products:

1. **Sponsored visibility**: CPM for human-visible cards.
2. **Sponsored click**: CPC.
3. **Sponsored install**: cost per MCP/server/tool install.
4. **Sponsored handoff**: cost per qualified task routed to a sponsor agent.
5. **Conversion CPA**: signup, API key created, repo connected, paid activation.

Do not bill “agent-only impressions” at launch. It is hard to prove value and creates a trust problem. Bill only when a human can inspect the sponsorship or when the user explicitly approves an action.

---

## 13. Initial advertiser categories

Best early advertisers:

* developer tools: CI, observability, issue tracking, code review, testing;
* cloud/dev infra: hosting, databases, serverless, GPU/compute;
* AI infra: evals, tracing, vector DBs, model routers, prompt management;
* security: SAST, secrets scanning, dependency scanning;
* productivity: docs, diagramming, project management;
* hiring/dev education only if not spammy.

Avoid at launch:

* gambling, adult, crypto speculation, political, medical, financial advice;
* anything that asks the agent to make claims it cannot verify;
* anything that requires collecting sensitive repo or employee data.

Kickbacks’ own ad terms prohibit illegal products, malware/phishing, misleading claims, many sensitive categories, and reserve the right to reject ads. We should be at least that strict. ([Kickbacks][4])

---

## 14. MVP build plan

### Week 1: OpenAgents mod + manual ads

Build a self-hosted OpenAgents workspace with an `agent_ads` mod. It listens to channel posts and agent-run events, classifies the task, and inserts a clearly labeled sponsored card in one surface: thread footer or run-complete card.

Use a manual campaign table:

```sql
campaigns(
  id,
  advertiser_name,
  creative_text,
  destination_url,
  bid_cpm,
  bid_cpc,
  targeting_json,
  status,
  daily_budget_cents
)
```

### Week 2: measurement + dashboard

Add event tracking:

```sql
ad_events(
  event_id,
  placement_id,
  workspace_id_hash,
  surface,
  event_type,
  visible_ms,
  clicked,
  approved_action,
  created_at
)
```

Add workspace ledger:

```sql
publisher_ledger(
  workspace_id_hash,
  gross_revenue_cents,
  net_revenue_cents,
  publisher_share_cents,
  status
)
```

### Week 3: sponsored tool suggestions

Add MCP/server/tool placements. OpenAgents can expose workspace resources as MCP tools, and MCP itself is an open standard for connecting AI apps to external data/tools. ([OpenAgents][11])

The agent can say:

> “I can use the built-in GitHub tool. Sponsored alternative: Sentry’s MCP can inspect production errors. Want me to connect it?”

Human approval is mandatory.

### Week 4: pilot

Run with 5–10 advertisers and 50–200 developers. Sell manually. Do not build self-serve billing until advertisers renew.

Core metrics:

* daily active workspaces;
* viewable impressions per active workspace;
* CTR;
* approved tool/action rate;
* advertiser conversion rate;
* revenue per active workspace;
* opt-out rate;
* complaint rate;
* uninstall rate;
* repeat advertiser spend.

---

## 15. Guardrails we should ship on day one

**Answer independence.** Ads cannot alter the organic answer. They appear as separate cards or explicitly labeled sponsored options.

**No hidden prompt injection.** Sponsored content enters as typed metadata, not as natural-language instructions in the system prompt.

**Human approval for actions.** Agents cannot install sponsored tools, create accounts, buy services, or send repo data to advertisers without explicit user approval.

**Context minimization.** The ad server receives coarse task labels, not code, prompts, files, repo names, secrets, or full chat transcripts.

**Workspace admin controls.** Enterprise workspaces can disable ads, allow only approved categories, require allowlisted advertisers, or keep revenue share at the organization level.

**Fraud controls.** One account per person/workspace, caps, signed event receipts, viewability thresholds, human-presence signals, bot-loop detection, advertiser refund logic.

**Clear payout terms.** Use Stripe Connect or equivalent; define threshold, tax handling, supported countries, holds, and forfeiture rules up front. Kickbacks has already had to cover these mechanics in detail. ([Kickbacks.ai][4])

**Platform compliance.** Do not route Claude requests through consumer OAuth credentials or build products on top of users’ Free/Pro/Max credentials. Anthropic’s Claude Code docs say third-party developers building products that interact with Claude capabilities should use API keys and that Anthropic does not permit routing requests through Free/Pro/Max credentials on behalf of users. ([Claude Code][13])

---

## 16. What to copy from Kickbacks

Copy these:

* tiny native units;
* opt-in monetization;
* fast advertiser onboarding;
* auctioned priority;
* 1,000-impression blocks;
* click multiplier or CPC overlay;
* live ledger;
* caps and anti-fraud from day one;
* kill switch;
* public client code or at least auditable client.

Do not copy these:

* patching third-party proprietary UI as the primary surface;
* making user earnings the only value proposition;
* ambiguous “agent saw it” billing;
* anything that reads local agent transcripts unless absolutely necessary;
* any hidden influence on agent answers.

---

## 17. Our best wedge

The strongest wedge is:

> **“Sponsored tools and services for AI agent workflows, with user-paid cashback and strict answer independence.”**

Kickbacks monetizes idle time. We should monetize **decision points**:

* “Which database should I use?”
* “Where should I deploy?”
* “Which CI failure tool should inspect this?”
* “Which observability provider should I connect?”
* “Which security scanner should review this PR?”
* “Which agent should handle this specialized subtask?”

That is more valuable than spinner impressions because it is closer to intent and conversion.

OpenAI’s Agentic Commerce Protocol is moving in the same direction for shopping: structured merchant data lets ChatGPT understand inventory and surface relevant products in context. ([OpenAI Developers][14]) Our version is the B2B/devtool analogue for agent workflows: structured sponsored capabilities, not banner ads.

---

## 18. Recommendation

Build an **OpenAgents Sponsored Suggestions MVP**, not a Claude Code spinner clone.

Start with one owned surface: OpenAgents thread footer or agent run-complete card. Sell manually to devtool advertisers. Use strict disclosure, human-visible billing, no prompt pollution, and revenue share to workspace owners. Once the model works, expand into sponsored MCP tools, task handoffs, and agent-service referrals.

Kickbacks proves that AI wait states are monetizable. The bigger opportunity is to become the trusted ad/offer layer for **agent action graphs**—where agents choose tools, APIs, services, and handoffs under human supervision.

[1]: https://kickbacks.ai/?utm_source=chatgpt.com "Kickbacks.ai - Get paid for waiting"
[2]: https://marketplace.visualstudio.com/items?itemName=Kickbacksai.kickbacks-ai&utm_source=chatgpt.com "
        Kickbacks.ai - Visual Studio Marketplace
    "
[3]: https://github.com/andrewmccalip/kickbacks.ai?utm_source=chatgpt.com "GitHub - andrewmccalip/kickbacks.ai: Get paid for waiting. The most-watched line on Earth now has a market. Kickbacks turns AI wait states into one sponsored status line, and users get 50% of ad revenue. · GitHub"
[4]: https://kickbacks.ai/terms?utm_source=chatgpt.com "Terms of Service · Kickbacks.ai"
[5]: https://kickbacks.ai/faq?utm_source=chatgpt.com "FAQ & Fraud Ground Rules · Kickbacks.ai"
[6]: https://kickbacks.ai/privacy?utm_source=chatgpt.com "Privacy Policy · Kickbacks.ai"
[7]: https://raw.githubusercontent.com/andrewmccalip/kickbacks.ai/main/src/portfolio/client.ts?utm_source=chatgpt.com "raw.githubusercontent.com"
[8]: https://openai.com/index/our-approach-to-advertising-and-expanding-access/?utm_source=chatgpt.com "Our approach to advertising and expanding access to ChatGPT | OpenAI"
[9]: https://openagents.org/docs/en/getting-started/overview?utm_source=chatgpt.com "OpenAgents Overview — The Collaboration OS for AI Agents"
[10]: https://github.com/openagents-org/openagents?utm_source=chatgpt.com "GitHub - openagents-org/openagents: OpenAgents - AI Agent Networks for Open Collaboration · GitHub"
[11]: https://openagents.org/docs/en/workspace/for-developers?utm_source=chatgpt.com "Workspace for Developers"
[12]: https://raw.githubusercontent.com/andrewmccalip/kickbacks.ai/main/src/metrics/client.ts?utm_source=chatgpt.com "raw.githubusercontent.com"
[13]: https://code.claude.com/docs/en/legal-and-compliance?utm_source=chatgpt.com "Legal and compliance - Claude Code Docs"
[14]: https://developers.openai.com/commerce?utm_source=chatgpt.com "Agentic Commerce Protocol | OpenAI Developers"
