# Ben OTEC Revision Readiness

Status: ready to start on operator command. Do not begin this revision until the
operator explicitly says to start.

## Current Production State

- Stable live URL: `https://sites.openagents.com/otec`
- Current active version: `site_version_otec_20260605_revision_2`
- Previous version: `site_version_otec_20260605_initial`
- Dedicated version URLs are available under
  `https://sites.openagents.com/otec/versions/<site_version_id>`.
- The latest customer feedback row is still queued:
  `site_feedback_8cc12f3fb4fc4d68a93b2126b1fcf5bc`.

## Ben's Pending Revision Request

Ben asked for the next revision to:

- switch the Site to light mode with a white background;
- pull in non-copyrighted web images where possible;
- center the concept around a mostly submerged 1000m tower;
- show an inner pipe bringing up cold deep seawater;
- describe the server area around that pipe;
- use approximate dimensions of 20m pipe diameter and 40m total tower diameter;
- mention environmental benefits from bringing up mineral-rich deep seawater;
- mention water aeration and inexpensive seawater mineral mining;
- use the 50m above-water portion for housing, tourism, or similar uses;
- frame future towers as the seed of new cities in international waters;
- mention UHPC, geopolymers, and shotcrete inside airforms;
- keep the Site simple, inspiring, and investor-focused rather than deeply
  technical; and
- mention that equatorial and some other locations avoid hurricane exposure.

## Visual Asset Guardrail

Issue #149 is now implemented in the OpenAgents product surface lifecycle path:

- task packets call out visual asset requirements when the customer request or
  operator notes ask for images;
- build validation blocks image-required candidates that only contain text,
  CSS, SVG, or CSS-only diagrams;
- direct Autopilot artifact callbacks check open revision feedback and the
  assignment objective before setting `customer_review_ready`;
- if images are required and missing, the version may be saved, but it is not
  activated as customer-review-ready and does not trigger the review-ready
  transactional email;
- an operator waiver requires `metadata.visualAssetWaiverReason` on the artifact
  receipt.

## Live Revision Procedure

When the operator says to begin:

1. Start the Autopilot/Adjutant revision run from the queued feedback, not by
   manually editing the active revision in place.
2. Require the task packet to include the Visual Asset Requirements section.
3. Ask the runner to use public-domain, permissively licensed, generated, or
   otherwise reviewed image assets only, and to include attribution/source notes
   in the result summary.
4. Save the output as a new `site_versions` row with a new dedicated version
   URL.
5. Let the lifecycle gate activate it only after the image requirement is
   satisfied or explicitly waived.
6. Verify the stable URL, the dedicated version URL, the customer dashboard
   revision list, and the review-ready email ledger.

## Fallback Procedure

If the remote Autopilot run fails, stalls, or returns a partial artifact:

1. Inspect the latest run events and artifact receipt.
2. Continue locally from the returned source, task packet, and customer
   feedback.
3. Preserve the same output contract: a separate saved Site version, image
   source notes, validation evidence, and a concise customer-facing change
   summary.
4. Do not overwrite prior version artifacts.
5. Do not send or allow the review-ready email until the new version is visible
   in Ben's order dashboard and the visual asset gate passes.
