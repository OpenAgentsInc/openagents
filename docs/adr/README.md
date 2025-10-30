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

### Using the ADR Creation Script (Recommended)

The easiest way to create a new ADR is to use the provided script:

```bash
# From the docs/adr directory
./new.sh "Your ADR Title Here"
```

**What the script does:**
- Automatically finds the next ADR number (maintains 4-digit formatting: 0001, 0002, etc.)
- Creates a properly formatted filename from your title
- Copies the template and fills in placeholders automatically:
  - `{ADR_NUMBER}` - The next available ADR number
  - `{ADR_TITLE}` - Your ADR title
  - `{DATE}` - Current date in YYYY-MM-DD format
  - `{AUTHOR_NAME}` - Your git user.name (or "Unknown" if not set)

**Example usage:**
```bash
cd docs/adr
./new.sh "WebSocket Connection Resilience Pattern"
# Creates: 0002-websocket-connection-resilience-pattern.md
```

**Why use the script?**
- **Consistency**: Ensures all ADRs follow the same naming and formatting conventions
- **No numbering conflicts**: Automatically finds the next available number
- **Template hygiene**: Properly fills all placeholders without manual errors
- **Efficiency**: Creates a complete ADR draft in seconds

### Manual Process

If you prefer to create ADRs manually:

1. Copy `docs/adr/template.md` to a new file with proper numbering: `docs/adr/XXXX-your-title.md`
2. Fill in the frontmatter and content sections
3. Use four-digit numbering starting from 0001 (e.g., `0001-adopting-architecture-decision-records.md`)
4. Replace all placeholder variables manually:
   - `{ADR_NUMBER}` - The ADR number
   - `{ADR_TITLE}` - Your ADR title
   - `{DATE}` - Current date
   - `{AUTHOR_NAME}` - Your name

### ADR Naming Convention

- Use four-digit padding: `0001`, `0002`, `0003`, etc.
- Use kebab-case for the title in the filename
- Keep titles descriptive but concise
- Example: `0005-websocket-connection-resilience-pattern.md`

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
| 0001 | Adopting Architecture Decision Records | Proposed | 2025-10-30 |

## Template

See `docs/adr/template.md` for the ADR template with OpenAgents-specific guidance.

## Related Documentation

- [Architecture Overview](../ARCHITECTURE.md)
- [JSONL Schema](../exec-jsonl-schema.md)
- [Permissions Model](../permissions.md)
- [Projects and Skills Schema](../projects-and-skills-schema.md)