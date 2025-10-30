# Architecture Decision Records (ADRs)

This directory contains Architecture Decision Records (ADRs) for the OpenAgents project. An ADR is a short, focused document that captures a significant architectural decision. The goal is to have a clear record of why a particular decision was made, the context surrounding it, and the consequences.

## Why ADRs for OpenAgents?

OpenAgents is a mobile-first architecture with multiple layers:
- **Expo App** (`expo/`): Mobile frontend with React Native
- **Bridge Service** (`crates/oa-bridge/`): Rust WebSocket bridge
- **Agent Integration**: Connection to AI coding agents

Given this complexity and the constraints of mobile development, ADRs help us:
- Document architectural decisions across mobile and backend components
- Track the evolution of our mobile-first architecture
- Understand the rationale behind platform-specific choices
- Maintain consistency in our WebSocket-based communication patterns
- Record decisions related to agent interaction patterns

## The ADR Lifecycle

1. **Proposed** - Initial draft, seeking feedback
2. **Draft** - Work in progress, being refined
3. **In Review** - Ready for team review
4. **Accepted** - Decision approved and implemented
5. **Rejected** - Decision not pursued
6. **Deprecated** - Previously accepted but no longer relevant
7. **Superseded** - Replaced by a newer decision

## Creating a New ADR

### Manual Process

1. Copy `docs/adr/template.md` to a new file with proper numbering: `docs/adr/XXX-your-title.md`
2. Fill in the frontmatter and content sections
3. Use three-digit numbering starting from 001 (e.g., `001-adopting-architecture-decision-records.md`)

### ADR Naming Convention

- Use three-digit padding: `001`, `002`, `003`, etc.
- Use kebab-case for the title in the filename
- Keep titles descriptive but concise
- Example: `005-websocket-connection-resilience-pattern.md`

## Review Process

1. **Write ADR** - Create the ADR using the template
2. **Open PR** - Create a pull request with the ADR
3. **Mark Status** - Change status to "In Review" and add reviewers
4. **Iterate** - Address feedback via PR comments
5. **Final Decision** - Update status to "Accepted" or "Rejected"
6. **Merge** - Merge the PR and implement the decision if accepted

## ADR Structure

Each ADR should follow these principles:

- **Focus on "Why"**: Explain the reasoning behind decisions
- **Be Concise**: Keep it short and focused
- **Consider Alternatives**: Show that other options were evaluated
- **Document Consequences**: Both positive and negative impacts
- **Reference Context**: Link to related ADRs, issues, or documentation

## OpenAgents-Specific Considerations

When writing ADRs for OpenAgents, consider:

- **Mobile Constraints**: Platform limitations, performance requirements, app store guidelines
- **WebSocket Architecture**: Real-time communication patterns, connection management
- **Bridge Service**: Rust service design, error handling, scalability
- **Agent Integration**: How decisions affect agent interactions and capabilities
- **Offline Support**: Mobile network reliability considerations
- **Security**: Mobile app security, authentication, data protection
- **Performance**: Mobile device limitations, battery usage, memory constraints

## Index of ADRs

<!-- This table will be updated as ADRs are added -->

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| 001 | Adopting Architecture Decision Records | Proposed | 2025-10-30 |

## Template

See `docs/adr/template.md` for the ADR template with OpenAgents-specific guidance.

## Related Documentation

- [Architecture Overview](../ARCHITECTURE.md)
- [JSONL Schema](../exec-jsonl-schema.md)
- [Permissions Model](../permissions.md)
- [Projects and Skills Schema](../projects-and-skills-schema.md)