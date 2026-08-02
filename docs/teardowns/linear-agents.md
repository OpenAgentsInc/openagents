# Linear Agents: Full Product Report

**As of August 2, 2026**

## Executive assessment

**Linear Agents is the most coherent agent-native product-development system currently on the market.** It is not simply a chatbot added to an issue tracker. Linear has built a layered system that combines:

* a native, workspace-aware **Linear Agent**;
* a platform where external agents appear as identifiable teammates;
* reusable **Skills** and organizational guidance;
* scheduled and event-triggered **Loops**;
* AI-assisted intake and triage;
* repository-aware investigation through **Code Intelligence**;
* cloud coding through **Coding Sessions**;
* and an integrated code-review surface through **Diffs**.

Linear’s strategic thesis is that the conventional issue tracker was designed to coordinate human handoffs. When agents can perform planning, investigation, implementation, and parts of review, the valuable product is no longer a queue of tickets—it is the **shared context and control plane through which humans and agents move work to production**. Linear explicitly describes itself as the system that holds feedback, intent, decisions, plans, and code and turns that context into execution. ([Linear][1])

My overall assessment is **8.4/10**:

| Dimension                        | Assessment |
| -------------------------------- | ---------: |
| Product vision and coherence     |     9.5/10 |
| Human-agent interaction design   |       9/10 |
| Coding and review integration    |     8.5/10 |
| Automation maturity              |     7.5/10 |
| Developer-platform maturity      |       7/10 |
| Cost controls                    |     6.5/10 |
| Governance and security maturity |       7/10 |
| Independent evidence of ROI      |     6.5/10 |

The central qualification is that **Linear is building an agent control plane, not a general-purpose agent runtime**. It is strongest when Linear already holds the product context and the work can be expressed as issues, projects, documents, code changes, or recurring product-development processes. It is not yet a full multi-agent orchestration engine, deterministic workflow system, or replacement for a durable execution cloud.

---

## 1. What “Linear Agents” actually refers to

There are two products that are easy to conflate.

### Linear for Agents / the Agent Platform

Launched in May 2025, this is the platform for **third-party and custom agents**. Agents are represented as “app users”: they have names, avatars, profiles, activity, team access, and an identifiable presence in the workspace. People can mention them, delegate issues to them, and observe their progress. The first launch included integrations such as Devin, ChatPRD, and Codegen, alongside APIs for building custom agents. ([Linear][2])

### Linear Agent

Launched on March 24, 2026, this is Linear’s own native agent. It is built directly into the workspace and can reason over issues, projects, milestones, initiatives, cycles, comments, documents, customer requests, and activity history. It can answer questions, summarize work, create or update Linear objects, and act from chat, issue threads, Slack, and Microsoft Teams. ([Linear][3])

The broader “Linear Agents” product family now looks like this:

| Component                  | Purpose                                                                |
| -------------------------- | ---------------------------------------------------------------------- |
| **Linear Agent**           | Native reasoning and action over workspace context                     |
| **Skills**                 | Reusable personal or team-level agent instructions                     |
| **Guidance**               | Persistent workspace and team conventions supplied to agents           |
| **Loops**                  | Scheduled or event-triggered background agent work                     |
| **Agent Platform**         | Identity and interaction layer for external/custom agents              |
| **Triage Intelligence**    | Classification, routing, duplicate detection, and property suggestions |
| **Code Intelligence**      | Repository-aware investigation and explanation                         |
| **Coding Sessions**        | Hosted coding work using Claude Code or Codex harnesses                |
| **Diffs / Guided Reviews** | PR inspection, review, steering, and merging                           |
| **MCP connectivity**       | Access to external tools and data sources                              |

This stack was assembled very rapidly. Linear introduced Code Intelligence in May, Coding Sessions in June, Loops on July 20, agent-assisted document editing and text attribution on July 23, and mobile code review, signed coding-session commits, Guided Reviews general availability, and GitHub Copilot delegation on July 30. ([Linear][4])

---

## 2. The product thesis: context is the durable asset

Linear is making a very specific bet:

> Models and coding agents will become interchangeable, but the accumulated understanding of what a company is building will remain valuable.

A coding model can read a ticket and a repository. But a well-informed product-development agent needs considerably more:

