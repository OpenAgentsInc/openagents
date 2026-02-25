# Landing Static Host Rollback Runbook (OA-RUST-082)

## Purpose

Define rollout and rollback procedure for the landing-only web surface served by
`apps/openagents.com/service`.

## Build Inputs

1. Control service image from `apps/openagents.com/service/Dockerfile`.
2. Landing static assets under `apps/openagents.com/service/static/`.
3. Desktop distribution URL via `OA_DESKTOP_DOWNLOAD_URL`.
4. Control static host path via `OA_CONTROL_STATIC_DIR` (container default: `/app/service/static`).

## Release Order (Required)

1. Build and validate control service image.
2. Confirm landing routes:
   - `GET /` returns landing page.
   - `GET /download-desktop` redirects to expected desktop artifact URL.
3. Deploy control service.
4. Validate health/ready:
   - `GET /healthz`
   - `GET /readyz`
5. Validate desktop download redirect target after deploy.

## Rollback Procedure

1. Roll back control service revision to previous known-good image.
2. Restore previous `OA_DESKTOP_DOWNLOAD_URL` value if changed in failed rollout.
3. Re-run route checks for `/`, `/download-desktop`, `/healthz`, `/readyz`.

## Verification

1. Landing route returns HTML and desktop CTA.
2. Download redirect points to intended release artifact location.
3. Control API routes required by retained clients remain responsive.
