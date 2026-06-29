## Report: Cursor **Origin**, the upcoming “git forge for the agentic era”

**As of June 28, 2026, Origin is not broadly available.** Cursor’s official Origin page is a waitlist landing page with the headline “A git forge for the agentic era” and the positioning line that “code is moving faster than any infrastructure was built to handle.” ([Cursor][1]) Cursor’s launch post says Origin is for “code storage and git hosting,” gives “teams and agents” a place to “host, review, and collaborate on code,” and says it will be “available this fall.” ([X (formerly Twitter)][2])

The short version is: **Origin is Cursor’s attempt to own the repository, review, merge, and agent-coordination layer—essentially a GitHub/GitLab-style forge, but designed around many AI agents cloning, branching, committing, reviewing, fixing, and merging in parallel.** The official public docs are still thin, so the most reliable way to read Origin right now is in three evidence tiers: confirmed official claims, stage-demo transcript claims, and secondary reports from people/coverage that watched the demo.

---

## 1. What Origin is

Origin is a **Git forge**: repository hosting plus collaboration and review workflows. Cursor’s own public page calls it a “git forge,” while Cursor’s launch wording calls it “code storage and git hosting.” ([Cursor][1]) In practical terms, that means Origin is being positioned to cover the GitHub/GitLab/Bitbucket job: repositories, code sharing, reviews, collaboration, merges, and automation around changes.

The key difference is the **primary user model**. Traditional forges assume humans are the main actors and automation is auxiliary. Origin assumes **humans and AI agents are both first-class code collaborators**. In the stage-demo transcript, Tomas Reimers described Origin as Cursor’s “agent-native Git platform” where “you and your agents can create repos, share code, and manage changes” inside the Cursor ecosystem. ([LinkedIn][3])

Origin also fits Cursor’s larger move beyond the editor. Cursor acquired Graphite, a code-review platform, after saying that writing code had become faster while “reviewing changes, merging them safely, and collaborating effectively” were becoming bottlenecks. Cursor also said it wanted tighter integrations between local development and pull requests, smarter code review, and more ambitious collaboration ideas. ([Cursor][4]) Origin looks like the infrastructure-layer expression of that thesis.

---

## 2. Availability and maturity

**Current state:** waitlist / limited access. Cursor’s official page says to join the waitlist and that Cursor will reach out when Origin is ready. ([Cursor][1]) The stage-demo transcript says Origin is already live internally and with “select design partners,” with rollout to everyone planned for fall 2026. ([LinkedIn][3])

**Not yet public:** pricing, enterprise packaging, migration tooling, exact Git protocol details, security model, access-control model, SLA, retention/backups, CI-provider compatibility, import/export behavior, and GitHub feature-parity details. Independent Cursor-focused coverage also warns that the public page is waitlist-first and that buyer/migration/security details are still limited. ([Learn Cursor][5])

---

## 3. Core feature set

### 3.1 Code storage and Git hosting

The officially confirmed base product is **repository storage and Git hosting**. Cursor’s launch copy says Origin gives teams and agents a place to host, review, and collaborate on code. ([X (formerly Twitter)][2]) The demo transcript says users and agents can “create repos, share code, and manage changes.” ([LinkedIn][3])

**Implication:** Origin is not just a Cursor IDE feature. It is a hosted source-control platform.

### 3.2 Agent-native repositories

Origin is designed around **high-concurrency AI-agent workloads**. Reimers said Cursor/Graphite had seen more lines of code, commits, and pull requests as companies adopted AI tooling, so they “went back to basics” and architected a new Git architecture using cloud-provider primitives for scalability, reliability, and performance. ([LinkedIn][3]) The Decoder’s report similarly says Cursor simulated thousands of agents reading from and writing to a single repository at the same time. ([The Decoder][6])

**What this means operationally:** Origin is being optimized for a world where one developer may launch many agents, each creating branches, making commits, opening review requests, responding to comments, and retrying work.

### 3.3 Code review and collaboration

