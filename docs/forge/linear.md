## Report: Linear **Diffs** and Linear’s expansion into coding

**As of June 28, 2026, Linear Diffs is live, not waitlisted.** Linear launched Diffs on May 28, 2026 as a way to review pull requests directly inside Linear, with the explicit premise that agents are generating more code while humans remain accountable for what gets merged. Linear says Diffs lets users review diffs from any issue with a PR, iterate with agents, ship from Linear, and sync reviews back to GitHub. ([Linear][1])

The shortest accurate read: **Linear is not becoming a git forge like Cursor Origin. It is becoming the “product-context and agent-orchestration layer” that sits above GitHub, coding agents, IDEs, and CI.** Diffs is the review surface in that strategy: the place where issue context, product intent, customer signal, PR changes, agent output, review comments, and final merge all converge.

---

# 1. What Linear Diffs is

Linear Diffs is a **native PR review experience inside Linear**. It pulls GitHub pull request metadata, changed files, checks, comments, review state, and conversation into Linear, then lets reviewers comment, approve, request changes, and merge without switching back to GitHub. Linear’s docs say Diffs keeps PR details, changed files, checks, and comments in sync with GitHub, and that opening a PR in Linear shows details, activity, CI checks, associated comments, and bidirectional updates. ([Linear][2])

The core product idea is that code review should happen **beside the issue that caused the code**, not in a detached PR queue. Linear’s product page says each diff is tied directly to the issue it resolves, while the launch essay says code should sit next to the issue, project, and customer signal behind the change. ([Linear][3])

That distinction matters. GitHub PRs organize review around repositories and branches. Linear Diffs organizes review around **work intent**: the issue, the customer complaint, the project, the product discussion, the agent session, and the code change.

---

# 2. What Diffs includes

## 2.1 Reviews tab and focused review queue

Diffs adds a **Reviews** section in Linear’s sidebar. The docs describe two main tabs: “For me,” which shows PRs you are involved in or responsible for, and “Created,” which shows PRs you authored. Users can group or sort by status, author, or repository, and can include draft/closed PRs or extra fields such as repository, failed checks, and preview links. ([Linear][2])

This turns PR review into a Linear work queue. Linear’s launch framing is that review requests get buried in email and every PR looks equally urgent in a normal queue; inside Linear, each diff inherits product priority because it is attached to the issue and project that produced it. ([Linear][4])

## 2.2 GitHub-synced commenting, approval, change requests, and merge

Linear supports inline comments, replies, emoji reactions, review submission, approvals, change requests, and merging from Linear. The docs say review actions update the review state and sync with GitHub, and that users with permission can merge directly from Linear once the PR is ready. ([Linear][2])

This is important strategically: Linear is not merely embedding a read-only diff viewer. It is trying to make Linear a **write-capable PR client** for GitHub.

## 2.3 Unified and split diff views

Linear supports both unified and split diff layouts. Unified shows changes in one column; split shows before/after side-by-side. The docs also mention keyboard toggling and a responsive fallback for smaller screens. ([Linear][2])

## 2.4 Structural highlighting

One of the headline features is **structural highlighting**. Linear says it strips away formatting-only edits so reviewers can focus on code changes rather than noise. ([Linear][3])

Linear has not publicly documented the exact implementation—whether it is AST-based, parser-based, language-specific, heuristic, or some combination. So the reliable claim is product-level: it is designed to suppress formatting churn and emphasize semantically meaningful changes. The deeper architecture is not yet public.

## 2.5 Guided Reviews / Guides beta

Guided Reviews, also called **Guides** in the docs, are a beta feature that organizes large PRs into structured sections with explanations of purpose and impact. Linear says Guides surface the core implementation first, separate supporting or lower-signal changes, and provide direct links back to the relevant underlying diffs. ([Linear][2])

Linear’s launch essay describes the experience as breaking a diff into “chapters” that follow the order the work was reasoned through, rather than whatever order the filesystem produced. The point is to make a large PR legible without forcing teams into stacked-diff workflows just to compensate for weak review tooling. ([Linear][4])

Availability: Diffs itself is available on all Linear plans, while Guided Reviews are free during beta and available to Business and Enterprise customers. ([Linear][1])

