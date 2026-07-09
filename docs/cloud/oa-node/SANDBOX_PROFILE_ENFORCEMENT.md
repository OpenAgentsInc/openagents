# oa-node Sandbox Profile Enforcement

`oa-node` treats sandbox compute as profile-bound execution, not open-ended
labor.

## Profile Registry

Register a local profile before accepting sandbox worker assignments:

```bash
cargo run -p oa-node -- sandbox profile register \
  --profile-id sandbox.posix.local \
  --profile-digest sha256:sandbox-posix-local-profile \
  --execution-class sandbox.posix.exec \
  --network-policy none \
  --filesystem-policy workspace_only \
  --timeout-ms 60000 \
  --max-artifact-bytes 10485760 \
  --secret-policy brokered_no_raw_secrets \
  --json
```

Profiles are written to `sandbox-profile-policies.json` and projected into
`status.capabilities.sandbox_profiles`. When at least one profile is present,
`status.policy.sandbox_policy` becomes `profile_enforced`.

## Forge Admission

Sandbox worker assignments must include a `sandbox` policy block declaring:

- profile id and digest
- execution class
- network policy
- filesystem policy
- secret policy

`oa-node forge assignment receive` refuses the assignment if the declared
sandbox block is missing, if any field differs from the registered profile, or
if the requested timeout/artifact budget exceeds the profile. Refusals are
normal Forge assignment receipts, so undeclared access attempts are auditable.

Open-ended labor assignments continue to be refused and routed back to
Forge/Probe instead of being accepted as sandbox compute.

## Psionic Receipts

Sandbox Psionic receipts must include `--profile-digest`. Non-sandbox Psionic
receipts can omit it.
