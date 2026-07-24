---
assurance_spec_format_version: "0.1"
assurance_spec_id: "assurance.omega.full.auto.host"
assurance_revision: 3
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
| Fabricated or stale host state | A run dispatches against a nonexistent thread, stale generation, or unready lane | OMEGA-FA-AC-01, OMEGA-FA-AC-03, OMEGA-FA-AC-06 |

## Obligations

| Criterion | Obligation | Evidence tier | Status |
| --- | --- | --- | --- |
| OMEGA-FA-AC-01 | Freeze digests + transition tests in omega-effectd extract | design + unit | needs_observation |
| OMEGA-FA-AC-02 | Capacity and lease tests prove limit 8 and one lease per thread | design + unit | needs_observation |
| OMEGA-FA-AC-03 | Non-overridable guardrail immunity tests | design + unit | needs_observation |
| OMEGA-FA-AC-04 | Default routing order and admitted lane set tests | design + unit | needs_observation |
| OMEGA-FA-AC-05 | Redaction tests for receipt, notification, and Sync projections | design + unit | needs_observation |
| OMEGA-FA-AC-06 | Architecture review: GPUI has no durable run store. Mutations go through run-actions. Generation-fenced host calls prove workspace, thread, lane, turn, interruption, and evidence facts without a second lifecycle. | design + integration | needs_observation |
| OMEGA-FA-AC-07 | Packet scope review for FA-01..07 excludes MemoHarness and initiative | design | needs_observation |
| OMEGA-FA-AC-08 | No composer-toggle or ambient preference path starts Full Auto | design + UI proof | needs_observation |

## Environments

| Profile | Capability | Gap |
| --- | --- | --- |
| openagents_docs | Freeze and ProductSpec validation | Not a runtime proof |
| omega_effectd_local | Supervised Node service (FA-01+) | Automated FA-07 matrix green |
| omega_gpui_dev | Launcher and monitor (FA-03+) | Code-level no-composer proof green |
| omega_packaged_rc | Packaged owner journey | Blocked until signed RC install + owner run |

## Falsifiers

- A GPUI view, ACP panel, or ordinary chat path becomes Full Auto run authority.
- A landed FA packet invents a second lifecycle enumeration or active-run limit.
- A Sync or notification payload carries raw objective text or credentials.
- A first-port FA packet implements MemoHarness or initiative without a new freeze.
- The service parses an oversized frame, or a stale, duplicate, or late host
  reply settles a pending operation or updates cached evidence.
- `omega-effectd` fabricates a workspace, thread, lane admission, live turn,
  interruption result, or evidence row when the Omega host is absent.

## Gates

- This AssuranceSpec stays `proposed`.
- FA-07 may produce observations (see
  `docs/omega/2026-07-24-omega-full-auto-proof-matrix.md`).
- Require independent admission before any release or public claim.
- Do not treat fixture-only or harness-only proof as packaged owner journey.