* the original customer signal;
* related requests and previous bugs;
* the decisions behind the feature;
* product and technical specifications;
* project scope and milestones;
* prior discussion;
* ownership and organizational conventions;
* affected customers;
* telemetry and external evidence;
* relevant code paths;
* and the current state of implementation.

Linear already stores a structured subset of this information. Its product strategy is to make that context usable by any authorized human or agent without requiring someone to reconstruct it manually in a new prompt. In Coding Sessions, for example, the issue, customer signal, product decisions, related work, and surrounding discussion are automatically attached to the coding task. The resulting session is shared with the organization rather than belonging privately to the person who initiated it. ([Linear][5])

This is the real moat. **Linear is not likely to win by having a uniquely superior foundation model.** It can win by owning:

1. the canonical product context;
2. the delegation interface;
3. the policy and permission layer;
4. the visible history of agent activity;
5. and the point where an agent’s output returns to a human for judgment.

That is a more durable position than selling a chat box tied to one model.

---

## 3. The most important interaction primitive: owner versus delegate

Linear makes a distinction between the human who is accountable and the agent doing the work.

An issue retains a human **assignee** as its owner. When an agent is selected, the issue is **delegated** to that agent rather than transferred away from the human. The human remains responsible while the agent performs the current task. Delegated issues remain visible in the human’s views, and agent participation can be filtered and analyzed separately. ([Linear][6])

This is an excellent design decision.

Many agent products force one of two bad abstractions:

* the agent is merely an invisible tool invoked by a person; or
* the agent is treated as though it can truly own an organizational obligation.

Linear avoids both. The agent has enough identity to be observable, addressable, permissioned, and measurable, but it does not become the accountable party.

The conceptual model is:

```text
Issue
├── Human assignee: accountable owner
├── Agent delegate: current executor
├── Agent session: bounded run
├── Activities: visible work history
├── Plan: current and upcoming steps
├── Result: comment, artifact, PR, or update
└── Human decision: accept, redirect, merge, or stop
```

That structure is considerably better than dropping an agent-generated comment into an issue and pretending the comment represents a managed execution.

---

## 4. Native Linear Agent

Linear Agent is available by default and can be disabled by an administrator. It acts within the initiating user’s existing permissions: it can only reference or modify material that user can access. Its native context includes teams and subteams, initiatives, projects, milestones, cycles, issues and relationships, comments, activity, and documents. ([Linear][7])

Its baseline capabilities include:

* creating and updating issues, projects, milestones, and initiatives;
* answering questions about workspace data;
* summarizing projects, cycles, threads, and customer feedback;
* drafting specifications, documents, and project updates;
* posting, editing, and deleting its own comments;
* grouping related issues and extracting shared requirements;
* and turning conversations in Slack into structured Linear work. ([Linear][3])

### Skills

A useful conversation or workflow can be saved as a **Skill**. Skills can be personal or team-shared and invoked manually, such as with a slash command, or selected by Linear Agent when relevant. Linear gives examples such as:

* producing a project from a PRD;
* triaging issues consistently;
* summarizing new customer feedback;
* preparing a weekly focus report;
* and drafting standardized project updates. ([Linear][7])

Skills are essentially versioned organizational prompts attached to a shared context system. Their value is less “prompt engineering” than **making a useful judgment procedure reusable and inspectable**.

### Guidance

Workspace and team guidance provide persistent instructions to native and external agents: repository conventions, expected review process, issue-linking rules, architectural notes, or other team-specific behavior. Team guidance can override workspace guidance, and guidance is maintained in Markdown with history. Whether an external agent obeys it ultimately depends on that integration’s implementation. ([Linear][6])

This establishes a sensible hierarchy:

```text
Personal preferences
        ↓
Workspace-wide guidance
        ↓
Team-specific guidance
        ↓
Skill or Loop instructions
        ↓
Issue and conversation context
        ↓
Current user prompt
```

That resembles an organizational policy stack more than an ordinary chatbot system prompt.

---

## 5. Loops: the operational automation layer

**Loops are the most strategically important addition after the native agent.** A Skill is a reusable procedure that someone invokes; a Loop runs in the background on a schedule or when issues meet defined conditions.

A Loop can, for example:

