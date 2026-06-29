# Probe Blueprint Program Run Evidence

Date: 2026-06-07

Status: implemented for Probe issue #177.

Probe now emits a local Blueprint-compatible Program Run evidence record for
Apple FM tool-stream turns. This is an evidence record, not a write authority
grant. It can be cited by OpenAgents product surface workrooms and receipts once the OpenAgents product surface Program
Run intake path exists, but it cannot authorize PR creation, deploys, email,
payments, public claims, or source-backed business mutations.

The record preserves actor, optional assignment/workroom/thread/order refs,
backend kind/profile/model, Program Type, Program Signature, Module Version,
lookup/menu/registry refs, input snapshot hash, prompt summary ref, route ref,
cost/usage ref, latency, evidence refs, receipt refs, and tool callback refs.
It always sets:

- `authorityBoundary: "evidence_only"`
- `directMutationDisabled: true`
- `noDeploy: true`
- `noEmail: true`
- `noSourceMutation: true`
- `noSpend: true`
- `contentRedacted: true`

The record stores `promptSummaryRef` and `inputSnapshotHash`, not raw prompts.
Typed output stores refs and aggregate facts such as tool-call count and usage
truth, not raw assistant text or file contents. Validation rejects records that
claim deploy, email, source mutation, or spend authority, and rejects
private-data-shaped fields such as raw prompts, callback URLs, callback tokens,
provider payloads, wallet material, and private repo/customer data.
