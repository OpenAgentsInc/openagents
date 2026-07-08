# Prompt Suggestions And Autocomplete Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #57 from the Bun/Effect terminal-agent systems list. It defines
how the terminal agent can suggest commands, prompts, files, symbols,
workflows, and follow-up actions without turning suggestions into hidden
intent routing.

## Target

Build a suggestion system that helps users compose better prompts and select
known actions while keeping final intent selection explicit and typed.

## User-Visible Capability

Users should be able to:

- Complete slash commands and command arguments.
- See file, symbol, issue, session, and artifact suggestions.
- Get prompt starters based on current workspace state.
- Accept, edit, or ignore suggestions.
- Disable suggestions.
- Understand when a suggestion will trigger a tool, permission prompt, or
  external action.

Suggestions should reduce friction. They should not silently decide what the
user meant.

## Suggestion Model

Each suggestion should include:

- Suggestion id.
- Kind.
- Display text.
- Inserted text or action ref.
- Confidence.
- Provenance refs.
- Required permissions.
- Privacy classification.
- Expiration or freshness.

Actions and completions should be separate. Inserting text is not the same as
executing a command.

## Bun/Effect Boundary

Use Effect services for:

- `SuggestionIndexService`: indexes commands, files, symbols, sessions, and
  artifacts.
- `SuggestionRankingService`: ranks candidates with typed and semantic signals.
- `AutocompleteService`: streams suggestions to the prompt UI.
- `SuggestionPolicyService`: filters private or unsafe candidates.
- `SuggestionAuditService`: records accepted action suggestions when relevant.

Use Schema for suggestion kinds and actions. Use Stream for incremental
suggestion updates. Use semantic selection where intent routing is broad; use
deterministic parsing only for bounded fields after route selection.

## Safety Rules

- Do not use ad hoc keyword matching for user intent routing.
- Do not show private files or artifacts outside the current scope.
- Do not execute a suggested command on accept unless it is explicitly an
  action selection.
- Do not suggest destructive commands without permission metadata.
- Suggestions from model output are untrusted until validated.
- Stale suggestions should expire.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has command, semantic retrieval, and repo memory
audits already imported, but the terminal-agent README does not yet include a
prompt suggestions/autocomplete audit.

Related anchors:

- Workspace invariant against ad hoc keyword routing.
- #4769 repo connect and data-scope UX for scoped file and repo suggestions.
- #4773 API parity contract for command/action discoverability.

No autocomplete claim should be green until suggestions are scoped, validated,
and separated from action execution.

## Tests

Minimum coverage:

- Complete slash commands and bounded arguments.
- Suggest files only within allowed workspace scope.
- Expire stale file, session, and artifact suggestions.
- Keep action suggestions separate from inserted text.
- Reject destructive action execution without approval.
- Rank semantic candidates without keyword-only routing.
- Disable suggestions through settings.
- Hide private suggestions in public or team contexts.

## Decision

Prompt suggestions should be a scoped assistive layer. The user and typed
runtime still own intent, action selection, and authority.

