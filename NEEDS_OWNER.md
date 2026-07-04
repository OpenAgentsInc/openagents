# NEEDS_OWNER

## QA Swarm Rate Card Draft

Source issue: OpenAgentsInc/openagents#8061

Resolved for public `/business` package bands by OpenAgentsInc/openagents#8079.
The prices may be quoted only with the operator-assisted caveats and receipt
plans from the page; checkout, self-serve hosted runs, first paid delivery
receipts, and broad hosted availability remain gated.

Modeled package bands from the QA Swarm product plan:

| Package          | Draft modeled band            | Notes before publication                                                                                              |
| ---------------- | ----------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Swarm Audit      | $1,000-$5,000 fixed           | Needs owner-approved scope limits, target-adapter caveats, redaction policy, and deliverable template.                |
| QA-on-every-push | $2,000-$10,000/month retainer | Needs hosted-run receipts, exact accounting, runner-capacity policy, and clear operator-assisted vs. self-serve copy. |
| Swarm Sprint     | $5,000-$15,000                | Needs backlog-size bounds, delivery acceptance criteria, and paid-delivery receipt path.                              |

Remaining owner-gated work: decide when QA Swarm hosted-run receipts,
checkout/intake receipts, and outward-facing third-party report artifacts may be
treated as live delivery evidence.

## Khala Code Desktop Signed Release Gate

Source issue: OpenAgentsInc/openagents#8245
Public install-truth surface: OpenAgentsInc/openagents#8246

Khala Code Desktop has a buildable Electrobun app, but RL-1 is not complete
until the release owner signs, notarizes, staples, uploads, and smoke-tests a
real macOS DMG. The repo may document and preflight the path, but the signing
identity, notary credentials, update-feed write, and clean-Mac first-run proof
are owner-gated.

Current state:

- `clients/khala-code-desktop/electrobun.config.ts` names the app
  `Khala Code` with bundle id `com.openagents.khala.code.desktop`.
- `bun run --cwd clients/khala-code-desktop build:rc` and
  `bun run --cwd clients/khala-code-desktop build:stable` are the unsigned
  channel build paths.
- `bun run --cwd clients/khala-code-desktop release:plan -- --version <version> --channel rc --artifact ./Khala-Code-<version>.dmg`
  validates the product tag, RC/stable/latest rules, GitHub tag convention, and
  Khala-specific updates-feed path before signing work starts.
- `bun run --cwd clients/khala-code-desktop release:macos -- --version <version> --channel rc`
  is the owner-run macOS lane. It reuses
  `apps/autopilot-desktop/scripts/notarize-macos.sh` with `OA_DESKTOP_APP_PATH`
  pointed at the Khala app, re-creates the DMG from the stapled `.app`,
  signs/notarizes/staples the DMG, stages
  `desktop/khala-code-desktop/<channel>/feed.json`, and refuses to upload or
  create a GitHub release unless the owner sets the explicit release env flags.
- No signed/notarized/stapled Khala Code DMG, owner-approved updates-feed path,
  GitHub release, or clean-Mac first-run smoke receipt is recorded here yet.
- `/code/download` and `GET /api/public/khala-code/download-counts` exist for
  public install truth, but they are not release receipts. The page must keep
  the desktop DMG marked as a pending public artifact until the receipt set
  below exists, and the counter may report only exact
  `khala_code_download_events` rows or an empty response with blocker refs.

NEEDS-OWNER: Provide or confirm the Developer ID/notary environment on an
owner-controlled machine, approve the Khala Code version and release channel,
approve the update-feed destination, run or authorize the signing/notarization
flow, re-create/sign/notarize/staple the DMG from the stapled `.app`, upload the
artifact, and record public-safe receipt refs for the clean-Mac smoke. The smoke
must prove the app boots from the DMG and, when Codex is missing or unauthenticated,
shows the honest `npm install -g @openai/codex` / `codex login` path without
claiming Khala Code bundles or replaces Codex.

Receipt refs required before RL-1 may be called complete:

- signed app
- notarized app
- stapled app
- recreated DMG from the stapled app
- signed DMG
- notarized DMG
- stapled DMG
- updates-feed upload
- GitHub release
- clean-Mac first-run smoke showing the missing-Codex install/login hint

## QS7 Rhys Sales Motion Owner Gate

Source issue: OpenAgentsInc/openagents#8067

This is an owner-review staging note for the outward-facing Swarm Audit demo PR
against `RhysSullivan/executor`. The external PR must not be opened, commented
on, or otherwise sent publicly until the owner signs off on the exact artifact
packet.

Current state:

- The former product-surface prerequisites have landed: QS2 provides the
  run-level share URL, QS8 provides chill-evals variant comparison, and QS9
  provides the third-party target-adapter contract.
- A live read-only browser audit against the executor public landing page was
  captured under run token `qs7-executor-20260703T151831Z`.
- The packet now has fingerprinted browser media, terminal asciicast, terminal
  snapshots, result verdict, committed e2e test, share projection, and
  chill-evals comparison refs in
  `docs/fable/2026-07-02-qs7-rhys-sales-motion-owner-review.md`.
- The observed browser verdict is `CONFIRMED`; the chill-evals comparison has a
  passing baseline and an intentionally false candidate that fails honestly.
- The share URL staged for review is
  `https://openagents.com/qa/qa-run.executor.qs7-public-home.20260703`.
- No external repository PR has been opened.

NEEDS-OWNER: Review
`docs/fable/2026-07-02-qs7-rhys-sales-motion-owner-review.md`, approve or edit
the public-safe artifact packet, choose the public attachment/media location,
and explicitly authorize the outward-facing `RhysSullivan/executor` PR.

## QA Swarm Hosted-Run Engagement Arming

Source issue: OpenAgentsInc/openagents#8070

The QS10 contract now models hosted-run receipts, exact-only metering rows, and
engagement receipts through the existing business quick-win lifecycle. The
settlement/payment seam is intentionally inert unless the owner explicitly arms
it.

NEEDS-OWNER: Decide when QA Swarm hosted-run engagement receipts may evidence
`buyer_paid` / `provider_settled` refs, which checkout/payment source is
approved for those refs, and what operator review is required before any paid
QA Swarm engagement is represented as paid or settled.
