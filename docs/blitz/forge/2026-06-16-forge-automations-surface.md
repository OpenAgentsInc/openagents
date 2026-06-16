# Forge Automations Surface

Date: 2026-06-16
Status: Implemented first operator surface for #5089
Related: #5088, #5089, #5091

## Contract

The `/forge` dashboard now has an Automations surface next to the production
line. It is deliberately built over the existing Autopilot Work control plane:
running an automation creates a real work order through `POST /api/autopilot/work`
using the same browser-session request path as the Forge cockpit.

## What Shipped

- A configured automation catalog with one unit for each canonical Forge stage:
  Signal, Triage, Code Gen, Validate, Release, Document, Monitor, and Deploy.
- Per-stage automation counts on the production line. These counts are tagged
  `configured`, separate from `live` run metrics and `seeded` placeholders.
- A catalog table where an operator can load a unit into the run template or run
  it immediately.
- An Add / tune form bound to the existing Autopilot work-order draft fields:
  objective, repository, branch, budget, and verification command.
- Success, failure, and submitting states reuse the existing work-order command
  and link the created work order when the API returns one.

## Evidence Boundary

The automation surface does not invent completion evidence. It creates the work
order; the work-order lifecycle remains the source of truth for assignment,
execution closeout, proof refs, review, accepted outcome, delivery receipt, and
settlement state.

Each configured automation has public-safe evidence-ref expectations in the
browser catalog. Those refs are labels for the expected work-order lifecycle;
they do not grant deploy, spend, payout, settlement, or accepted-work authority.

## Remaining Follow-Up

This first version keeps automation configuration in the checked-in browser
catalog plus the editable in-memory run template. A persistent automation store
would be a separate API/schema migration and should preserve the same authority
boundary: configuration may create work-order candidates, but only the existing
review, receipt, and deployment authorities can complete or promote them.