Origin is meant to include a review surface, not just [118;1:3ubare Git remotes. Cursor’s launch wording explicitly includes “review” and “collaborate.” ([X (formerly Twitter)][2]) Graphite’s existing product already provides stacked PRs, a PR page, AI code review, chat, merge queue, PR inbox, and developer metrics; it also advertises a stack-aware merge queue, AI review, CI-failure help, and Git/GitHub integration. ([Graphite][7])

**Careful reading:** not every Graphite feature is officially documented as an Origin feature yet. But Graphite is clearly the review/workflow substrate Cursor brought in, and the demo was led by a Graphite cofounder. ([LinkedIn][3])

### 3.4 Automated merge-conflict resolution

This is one of Origin’s marquee “agent-native” features. The demo transcript says Origin can “resolve merge conflicts.” ([LinkedIn][3]) The Decoder also reports that Origin resolves merge conflicts, fixes failed CI tests, and handles comments. ([The Decoder][6])

**Interpretation:** Origin is trying to move conflict resolution from a human-only operation into an agent-assisted or agent-executed workflow. That is especially important when many agents are editing overlapping files in parallel.

### 3.5 CI failure repair

Origin is described as able to **fix CI failures**. Reimers said Origin is powered by the same intelligence that powers Cursor and can fix CI failures. ([LinkedIn][3]) Graphite already advertises PR-page chat that can fix CI failures and improve PRs, which supports the idea that this capability sits naturally in the review layer. ([Graphite][7])

**Likely workflow:** a PR fails a check; Origin identifies the failed check, determines the likely cause, dispatches or invokes an agent, pushes a fix commit, and re-runs the check.

### 3.6 Comment handling and PR next-step automation

The transcript says Origin can “address comments,” automatically figure out next steps for each PR, and tag the human only when needed. ([LinkedIn][3])

This is a big product distinction. In a human-centered forge, comments are primarily messages. In an agent-native forge, comments become **actionable tasks**: “fix this,” “explain this,” “split this PR,” “add tests,” “rerun CI,” or “resolve conflict.”

### 3.7 API, MCP, and third-party extensibility

The demo transcript says extensibility was the second major design point: Origin would have an API, MCP support, and a third-party app platform, with “generous” rate limits and a comprehensive API. ([LinkedIn][3]) Secondary coverage also describes Origin as Git-compatible and API/MCP-extensible. ([eesel AI][8])

**Why MCP matters:** Model Context Protocol support would let agents interact with repository and PR state in a structured way instead of scraping a web UI. That means agents could ask “what is blocking this PR?”, fetch review comments, inspect CI state, create or update branches, and take follow-up actions.

### 3.8 Git compatibility

Multiple secondary reports describe Origin as **Git-compatible**, meaning the existing Git mental model and tooling should continue to work rather than forcing users onto a completely new VCS. ([eesel AI][8]) Cursor’s official page and launch wording use “git forge” and “git hosting,” which strongly supports that direction, but Cursor has not yet published protocol-level docs for SSH/HTTPS remotes, Git LFS, partial clone, submodules, signed commits, or server-side hooks. ([Cursor][1])

---

## 4. Architecture

### 4.1 High-level product architecture

A reasonable public-evidence model of Origin looks like this:

```text
Cursor IDE / CLI / Cloud Agents
        │
        ▼
Agent control + task orchestration
        │
        ▼
Origin forge API / MCP / app platform
        │
        ├── Repository service: create repos, clone, fetch, push
        ├── Review service: PRs, comments, review state, next steps
        ├── Merge service: merge queue, stacked changes, conflict handling
        ├── CI integration: check status, failed-build diagnosis, fix commits
        └── Policy layer: permissions, review rules, audit, protections
        │
        ▼
Git storage plane
        ├── NVMe-backed Git file servers / hot path
        ├── S3/object storage as durable backing store
        └── replicated global cache / sync / failover layer
```

The top half—IDE, agents, review workflow, API/MCP—is supported by Cursor’s launch copy, the demo transcript, and the Graphite acquisition/product context. ([X (formerly Twitter)][2]) The bottom half—NVMe plus S3, replicas, failover, and sync latency—is from secondary/demo coverage rather than official Origin docs. ([Tech Times][9])

### 4.2 Storage and serving architecture

The reported storage design is **NVMe-backed Git file servers in front of S3/object storage**. Secondary coverage says the architecture uses NVMe-backed Git file servers as the fast path, S3 as the source of truth, and replica scaling/failover around that design. ([Tech Times][9]) Digg’s summary of the launch also describes a hybrid NVMe + S3 architecture that supports “infinite replicas.” ([Digg][10])

**How that likely works:**

```text
Push path:
agent/human git push
  → front-door Git service
  → validate permissions/policies
  → update repo refs/objects on hot NVMe server
  → persist objects/packfiles/metadata to durable object storage
  → fan out invalidation/sync to replicas
  → trigger review/CI/agent workflows

Clone/fetch path:
agent/human git clone/fetch
  → nearest/available Git fileserver
  → serve hot packfiles/refs from NVMe
  → hydrate missing objects from S3/object store
  → cache locally for repeated agent clones
```

That design makes sense for agent workloads because cloning/fetching becomes a very hot read path, while S3-style object storage gives durability and broad replica fan-out. But Cursor has not published low-level details about packfile layout, ref transactions, consistency guarantees, lock management, object deduplication, garbage collection, or failure semantics.

### 4.3 Concurrency architecture

Origin’s key architectural challenge is **many concurrent writers to one repository**. The demo transcript says Cursor simulated thousands of agents and showed push/pull concurrency to a single repo. ([LinkedIn][3]) The Decoder reports the same broad test shape: thousands of agents reading from and writing to a single repository. ([The Decoder][6])

That implies Origin needs more than fast disks. It needs:

1. **High-throughput ref updates** so branches and PR heads can move quickly.
2. **Conflict detection and intent-aware resolution** when agents change overlapping code.
3. **Merge queue / stacking semantics** so related changes can land in order.
4. **CI-aware gating** so agent fixes do not create infinite failing loops.
5. **Backpressure and rate limits** so agents cannot overwhelm the review or build system.

The demo reported very high throughput numbers: 22.6 commits per second in a single repo, roughly 296,000 clones per hour, roughly 81,000 pushes per hour, global synchronization under 400 ms, and automatic failover under 10 ms. These figures are widely reported from the stage/demo, but they should be treated as staged benchmark claims until Cursor publishes reproducible production benchmarks. ([Tech Times][9])

### 4.4 Review-layer architecture

Origin appears to put a **review orchestration layer above Git storage**. That is where Graphite matters. Graphite already has stacked PRs, AI reviews, PR inbox, merge queue, Graphite Chat, protections, reviewer assignment, automations, and Git integration. ([Graphite][7]) Cursor’s acquisition post explicitly framed Graphite around reviewing, merging safely, collaboration, and tighter local-dev-to-PR integration. ([Cursor][4])

For Origin, this likely means Git commits are not the only unit of work. The central unit is probably the **change**, **PR**, or **stack**:

```text
Agent task
  → branch / change
  → PR or stacked PR
  → review comments
  → CI checks
  → agent fixes / human review
  → merge queue
  → protected landing
```

This architecture is much better suited to agent fleets than a simple Git remote because agents need structured state about what is blocked, what passed, what failed, and what still needs human approval.

### 4.5 Agent automation architecture

The demo transcript says Origin is powered by the same intelligence as Cursor and can resolve merge conflicts, fix CI failures, address comments, determine PR next steps, and tag a human only when needed. ([LinkedIn][3]) That suggests an internal agent loop like:

```text
Observe PR state
  → classify blocker:
      conflict / CI failure / review comment / missing approval / stale branch
  → gather repo + diff + CI + comment context
  → generate plan
  → apply patch or respond
  → push fix commit / update PR
  → wait for checks or human approval
```

The core product bet is not merely “faster Git.” It is **automated change management**: keeping code moving after agents generate it.

---

## 5. Feature inventory by confidence level

| Feature                                              |                       Status | Notes                                                                                                                                 |
| ---------------------------------------------------- | ---------------------------: | ------------------------------------------------------------------------------------------------------------------------------------- |
| Code storage and Git hosting                         |                    Confirmed | Cursor’s launch wording explicitly says this. ([X (formerly Twitter)][2])                                                             |
| Waitlist and fall 2026 availability                  |                    Confirmed | Official page is waitlist; launch post says fall. ([Cursor][1])                                                                       |
| Teams + agents as users                              |                    Confirmed | Cursor says Origin is for teams and agents; demo calls it agent-native. ([X (formerly Twitter)][2])                                   |
| Create repos / share code / manage changes           |             Stage-demo claim | Stated in the Origin demo transcript. ([LinkedIn][3])                                                                                 |
| High-concurrency push/pull to one repo               |             Stage-demo claim | Demo transcript says thousands of agents were simulated. ([LinkedIn][3])                                                              |
| Merge-conflict resolution                            |             Stage-demo claim | Stated in transcript and reported by The Decoder. ([LinkedIn][3])                                                                     |
| CI failure fixing                                    |             Stage-demo claim | Stated in transcript; Graphite already advertises CI-failure assistance in PR chat. ([LinkedIn][3])                                   |
| Comment handling                                     |             Stage-demo claim | Transcript says Origin can address comments and tag humans only when needed. ([LinkedIn][3])                                          |
| API, MCP, third-party app platform                   |             Stage-demo claim | Reimers described API/MCP/app-platform extensibility. ([LinkedIn][3])                                                                 |
| Git compatibility                                    |             Reported, likely | Secondary reports say Git-compatible; official wording says git forge/hosting but protocol details are not published. ([eesel AI][8]) |
| NVMe + S3 storage architecture                       |  Reported from demo/coverage | Not yet in official docs; treat as credible but not fully specified. ([Tech Times][9])                                                |
| 22.6 commits/sec, 296k clones/hr, 81k pushes/hr      |           Reported benchmark | Demo metric, not independently verified production SLA. ([Tech Times][9])                                                             |
| Stacked PRs / merge queues                           | Strongly likely via Graphite | Graphite provides these; exact Origin packaging not documented. ([Graphite][7])                                                       |
| Enterprise controls, SLAs, data residency, migration |                      Unknown | Public details remain thin. ([Learn Cursor][5])                                                                                       |

---

## 6. Security, privacy, and governance considerations

Origin will be unusually sensitive because a Git forge stores source code, review history, CI metadata, secrets-adjacent context, and potentially agent traces. Cursor’s general security page says it has a SOC 2 Type II attestation available on request, commits to at-least-annual third-party penetration testing, publishes subprocessors through its trust portal, and grants infrastructure access by least privilege with MFA and monitoring. ([Cursor][11])

Cursor’s general data-use page says that with **Privacy Mode** enabled, customer data is not used for Cursor training, Cursor maintains zero-data-retention agreements with providers, and model providers do not store or train on the data; with Privacy Mode off, Cursor may use/store codebase data, prompts, editor actions, snippets, and other code/actions to improve AI features and train models. ([Cursor][12])

However, **Origin-specific** security and data terms are not yet public. Before storing production source code there, an enterprise should validate at least: SSO/SAML/SCIM, RBAC, repo-level and branch-level permissions, audit logs, signed commits, deploy keys, secret scanning, data residency, retention/deletion, backups, export/mirroring, model-training exclusions, subprocessor scope, incident response, and SLA.

---

## 7. Strategic read

Origin is Cursor moving from **“where code is written”** to **“where code lives, gets reviewed, and gets merged.”** The Graphite acquisition gave Cursor a mature review/workflow layer; Origin adds the hosting/storage layer underneath it. Cursor’s own acquisition post framed the bottleneck as review, merge safety, and collaboration, not typing speed. ([Cursor][4])

The bet is that AI coding makes generation cheap but makes coordination expensive. If one engineer can launch many agents, then the bottleneck becomes: Which changes are safe? Which PRs are blocked? Which CI failures are real? Which conflicts can be auto-resolved? Which changes require human review? Origin is designed to be that coordination plane.

---

## 8. Biggest unknowns

The biggest unresolved questions are not whether Origin can host Git repositories. The hard questions are:

**Trust:** Can an AI-resolved merge conflict be trusted, audited, and explained? A clean merge is not necessarily a correct merge.

**Governance:** Will Origin match GitHub/GitLab enterprise features such as CODEOWNERS, branch protections, environments, audit trails, signed commits, fine-grained tokens, deploy keys, protected tags, and compliance logging?

**Portability:** Will teams be able to mirror out, export complete repo/review history, or run hybrid GitHub/Origin workflows?

**CI/CD integration:** Does Origin provide native CI, integrate with existing providers, or rely on external checks? How will it avoid agent retry loops on flaky tests?

**API/MCP boundaries:** What can agents do by default? What requires human approval? How are rate limits, permissions, and audit events represented?

**Storage semantics:** The reported NVMe + S3 design sounds scalable, but Cursor has not published consistency, failover, durability, backup, garbage-collection, LFS, partial-clone, or monorepo behavior.

---

## Bottom line

Origin is best understood as **Cursor’s agent-native GitHub competitor**, but the “GitHub competitor” framing understates the point. It is a forge designed for a future where AI agents are high-volume contributors and humans supervise, approve, and intervene only when necessary. Confirmed features include code storage, Git hosting, review/collaboration, waitlist access, and fall availability. Stage-demo claims add repo creation, parallel agent workloads, conflict resolution, CI-failure fixing, comment handling, API/MCP extensibility, and PR next-step automation. Reported architecture points to NVMe-backed Git serving with S3/object storage underneath and global replicas for scale.

For now, treat it as **promising infrastructure in private/limited rollout**, not a fully specified production replacement for GitHub/GitLab. The architecture is ambitious; the adoption decision will depend on the still-unpublished details around security, governance, migration, API semantics, and whether AI-resolved code changes are trustworthy enough for your production repos.

[1]: https://cursor.com/origin "Cursor · Origin"
[2]: https://x.com/cursor_ai/status/2067012220832329782?utm_source=chatgpt.com "Cursor · Origin"
[3]: https://www.linkedin.com/posts/corey-stay-adaptatum-3a5506255_tomasreimers-cursor-graphite-activity-7475685319082905600-ojRs?utm_source=chatgpt.com "#tomasreimers #cursor #graphite #shopify #snowflake ..."
[4]: https://cursor.com/blog/graphite "Graphite is joining Cursor · Cursor"
[5]: https://www.learncursor.dev/guides/cursor-origin "Cursor Origin Guide · Learn Cursor"
[6]: https://the-decoder.com/cursor-announces-its-own-ai-model-a-new-git-platform-and-a-mobile-app/ "Cursor announces its own AI model, a new Git platform, and a mobile app"
[7]: https://graphite.dev/ "Graphite - Code review for the age of AI"
[8]: https://www.eesel.ai/blog/what-is-cursor-origin "What is Cursor Origin? Cursor's Git forge for the agentic era | eesel AI"
[9]: https://www.techtimes.com/articles/319031/20260624/cursors-github-rival-origin-new-spacex-model-raise-code-custody-stakes.htm?utm_source=chatgpt.com "Cursor's GitHub Rival Origin and New SpaceX Model ..."
[10]: https://digg.com/tech/4mrw6l09?utm_source=chatgpt.com "Cursor announces Origin, a Git-compatible, MCP ..."
[11]: https://cursor.com/en-US/security "Cursor · Security"
[12]: https://cursor.com/en-US/data-use "Cursor · Data Use & Privacy Overview"