## 2.6 Review notifications

Linear routes PR activity into Linear’s notification system. Diffs can notify users about PR activity, new comments and reviews, review requests, mentions, and CI failures. Users can choose notification modes such as all activity, reviews/comments only, and “by people” modes that filter GitHub-identified bot actors to reduce automated noise. ([Linear][2])

That bot-filtering detail is significant in the agent era. Once coding agents, CI bots, review bots, and automation all comment on PRs, notification hygiene becomes a core product problem.

## 2.7 Preview links and issue context

If a PR contains preview links, Linear can show a preview shortcut on the associated issue. The docs also include a `linear.review` URL pattern: replacing `github.com` with `linear.review` in a GitHub PR URL redirects to the matching Linear review page. ([Linear][2])

This is another sign that Linear wants to be the default daily surface for review, even when GitHub remains the underlying system of record.

## 2.8 Agent iteration from the diff

Linear’s product page says reviewers can make changes from the diff with agents and watch updates appear inline; agents can handle refactors, tests, and follow-up edits without leaving the review surface. ([Linear][3])

The docs for Coding Sessions make this more explicit: when a coding session produces a PR, the workflow continues in the Reviews tab, and users can delegate review actions to Linear Agent to address review comments, rebase open PRs onto master, or fix lint issues. ([Linear][5])

This is the most important feature strategically. Linear Diffs is not just a nicer PR UI; it is the review terminal for agent-written code.

---

# 3. Setup and architecture

## 3.1 Source of truth: GitHub, not Linear

Today, Diffs is built on GitHub PRs. Linear’s docs repeatedly describe Diffs in terms of GitHub integration, GitHub pull requests, GitHub comments, GitHub review state, and GitHub code access. To display diffs and file changes, a workspace needs to grant Linear access to repository code through the GitHub integration. ([Linear][2])

So the architecture is **not**:

```text
Linear as git host
  → repos stored in Linear
  → PRs native to Linear
```

It is closer to:

```text
GitHub repositories and PRs
  ↕ GitHub App / code access / webhooks / user auth
Linear Diffs
  ↕ issue, project, customer, agent, notification context
Linear workspace
```

That makes Linear Diffs very different from Cursor Origin. Origin is positioned as git hosting / code storage; Linear Diffs is a review and orchestration layer on top of GitHub.

## 3.2 Permissions and GitHub identity

Linear requires organization-level code access and personal GitHub connections. The docs say repository code access must be granted through the GitHub integration, and that personal GitHub connections are required for accessing PRs, repository code, and review information associated with a specific user. ([Linear][2])

Review actions are performed on behalf of the authenticated GitHub user. Linear’s docs note that GitHub IP allow-list restrictions still apply to those user-performed review actions, so some GitHub organizations must add Linear’s IPs to their GitHub organization allow list. ([Linear][2])

Architecturally, that means Linear is not bypassing GitHub’s permission model. It is acting as a GitHub client with additional context and workflow state.

## 3.3 Sync model

Linear says reviews sync bidirectionally with GitHub, so comments, approvals, and PR status stay current. ([Linear][3]) The docs also acknowledge that if a webhook is missed, a PR can show the wrong state in Linear, and the workaround is to make a small edit to the PR description in GitHub to trigger an update. ([Linear][2])

That implies a sync architecture roughly like this:

```text
GitHub PR event
  → GitHub webhook / API fetch
  → Linear PR mirror: metadata, files, checks, comments, review state
  → Linear Reviews tab + issue attachment

Reviewer action in Linear
  → authenticated GitHub user action
  → GitHub PR comment / review / approval / merge
  → GitHub state update
  → Linear state refresh
```

The product value is the Linear-side context and interface; GitHub remains the authoritative PR backend.

## 3.4 AI guide generation

Guided Reviews likely use PR diff content plus issue/product context to generate structured “what changed and why” review guides. Linear says each section pairs explanation with relevant diffs, and that guides can be disabled through the GitHub integration setting for generating PR guides. ([Linear][2])

A plausible architecture:

```text
PR diff + file metadata
  + linked issue / project / discussion context
  + possibly code intelligence context
  → guide generation
  → sections ordered by implementation logic
  → reviewer jumps from guide section to underlying diff
```

