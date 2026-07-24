---
assurance_spec_format_version: "0.1"
assurance_spec_id: "assurance.omega.full.auto.host"
assurance_revision: 1
title: "Omega Full Auto Host AssuranceSpec"
artifact_type: "product_assurance"
lifecycle_state: "proposed"
author: "OpenAgents"
---

## Assurance Objective

This proposed AssuranceSpec binds proof design for the Omega Full Auto host
delta (`specs/omega/full-auto.product-spec.md` rev 1).

It does not admit release.
It does not admit public claims.
It does not replace Desktop Full Auto AssuranceSpec rev 6.

## Subject

```assurancespec-subject
{
  "product_spec": {
    "path": "specs/omega/full-auto.product-spec.md",
    "spec_revision": 1,
    "criterion_refs": [
      "OMEGA-FA-AC-01",
      "OMEGA-FA-AC-02",
      "OMEGA-FA-AC-03",
      "OMEGA-FA-AC-04",
      "OMEGA-FA-AC-05",
      "OMEGA-FA-AC-06",
      "OMEGA-FA-AC-07",
      "OMEGA-FA-AC-08"
    ]
  },
  "upstream_authority": {
    "desktop_product_spec": "specs/desktop/full-auto.product-spec.md",
    "desktop_product_spec_revision": 14,
    "desktop_assurance_spec": "specs/desktop/full-auto.assurance-spec.md",
    "desktop_assurance_revision": 6,
    "freeze": "docs/omega/2026-07-24-full-auto-contract-freeze.md"
  }
}
```

## Risk Model

| Risk | Why it matters | Control |
| --- | --- | --- |
| Second lifecycle in GPUI | Breaks lease and closeout truth | OMEGA-FA-AC-01, OMEGA-FA-AC-06 |
| Guardrail weaken through UI | Own-capacity and workspace binding fail open | OMEGA-FA-AC-03 |
| Receipt leakage to Sync/mobile | Private mission text leaves the host | OMEGA-FA-AC-05 |
| MemoHarness or initiative smuggled into first port | Expands authority without freeze | OMEGA-FA-AC-07 |
| Composer-toggle regression | Ambiguous chat vs unattended run | OMEGA-FA-AC-08 |

## Obligations

| Criterion | Obligation | Evidence tier | Status |
| --- | --- | --- | --- |
| OMEGA-FA-AC-01 | Freeze digests + transition tests in omega-effectd extract | design + unit | needs_observation |
| OMEGA-FA-AC-02 | Capacity and lease tests prove limit 8 and one lease per thread | design + unit | needs_observation |
| OMEGA-FA-AC-03 | Non-overridable guardrail immunity tests | design + unit | needs_observation |
| OMEGA-FA-AC-04 | Default routing order and admitted lane set tests | design + unit | needs_observation |
| OMEGA-FA-AC-05 | Redaction tests for receipt, notification, and Sync projections | design + unit | needs_observation |
| OMEGA-FA-AC-06 | Architecture review: GPUI has no durable run store. Mutations go through run-actions. | design + integration | needs_observation |
| OMEGA-FA-AC-07 | Packet scope review for FA-01..07 excludes MemoHarness and initiative | design | needs_observation |
| OMEGA-FA-AC-08 | No composer-toggle or ambient preference path starts Full Auto | design + UI proof | needs_observation |

## Environments

| Profile | Capability | Gap |
| --- | --- | --- |
| openagents_docs | Freeze and ProductSpec validation | Not a runtime proof |
| omega_effectd_local | Supervised Node service after FA-01 | Not available until FA-01 |
| omega_gpui_dev | Launcher and monitor after FA-03 | Not available until FA-03 |
| omega_packaged_rc | Packaged owner journey after FA-07 | Not available until FA-07 |

## Falsifiers

- A GPUI view, ACP panel, or ordinary chat path becomes Full Auto run authority.
- A landed FA packet invents a second lifecycle enumeration or active-run limit.
- A Sync or notification payload carries raw objective text or credentials.
- A first-port FA packet implements MemoHarness or initiative without a new freeze.

## Gates

- This AssuranceSpec stays `proposed`.
- FA-07 owner journey proof may produce observations.
- Require independent admission before any release or public claim.
