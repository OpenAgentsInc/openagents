# Autopilot Project Management — Step 0 Internal Dogfood Package

**Status:** Ready-to-use internal operating package  
**Scope:** Step 0 only — GitHub-native internal dogfooding layer  
**Goal:** Let the team start managing work immediately with minimal process overhead and without waiting for native Autopilot PM implementation.

---

## 1. Step 0 operating rules

### What Step 0 is

Step 0 is a lightweight internal operating system built on:

- GitHub Issues
- Labels
- Milestones as cycles
- Issue comments for async status
- Pull requests linked back to work items

It is intentionally simple. Its job is to make work visible, prioritized, and finishable while preserving alignment with the OpenAgents MVP.

### What Step 0 is not

- Not a full Linear/Jira replacement
- Not a governance-heavy process
- Not a long-term storage or protocol decision
- Not a reason to slow down core MVP delivery

### Required issue metadata for internal use

Every active issue should have:

- exactly **1** `type:*` label
- exactly **1** `prio:*` label
- exactly **1** `state:*` label
- exactly **1** `team:*` label once triaged
- **0-2** `area:*` labels
- a clear title
- a short problem or outcome statement
- an assignee when work is committed or started

Optional but recommended:

- milestone for the current cycle
- parent epic link
- due date if externally constrained
- linked PR or commit once implementation starts

### Canonical workflow

Use one shared workflow only:

`Backlog -> Todo -> In Progress -> In Review -> Done -> Cancelled`

Definitions:

- **Backlog** — captured, not yet committed
- **Todo** — accepted into a cycle, not started
- **In Progress** — actively being worked by one clear owner
- **In Review** — implementation complete, awaiting review/verification/merge
- **Done** — accepted and complete
- **Cancelled** — deliberately dropped or superseded

### MVP-first prioritization scale

- **Urgent** — directly blocks the MVP money-printing loop, correctness, or team execution right now
- **High** — should land in the current or next cycle
- **Medium** — valuable, but can wait behind MVP-critical work
- **Low** — useful cleanup or enhancement
- **None** — capture only; not currently prioritized

---

## 2. GitHub label taxonomy

### Label usage rules

1. Keep the taxonomy small.
2. Prefer one meaning per label.
3. Avoid duplicate labels that encode the same thing.
4. Do not invent team-specific labels unless the team is active.
5. If a label stops helping decisions, remove it.

### Label groups overview

| Group | Purpose | When to use it | Example labels |
| --- | --- | --- | --- |
| `type:*` | Classifies the kind of work | Always; every issue gets exactly one | `type:bug`, `type:feature`, `type:research` |
| `prio:*` | Signals urgency and ordering | Always; every issue gets exactly one | `prio:urgent`, `prio:high`, `prio:medium` |
| `state:*` | Tracks workflow stage | Always; every issue gets exactly one | `state:backlog`, `state:todo`, `state:in-progress` |
| `team:*` | Marks the owning team | After triage; every committed issue gets one | `team:desktop`, `team:runtime`, `team:protocol` |
| `area:*` | Gives topical context | Optional; use 0-2 only | `area:wallet`, `area:nostr`, `area:sync` |
| operational | Flags special handling | Only when it changes behavior | `blocked`, `needs-decision`, `needs-repro` |

### `type:*` labels

**Purpose**

- Say what kind of work this is.
- Make saved views and triage faster.

**When to use**

- Always.
- Apply exactly one.

**Starter set**

- `type:bug` — incorrect behavior, regression, broken flow
- `type:feature` — new user-facing capability
- `type:improvement` — upgrade to an existing capability
- `type:task` — implementation work that is not best framed as a feature or bug
- `type:epic` — larger initiative that owns child issues
- `type:research` — investigation, framing, analysis, or decision support
- `type:agent-task` — work intended to be delegated to or executed by an agent

### `prio:*` labels

**Purpose**

- Force explicit ordering decisions.
- Keep the team aligned on what matters now.

**When to use**

- Always.
- Apply exactly one.

**Starter set**

- `prio:urgent`
- `prio:high`
- `prio:medium`
- `prio:low`
- `prio:none`

### `state:*` labels

**Purpose**

- Give the team one shared workflow language.
- Make status visible without meetings.

**When to use**

- Always.
- Apply exactly one.
- When an issue is moved to `Done` or `Cancelled`, update the label and then close the issue.