Linear has not published implementation details for prompt design, model choice for Guides, caching, language coverage, or how it handles extremely large diffs.

---

# 4. Feature inventory by confidence level

| Capability                                |                    Status | Notes                                                                                                                  |
| ----------------------------------------- | ------------------------: | ---------------------------------------------------------------------------------------------------------------------- |
| Review GitHub PRs inside Linear           |                 Confirmed | Linear docs explicitly describe reviewing PRs, changed files, checks, comments, and syncing with GitHub. ([Linear][2]) |
| Inline comments, replies, reactions       |                 Confirmed | Supported directly in Linear’s review UI. ([Linear][2])                                                                |
| Approve / request changes / submit review |                 Confirmed | Review state syncs back to GitHub. ([Linear][2])                                                                       |
| Merge from Linear                         |                 Confirmed | Supported when PR is ready and user has permission. ([Linear][2])                                                      |
| Unified and split diff views              |                 Confirmed | Both layouts are documented. ([Linear][2])                                                                             |
| Reviews tab / focused queue               |                 Confirmed | “For me” and “Created” tabs are documented. ([Linear][2])                                                              |
| PR notifications in Linear                |                 Confirmed | Includes comments, reviews, review requests, mentions, and CI failures. ([Linear][2])                                  |
| Bot-noise filtering                       |                 Confirmed | “By people” modes filter activity GitHub identifies as bot actors. ([Linear][2])                                       |
| Structural highlighting                   | Confirmed product feature | Linear says it strips formatting-only edits; implementation details are not public. ([Linear][3])                      |
| Guided Reviews / Guides                   |            Confirmed beta | Available on Business/Enterprise during beta; organizes large PRs into sections with explanations. ([Linear][2])       |
| Agent iteration from review               |                 Confirmed | Agents can address review comments, rebase, and fix lint issues from the review flow. ([Linear][5])                    |
| GitHub as current backend                 |                 Confirmed | Setup depends on GitHub integration and code access. ([Linear][2])                                                     |
| GitLab Diffs support                      |             Not confirmed | Linear supports GitLab issue/MR workflow automation separately, but Diffs docs are GitHub-centered.                    |
| Commit-by-commit review                   |                   Not yet | Linear says PR-level only for now. ([Linear][2])                                                                       |
| Rich inline CI annotations                |                   Not yet | Linear shows overall check status and basic details, not rich check-run annotations. ([Linear][2])                     |
| GitHub draft review sync                  |                   Limited | GitHub draft reviews do not sync into Linear until submitted. ([Linear][2])                                            |
| Linear-hosted git repos                   |         Not part of Diffs | Diffs is a PR/review layer, not code hosting.                                                                          |

---

# 5. How Diffs fits Linear’s coding expansion

Linear’s coding expansion is not a single feature. It is a sequence:

```text
1. Capture product context
   → issues, customer requests, Slack/Teams, triage, docs, projects

2. Give agents access to that context
   → Linear Agent, Skills, Automations, MCP

3. Let agents understand code
   → Code Intelligence

4. Let agents write code
   → Coding Sessions using Claude Code / Codex

5. Review and merge the result
   → Diffs

6. Monitor progress
   → Insights, Pulse, dashboards, agent-related analytics
```

Linear’s own “Issue tracking is dead” manifesto says the old handoff model is giving way to a system designed around context and agents. It says Linear is becoming a shared product system that holds feedback, intent, decisions, plans, and code, then helps humans and agents carry work to production. ([Linear][6])

## 5.1 Linear Agent: the context-native agent

Linear Agent launched in public beta on March 24, 2026. Linear describes it as built directly into Linear and able to understand roadmap, issues, and code; it can synthesize context, make recommendations, and take action. ([Linear][7])

Agents in Linear behave like “app users”: they can be mentioned, delegated issues through assignment, create and reply to comments, and collaborate on projects and documents. Importantly, Linear says agents are not traditional assignees; assigning an issue to an agent triggers delegation, while the human teammate remains responsible for completion. ([Linear][8])

That “human remains responsible” model is central. Linear is trying to add agent execution without dissolving accountability.

## 5.2 Skills and automations

