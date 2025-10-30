---

title: "ADR-{ADR_NUMBER}: {ADR_TITLE}"
date: "{DATE}"
status: "Proposed" # Proposed | Draft | In Review | Accepted | Rejected | Deprecated | Superseded
authors:
  - "{AUTHOR_NAME}"
reviewers: [] # When ready for review, add GitHub handles as strings

---

<!--
⚠️  IMPORTANT: If you're an AI agent, read AGENT.md before using this template.

This template is designed for direct, honest architectural decisions.
- Focus on real problems, not abstract benefits
- Acknowledge costs and trade-offs, don't hide them
- Use specific examples from the OpenAgents codebase
- Be realistic about maintenance overhead and adoption challenges

How to use this template:
1. Copy this template to create a new ADR with the proper four-digit numbering (e.g., `0001-your-title.md`)
2. Fill in the frontmatter with the ADR details
3. Write the content of the ADR in the sections below
4. Focus on "why this solves a real problem" rather than "industry best practices"
5. Include failure indicators - when will we know this decision was wrong?
-->

## 1. Context and Problem Statement

<!--
Start with the specific problem this decision solves. Be direct about the pain points.

What concrete issue are we facing right now?
Why does this decision need to be made at this specific moment?
What happens if we don't make this decision?

Ground this in OpenAgents-specific context:
- Mobile constraints (React Native/Expo, iOS/Android)
- Bridge architecture (Rust WebSocket service)
- Agent integration complexities
- Cross-platform considerations (mobile, desktop, web)
-->

## 2. Decision Drivers

<!--
What immediate pain points are forcing this decision?

Focus on concrete problems, not abstract benefits:
- Coordination overhead (people asking the same questions repeatedly)
- Implementation inconsistencies across mobile, desktop, bridge layers
- Onboarding friction for new contributors
- Agent confusion due to lack of architectural context
- Performance bottlenecks or reliability issues
- Maintenance nightmares

Be specific about what's not working right now.
-->

## 3. Considered Options

<!--
List the realistic options that were actually considered. Avoid straw man arguments.

For each option, be honest about pros and cons:
- What does this option actually look like in practice?
- What are the real costs and benefits?
- What assumptions does this option make?

Focus on practical trade-offs, not theoretical advantages.

### Option 1: [Clear, descriptive name]

*   **Description:** What this option actually looks like in implementation
*   **Real-world impact:** How this would change our day-to-day work
*   **Pros:**
    *   Specific benefit 1 (avoid abstract language like "improved efficiency")
    *   Specific benefit 2
*   **Cons:**
    *   Real cost or drawback 1 (be honest about friction/overhead)
    *   Real cost or drawback 2

### Option 2: [Clear, descriptive name]
[...]
-->

## 4. Decision Outcome

<!--
State the chosen option and explain why this specific option solves the real problems identified earlier.

Be direct and honest about the reasoning. Don't use corporate language.

**Chosen Option:** [Name of Chosen Option]

**Rationale:**
Why this specific option over the others?
Connect directly back to the pain points from Decision Drivers.
Acknowledge the trade-offs - this isn't a perfect solution, but it's the best option given our constraints.

Avoid:
- "Industry best practices" (not a reason)
- Abstract benefits without concrete outcomes
- Ignoring the real costs mentioned in Cons section
-->

## 5. Consequences

<!--
Be realistic about what this decision actually costs and benefits us.

### What We Get (The Good Stuff)

- Specific benefit 1 (avoid abstract language like "improved collaboration")
- Specific benefit 2 (focus on concrete outcomes)

### What It Costs (The Real Trade-offs)

- Real cost 1 (be honest about time, complexity, or friction)
- Real cost 2 (acknowledge maintenance overhead or discipline required)

### What Changes (The Cultural Impact)

- How this changes our day-to-day work or decision-making
- Any process overhead or new requirements

**Bottom line:** Acknowledge that this adds overhead but hopefully solves bigger problems.
-->

## 6. Validation Plan

<!--
How will we know if this decision actually worked or was a mistake?

**Success indicators (the "does this actually help?" test):**
- Concrete measurable outcomes
- How we'll know the original problem is solved
- What observable changes we expect to see

**Failure indicators (time to reconsider):**
- What would tell us this decision was wrong
- When to abandon or adjust this approach
- Warning signs that this isn't working

Include a timeline for re-evaluation. Be honest about when we'll pull the plug if this isn't providing value.
-->

## 7. References

<!--
(Optional) List any links to other documents, articles, or resources that are relevant to this ADR.
Examples: related ADRs, technical specifications, documentation, GitHub issues, etc.
-->