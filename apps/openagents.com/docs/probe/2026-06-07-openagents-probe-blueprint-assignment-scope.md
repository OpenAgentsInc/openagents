# OpenAgents product surface Probe Blueprint Assignment Scope

Issue #495 adds the OpenAgents product surface-side assignment surface for Blueprint-scoped Probe
runs. The shared `AgentRunAssignment` schema can now carry an optional
`blueprint` section using the same field names Probe accepts:

- `registryVersionRef`
- `programTypeRefs`
- `programSignatureRefs`
- `moduleVersionRefs`
- `contextPackRefs`
- `sourceAuthorityRefs`
- `toolScopeRefs`
- `releaseGateRefs`
- `backendCapabilityRefs`
- `actionSubmissionPolicyRef`
- `programRunPurposeRef`
- optional `registry`
- optional `contractExport`

The scope is produced through
`workers/api/src/probe-blueprint-assignment-scope.ts`, not by hand at each
dispatch call. That helper derives Program Type, Signature, Module Version,
Release Gate, purpose, tool, and action-submission policy refs from the OpenAgents product surface
Blueprint registry. It rejects raw prompts, callback URLs or tokens, provider
payloads, source archives, private repo material, wallet or payment material,
customer-private data, and unsafe inline registry or contract export slices.

This scope narrows a Probe run. It does not grant runner authority. Current
non-Blueprint assignments continue to omit the section. When a scope is present,
the flat SHC control payload includes `blueprint` beside the existing run,
repository, grant, sandbox, and callback refs. The full assignment object is
still not sent as a raw nested `assignment` payload.

The same module exposes
`probeBlueprintCapabilitySupportCoversAssignmentScope`, which lets SHC/Pylon
routing compare a runner capability report against the assignment's required
registry, signature, module, tool, and backend-capability refs. Capability
reports can prove compatibility, but they cannot widen the assignment scope.
