# OpenAgents product surface Probe Contribution Release-Gate Intake

Issue #498 adds the OpenAgents product surface intake path for Probe-originated Blueprint
contributions.

The route is:

- `POST /api/blueprint/contributions`
- `GET /api/blueprint/contributions`

Probe submits `ProbeBlueprintContributionDraft` records when a Probe run wants
to contribute Program Signatures, Module Versions, tool packages, context
packages, UI bindings, outcome templates, or backend projection adapters back to
OpenAgents product surface. OpenAgents product surface normalizes accepted records into `BlueprintProbeContributionRecord`
rows in D1. The POST route accepts runner-callback authorization with admin API
token fallback for operator verification. The GET route is operator-read only.

Accepted records remain contribution evidence, not runtime authority. OpenAgents product surface
requires every submission to be content-redacted, non-authoritative, and marked
with `noProductionRuntimeAuthority: true`. The intake path rejects self
promotion, raw prompts, source archives, runner logs, provider material,
private repo refs, callback material, wallet or payment secrets, customer
private data, raw timestamps, and secret-shaped strings before schema decoding.

Candidate runtime use is dogfood-only. OpenAgents product surface records
`candidateRuntimeAllowed: true` only when the contribution has a dogfood scope,
has no runtime authority, is not rejected or archived, has no rejection ref, and
is not self-promoting. Candidate use does not make the contribution production
dispatch authority.

Production runtime use requires promotion. OpenAgents product surface records
`productionRuntimeAllowed: true` only for promoted contributions with approved
review, a promotion ref, no rejection, no runtime authority, no self-promotion,
target refs, release gate refs, fixture refs, and retained failure refs.
Drafts, submitted records, in-review records, rejected records, archived
records, and approved-but-not-promoted release-gate candidates cannot become
production runtime inputs through this route.

The current normalized contribution kinds are:

- `signature_contribution` for Program Type, Program Signature, and Module
  Version proposals; and
- `developer_package_contribution` for backend projection adapters, context
  packages, outcome templates, tool packages, UI bindings, and related package
  refs.

The route is now listed in the Blueprint contract export seed so Probe, Pylon,
Nexus, Psionic, and other operator-side consumers can discover the intake
contract without treating draft contribution refs as dispatchable production
authority.
