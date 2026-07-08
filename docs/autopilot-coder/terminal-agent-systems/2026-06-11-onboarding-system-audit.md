# Onboarding System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #55 from the Bun/Effect terminal-agent systems list. It defines
how new users, repositories, teams, Pylons, providers, and agents should be
introduced to the terminal runtime without overclaiming what is live.

## Target

Build an onboarding system that guides setup through typed checks, scoped
permissions, provider readiness, repository profile creation, first-run
smokes, and honest capability declarations.

## User-Visible Capability

Users should be able to:

- Install and start the terminal agent.
- Connect a repository or workspace.
- Review project instructions and invariants.
- Choose local-only, team, or API-connected mode.
- Connect optional providers and integrations.
- Run a first task smoke.
- See which capabilities are ready, missing, blocked, or planned.
- Skip optional setup without breaking core operation.

Onboarding should produce a resolved capability snapshot, not just a friendly
welcome screen.

## Onboarding Model

Each onboarding run should include:

- User or device ref.
- Workspace ref.
- Repository profile ref.
- Selected mode.
- Capability probe results.
- Provider readiness refs.
- Permission decisions.
- Data-scope choices.
- First-run smoke refs.
- Blocker refs.
- Completion and skip receipts.

The same model should support first run, re-onboarding after major updates,
team invitation, and Pylon/provider setup.

## Bun/Effect Boundary

Use Effect services for:

- `OnboardingFlowService`: runs staged setup.
- `CapabilityProbeService`: detects local and account capabilities.
- `RepositoryProfileService`: creates or refreshes repo profiles.
- `OnboardingPolicyService`: resolves required and optional steps.
- `FirstRunSmokeService`: verifies basic runtime behavior.
- `OnboardingProjectionService`: shows setup state.

Use Schema for onboarding steps, probes, capability states, and receipts. Use
Stream for interactive progress. Use Layer to swap local, team, and managed
flows.

## Safety Rules

- Do not ask for secrets in plain text.
- Provider credentials are presence-checked and stored through credential
  policy, not written into config.
- Project instructions and invariants are displayed before actions that rely
  on them.
- Optional integrations stay disabled until approved.
- Capability readiness does not imply payment, payout, provider, or settlement
  authority.
- Re-onboarding must preserve user choices unless policy changes.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has Pylon setup docs, public agent onboarding
instructions, repo/profile concepts, and product-promise gates. The
terminal-agent README does not yet include an onboarding audit.

Related open issue anchors:

- #4769 repo connect and per-mission data-scope UX.
- #4766 account-pool dashboard.
- #4773 API parity contract.
- #4772 MVP exit review.

No onboarding claim should say a user is fully ready for paid, remote,
market-provider, or settlement workflows until the relevant probes and receipts
exist.

## Tests

Minimum coverage:

- Complete first-run setup with no optional integrations.
- Connect a repository and record profile refs.
- Skip optional providers cleanly.
- Reject unsafe secret input.
- Run first-run smoke and record result.
- Refresh onboarding after settings or policy changes.
- Project capability states accurately.
- Preserve data-scope choices.

## Decision

Onboarding should be a capability and policy resolver. It should help users get
started while making missing, blocked, and planned capabilities explicit.