* inspect incoming bugs;
* research likely root causes;
* delegate suitable bugs to a coding agent;
* create follow-up work from incident reports or meeting transcripts;
* keep specifications or launch plans updated;
* monitor recently started projects;
* and notify teams through connected tools.

Loops are configured with a trigger, plain-language instructions, optional tools, scope, and permissions. They run at the team or workspace level and provide shared visibility into configuration and previous runs. Changes are drafted and then published, allowing configuration to be reviewed before it becomes active. ([Linear][8])

### Why Loops matter

Loops convert Linear Agent from a reactive assistant into a rudimentary **event-driven operational system**:

```text
Event or schedule
       ↓
Gather Linear and connected context
       ↓
Apply instructions and judgment
       ↓
Take Linear, messaging, or coding actions
       ↓
Record the run
       ↓
Escalate or return work to humans
```

This is qualitatively different from adding AI actions to a traditional rules engine. A rule says, “When label equals X, set priority to Y.” A Loop can say, “When a plausible customer-impacting bug arrives, investigate it, inspect the code, decide whether the evidence supports a safe fix, and either open a coding session or summarize what an engineer should investigate.”

### Limitations

Loops are best understood as **agentic cron jobs and event handlers**, not a mature DAG or workflow-orchestration platform.

Their instructions are natural language, and the agent applies judgment. That gives them flexibility, but it also means:

* behavior can be nondeterministic;
* external content can influence the run;
* retries and partial execution need careful treatment;
* broad permissions increase potential blast radius;
* and the same Loop can produce different actions as models change.

Linear exposes granular Loop permissions and recommends granting only what is needed. Workspace and team owners can control who creates or manages Loops. ([Linear][8])

For serious production use, teams should place Loops behind narrow scopes, explicit negative instructions, confidence gates, human review points, and cost monitoring.

---

## 6. Triage Intelligence and intake

Triage Intelligence analyzes incoming issues and suggests:

* destination team;
* assignee;
* project;
* labels;
* duplicates;
* and relationships to existing issues.

Users can accept or decline suggestions and inspect the explanation. Selected property types can also be configured to auto-apply. The system compares a new issue against existing workspace data and historical patterns rather than merely classifying the issue’s text in isolation. It is available on Business and Enterprise plans. ([Linear][9])

This matters because intake is where agents can produce disproportionate value. Poorly routed, duplicated, or context-starved work poisons every downstream step. Linear can now turn a Slack thread, support request, email, or issue into:

1. a structured issue;
2. a proposed classification;
3. links to related work;
4. an initial code investigation;
5. and potentially a draft fix.

Linear’s strongest end-to-end workflow is therefore not “ask a chatbot a question.” It is:

```text
Customer report or internal message
              ↓
Structured intake
              ↓
Triage, routing, and relationship detection
              ↓
Workspace and code investigation
              ↓
Agent delegation
              ↓
Implementation
              ↓
Human review
              ↓
Merge and status update
```

That is a credible product-development loop.

---

## 7. Code Intelligence, Coding Sessions, and Diffs

### Code Intelligence

Code Intelligence allows Linear Agent to inspect connected GitHub repositories and use repository structure, commits, and relevant code paths while answering questions or investigating issues. Teams can supply architectural and repository-specific guidance. ([Linear][10])

This is particularly useful for support and product personnel. A person can ask why a workflow might fail or which recent changes are relevant without first finding an engineer who remembers that portion of the codebase.

### Coding Sessions

Coding Sessions are hosted agentic coding workflows. When an issue is delegated to Linear Agent, Linear can start a secure cloud session through Claude Code or Codex, inspect the issue and codebase, propose an approach, edit code, open a pull request, and attach a diff to the issue. A user can then review and merge from Linear. Coding Sessions are available on Basic, Business, and Enterprise plans and consume AI credits. ([Linear][11])

The important feature is not that Linear has access to Claude Code or Codex. Many products can invoke those. The advantage is that the coding run begins with the product context already attached and remains a **multiplayer session**:

* teammates can observe progress;
* add missing context;
* redirect the implementation;
* request changes;
* or take over.

The session belongs to the organization instead of being trapped inside one engineer’s private IDE conversation. ([Linear][5])

### Diffs and review

