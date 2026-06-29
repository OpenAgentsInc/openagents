# Probe/OpenAgents product surface Run Assignment And Grant Resolution

Date: 2026-06-07

Status: implemented contract slice for Probe issue #158.

## Assignment Shape

Probe runtime assignments may carry OpenAgents product surface provider auth references:

- `provider: "chatgpt_codex"`
- `providerAccountRef`
- `authGrantRef`
- `runnerSessionId`
- `assignmentId`
- optional `leaseRef`
- repo, goal, callback, and sandbox metadata

Assignments must carry refs and grants only. They must not include raw
ChatGPT/OAuth token material.

Assignments may also select a no-auth backend instead of a provider account.
The first implemented backend selection is:

```json
{
  "backend": {
    "kind": "apple_fm_bridge",
    "profile": "apple-fm-local"
  }
}
```

That Apple FM path uses local attach configuration and live health. It does not
use `providerAccountRef`, `authGrantRef`, ChatGPT account linking, or OpenAgents product surface
grant resolution.

The first Gemini backend selection path is also no-auth from OpenAgents product surface's
perspective:

```json
{
  "backend": {
    "kind": "gemini_api",
    "backendProfileId": "gemini-api"
  }
}
```

That path resolves a local runner API key from `GOOGLE_GENERATIVE_AI_API_KEY`
or `GEMINI_API_KEY` and requires `probe.backend.gemini_api`.

OpenAgents product surface-managed Gemini key assignments can additionally carry:

```json
{
  "provider": "google_gemini",
  "providerAccountRef": "provider-account_google_gemini_primary",
  "authGrantRef": "provider-auth-grant_google_gemini_1",
  "backend": {
    "kind": "gemini_api",
    "backendProfileId": "gemini-api"
  }
}
```

That managed path requires `openagents.grant.resolve` in addition to
`probe.backend.gemini_api`, and Probe materializes the resolved key into
`GOOGLE_GENERATIVE_AI_API_KEY`.

## Blueprint Scope

OpenAgents product surface or Pylon dispatch can attach a nested `blueprint` section to narrow the
run without requiring Probe to fetch the full registry on every assignment:

```json
{
  "assignmentId": "assignment_blueprint_1",
  "runnerSessionId": "runner_session_1",
  "goal": "Project the allowed local repo tools.",
  "backend": {
    "kind": "apple_fm_bridge",
    "profile": "apple-fm-local"
  },
  "blueprint": {
    "registryVersionRef": "blueprint_registry.probe_static_fixture.v1",
    "programTypeRefs": ["program_type.probe.tool_menu.project"],
    "programSignatureRefs": ["program_signature.probe.tool_menu.project.v1"],
    "moduleVersionRefs": ["module_version.probe.tool_menu.seed.v1"],
    "contextPackRefs": ["context_pack.openagents.thread_1"],
    "sourceAuthorityRefs": ["source_authority.repo.openagents.probe"],
    "toolScopeRefs": ["tool.probe.read_file", "tool.probe.code_search"],
    "releaseGateRefs": ["release_gate.probe.tool_menu.seed.v1"],
    "backendCapabilityRefs": ["probe.backend.apple_fm_bridge"],
    "actionSubmissionPolicyRef": "policy.blueprint.action_submission.proposals_only.v1",
    "programRunPurposeRef": "purpose.probe.tool_menu.project"
  }
}
```

The optional inline registry slice lives under `blueprint.registry`, with an
optional `blueprint.contractExport`. Probe treats those as untrusted until they
decode against the Blueprint schemas and pass the same safe-projection checks
used by the registry client.

Blueprint refs can narrow runtime scope but cannot widen runner authority. For
example, `backendCapabilityRefs` must match the selected backend; an Apple FM
assignment can name `probe.backend.apple_fm_bridge`, but cannot claim some
other backend capability. Inline registry slices are also cross-checked: when
a slice is present, requested Program Type, Program Signature, Module Version,
and Release Gate refs must appear inside that slice.

Blueprint assignment sections are public/operator refs only. The parser rejects
raw prompts, callback URLs, callback tokens, provider payloads, private repo
content, wallet material, and private customer data. The assignment sanitizer
preserves safe Blueprint refs and strips private-data-shaped Blueprint fields.

## Grant Resolution

`packages/runtime/src/openagents/grant-client.ts` implements an Effect-based OpenAgents product surface
grant resolver. It posts assignment refs to:

`/api/provider-accounts/chatgpt-codex/grants/resolve`

The resolved grant must match the assignment's provider account ref, grant ref,
and runner session. The grant must be unexpired and must include a Probe-shaped
materialization plan, not an OpenCode-shaped env hint.

Current Probe materialization plans use:

- `kind: "probe_chatgpt_auth"`
- `target.name: "PROBE_CHATGPT_AUTH_CONTENT"` for env materialization
- `homeIsolation: "per_run"`
- `scrubAfterCloseout: true`

OpenAgents product surface may return `status: "used"` after a successful one-time resolve. Probe
accepts that resolved response only when the payload includes the
Probe-compatible materialization plan.

## Tests

`packages/runtime/tests/grant-client.test.ts` covers:

- assignment decoding
- fake grant resolution
- provider-account mismatches
- expired grants
- already-used grant records without materialization
- OpenCode env name rejection
- unavailable OpenAgents product surface responses

`packages/runtime/tests/blueprint-assignment.test.ts` covers:

- valid Blueprint-scoped Apple FM assignments
- no-auth Apple FM assignments without Blueprint fields
- missing and invalid registry version refs
- private-data-shaped Blueprint fields
- assignment sanitization preserving refs while stripping unsafe content
- unsafe inline registry slices
- mismatched backend capability refs
- refs outside an attached inline registry slice
