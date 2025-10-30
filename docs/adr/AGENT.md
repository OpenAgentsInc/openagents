# AGENT.md

## AI Agent Guidelines for ADR Creation

This document provides guidance for AI agents working with Architecture Decision Records (ADRs) in the OpenAgents project.

### Project Voice and Tone

**Direct and Honest:**
- Avoid corporate "best practices" language
- Be specific about real problems and concrete solutions
- Acknowledge trade-offs and costs, not just benefits
- Use "we need this because" rather than "it is recommended that"

**Experienced Developer Perspective:**
- Focus on practical outcomes over theoretical benefits
- Acknowledge when processes might become bureaucratic
- Be realistic about maintenance overhead and discipline required
- Reference real coordination problems, not abstract concerns

**Key Tone Indicators:**
- Slightly impatient with processes that don't add value
- Direct about costs and friction
- Grounded in specific project context (mobile, bridge, agent integration)
- Skeptical of "industry standards" adopted without justification

### Content Principles

**Problem-First Approach:**
- Start with specific, real problems the project is facing
- Connect every architectural decision to an actual pain point
- Avoid abstract justifications

**Honest Assessment:**
- Acknowledge when solutions add overhead
- Be clear about what won't be solved by the approach
- Include "failure indicators" - when to reconsider the decision

**Concrete Over Abstract:**
- Replace "improve collaboration" with "fewers PR debates about settled decisions"
- Replace "better documentation" with "new contributors don't need to ask why we did X"
- Use specific examples from the OpenAgents codebase

### Common Pitfalls to Avoid

**Corporate Language:**
- ❌ "Industry best practices"
- ❌ "Synergies" or "paradigms"
- ❌ "Leverage" or "optimize"
- ❌ Abstract benefits without concrete outcomes

**Over-Optimism:**
- ❌ Ignoring maintenance costs
- ❌ Assuming perfect adoption
- ❌ Promising benefits without acknowledging trade-offs
- ❌ "Seamless integration" claims

**Process for Process Sake:**
- ❌ Documenting trivial decisions
- ❌ Creating bureaucracy without clear value
- ❌ Templates that encourage box-ticking
- ❌ Metrics that don't measure actual outcomes

### Project-Specific Context

**Always Ground in:**
- Mobile constraints (React Native/Expo, iOS/Android)
- Bridge architecture (Rust WebSocket service)
- Agent integration complexities
- Cross-platform considerations (mobile, desktop, web)

**Real Problems to Reference:**
- No centralized communication platform (Discord/Slack)
- Scattered decision-making across GitHub issues
- Agent confusion due to lack of architectural context
- Implementation inconsistencies across layers

### ADR Structure Guidelines

**Use This Section Flow:**
1. **Context** - What real problem are we solving right now?
2. **Decision Drivers** - What immediate pain points triggered this decision?
3. **Options** - What were the realistic alternatives (not straw men)?
4. **Outcome** - Why this specific option over others?
5. **Consequences** - What does this actually cost us in time and complexity?
6. **Validation** - How will we know if this worked or failed?

**Remember:** Every architectural decision is a trade-off. The goal is to make those trade-offs explicit, not pretend they don't exist.

### Example Voice Comparison

**Instead of:**
> "This approach aligns with industry best practices and will improve team collaboration while reducing technical debt through structured decision-making processes."

**Use:**
> "We're already paying the cost of poor documentation in repeated debates and confused contributors. This makes that cost explicit and gives us a way to avoid having the same arguments every month."

The goal is clarity and honesty, not corporate compliance.