# Gmail Inbox OAuth + Secret Rotation Runbook

Status: active  
Last updated: 2026-02-24

## Purpose

Operational checklist for production Gmail-backed inbox:

1. Google OAuth app configuration (dev/staging/prod)
2. Control-service environment configuration
3. Integration-secret encryption key rotation and break-glass recovery

## Required Environment Variables

Control service must set:

- `GOOGLE_OAUTH_CLIENT_ID`
- `GOOGLE_OAUTH_CLIENT_SECRET`
- `GOOGLE_OAUTH_REDIRECT_URI`
- `GOOGLE_OAUTH_SCOPES`
- `GOOGLE_OAUTH_TOKEN_URL`
- `OA_INTEGRATION_SECRET_ENCRYPTION_KEY`
- `OA_INTEGRATION_SECRET_KEY_ID`

Notes:

- `GOOGLE_OAUTH_SCOPES` must include at least:
  - `https://www.googleapis.com/auth/gmail.readonly`
  - `https://www.googleapis.com/auth/gmail.send`
- `OA_INTEGRATION_SECRET_ENCRYPTION_KEY` must be a base64/base64url-encoded 32-byte key.
- `OA_INTEGRATION_SECRET_KEY_ID` is embedded in encrypted envelopes (`enc:v1:<key_id>:...`) and is required for decryption.

## Secret Manager Paths

Recommended Secret Manager locations:

- `projects/openagentsgemini/secrets/openagents-control-google-oauth-client-secret`
- `projects/openagentsgemini/secrets/openagents-control-integration-secret-key`

Deployment must source these into Cloud Run env vars for:

- `openagents-control-service`
- `openagents-control-service-staging`

## Redirect URI Matrix

Use exact URI values registered in Google Cloud OAuth credentials:

- Dev: `http://localhost:8080/settings/integrations/google/callback`
- Staging: `https://staging.openagents.com/settings/integrations/google/callback`
- Production: `https://openagents.com/settings/integrations/google/callback`

Mismatch between deployed `GOOGLE_OAUTH_REDIRECT_URI` and Google Console config will block connect flow.

## OAuth Rotation Procedure

1. Create a new Google OAuth client secret in Google Cloud Console.
2. Write new secret version to Secret Manager.
3. Deploy staging control service with updated secret reference.
4. Validate connect callback, inbox refresh, and reply send in staging.
5. Promote the same secret version to production deploy.
6. Re-run staging/prod validation checklist (`docs/RUST_STAGING_PROD_VALIDATION.md`).
7. Disable old OAuth secret version only after production validation passes.

## Integration Secret Key Rotation Procedure

1. Generate new 32-byte key.
2. Set new secret version in Secret Manager.
3. Deploy control service with:
   - new `OA_INTEGRATION_SECRET_ENCRYPTION_KEY`
   - new `OA_INTEGRATION_SECRET_KEY_ID`
4. Existing plaintext secrets migrate lazily on read; existing encrypted secrets require matching key id.
5. Confirm runtime internal secret fetch + inbox routes succeed for existing integrations.
6. Keep previous key available for rollback window until all critical integrations are verified.

## Break-Glass

Use only during incident mitigation.

1. If decrypt failures start after rotation, immediately redeploy with prior key id/material.
2. Confirm `/api/inbox/threads` and `/api/internal/runtime/integrations/secrets/fetch` recover.
3. Freeze further key rotations.
4. Export affected integration ids and user ids from domain store snapshot for remediation.
5. Re-run rotation in staging before retrying production.

## Client Header Compatibility Window

- Canonical desktop auth header: `X-Client: autopilot-desktop`
- Temporary compatibility alias accepted by control service: `openagents-expo`
- Planned alias removal date: **June 30, 2026**