Linear Diffs places the PR review beside the issue and discussion that produced the change. It includes structural diffing and AI-guided explanations intended to focus attention on meaningful changes. Linear Agent remains available during review to explain or modify the implementation. ([Linear][5])

As of July 30, Linear also supports:

* reviewing and steering coding sessions from mobile;
* line-specific mobile comments;
* Guided Reviews on Business and Enterprise;
* signed commits for coding sessions;
* GitHub team review assignment;
* and delegation from Linear into GitHub Copilot’s cloud agent. ([Linear][12])

This acknowledges the next obvious bottleneck: once agents generate more code, **review capacity—not implementation capacity—becomes scarce**. Ramp reported that its internal agent accelerated changes but moved the constraint to review. ([Linear][13])

---

## 8. The third-party Agent Platform

The external Agent Platform is architecturally more interesting than the native assistant.

An agent is built as an OAuth application using `actor=app`. The application becomes an app user in each installed workspace, receives team-scoped access, and may be configured as mentionable or assignable. App-mode integrations cannot request Linear’s administrative scope. Installed agents do not count as paid seats. The APIs remain in **Developer Preview** and may change before general availability. ([Linear][14])

### The interaction protocol

The basic execution path is:

```text
User mentions agent or delegates issue
                ↓
Linear creates AgentSession
                ↓
Webhook sent to external agent runtime
                ↓
Runtime receives prompt and packaged context
                ↓
Agent posts activities and updates session state
                ↓
Linear renders progress, plan, actions, questions, and result
                ↓
Agent links PR or external execution session
```

The external runtime must respond to a webhook within five seconds and should publish an initial activity or external-session URL within ten seconds, or Linear may mark the session unresponsive. ([Linear][15])

Agents communicate through semantically typed activities:

| Activity      | Meaning                                            |
| ------------- | -------------------------------------------------- |
| `thought`     | Current reasoning or progress note                 |
| `elicitation` | Request for user input or confirmation             |
| `action`      | Tool call or external operation                    |
| `response`    | Completed result                                   |
| `error`       | Failure or blocked state                           |
| `prompt`      | User-authored follow-up; agents cannot generate it |

Activities can expose tool parameters and results, while ephemeral thoughts or actions can represent transient status. Agent plans are structured lists whose steps can be pending, in progress, completed, or canceled. ([Linear][15])

This is a strong protocol design. It creates a standardized, model-independent interface around an arbitrary external runtime:

* stable identity;
* bounded session;
* typed state;
* visible progress;
* structured plan;
* user questions;
* explicit result;
* and links to external artifacts.

### What Linear does and does not provide

For custom third-party agents, the documented model is an external application that receives webhooks and posts activities. **Linear provides the context, session, identity, UI, permissions, and collaboration layer; the developer provides the actual execution runtime.**

Linear’s native Coding Sessions and Loops do provide managed execution, but the Agent Platform itself is not a generic hosted sandbox, durable job system, multi-agent coordinator, or inference marketplace.

That distinction positions Linear as an **agent interaction and governance protocol attached to a vertical system of record**.

---

## 9. Pricing and economics

Linear’s current base pricing is:

| Plan           |   Annual price | Relevant agent functionality                                                      |
| -------------- | -------------: | --------------------------------------------------------------------------------- |
| **Free**       |             $0 | Agent Platform, Linear Agent, 2 teams, 250 issues                                 |
| **Basic**      | $10/user/month | Unlimited issues; Coding Sessions available with credits                          |
| **Business**   | $16/user/month | Loops, Triage Intelligence, Code Intelligence beta, private teams, Insights, Asks |
| **Enterprise** |         Custom | Business features plus SAML, SCIM, granular controls, and enterprise support      |

Linear says agents themselves are not billable seats, although the company supplying a third-party agent may impose its own fees. ([Linear][16])

### AI credits

Coding Sessions and Loops use a prepaid, pooled workspace balance. Linear gives the following indicative costs:

| Work                          | Typical credit cost |
| ----------------------------- | ------------------: |
| Loop without coding           |         $0.07–$0.20 |
| Copy or styling coding change |            $0.50–$1 |
| Small bug fix                 |               $3–$5 |
| More complex coding task      |                 $5+ |

