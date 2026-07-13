---
assurance_spec_format_version: "0.1"
assurance_spec_id: "assurance.example.checkout.designed"
assurance_revision: 2
title: "Example Checkout Assurance Spec (Designed)"
artifact_type: "product_assurance"
lifecycle_state: "proposed"
author: "OpenAgents"
---

## Assurance Objective

This assurance spec is designed to establish that the example checkout receipt claim holds in a bounded local environment through one deterministic oracle and one falsifier. It explicitly cannot establish release readiness, production behavior, or any claim outside the single bound criterion, and it grants no execution or admission authority to anyone.

## Subject

The subject binding below pins the exact ProductSpec bytes, revision, repository-relative path, and stable criterion identifier so this proof design cannot silently drift to a different product intent. Any change to the bound document digest stales this assurance spec until a human reconciles the difference explicitly and deliberately.

```assurancespec-subject
{
  "product_spec": {
    "criterion_refs": [
      "EX-AC-01"
    ],
    "document_digest": "sha256:ed42e7bbafe2053c96b73c41ad76ef3bc7959b39939c556d3014ecbc51dbacf6",
    "path": "specs/example/checkout.product-spec.md",
    "profile": "openagents_executable_v0.1_exact_document",
    "spec_format_version": "0.1",
    "spec_revision": 1
  }
}
```

## Risk Model

The primary designed risk is a settled payment completing without a durable receipt, which would break the customer evidence chain. The risk object below was written by a reviewer after reading the product spec source snapshot, not inferred mechanically from prose, and it names the harm in falsifiable operational terms.

```assurancespec-risks
{
  "risks": [
    {
      "id": "RISK-EX-01",
      "statement": "A settled payment completes without a durable receipt."
    }
  ],
  "source_digest": "sha256:6b0f0694a0fe42f5d2425f96a5e72d5ac81c180938d29450498656b386621d99",
  "source_snapshot": "A settled payment could complete without producing a durable receipt."
}
```

## Assurance Scope

Every executable criterion of the bound ProductSpec is in assurance scope for this revision. The scope covers the local deterministic test environment only, and it deliberately excludes production traffic, third-party payment provider outages, and load behavior, which are exposed as designed gaps rather than silently ignored by this document.

## Environments

One proposed environment profile describes the bounded local execution context for the designed oracle and falsifier. Repository facts remain proposal context only, and no adapter, capability, or permission is selected by inventory. The profile identifier below is referenced by the single obligation so environment binding stays explicit and checkable.

```assurancespec-environments
{
  "profiles": [
    {
      "id": "ENV-EX-LOCAL",
      "status": "proposed"
    }
  ],
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

The single obligation binds the receipt criterion to a designed oracle, a known-bad falsifier fixture, explicit evidence requirements, and an independence rule that forbids the producer from verifying their own work. Nothing in this section claims the obligation has executed; readiness here means the proof design is complete, not observed.

```assurancespec-obligations
[
  {
    "activation_gate": "GATE-EX-READY",
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "EX-AC-01"
    ],
    "disposition": "required",
    "domains": [
      "contract"
    ],
    "environment_refs": [
      "ENV-EX-LOCAL"
    ],
    "evidence": {
      "proof_rung": "local_fixture",
      "required_kinds": [
        "test_receipt"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "known_bad_fixture",
      "ref": "fixtures/example/settled-without-receipt.json"
    },
    "id": "AO-EX-AC-01-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "tests/example/receipt-oracle.test.ts",
      "statement": "A settled payment fixture yields exactly one durable receipt."
    },
    "source_claim_digest": "sha256:28654dc10c1f34aa2318ce4fbd50f34abba37795ef1caf8c186b7f5670dd4526",
    "source_claim_snapshot": "The checkout flow issues a durable receipt for every settled payment.",
    "technique": "deterministic_test",
    "title": "Assure EX-AC-01"
  }
]
```

## Gates

One activation gate arms the obligation when the designed evidence set exists and remains fresh. The gate expression below is a designed policy statement for later compilation, and evaluating it is a separate execution concern. A gate never admits, approves, or releases anything by itself under the declared authority boundaries.

```assurancespec-gates
[
  {
    "expression": "all required obligations have fresh oracle and falsifier observations",
    "id": "GATE-EX-READY"
  }
]
```

## Evidence Policy

Links are pointers and never verdicts, so missing or unreviewed evidence stays INCONCLUSIVE rather than passing silently. The designed policy requires an oracle observation, a falsifier observation, an environment binding, and an independent review for every ready obligation before any downstream policy may treat the criterion as demonstrated.

```assurancespec-evidence-policy
{
  "links_are_verdicts": false,
  "missing_evidence_verdict": "INCONCLUSIVE",
  "policy_state": "designed",
  "required_for_ready_obligation": [
    "oracle_observation",
    "falsifier_observation",
    "environment_binding",
    "independent_review"
  ]
}
```

## Authority Boundaries

This document cannot admit itself, execute anything, verify results, waive obligations, release software, or change a public promise. The designed roles below name who may admit proof design, who may verify independently, and who may release, and every one of those decisions produces its own receipt outside this artifact.

```assurancespec-authority
{
  "admitted_roles": [
    "assurance_reviewer"
  ],
  "policy_state": "designed",
  "proposal_may_change_public_promises": false,
  "proposal_may_execute": false,
  "proposal_may_release": false,
  "proposal_may_self_admit": false,
  "proposal_may_verify": false,
  "release_roles": [
    "release_owner"
  ],
  "verifier_roles": [
    "independent_verifier"
  ]
}
```