**Starter set**

- `state:backlog`
- `state:todo`
- `state:in-progress`
- `state:in-review`
- `state:done`
- `state:cancelled`

### `team:*` labels

**Purpose**

- Make ownership obvious.
- Support team-level views and cycle planning.

**When to use**

- Use once triage determines the owning team.
- Apply exactly one on any issue that has entered `Todo`, `In Progress`, or `In Review`.

**Example labels**

- `team:desktop`
- `team:runtime`
- `team:protocol`
- `team:wallet`
- `team:design`

Only create labels for teams that actively exist.

### `area:*` labels

**Purpose**

- Add light topical context without turning labels into a taxonomy project.
- Help find related work across teams.

**When to use**

- Optional.
- Use 0-2 only.
- Prefer stable product or system areas, not temporary initiatives.

**Starter set**

- `area:ui`
- `area:sync`
- `area:nostr`
- `area:wallet`
- `area:provider`
- `area:auth`
- `area:build`
- `area:docs`

### Operational labels

**Purpose**

- Mark issues that need special handling right now.

**When to use**

- Only when the label changes how the team responds.
- Remove the label as soon as the special condition is gone.

**Starter set**

- `blocked` — cannot progress because of an external dependency or unresolved prerequisite
- `needs-decision` — requires a product, architecture, or owner decision
- `needs-repro` — bug exists but reproduction is not yet reliable

**Important note**

- Do **not** create a separate `agent-task` operational label; use `type:agent-task` only.

### Recommended naming conventions for labels

- Use lowercase only.
- Use singular nouns where possible.
- Use prefixes consistently: `type:*`, `prio:*`, `state:*`, `team:*`, `area:*`.
- Avoid spaces except in rare plain operational labels.
- Prefer stable names over clever names.

---

## 3. Issue templates

These templates are designed for low-friction team use. They are intentionally short. The goal is to create useful issues quickly, not write mini-specs.

### 3.1 Bug template

**Use when**

- Something is broken, incorrect, misleading, or regressed.

**Required fields**

- problem summary
- expected behavior
- actual behavior
- impact
- labels: `type:bug`, one `prio:*`, one `state:*`, one `team:*` once triaged

**Optional fields**

- reproduction steps
- logs/screenshots
- suspected area
- linked PR or issue

**Template**

```md
## Summary
<one sentence: what is broken?>

## Expected behavior
<what should happen?>

## Actual behavior
<what happens instead?>

## Impact
- Who or what is affected?
- Does this block MVP-critical flow?

## Reproduction
1. 
2. 
3. 

## Notes
<logs, screenshots, guesses, linked issues/PRs>
```

### 3.2 Feature template

**Use when**

- A new user-facing or team-facing capability is needed.

**Required fields**

- problem or opportunity
- desired outcome
- scope of this issue
- labels: `type:feature`, one `prio:*`, one `state:*`, one `team:*` once triaged

**Optional fields**

- acceptance notes
- linked epic
- dependencies
- rollout notes

**Template**

```md
## Problem / opportunity
<what need are we addressing?>

## Desired outcome
<what should be true when this is done?>

## Scope
- In scope:
- Out of scope:

## Acceptance notes
- 
- 

## Dependencies / links
<epic, related issues, PRs, docs>
```

### 3.3 Research template

**Use when**

- The team needs investigation, comparison, framing, or a recommendation before implementation.

**Required fields**

- question to answer
- why it matters now
- expected output
- labels: `type:research`, one `prio:*`, one `state:*`, one `team:*` once triaged

**Optional fields**

- decision deadline
- references
- linked epic or blocker

**Template**

```md
## Question
<what do we need to learn or decide?>

## Why this matters now
<why is this worth doing in this cycle?>

## Expected output
- Recommendation
- Tradeoffs
- Next-step proposal

## Inputs / references
- 
- 

## Done when
<what artifact or decision closes this?>
```

### 3.4 Agent Task template

**Use when**

- The work is intended for an AI agent or needs to be structured so an agent can execute it with minimal ambiguity.

**Required fields**

- task objective
- constraints
- inputs/context
- expected output
- verification method
- labels: `type:agent-task`, one `prio:*`, one `state:*`, one `team:*` once triaged

**Optional fields**

- repo/path scope
- budget or timebox
- linked human owner
- follow-up tasks

**Template**

