# Architecture Decision Records (ADRs)

This directory contains Architecture Decision Records (ADRs). An ADR is a short, focused document that captures a significant architectural decision. The goal is to have a clear record of why a particular decision was made, the context surrounding it, and the consequences.

## Why ADRs?

Architecture Decision Records help teams:
- Document architectural decisions and their rationale
- Track the evolution of system architecture over time
- Understand the reasoning behind design choices
- Maintain consistency in architectural patterns
- Record decisions for future reference and team onboarding
- Facilitate better decision-making through structured evaluation

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
./new.sh "Database Connection Pooling Strategy"
# Creates: 0002-database-connection-pooling-strategy.md
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
- Example: `0005-database-connection-pooling-strategy.md`

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

## Project-Specific Considerations

When writing ADRs, consider aspects relevant to your project:

- **Technology Stack**: Framework constraints, platform requirements, compatibility
- **Performance Requirements**: Latency, throughput, resource utilization
- **Security**: Authentication, authorization, data protection, compliance
- **Scalability**: Load handling, growth projections, capacity planning
- **Maintainability**: Code organization, testing, documentation standards
- **Team Considerations**: Skill sets, development workflow, deployment processes
- **Business Constraints**: Budget, timeline, stakeholder requirements

## Index of ADRs

<!-- This table will be updated as ADRs are added -->

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| 0001 | Adopting Architecture Decision Records | Proposed | 2025-10-30 |

## Template

See `docs/adr/template.md` for the ADR template with guidance for creating effective ADRs.

## Related Documentation

- [Architecture Decision Records](https://adr.github.io/) - ADR specification and examples
- [MADR - Markdown Architectural Decision Records](https://github.com/joelparkerhenderson/architecture_decision_record) - Template and process guidance