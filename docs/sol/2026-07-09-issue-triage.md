# Issue triage receipt — Sarah Fleet Command roadmap reset

- Date: 2026-07-09
- Before: 30 open issues from the pre-Sarah-Fleet-Command roadmap
- After: 15 open issues under [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md)

> Reconciliation note: this audit records the initial reset exactly. The
> subsequent live set first reached 19 open issues after the dependency-held
> and GL additions. On 2026-07-10, #8639, #8648, and #8649 closed, leaving 16
> open roadmap issues: 14 active P0/P1 program issues and two dependency-held
> P2 issues. This note updates current arithmetic without changing the original
> disposition receipt below.
- GitHub label: `roadmap:sol`

## Triage rules

- Keep an old issue only when its remaining acceptance maps cleanly to the new
  product and is not better owned by a new end-to-end lane.
- Close, rather than leave open, completed slices whose residual moved.
- Close broad conversion work when the new decision is deletion.
- Fold small quality experiments into one parallel presentation lane.
- Postpone sales/payment rather than letting them obscure the coding unblock.
- Preserve landed code and receipts even when the planning issue closes.

## Retained and rewritten issues

| Issue | New role |
| --- | --- |
| #8547 | FC-CLOUD-1: Codex inside Agent Computers |
| #8566 | APP-1 epic: three OpenAgents Effect Native apps |
| #8574 | APP-DESKTOP: OpenAgents Desktop |
| #8595 | APP-WEB-LANDING: retained root landing |
| #8597 | APP-MOBILE: OpenAgents mobile |
| #8600 | FC-BRAIN: Khala inference hardening |
| #8610 | One parallel Sarah presentation/quality lane |

## New issues

| Issue | Role |
| --- | --- |
| #8638 | P0 Sarah Fleet Command epic |
| #8637 | FC-1 Sarah tool + durable run contract |
| #8633 | FC-2 real mixed-harness standing executor |
| #8639 | FC-3 Sarah supervision and steering |
| #8636 | FC-4 hybrid local/cloud routing |
| #8640 | FC-5 live multi-stream dogfood burn |
| #8634 | One web host + public-page retirement |
| #8635 | Retained Forum on Effect Native |

## Closed as superseded or postponed

| Old issue | Disposition | Successor/current owner |
| --- | --- | --- |
| #8467 | Old Khala Code mobile-only epic closed | #8566, #8597 |
| #8543 | Old standalone mobile launch lane closed | #8597 retains its E2E floor |
| #8546 | Mobile-specific Codex connect issue closed | #8597 settings, #8633 capacity |
| #8548 | Per-thread harness-pill issue closed | #8637 policy, #8639 supervision |
| #8549 | Local Claude parity done; cloud residual moved | #8636, #8547 |
| #8550 | Standalone ephemeral resume issue closed | #8633, #8636 |
| #8551 | Standalone cloud account concurrency issue closed | #8633, #8636 |
| #8552 | Old mobile daily-driver ergonomics issue closed | #8639, #8597 |
| #8553 | Old Khala Code mobile dogfood exit closed | #8640 |
| #8558 | Outbound identity residual postponed | Re-file after P0 when sales returns |
| #8561 | Outbound approval/reply residual postponed | Re-file after P0 when sales returns |
| #8571 | effectnative.org owner verification removed from roadmap | Owner follow-up only |
| #8573 | Broad web absorption closed | #8634 deletes most pages instead |
| #8575 | Broad Verse/canvas conversion closed | #8639/#8574 for retained canvas use |
| #8578 | Independent pylon-core extraction program closed | #8633/#8574 consume required pieces |
| #8579 | Separate Pylon cockpit issue closed | #8574 + #8639 |
| #8580 | OpenTUI retirement deferred | Revisit after #8574 parity |
| #8588 | Standalone cloud multi-harness phase closed | #8636 |
| #8607 | In-conversation payments postponed | Re-file as later Sarah capability |
| #8615 | Pre-rendered takes folded | #8610 |
| #8616 | Quality ladder folded | #8610 |
| #8619 | Experiment matrix folded | #8610 |
| #8620 | Perfect opener library folded | #8610 |

All 23 closed issues received `superseded:sol` and a closeout comment naming
the successor or explicit postponement. They were closed as `not planned`, not
misrepresented as fully completed.

## Important consequences

- The old queue head (#8615) is gone. Presentation is no longer the serial
  blocker.
- P0 begins with #8637 and #8633, then #8639 and #8640 Phase A.
- Managed cloud (#8547/#8636) runs alongside the local unblock.
- There is no active generic route-conversion issue. #8634 retires pages.
- There is no active Khala Code product epic. Mobile and desktop are greenfield
  OpenAgents applications under #8566/#8597/#8574; the old RN/Swift mobile and
  Electrobun desktop clients are frozen non-shipping extraction sources.