```md
## Objective
<what should the agent accomplish?>

## Inputs / context
- Repo/path scope:
- Related issue/epic:
- Relevant docs:

## Constraints
- Do not:
- Must preserve:
- Timebox / budget:

## Expected output
- 
- 

## Verification
<how will a human or system verify completion?>

## Human owner
<who will review or unblock this?>
```

---

## 4. Epic template

Use an epic only when the work clearly spans multiple deliverables or multiple cycles. If the work can be completed as one issue, do not create an epic.

### Required fields

- problem statement
- desired outcome
- scope
- non-goals
- milestones/checkpoints
- child issue checklist
- risks/dependencies
- definition of done

### Optional fields

- owner
- target cycle range
- linked docs
- status summary

### Template

```md
## Problem statement
<what problem is this initiative solving and why now?>

## Desired outcome
<what should be true when this epic is complete?>

## Scope
- In scope:
- In scope:
- Out of scope:

## Non-goals
- 
- 

## Milestones / checkpoints
1. 
2. 
3. 

## Child issues
- [ ] Child issue title
- [ ] Child issue title
- [ ] Child issue title

## Risks / dependencies
- Dependency:
- Risk:

## Definition of done
- 
- 
- 
```

### Epic usage rules

- Give every child issue a backlink to the epic.
- Do not put day-to-day implementation notes only in the epic; keep execution details in child issues.
- Close the epic only when all required child issues are done or intentionally cancelled.

---

## 5. Cycle ritual and lightweight operating runbook

### 5.1 How work gets created

1. Any team member may create a new issue.
2. New work starts in `state:backlog` unless it is already agreed for the current cycle.
3. The creator should choose the best `type:*` and a rough `prio:*`.
4. During triage, the owning team, final priority, and next action are clarified.
5. If the issue is larger than one deliverable, open an epic and then split child issues.

### 5.2 How work gets prioritized

Use these rules:

- Prioritize work that protects or advances the core OpenAgents MVP loop first.
- Prefer finishing active work over starting new work.
- If two items compete, the item with clearer user impact or unblock value wins.
- If work is interesting but not timely, give it `prio:none` and keep it in backlog.

Practical triage order:

1. Urgent bugs
2. Active blockers
3. Current cycle commitments
4. Near-term MVP work
5. Research and lower-priority backlog

### 5.3 How issues move across states

#### `state:backlog`

- Default for newly captured work
- Not yet committed
- May be incomplete, but must have enough information to understand the request

#### `state:todo`

- The issue is accepted into the current cycle
- A team owns it
- It is ready to start without another planning meeting

#### `state:in-progress`

- One owner is actively working it
- The issue should have a clear next step
- If no progress occurs for multiple days, add a status comment or move it back

#### `state:in-review`

- Implementation or investigation is complete enough for review
- PR, validation, or signoff is pending
- Review comments should stay linked from the issue/PR pair

#### `state:done`

- Acceptance criteria are met
- PR is merged or the agreed artifact exists
- The issue is closed after the label is updated

#### `state:cancelled`

- The team explicitly decided not to do it now
- The issue is closed after a short reason is left in a comment

### 5.4 How blockers are handled

When an issue is blocked:

1. Add the `blocked` label.
2. Leave a short comment with:
   - what is blocked
   - why it is blocked
   - who or what is needed to unblock it
   - the next check-in date if known
3. If the blocker is a decision, also add `needs-decision`.
4. If the blocker lasts beyond the current cycle, decide whether to:
   - carry it over,
   - move it back to backlog,
   - or cancel it.

**Blocked comment format**

```md
Blocked on: <dependency or decision>
Need from: <owner/team>
Next unblock check: <date or trigger>
```

### 5.5 Weekly planning and backlog grooming

Run one short weekly session per active team.

**Recommended duration:** 30-45 minutes

**Agenda**

1. Review urgent bugs.
2. Review blocked issues.
3. Review in-progress work that may need help.
4. Pull the next highest-value issues from backlog into `Todo`.
5. Assign the cycle milestone.
6. Confirm each committed issue has:
   - owner/team
   - priority
   - state
   - enough detail to start

### 5.6 Daily async update expectations

Keep this lightweight.

Post a short async update in the issue comment thread when:

- the issue enters `In Progress`
- the issue is blocked
- the issue changes materially
- the issue stays active across multiple days

**Recommended update format**