Linear Agent includes reusable **Skills** and **Automations**. Skills save repeatable workflows; automations can trigger agent workflows when issues enter triage. ([Linear][7])

This moves Linear from passive tracker to active workflow engine. Instead of “a human reads a ticket and decides what to do,” Linear wants “an issue enters triage, an agent classifies it, investigates it, drafts work, and escalates when needed.”

## 5.3 MCP: Linear as agent-accessible infrastructure

Linear exposes a remote MCP server so compatible AI clients can access Linear data and actions. The docs say Linear’s MCP server lets AI models and agents access Linear data securely, with tools for finding, creating, and updating objects such as issues, projects, and comments. It supports clients including Claude, Claude Code, Codex, Cursor, Jules, VS Code, v0, Windsurf, and Zed. ([Linear][9])

This is strategically important because it makes Linear useful to agents **outside Linear’s own UI**. A Cursor, Claude Code, Codex, or Zed agent can use Linear as the source of work context.

## 5.4 Code Intelligence: repositories become product context

Code Intelligence launched in beta on May 14, 2026 for Business and Enterprise plans. Linear says it gives controlled access to connected GitHub repositories so teams can ask how the product works without leaving Linear. It can return grounded answers with links to files, commits, or PRs. ([Linear][10])

The permissions model matters: by default, Code Intelligence only searches connected GitHub repositories that the member can access, but admins can choose repositories and optionally extend access to all workspace members. ([Linear][10])

This is Linear moving from “project management knows what we planned” to “project management can reason about the codebase.” That is a major expansion into engineering workflows.

## 5.5 Coding Sessions: Linear Agent writes code

Coding Sessions launched June 11, 2026. Linear says that when a user delegates an issue to Linear, it starts a secure coding session through Claude Code or Codex, drafts a PR, adds a diff to the issue, and then the user can review and merge from Linear. ([Linear][5])

Linear’s announcement says the agent reads the issue and surrounding discussion, investigates the codebase, proposes an approach, writes code, and opens a pull request in the cloud using frontier models and harnesses like Claude Code or Codex. ([Linear][11])

The current docs say Coding Sessions are supported on Basic, Business, and Enterprise plans and draw from workspace AI credits. They also list current supported model choices as Claude Opus 4.8, Claude Sonnet 4.6, GPT-5.5, and GPT-5.4, with “Auto” defaulting to Claude Opus 4.8 at the time of the docs. ([Linear][5])

## 5.6 Automated triage-to-PR loop

Linear now supports a loop where new incoming issues can trigger agent automation. The docs say triage automations can run agent behaviors, including Coding Sessions, when an issue arrives in Triage, optionally filtered by criteria like label or creator. Linear says it uses this internally to have the agent take the first pass on bugs, investigate, and draft a PR if called for. ([Linear][5])

The launch changelog makes a stronger internal claim: Linear says it uses this workflow to resolve roughly 30% of incoming bug reports, mostly on the first pass. ([Linear][12])

That is the real endpoint of the expansion: not just “open a ticket in Claude Code,” but **a self-starting product-development loop**.

---

# 6. Linear’s emerging architecture

A practical architecture model looks like this:

```text
External inputs
  ├── Slack / Teams discussions
  ├── customer emails / support tools
  ├── bug reports / Sentry / Datadog via MCP
  ├── product specs / docs / projects
  └── GitHub repositories and PRs

Linear context layer
  ├── issues
  ├── projects / initiatives
  ├── customer requests
  ├── discussions and decisions
  ├── agent sessions
  ├── code intelligence index / repository context
  └── review state

Agent orchestration layer
  ├── Linear Agent
  ├── skills
  ├── triage automations
  ├── MCP tools
  ├── custom agents / app users
  └── coding sessions via Claude Code / Codex

Code execution and review layer
  ├── cloud coding session
  ├── branch / PR in GitHub
  ├── Linear Diffs review surface
  ├── agent fixes from review comments
  └── merge from Linear back into GitHub
```

The important architectural choice is that Linear is **not trying to own every execution environment**. It integrates with Claude Code, Codex, Cursor, GitHub, Slack, Teams, Sentry, Datadog, and MCP-compatible clients. Linear’s bet is that the durable layer is context, work state, permissions, review, and accountability—not necessarily the editor or model runtime.