Other Linear AI features are included in the relevant plan and do not draw from that balance. Coding-session cost varies with model, work duration, and task complexity. ([Linear][17])

The weakness is cost governance. The balance is shared across the workspace, and Linear presently says an administrator cannot restrict credit use to particular authorized members once credits are enabled. Failed runs, retries, and partial completions are billable for consumed resources. Administrators can review usage by feature and user and can prevent guests from using agent functionality, but this is not equivalent to per-user or per-workflow budgets. ([Linear][17])

At the report date, Business and Enterprise workspaces can receive promotional Loop credits worth $20 per seat; those credits expire on August 20, 2026. ([Linear][18])

---

## 10. Security, privacy, and governance

Linear Agent operates under the current user’s permissions. External agents must be installed by an administrator, are granted access to selected teams, and cannot sign in as ordinary users, administer the workspace, or manage members. Loops have their own configurable capabilities, and Linear recommends least-privilege access. ([Linear][7])

Linear states that it does not train on customer data. Its documentation separately warns that third-party agents are governed by their providers’ own data handling policies. Coding Sessions use zero-data-retention models by default, according to the credit documentation. ([Linear][6])

The product nevertheless creates a structural security concern: its value grows as it connects more product, customer, code, messaging, and operational context. That also makes permission mistakes and prompt-injection paths more consequential. MCP, web access, support-system input, issue text, code, and external agents all expand the trust boundary.

### March 24, 2026 incident

On March 24, a Linear access-control regression allowed private-team data to be accessible to other members—including guests—inside the same workspace for approximately one hour. No data was exposed outside the affected workspace, and Linear reported no credential or token exposure and no evidence of malicious abuse. However, issues, comments, attachments, projects, documents, notifications, API responses, and some cached client data could have crossed team boundaries. ([Linear][19])

The root cause was a variable-shadowing bug that caused user-specific permission resolvers to be omitted on a code path not covered by existing tests. Linear reverted the change, reset clients and sessions, notified affected customers, expanded permission-boundary testing, and committed to additional security review and monitoring. Linear specifically noted that the defective change was written and reviewed without AI assistance; the failure was a test-coverage and authorization-validation problem. ([Linear][19])

This incident does not invalidate the product, but it is highly relevant. **Linear Agents depends on broad, accurate context access, so authorization correctness is part of the agent product itself—not merely an infrastructure concern.**

---

## 11. Reliability: where autonomy works and where it breaks

Linear’s own experimentation provides one of the most useful reality checks.

The company initially tried having its agent attempt a first-pass fix on every incoming bug. That performed poorly because many issues did not contain enough context for a one-shot implementation. Linear narrowed the workflow to categories where the agent was consistently useful, added better instructions and tools, and inserted confidence gates requiring the agent to demonstrate an adequate understanding before writing code. ([Linear][20])

The lesson is that autonomy is not a binary property of “the agent.” It depends on:

```text
Task specificity
× Available context
× Tool quality
× Repository conventions
× Verification strength
× Reversibility
× Acceptable risk
```

The strongest current categories are:

* repetitive, scoped maintenance;
* well-understood issue types;
* tasks with clear expected output;
* work that can be tested automatically;
* changes that produce inspectable diffs;
* and investigations that remain useful even when the agent cannot safely implement a fix.

The weakest categories are:

* ambiguous cross-cutting changes;
* architecture requiring tacit organizational knowledge;
* UI work with subjective requirements and no visual validation;
* tasks whose success cannot be tested;
* production actions with irreversible effects;
* and open-ended instructions such as “fix everything in triage.”

This is not a unique Linear limitation. It is the correct operating reality for present agent systems. Linear deserves credit for documenting the failed broad approach rather than only publishing success cases.

---

## 12. Adoption and evidence

Linear reports that, as of March 2026:

* coding agents were installed in more than 75% of its enterprise workspaces;
* agent-completed work had increased fivefold over the preceding three months;
* and agents authored nearly 25% of new issues. ([Linear][1])

Linear also reported that almost 700 agent-authored PRs were merged internally during the month preceding its June Coding Sessions announcement. It cites Ramp’s internal Inspect agent as authoring more than 60% of Ramp’s merged PRs and Coinbase’s Forge as another custom agent built around Linear context. ([Linear][5])

