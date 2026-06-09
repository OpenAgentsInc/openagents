# Blueprint Developer Package Contribution v1

Date: 2026-06-06

Status: implemented for issue #326 / `OPENAGENTS-079`.

## Purpose

`BlueprintDeveloperPackageContributionRecord` models reviewed developer
package submissions before runtime authority.

This extends the earlier Program Signature contribution draft shape with the
package refs Epic M needs:

- context packages;
- outcome templates;
- UI bindings;
- backend projection adapters;
- tool packages;
- Program Type refs;
- Program Signature refs;
- Module Version refs;
- fixture refs; and
- release-gate refs.

The implementation lives in
`workers/api/src/blueprint/schemas/developer-package-contribution.ts` and
`workers/api/src/blueprint/services/developer-package-contribution.ts`.

## Capability Families

The current capability-family enum is:

- `agent_tool`;
- `backend_projection_adapter`;
- `context_package`;
- `outcome_template`;
- `program_signature`;
- `retrieval_package`;
- `route_policy`;
- `tool_package`;
- `ui_binding`; and
- `workroom_template`.

The record still carries an intended Blueprint Program family so package
review can tie back to continuation, routing, review, research, proof,
billing, and other Program Signature families.

## Authority Boundary

Developer package contributions are evidence-only. The default authority block
denies:

- execute;
- runtime dispatch;
- deploy;
- spend;
- send email;
- mutate repository;
- post publicly;
- create Site; and
- change public claims.

`blueprintDeveloperPackageContributionHasRuntimeAuthority` detects accidental
authority. `blueprintDeveloperPackageContributionBlockerRefs` reports exactly
why a contribution cannot move forward.

Probe-originated developer package contributions also carry
`noProductionRuntimeAuthority`, `selfPromotionAttempt`, dogfood scope refs,
payment attribution refs, retained failure refs, backend projection adapter
refs, and tool package refs. Release-gate entry requires the no-production
authority flag to remain true and self-promotion to remain false.

## Release-Gate Readiness

`blueprintDeveloperPackageContributionCanEnterReleaseGate` returns true only
when:

- no runtime authority is present;
- status is `approved_for_release_gate`;
- review status is `approved`;
- no rejection or promotion ref exists;
- required fixture refs exist;
- release-gate refs exist; and
- at least one target ref exists across Program Type, Program Signature, Module
  Version, backend projection adapter, context package, outcome template, tool
  package, or UI binding refs.

Release-gate readiness still does not grant runtime authority. It means the
package can enter the reviewed promotion path.

## Projection And Redaction

`projectBlueprintDeveloperPackageContribution` keeps public/customer-safe refs
and removes private or secret-shaped refs.

Projection filters reject raw prompts, raw source archives, raw runner logs,
provider payloads, provider tokens, private repo refs, customer emails, bearer
tokens, OAuth material, wallet material, raw invoices, payment hashes,
preimages, payout targets, private keys, mnemonics, and raw timestamps.

## Tests

`workers/api/src/blueprint/services/developer-package-contribution.test.ts`
covers:

- schema/projection decoding;
- release-gate readiness;
- denial of deploy, spend, email, repository mutation, public posting, Site
  creation, and runtime dispatch;
- blocker refs for incomplete or authoritative contributions;
- package-only targets through context/outcome/UI refs; and
- redaction of unsafe package refs.
