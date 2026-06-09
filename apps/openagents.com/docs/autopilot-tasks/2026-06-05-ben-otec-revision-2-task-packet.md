# Ben OTEC Site Revision 2 Task Packet

Status: active Adjutant Site adjustment packet for Ben's OTEC Site revision.

## Identity

- Assignment ID: `adjutant_assignment_d98b2a644ff742a2b21283653020a8e1`
- Adjustment ID: `adjutant_adjustment_fd98742fe2394802b9d32d55f65cba8a`
- Customer feedback ID: `site_feedback_8cc12f3fb4fc4d68a93b2126b1fcf5bc`
- Software order ID: `software_order_c34f3a52d60b41d699b71525365b6ee5`
- Site ID: `site_project_otec`
- Current public URL: `https://sites.openagents.com/otec`
- Current version ID: `site_version_otec_20260605_initial`
- Current deployment ID: `site_deployment_otec_20260605_initial`
- Target revision: Revision 2, saved as a separate `site_versions` row.

## Customer Feedback To Apply

Ben asked for a simpler, more inspiring, investor-focused OTEC floating
datacenter Site:

- Move the Site to light mode with a white background.
- Use web imagery only when it is public-domain, permissively licensed, or
  otherwise safe to use; otherwise use generated, diagrammatic, or CSS-native
  visuals rather than copyrighted images.
- Present a 1000m ocean tower concept that is mostly below water.
- Show an inner pipe bringing cold deep seawater upward.
- Explain that the inner pipe diameter is about 20m and the total tower
  diameter is about 40m.
- Place server/datacenter space around the cold-water pipe.
- Include environmental benefits: bringing up mineral-rich deep seawater,
  aerating water, and enabling inexpensive seawater mineral mining.
- Explain that the roughly 50m above-water section can support housing,
  tourism, and other human uses.
- Make the long-term vision inspiring: these towers can grow into new cities
  in international waters.
- Mention UHPC, geopolymers, and shotcrete inside airforms as the construction
  approach.
- Keep the Site fairly simple and not too technically dense.
- Mention that equatorial and many other candidate areas do not have hurricane
  exposure.

## Required Revision Workflow

Do not edit the existing live Site artifact in place.

1. Build a new OTEC Revision 2 artifact.
2. Save the output as a new Site version for `site_project_otec`.
3. Link the new version to the adjustment request and feedback record above.
4. Leave the current public URL stable; the revision workflow may later make
   the latest approved version active.
5. Emit an `openagents.adjutant.site_artifact_receipt.v1` payload so OpenAgents product surface can
   ingest the new `site_versions` row.
6. Mark the adjustment ready for review, not customer-accepted.
7. Trigger the review-ready notification path through the approved
   `EmailService` lifecycle, or leave the state needed for the operator
   review-ready smoke endpoint to send the email.

## Design Direction

The current dark, oversized terminal-style page is not acceptable as customer
delivery. Revision 2 should feel like a credible investor concept page:

- first viewport: clear OTEC floating datacenter thesis, light visual system,
  and a strong tower/ocean diagram or safe image-led composition;
- page tone: calm, premium, legible, and fundable rather than gimmicky;
- avoid giant cropped monospace text;
- avoid "proof", "agent challenge", or other internal process controls on the
  customer-facing Site;
- include concise sections for infrastructure concept, environmental upside,
  construction approach, and city-scale future;
- include enough technical specificity to make the concept concrete without
  becoming an engineering whitepaper;
- make the stable URL and revision artifact suitable for customer review.

## Acceptance Criteria

- A new Site version exists after the run; it is not the initial version ID.
- The new revision visibly incorporates Ben's light-mode investor-focused
  feedback.
- The current live OTEC URL is not manually overwritten by a foreground agent.
- The resulting revision can be listed on the customer order page as the latest
  revision.
- The revision-ready transactional email can be sent to Ben with:
  - a simple revision link;
  - the product/order page link;
  - the stable Site URL for background;
  - a short description of what changed.

## Safety

- Do not include secrets, provider account refs, auth grant refs, raw runner
  logs, internal prompts, private operator notes, callback tokens, or OAuth
  material in the Site, artifacts, receipts, or customer email.
- Do not use copyrighted images unless the source license is explicitly safe
  for this use and the source is documented in public-safe metadata.
- Do not imply engineering guarantees, regulatory approval, or financial
  returns.