The Ramp case is particularly informative. Inspect combines Linear product context with internal code, documentation, telemetry, feature flags, tests, screenshots, and sandboxed execution. Ramp says the Linear integration gave the agent a structured route from customer request to specification to code. It also says a coding agent produced roughly 90% of the initial Linear integration in around 30 minutes. ([Linear][13])

However, these figures should be treated as **company-reported adoption and customer-case-study evidence**, not independent benchmarks. There is not yet enough public evidence to establish:

* defect rate relative to human-written changes;
* review burden per agent-authored PR;
* cycle-time improvement after review;
* total cost per accepted change;
* percentage of attempted tasks that reach production;
* rollback or incident rate;
* or long-term effect on codebase maintainability.

The evidence supports “real use and strong momentum.” It does not yet prove a universal productivity multiple.

---

## 13. Competitive position

| Product                                 | Center of gravity                                  | Relative position                                                                      |
| --------------------------------------- | -------------------------------------------------- | -------------------------------------------------------------------------------------- |
| **Linear Agents**                       | Product context, delegation, execution, review     | Most coherent vertical product-development control plane                               |
| **Atlassian Rovo**                      | Enterprise knowledge and broad Atlassian workflows | Broader organizational graph, connectors, compliance, and general agent builder        |
| **GitHub Copilot cloud agent**          | Repository and code execution                      | Deeper ownership of code, Actions, PRs, security checks, and developer surface         |
| **Asana AI Teammates**                  | General cross-functional work management           | Broader non-software roles and no-code teammate construction                           |
| **Cursor and standalone coding agents** | IDE and code-generation loop                       | Faster, richer interactive engineering experience but thinner upstream product context |

Atlassian Rovo combines enterprise search, chat, custom agents, automation, and a Teamwork Graph spanning Atlassian and third-party applications. It is a larger and broader platform, although that breadth comes with more suite complexity. ([Atlassian][21])

GitHub Copilot’s cloud agent owns the repository-native execution surface and can be started from GitHub, IDEs, APIs, Slack, Teams, Jira, Azure Boards, and Linear. Linear and GitHub are therefore both competitors and partners: GitHub owns the code and development infrastructure; Linear wants to own the product context and review workflow surrounding it. ([GitHub Docs][22])

Asana AI Teammates are assignable agents with project context, checkpoints, guardrails, and no-code customization across marketing, operations, IT, planning, and product work. Asana is broader horizontally; Linear is substantially deeper in the software lifecycle. ([Asana][23])

### Linear’s defensible position

Linear’s best position is not “our agent writes better code than GitHub or Cursor.” It is:

> **Linear knows why the code should change, who remains accountable, what related decisions matter, where the run belongs, and how the result returns to the team.**

The primary strategic threat is that GitHub, Cursor, or another code-native agent gradually moves upstream into product context. The second threat is that Atlassian delivers a sufficiently good end-to-end agent experience while leveraging its much larger enterprise footprint.

---

## 14. Main strengths

### 1. A coherent end-to-end loop

The product spans intake, planning, investigation, implementation, review, and recurring automation instead of offering disconnected AI features.

### 2. First-class agent identity

Agents can be discovered, mentioned, delegated to, permissioned, monitored, and measured independently of the person initiating a task.

### 3. Human accountability remains explicit

The delegate/assignee distinction avoids pretending an agent can hold responsibility.

### 4. Shared rather than private execution

Agent work is attached to organizational objects and can be observed or redirected by other teammates.

### 5. Model independence

The durable layer is Linear context and workflow. Coding Sessions already use multiple underlying harnesses, and external agents can supply their own runtimes.

### 6. Good provenance and review direction

Agent activity, plans, diffs, text attribution, version history, and checkpoints make generated changes more inspectable than ordinary chat output.

### 7. Strong extensibility model

AgentSession and AgentActivity are clean primitives for integrating external systems without forcing every agent to invent a new Linear-facing UX.

---

## 15. Main weaknesses and risks

### 1. Developer APIs remain preview-quality

The Agent APIs may change, which creates integration maintenance and limits confidence for deeply embedded enterprise systems. ([Linear][14])

### 2. Loops are not deterministic orchestration

