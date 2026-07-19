# OpenAgents agent compact profile

- Revision: `openagents-agent-compact-v1`
- Base: ASD-STE100 Issue 9
- Status: active overlay
- Owner: OpenAgents documentation

## Purpose

Use STE for communication between agents when possible.
Clear STE text usually gives agents the fastest and safest input.
Some agent records need dense technical data and exact system terms.
This profile permits that content in a controlled form.

This profile extends the OpenAgents vocabulary.
It does not replace ASD-STE100 and does not claim ASD conformance for an extension.

## Permitted use

Use this profile only for text that has agents as its primary readers.
Examples include claims, handoffs, receipts, agent changelogs, and machine review records.

A dual-audience document must identify each audience with a heading.
Use `Human changelog` and `Agent changelog` in a release document.
Apply the base STE profile to all human sections.
Apply this overlay only below the agent heading.

## Permitted extensions

An agent section can use these extensions:

- Exact identifiers, paths, issue references, commit IDs, schema names, and protocol values
- Approved terms from [`agent-compact-terms.v1.json`](./agent-compact-terms.v1.json)
- Labeled record fragments for scope, contracts, invariants, evidence, state, and lane
- Compact tables or lists when each field has one stable meaning
- Compact control clauses when each clause has one obligation
- Dense technical sentences when a split would separate a condition from its control
- Dense record paragraphs when a split would make the agent scan more units
- A complex technical term when a shorter term would increase ambiguity

Prefer the shortest text that keeps the complete meaning.
Do not remove a dependency, condition, exception, evidence link, or authority reference.

## Controls that do not change

Do not use this profile to relax a safety or authority requirement.
Do not use it to hide uncertainty or an incomplete proof state.
Do not use a term with more than one meaning in the same context.
Do not use semicolons or contractions in full sentences.

Keep sentence and paragraph limits when they help an agent read the text.
An identified reviewer can accept a sentence or paragraph limit diagnostic in an agent-only document.
The reviewer must confirm that the dense unit is faster to parse and keeps one control context.
This exception cannot apply to a human or dual-audience document.

Human-facing text cannot use this overlay.
Public instructions, warnings, consent text, and operator procedures require the base STE profile.
Code and immutable evidence remain source data.

## Review

The author must identify the audience and the compact-profile revision.
The reviewer must confirm that each extension makes agent communication faster or less ambiguous.
The reviewer must also confirm that a human section does not depend on the agent section.

The RC.25 dual changelog is the reference pattern.
Its human section explains user actions and release limits.
Its agent section records issues, commits, contracts, invariants, evidence, and lane ownership.
The released RC.25 file is immutable evidence and must not be rewritten.