---

# 7. Strategic analysis: what Linear is really doing

## 7.1 Linear is moving from issue tracker to execution control plane

Linear’s CEO explicitly frames issue tracking as a handoff-era system. The company says agents compress planning, implementation, and review by absorbing procedural work, leaving humans more focused on intent, judgment, and taste. ([Linear][6])

Diffs is therefore not an isolated “GitHub PR viewer.” It is Linear claiming that review belongs in the same system as product intent. If the issue contains the customer need, the scope, the discussion, the agent session, and the PR, then the reviewer can judge whether the change is useful—not just whether the code compiles.

## 7.2 Linear is attacking GitHub’s PR UI, not GitHub’s repo hosting

With Diffs, Linear is taking over one of GitHub’s highest-frequency workflows: PR review. But because it syncs with GitHub and uses GitHub code access, Linear is not yet attacking GitHub’s underlying repository hosting. ([Linear][2])

That makes its positioning different from Cursor Origin. Cursor Origin says, in effect, “agentic coding needs a new forge.” Linear says, “agentic coding needs the review and execution loop to live next to product context.”

## 7.3 Linear is making coding a team workflow, not a solo agent workflow

Linear’s Coding Sessions announcement argues that coding agents are still mostly individual productivity tools, while product development is a team activity whose decisions and reasoning form shared context. ([Linear][11])

That is a sharp product thesis. Cursor, Claude Code, Codex, Windsurf, and Zed all compete for the developer’s coding surface. Linear is competing for the **shared organizational surface**: where work begins, where context is stored, where agents are delegated, where review happens, and where progress is measured.

## 7.4 Linear’s wedge is “agent accountability”

Linear repeatedly emphasizes that humans remain accountable. Diffs’ launch copy says agents generate large volumes of code but individuals are still accountable for what merges. ([Linear][1]) The agents docs say delegating to an agent does not make the agent the accountable assignee; the human remains responsible. ([Linear][8])

This gives Linear a credible enterprise story: agents can do work, but Linear preserves ownership, review, audit trail, and escalation.

## 7.5 Linear is expanding “left and right” of code

Linear is expanding leftward into product intake and planning, and rightward into code execution and review:

```text
Customer signal
  → triage
  → issue / project / spec
  → code intelligence
  → coding session
  → pull request
  → Diffs review
  → merge
  → progress monitoring
```

This is broader than “AI issue tracking.” It is closer to an agentic software delivery system.

---

# 8. Weaknesses and open questions

## 8.1 GitHub dependency

Diffs currently appears GitHub-first. That is pragmatic, but it means Linear is dependent on GitHub APIs, permissions, review semantics, webhook reliability, and limitations. The docs explicitly mention missed webhook scenarios and GitHub API limitations around draft review syncing. ([Linear][2])

## 8.2 Review feature gaps

Linear Diffs does not yet support commit-by-commit review. It also does not show rich external check-run annotations, only overall check status and basic details. ([Linear][2])

For teams with deep GitHub/GitLab review workflows, code-owner rules, complex CI annotations, stacked PR tools, or heavy commit-by-commit review culture, Diffs may be a complement rather than a full replacement.

## 8.3 AI-generated guide trust

Guided Reviews are useful if they faithfully explain the change. They are risky if they summarize away important edge cases or make the reviewer feel they understood a PR they did not actually inspect. Linear’s own framing says the human role shifts toward judgment, but that only works if the guide is reliable enough to support judgment rather than substitute for it. ([Linear][13])

## 8.4 Coding agents still need guardrails

Recent research on coding agents found that even state-of-the-art agents can exhibit “action bias”: on stale bug reports where no code change is needed, tested agents still proposed undesirable code changes in 35% to 65% of cases. ([arXiv][14])

That risk maps directly onto Linear’s automated triage-to-PR vision. The product will need strong issue scoping, reproduction requirements, tests, human review, and explicit “do nothing if already fixed” instructions. Linear’s own Coding Sessions docs emphasize that well-scoped issues reduce ambiguity, codebase exploration, and AI-credit usage. ([Linear][5])

## 8.5 Security and access control