There is no visible full workflow graph, typed transition policy, transaction model, idempotency framework, or native multi-agent hierarchy comparable to a dedicated orchestration system.

### 3. Cost controls are too coarse

A shared credit balance without member-level spend authorization is inadequate for very large or cost-sensitive deployments. ([Linear][17])

### 4. Agent quality depends on organizational hygiene

A workspace full of stale issues, thin specifications, conflicting decisions, and undocumented conventions will produce poor context regardless of model quality.

### 5. Review becomes the bottleneck

Generating more candidate changes can simply transfer labor from implementation to review. Linear is addressing this through Diffs and Guided Reviews, but the fundamental constraint remains.

### 6. Context aggregation increases security impact

Every additional connected data source improves agent usefulness while increasing the consequences of access-control errors, malicious issue content, compromised integrations, or misconfigured Loops.

### 7. Evidence remains vendor-heavy

Adoption looks real, but independently measured correctness, ROI, and maintenance outcomes remain limited.

---

## 16. Ideal and poor-fit customers

### Best fit

Linear Agents is particularly well suited to a company that:

* already runs product and engineering work through Linear;
* has GitHub repositories and strong issue-to-PR conventions;
* captures customer feedback and decisions in structured form;
* uses Slack or Teams heavily;
* has repeatable triage and maintenance work;
* can automatically test agent-generated changes;
* and is comfortable keeping humans responsible for acceptance and merge.

### Poorer fit

It is less suitable as the primary automation platform for an organization that:

* remains deeply dependent on customized Jira workflows;
* needs on-premises or air-gapped execution;
* requires deterministic business-process automation;
* needs strict per-user or per-workflow spend limits;
* lacks clean product and technical documentation;
* performs mostly non-software work;
* or needs autonomous actions whose success cannot be verified.

---

## 17. Recommended rollout

A safe deployment sequence would be:

1. **Start with observation and synthesis.** Use Linear Agent for summaries, project updates, related-issue discovery, and workspace questions.

2. **Enable Triage Intelligence with suggestions, not automatic application.** Measure acceptance rates by property type before turning on auto-apply.

3. **Codify proven procedures as Skills and guidance.** Give each skill explicit expected output, scope, and exclusions.

4. **Delegate bounded investigations.** Let agents gather code paths, commits, telemetry, and likely causes before permitting edits.

5. **Enable Coding Sessions for narrow, reversible task classes.** Good initial categories include dead-code cleanup, feature-flag removal, tests, copy changes, and well-isolated defects.

6. **Require confidence and verification gates.** The agent should stop before implementation when evidence, test coverage, or scope is inadequate.

7. **Introduce Loops only after the manual workflow is reliable.** Review every early run, restrict permissions, and monitor both cost and acceptance rate.

8. **Measure accepted outcomes rather than raw agent activity.** Useful metrics include accepted PRs per attempted session, review minutes, rework rate, rollback rate, cost per merged change, and time from intake to verified resolution.

---

## 18. What OpenAgents should take from it

For OpenAgents, Linear is both a design reference and a potentially valuable control surface.

### Patterns worth copying directly

**Human owner plus agent delegate.** An agent can execute without becoming the accountable entity.

**First-class agent identity.** Agents need profiles, capabilities, permissions, activity history, and measurable participation.

**A bounded session object.** Each run should have a state, context package, plan, activities, artifacts, external references, and a clear terminal result.

**Typed visible activities.** Separating reasoning/progress, tool actions, questions, results, and errors produces a much better trust surface than a single streaming transcript.

**Immediate acknowledgment.** Linear’s five-second webhook response and ten-second first-activity expectations recognize that perceived responsiveness is part of agent reliability.

**Organizational guidance with precedence and history.** Persistent workspace and team policies should be versioned inputs to every run.

**Shared execution.** Runs should belong to a thread or workroom, not to the private local state of the person who started them.

### Where OpenAgents can go materially beyond Linear

Linear currently offers a primarily vertical, single-agent control layer. OpenAgents can differentiate through:

* dynamic teams of specialist agents;
* explicit agent-to-agent delegation and handoff;
* provider rotation and failover;
* durable runs that survive client and provider failure;
* typed acceptance contracts;
* independent grading and replay;
* artifact and execution receipts;
* policy-controlled budgets;
* capability grants;
* cross-application workrooms;
* execution across local, cloud, and contributed compute;
* and settlement tied to accepted outcomes.

