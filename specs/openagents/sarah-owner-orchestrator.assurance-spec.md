---
assurance_spec_format_version: "0.1"
assurance_spec_id: "assurance.sarah.owner.orchestrator"
assurance_revision: 1
title: "Sarah Owner Orchestrator Assurance Spec"
artifact_type: "product_assurance"
lifecycle_state: "proposed"
author: "OpenAgents"
---

## Assurance Objective

This proposed AssuranceSpec creates exact criterion-to-obligation coverage without claiming that proof design, execution, evidence, admission, or release is complete.

## Subject

The proposal is bound to the exact ProductSpec bytes, revision, path, and stable criterion identifiers below.

```assurancespec-subject
{
  "product_spec": {
    "criterion_refs": [
      "SARAH-AC-01",
      "SARAH-AC-02",
      "SARAH-AC-03",
      "SARAH-AC-04",
      "SARAH-AC-05",
      "SARAH-AC-06",
      "SARAH-AC-07",
      "SARAH-AC-08",
      "SARAH-AC-09",
      "SARAH-AC-10"
    ],
    "document_digest": "sha256:4fe225347dd88c6a0420f2b4749c6647f58f5494b92318dfbcb9254a05d016fc",
    "path": "specs/openagents/sarah-owner-orchestrator.product-spec.md",
    "profile": "openagents_executable_v0.1_exact_document",
    "spec_format_version": "0.1",
    "spec_revision": 1
  }
}
```

## Risk Model

No risk objects are inferred from ProductSpec prose. Reviewers must design the applicable risk model.

```assurancespec-risks
{
  "risks": [],
  "source_digest": "sha256:b5ceafa73367e1d8bb9a6647648ff9fd8c858c84c45e0e1062f21e18800f41d3",
  "source_snapshot": "- “Full knowledge” can become an unsafe database dump. Only purpose-built,\n  bounded, owner-scoped projections enter context.\n- A human name can obscure that Sarah is an AI. The system prompt and product\n  copy identify her as AI and prohibit impersonation.\n- One point of contact can become one point of failure. Her thread is durable,\n  context sources fail independently, and every action stays in existing\n  systems with its own receipts and rollback.\n- Broad company decision authority can collapse separation of duties. Sarah\n  may decide and delegate, but cannot self-verify assurance or self-release\n  from her own evidence."
}
```

## Assurance Scope

Every executable ProductSpec criterion is in assurance scope. No criterion is silently excluded or marked not applicable.

## Environments

Repository facts are proposal context only. No Environment Profile, adapter, capability, or permission is selected by inventory.

```assurancespec-environments
{
  "profiles": [],
  "repository_inventory": {
    "candidate_artifact_refs": [],
    "declared_scripts": [],
    "diagnostics": [
      "repository_not_supplied"
    ],
    "inventory_digest": "sha256:13cef510a746daf9c1d6b2766fef971b7f66c7392a70709fd61ccd271f1b02e4",
    "repository_label": "not-supplied",
    "state": "absent",
    "tracked_file_count": 0,
    "truncated": false
  }
}
```

## Obligations

Each criterion receives one incomplete proposed obligation. Missing proof-design fields project as needs_design and prevent admission or execution.

