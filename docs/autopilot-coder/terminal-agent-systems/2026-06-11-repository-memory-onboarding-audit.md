# Repository Memory And Onboarding Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #18 from the Bun/Effect terminal-agent systems list. It defines
how a terminal coding agent should learn a repository, produce durable project
memory, detect developer workflow conventions, and refresh that profile when
the repo changes.

## Target

Build an onboarding system that turns the current workspace into a typed
repository profile.

The profile should help the agent start correctly without repeatedly scanning
the same files, guessing commands, or stuffing raw repository content into the
model context.

## User-Visible Capability

The user should be able to:

- Run a first-time project setup.
- See what the agent learned about the repo.
- Keep shared project guidance separate from private local preferences.
- Review suggested setup changes before they are written.
- Refresh the repo profile after dependency, test, or policy changes.
- Ask which build, test, lint, and format commands the agent will use.
- Know when onboarding skipped a missing or unreadable file.
- Avoid generic boilerplate in project memory.

Onboarding should reduce repeated setup cost while staying transparent.

## Repository Profile Shape

The repository profile should include:

- Profile id and workspace root.
- Git identity, default branch, and remote summary.
- Project type and primary languages.
- Package managers and lockfiles.
- Build, test, lint, format, typecheck, and smoke commands.
- Runtime requirements and local services.
- Agent guidance files and invariant ledgers.
- CI entry points.
- Important directories and ownership notes.
- Known generated, vendored, ignored, or read-only paths.
- Workspace safety boundaries.
- Last refresh timestamp and source evidence refs.

Each field should record whether it was derived automatically, confirmed by the
user, imported from a policy file, or manually edited.

## Core Design

Define a `RepositoryOnboardingService` that owns scanning, profile creation,
refresh, and user-confirmed edits.

Suggested service boundary:

```ts
interface RepositoryOnboardingService {
  inspect(request: RepositoryInspectRequest): Effect.Effect<RepositoryInspection, RepositoryOnboardingError>
  propose(request: RepositoryProfileProposalRequest): Effect.Effect<RepositoryProfileProposal, RepositoryOnboardingError>
  apply(request: RepositoryProfileApplyRequest): Effect.Effect<RepositoryProfileReceipt, RepositoryOnboardingError>
  refresh(request: RepositoryProfileRefreshRequest): Effect.Effect<RepositoryProfileRefreshResult, RepositoryOnboardingError>
  load(request: RepositoryProfileLoadRequest): Effect.Effect<RepositoryProfile, RepositoryOnboardingError>
}
```

The context assembler should read the typed profile. It should not rerun broad
repo discovery on every turn.

## Discovery Phases

Use staged discovery:

1. Workspace identity: root, git state, remotes, branch, nested repos, and
   read/write policy.
2. Policy discovery: agent guidance, invariants, ownership docs, and security
   warnings.
3. Build discovery: package managers, lockfiles, config files, scripts, and
   command hints.
4. Codebase map: primary app folders, tests, docs, generated files, and
   external references.
5. Workflow discovery: CI, local services, test tiers, deploy surfaces, and
   release gates.
6. Gap questions: only ask for details that cannot be derived from the repo.
7. Profile write: store a minimal typed profile plus optional human-readable
   guidance.

The scanner should prefer structured project files over prose guessing.

## Proposal Rules

Onboarding may propose changes, but should not silently overwrite existing
project guidance.

Proposal rules:

- Read existing guidance before proposing edits.
- Preserve hand-written local conventions.
- Avoid generic advice that applies to every repo.
- Prefer precise command inventories over broad guesses.
- Separate shared files from private machine-local notes.
- Include a diff or receipt for every write.
- Ask before adding new persistent files.

The best onboarding output is short, factual, and specific to the repo.

## Refresh Model

Refreshing should be incremental.

Triggers may include:

- Lockfile changes.
- Config file changes.
- Script changes.
- CI workflow changes.
- Guidance or invariant file changes.
- User-requested refresh.
- Workspace root change.

Refresh should produce a change summary:

- Added facts.
- Removed facts.
- Changed commands.
- New uncertainties.
- Stale fields retained with a timestamp.

The agent should not treat an old profile as authoritative without showing
freshness.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for profile operations.
- `Schema` for repository profile, evidence refs, proposals, and receipts.
- `Layer` for filesystem, git, package-manager, and settings providers.
- `Stream` for scanning large file sets.
- `Schedule` for refresh retries.
- `Cache` for derived repo identity and command inventory.
- `Redacted` or equivalent wrappers for any local-only sensitive values.

Discovery should be interruptible. Partial results should be usable when a
non-critical source fails.

## Safety Rules

- Do not store secrets, raw environment values, or credentials in the profile.
- Do not include raw private file contents unless the user explicitly asks.
- Do not infer authority from file names alone.
- Do not route work to deprecated or archived directories without policy
  evidence.
- Do not overwrite existing guidance without a proposal and approval.
- Do not use a stale command inventory for destructive actions.
- Do not let project memory override higher-priority runtime policy.
- Do not scan outside the active workspace unless explicitly allowed.

## Tests

Minimum regression coverage:

- Detect git identity from common remote forms.
- Detect package manager and lockfile combinations.
- Extract command inventory from structured project manifests.
- Discover guidance and invariant files without reading unrelated large files.
- Preserve existing guidance when proposing edits.
- Ask a gap question only for missing non-derivable facts.
- Refresh a profile after command changes.
- Mark stale profile fields after source deletion.
- Redact secret-like values from profile storage and receipts.
- Keep nested repo boundaries explicit.

## OpenAgents Translation Notes

When promoted, map repository profiles to OpenAgents workspace refs, policy
refs, invariant refs, capability refs, and onboarding receipts. Verify live
issue state before claiming any repository-memory behavior is implemented.

## Decision

Repository onboarding should be a typed profile system, not a one-time prompt
dump. The agent should derive facts from structured sources, ask only for true
gaps, preserve user control over writes, and use freshness metadata whenever
repo memory guides future work.