The natural integration architecture would be:

```text
Linear issue or Loop
        ↓
OpenAgents app user receives AgentSession
        ↓
Session maps to an OpenAgents durable workroom/run
        ↓
OpenAgents routes work across one or more agents
        ↓
Plans and major events stream back as AgentActivities
        ↓
Artifacts, reports, commits, and PRs attach to Linear
        ↓
Human acceptance occurs in Linear or OpenAgents
        ↓
Receipts and final status close the run
```

In that configuration, **Linear is the product-development context and human control surface; OpenAgents is the durable multi-agent execution and economics layer underneath it.**

---

## Bottom line

Linear Agents is not marketing vapor. It is a serious rearchitecture of a product-management application around human-agent collaboration.

Its most important accomplishments are not code generation or chat. They are:

* preserving human accountability while delegating execution;
* giving agents first-class, permissioned identities;
* attaching runs to shared organizational context;
* making agent state and activity visible;
* connecting intake directly to investigation and implementation;
* and returning generated work to an integrated human-review surface.

The product is still young. Its APIs are preview-stage, Loops lack the guarantees of a mature orchestration engine, cost governance is coarse, broad autonomy still fails without careful scoping, and the public performance evidence is primarily self-reported. But the product direction is correct.

**The enduring product is not “Linear’s AI assistant.” It is Linear becoming the operating context in which people and interchangeable agents jointly build software.**

[1]: https://linear.app/next "Issue tracking is dead – Linear"
[2]: https://linear.app/changelog/2025-05-20-linear-for-agents "https://linear.app/changelog/2025-05-20-linear-for-agents"
[3]: https://linear.app/changelog/2026-03-24-introducing-linear-agent "https://linear.app/changelog/2026-03-24-introducing-linear-agent"
[4]: https://linear.app/now/code-intelligence-for-linear-agent "https://linear.app/now/code-intelligence-for-linear-agent"
[5]: https://linear.app/now/coding-sessions-for-linear-agent "Now Linear writes the code, too - Linear"
[6]: https://linear.app/docs/agents-in-linear "https://linear.app/docs/agents-in-linear"
[7]: https://linear.app/docs/linear-agent "https://linear.app/docs/linear-agent"
[8]: https://linear.app/docs/loops "https://linear.app/docs/loops"
[9]: https://linear.app/docs/triage-intelligence "https://linear.app/docs/triage-intelligence"
[10]: https://linear.app/docs/code-intelligence "https://linear.app/docs/code-intelligence"
[11]: https://linear.app/docs/coding-sessions "https://linear.app/docs/coding-sessions"
[12]: https://linear.app/changelog/2026-07-30-coding-sessions-on-mobile "https://linear.app/changelog/2026-07-30-coding-sessions-on-mobile"
[13]: https://linear.app/customers/ramp "https://linear.app/customers/ramp"
[14]: https://linear.app/developers/agents "https://linear.app/developers/agents"
[15]: https://linear.app/developers/agent-interaction "https://linear.app/developers/agent-interaction"
[16]: https://linear.app/pricing "https://linear.app/pricing"
[17]: https://linear.app/docs/ai-credits "https://linear.app/docs/ai-credits"
[18]: https://linear.app/changelog/2026-07-20-introducing-loops "https://linear.app/changelog/2026-07-20-introducing-loops"
[19]: https://linear.app/now/linear-incident-on-mar-24th-2026 "https://linear.app/now/linear-incident-on-mar-24th-2026"
[20]: https://linear.app/now/linear-agent-bug-fix "https://linear.app/now/linear-agent-bug-fix"
[21]: https://www.atlassian.com/software/rovo "https://www.atlassian.com/software/rovo"
[22]: https://docs.github.com/en/copilot/how-tos/use-copilot-agents/cloud-agent/start-copilot-sessions "https://docs.github.com/en/copilot/how-tos/use-copilot-agents/cloud-agent/start-copilot-sessions"
[23]: https://asana.com/product/ai/ai-teammates "https://asana.com/product/ai/ai-teammates"