```assurancespec-obligations
[
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "SARAH-AC-01"
    ],
    "disposition": "required",
    "id": "AO-SARAH-AC-01-01",
    "source_claim_digest": "sha256:7f7973342447e01fdcf6f0f5e9f80066c64c330ca3e859d86c71d70c692d3a4d",
    "source_claim_snapshot": "One authenticated owner maps deterministically to one opaque\nSarah thread; another owner cannot observe or mutate it.",
    "title": "Assure SARAH-AC-01"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "SARAH-AC-02"
    ],
    "disposition": "required",
    "id": "AO-SARAH-AC-02-01",
    "source_claim_digest": "sha256:8e759f08b913e5056cf46dc6854ebdce03a00837248e0c0bfd49067911e156d6",
    "source_claim_snapshot": "The same thread and history survive app restart, device\nchange, and repeated bootstrap without duplicate identities or conversations.",
    "title": "Assure SARAH-AC-02"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "SARAH-AC-03"
    ],
    "disposition": "required",
    "id": "AO-SARAH-AC-03-01",
    "source_claim_digest": "sha256:7ca3699a7eb247bf9e08f54de87d8a61207ced1224da06128a4ca893d6ba5a92",
    "source_claim_snapshot": "Mobile pins Sarah inside the existing conversation UI and\nsends Sarah messages through the hosted Khala runtime; public `/sarah` stays\n404 and no second persona state machine returns.",
    "title": "Assure SARAH-AC-03"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "SARAH-AC-04"
    ],
    "disposition": "required",
    "id": "AO-SARAH-AC-04-01",
    "source_claim_digest": "sha256:9ebe41a70fbd189355dc72742f7b97f415b15daeabcb3e5f6116cf4573bdfda1",
    "source_claim_snapshot": "Current business claims cite exact bounded sources with\nfreshness and owner/private classification; missing sources fail soft and\nremain explicit.",
    "title": "Assure SARAH-AC-04"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "SARAH-AC-05"
    ],
    "disposition": "required",
    "id": "AO-SARAH-AC-05-01",
    "source_claim_digest": "sha256:562effb4dfeab23f4d6416d91bd0c56751db79e27aca30c01495002d26262e6c",
    "source_claim_snapshot": "Model context contains no raw tokens, credentials,\nmnemonics, private filesystem paths, customer-private payloads, or unbounded\ndatabase/tool output.",
    "title": "Assure SARAH-AC-05"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "SARAH-AC-06"
    ],
    "disposition": "required",
    "id": "AO-SARAH-AC-06-01",
    "source_claim_digest": "sha256:c4391a210f8550d984c08067c7c66e2d0b11c4d7013cdecabeaf84c3e3ae8cae",
    "source_claim_snapshot": "Sarah's effective authority is the intersection of the root\nprofile, Sarah profile, active program, target policy, and exact capability;\nexplicit deny wins and self-amplification is impossible.",
    "title": "Assure SARAH-AC-06"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "SARAH-AC-07"
    ],
    "disposition": "required",
    "id": "AO-SARAH-AC-07-01",
    "source_claim_digest": "sha256:15b879b15a4e992f9a6cd46217342620704783024bc2e2ba28539ee60c76b782",
    "source_claim_snapshot": "Visibility never implies mutation. Repository, GCP, release,\nGitHub, Forum, and Full Auto actions enter their existing typed adapters and\nemit bounded authority plus target receipts.",
    "title": "Assure SARAH-AC-07"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "SARAH-AC-08"
    ],
    "disposition": "required",
    "id": "AO-SARAH-AC-08-01",
    "source_claim_digest": "sha256:20466868bd0dbd4d2ea7936f9b4986a57651133f8d4f96c675e517121346b881",
    "source_claim_snapshot": "Financial custody, legal/employment, destructive customer\ndata, stable release without direction, invariant weakening, unsupported\nclaims, and secret export are refused.",
    "title": "Assure SARAH-AC-08"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "SARAH-AC-09"
    ],
    "disposition": "required",
    "id": "AO-SARAH-AC-09-01",
    "source_claim_digest": "sha256:47cd3bd789f514ed0ae91548c3c5407b5214d59f5ed887047697a9a6f53566a3",
    "source_claim_snapshot": "Sarah distinguishes observed fact, inference,\nrecommendation, delegated action, succeeded action, refusal, and unavailable\nstate in owner-visible language.",
    "title": "Assure SARAH-AC-09"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "SARAH-AC-10"
    ],
    "disposition": "required",
    "id": "AO-SARAH-AC-10-01",
    "source_claim_digest": "sha256:5d8960f426ae9487115bd61b5636e9e47d44b340d37c2bd091b9218870e4c5d5",
    "source_claim_snapshot": "Revocation or supersession stops new actions immediately;\nan in-flight action reaches only its safest bounded checkpoint.",
    "title": "Assure SARAH-AC-10"
  }
]
```

## Gates

No execution or release gates are inferred. Gate design remains blocked pending review.

```assurancespec-gates
[]
```

## Evidence Policy

Links are pointers, not verdicts. Missing or unreviewed evidence remains INCONCLUSIVE.

```assurancespec-evidence-policy
{
  "links_are_verdicts": false,
  "missing_evidence_verdict": "INCONCLUSIVE",
  "policy_state": "needs_design",
  "required_for_ready_obligation": [
    "oracle_observation",
    "falsifier_observation",
    "environment_binding",
    "independent_review"
  ]
}
```

## Authority Boundaries

This proposal cannot admit, execute, verify, waive, release, or change a public promise.

```assurancespec-authority
{
  "admitted_roles": [],
  "policy_state": "needs_design",
  "proposal_may_change_public_promises": false,
  "proposal_may_execute": false,
  "proposal_may_release": false,
  "proposal_may_self_admit": false,
  "proposal_may_verify": false,
  "release_roles": [],
  "verifier_roles": []
}
```