```md
Yesterday: <what changed>
Today: <next step>
Blockers: <none / blocker>
```

Do not require a daily comment on dormant backlog items.

### 5.7 End-of-cycle review

At the end of each cycle:

1. Review all `Done` issues.
2. Review all unfinished `Todo`, `In Progress`, and `In Review` issues.
3. Ask why unfinished work did not complete:
   - unclear scope
   - hidden dependency
   - too large
   - wrong priority
   - resourcing gap
4. Capture a short list of friction points in one retrospective note.
5. Identify the smallest process fix for the next cycle.

### 5.8 Carry-over rules

Carry work over only when all of the following are true:

- it is still valuable now
- there is a clear owner
- the next step is known
- the work is small enough to finish in the next cycle, or is explicitly split first

Otherwise:

- move it back to `state:backlog`, or
- close it as `state:cancelled` with a reason

### 5.9 Definition of good Step 0 hygiene

The workflow is healthy when:

- active issues have owners
- blocked issues are visibly marked
- the backlog is not a dumping ground for half-written work
- most in-progress items have visible next steps
- cycle carry-over is the exception, not the default

---

## 6. Recommended saved views and naming conventions

### 6.1 Saved views

Use a small set of high-value views first.

| View | What it shows | Why it matters | Suggested filter logic |
| --- | --- | --- | --- |
| My Work | Open issues assigned to me | Personal daily queue | `is:open assignee:@me sort:updated-desc` |
| Current Cycle | Open issues in the active milestone | Team commitment for the week/cycle | `is:open milestone:<current-cycle> sort:updated-desc` |
| Blocked | Open issues that cannot move | Fastest way to find stuck work | `is:open label:blocked sort:updated-desc` |
| Bugs | Open bug issues | Keeps broken behavior visible | `is:open label:"type:bug" sort:updated-desc` |
| Recently Updated | Most recently touched open issues | Good default triage and review view | `is:open sort:updated-desc` |
| Agent Tasks | Open work intended for agent execution | Separates agent-shaped work from human-owned work | `is:open label:"type:agent-task" sort:updated-desc` |

### 6.2 Optional extra view if needed

- **In Review** — `is:open label:"state:in-review" sort:updated-desc`

Only add more views after the team is consistently using the starter set.

### 6.3 Naming conventions

#### Epics

- Format: `[Epic] <outcome-oriented title>`
- Example: `[Epic] Make provider online/offline state legible in desktop`

Rule: epic titles should describe the outcome, not the implementation bucket.

#### Issues

- Format: `<verb> <object> <optional constraint or context>`
- Good examples:
  - `Show wallet sync health in the wallet pane`
  - `Fix stale provider heartbeat after reconnect`
  - `Research Nostr event shape for PM comments`

Avoid vague titles like:

- `Wallet stuff`
- `PM improvements`
- `Refactor issue model`

#### Cycles

- Format: `Cycle YYYY-Www`
- Example: `Cycle 2026-W11`

This is simple, sortable, and works well as a GitHub milestone name.

#### Labels

- Lowercase
- Prefix-based for grouped labels
- Stable terms only
- Examples:
  - `type:bug`
  - `prio:high`
  - `state:in-review`
  - `team:desktop`
  - `area:wallet`

#### Team and project identifiers

- Use short lowercase identifiers.
- Prefer durable names over temporary code names.
- Team examples:
  - `desktop`
  - `runtime`
  - `protocol`
  - `wallet`
- Project examples:
  - `autopilot`
  - `provider-runtime`
  - `spark-wallet`

---

## 7. Immediate setup checklist

Use this as the first adoption pass.

1. Create the starter labels from Section 2.
2. Create one current-cycle milestone using `Cycle YYYY-Www`.
3. Create starter issue templates from Sections 3 and 4.
4. Share this runbook with the pilot team.
5. Move one real slice of work into the workflow.
6. Use the workflow for one full cycle before changing the taxonomy.
7. At cycle end, remove or simplify anything the team did not actually use.

---

## 8. Final recommendation for Step 0

Optimize for clarity and finish rate, not completeness.

If the team can reliably answer these questions from GitHub alone, Step 0 is working:

- What are we doing now?
- What is blocked?
- Who owns it?
- What should happen next?
- What finished this cycle?

If that is true, the dogfood layer is good enough to support internal adoption and inform the later native implementation.