Code Intelligence can make repository knowledge available to non-engineering teams. That is powerful, but if admins extend access to all members, Linear can expose technical context to people who do not have direct GitHub access. Linear documents this as an admin-controlled option, but enterprises will need clear policy around which repos are exposed and to whom. ([Linear][10])

For AI privacy, Linear says it does not use customer data to train its own AI models and that AI subprocessors process data only to deliver AI functionality, without permission to train on the provided data. ([Linear][2]) Linear also warns that third-party agents are separate integrations, so teams must evaluate the data practices of the agent provider itself. ([Linear][8])

---

# 9. Linear Diffs vs Cursor Origin

| Dimension                    | Linear Diffs                                                      | Cursor Origin                                                 |
| ---------------------------- | ----------------------------------------------------------------- | ------------------------------------------------------------- |
| Core identity                | PR review and agent iteration inside Linear                       | Git forge / code hosting for agents                           |
| Source of truth              | GitHub PRs and repositories                                       | Cursor-controlled git hosting, based on announced positioning |
| Primary surface              | Issue + review + product context                                  | Repo + branch + PR/change management                          |
| Main user promise            | Review code in context; keep humans accountable                   | Scale git workflows for agent-heavy coding                    |
| Agent role                   | Agents write/fix/rebase/address comments through Linear workflows | Agents are first-class contributors to hosted repos           |
| Strategic wedge              | Own the product-development context layer                         | Own the git/storage/review infrastructure layer               |
| Near-term replacement target | GitHub PR UI, Jira-like handoffs, fragmented review queues        | GitHub/GitLab repository hosting and review infrastructure    |
| Biggest dependency           | GitHub integration/API semantics                                  | New forge adoption, migration, trust, enterprise parity       |

A simple way to distinguish them:

```text
Cursor Origin: “Where code lives for agents.”
Linear Diffs: “Where agent-written code is judged against product intent.”
```

They are adjacent, not identical. A team could theoretically use Linear for planning/review context and Cursor Origin for git hosting, if integrations emerge. But strategically, both companies are converging on the same bottleneck: **AI can generate code faster than teams can coordinate, review, and safely ship it.**

---

# 10. Bottom line

Linear Diffs is a major expansion of Linear from product management into engineering execution. It brings GitHub PR review into Linear, adds structured and AI-guided review, ties code changes directly to issues and product context, and lets agents continue iterating from the diff surface. It is available now, with Guided Reviews in beta for Business and Enterprise customers. ([Linear][1])

The bigger story is that Linear is building an agentic product-development loop: capture work, enrich it with context, let agents understand the codebase, let agents write code, review the result in Linear, and merge. Diffs is the review and accountability layer that makes that loop usable by teams rather than just individuals. Linear is not yet replacing GitHub as a forge; it is trying to replace GitHub PR review plus Jira-style handoffs as the place where product intent becomes shipped code.

[1]: https://linear.app/changelog/2026-05-27-linear-diffs "Linear Diffs – Changelog"
[2]: https://linear.app/docs/diffs "Reviews – Linear Docs"
[3]: https://linear.app/diffs "Linear Diffs – Review code faster"
[4]: https://linear.app/now/code-review-should-be-fast "Code review should be fast - Linear"
[5]: https://linear.app/docs/coding-sessions "Coding sessions – Linear Docs"
[6]: https://linear.app/next "Issue tracking is dead – Linear"
[7]: https://linear.app/changelog/2026-03-24-introducing-linear-agent "Introducing Linear Agent – Changelog"
[8]: https://linear.app/docs/agents-in-linear "AI Agents – Linear Docs"
[9]: https://linear.app/docs/mcp "MCP server – Linear Docs"
[10]: https://linear.app/docs/code-intelligence "Code Intelligence – Linear Docs"
[11]: https://linear.app/now/coding-sessions-for-linear-agent "Now Linear writes the code, too - Linear"
[12]: https://linear.app/changelog/2026-06-11-coding-sessions "Coding sessions in Linear – Changelog"
[13]: https://linear.app/now/reviewing-code-in-the-agent-era "Reviewing code in the agent era - Linear"
[14]: https://arxiv.org/abs/2605.07769 "Coding Agents Don't Know When to Act"


