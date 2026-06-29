# Probe Blueprint Contribution Release Gates

Date: 2026-06-07

Status: implemented for Probe issue #181.

Probe-originated Program Signatures, Module Versions, tool packages, context
packages, and backend projection adapters are modeled as Blueprint contribution
drafts. Probe can draft and validate those records, but OpenAgents product surface remains the
canonical Blueprint contribution and promotion authority.

The Probe draft shape mirrors OpenAgents product surface's Signature Contribution Draft and
Developer Package Contribution semantics:

- contributor refs
- source refs
- capability summary ref
- intended Program family and risk class
- proposed Program Type, Program Signature, and Module Version refs
- tool package, context package, outcome template, UI binding, and backend
  projection adapter refs
- fixture refs and retained failure refs
- release gate refs
- review status, rejection ref, and promotion ref
- payment and attribution refs for future promoted packages
- explicit no-runtime-authority flags

Contribution drafts cannot execute, dispatch runtime, mutate repositories,
deploy, spend, send email, post publicly, create Sites, or change public
claims. Optimizer-generated Module Versions cannot self-promote.

Release-gate readiness requires all of the following:

- no runtime authority
- no self-promotion attempt
- `status: "approved_for_release_gate"`
- `reviewStatus: "approved"`
- no rejection or promotion ref
- at least one fixture ref
- at least one release gate ref
- at least one target ref

Readiness still does not grant runtime authority. Candidate or dogfood runtime
use is separate from production runtime use. Probe only treats a candidate as
eligible when an assignment explicitly allows candidate mode and the
contribution is non-authoritative and unrejected. Production runtime use
requires a promoted status and promotion ref.

Public-safe contribution projections must not include raw prompts, raw source
archives, raw runner logs, provider payloads, provider tokens, private repo
refs, customer emails, bearer tokens, OAuth material, wallet material, raw
invoices, payment hashes, preimages, payout targets, private keys, mnemonics,
or raw timestamps.

Future payment and attribution should attach to promoted package refs such as
`tool_package.*`, `context_package.*`, `module_version.*`, and
`program_signature.*`. Probe must not copy raw prompts, private source
archives, or payment material into public contribution projections.
