# WorkOS Production Cutover, B2B Org Switcher & `gamebuilder.org` Runbook (Phase 5)

**Date:** 2026-11-26  
**Tracking issues:** [#14103](https://github.com/OpenAgentsInc/openagents/issues/14103) (Master), [#14097](https://github.com/OpenAgentsInc/openagents/issues/14097) (Phase 0), [#14100](https://github.com/OpenAgentsInc/openagents/issues/14100) (Phase 3), [#14101](https://github.com/OpenAgentsInc/openagents/issues/14101) (Phase 4), [#14102](https://github.com/OpenAgentsInc/openagents/issues/14102) (Phase 5)  
**Design reference:** [docs/auth/2026-11-26-workos-coax-sdk.md](2026-11-26-workos-coax-sdk.md)

---

## 1. Pre-Cutover Readiness Gate

Do **not** flip `OA_WORKOS_ENABLED=1` in production until every box below is green:

1. **Phase 0 Owner Provisioning (`NEEDS_OWNER.md`, #14097):**
   - Staging and production WorkOS environments exist with Magic Auth, Email + Password, Google OAuth, GitHub OAuth, Apple OAuth (iOS Guideline 4.8), Enterprise SSO, Optional Organizations, Radar, and AuthKit Actions enabled; Phone/SMS/Radar-SMS explicitly **disabled** (D3).
   - All 4 staging and 4 production Secret Manager secrets exist and are bound strictly to `157437760789-compute@developer.gserviceaccount.com`.
   - `coax/web/api/workos-env-check.sh --staging` passes.
2. **Staging End-to-End Verification (`staging.openagents.com`):**
   - **Web (`/login`):** Custom Coax GPU UI renders Magic Auth 6-digit flow, Google/GitHub/Apple OAuth, Enterprise SSO domain discovery, `LG_ORG_SELECT` multi-org picker, and signed-in `H_SWITCH_ORG` org switcher; `gamebuilder.org` remains on legacy email codes (`OA_WORKOS_GAMEBUILDER_ENABLED=0`).
   - **Option A1 Security Invariants:** WorkOS login mints `L1` session (`__Host-oa_session` on web; device-bound `oa_at_`/`oa_rt_` with `OA-Device-Proof` on native); `POST /api/wallet/backup` (`L2-fresh`) and `POST /api/agents` (`L2`) still require first-party Passkey/Nostr step-up.
   - **Account Linking & Freeze:** Verified-email (`email_verified: true`) WorkOS login auto-links to existing `acct_`, logs `workos.auto_link` in `auth.audit`, emits security notice (`workos.link`), and writes `external_id = acct_...` back to WorkOS. Freezing the account ends both local sessions and WorkOS sessions and causes `/api/auth/workos/action` to return HMAC-signed `Deny`.
   - **Native Clients (#14101):**
     - iOS Simulator / TestFlight: Magic Auth, Sign in with Apple / Google / GitHub / SSO via `ASWebAuthenticationSession` (`openagents://auth/callback`, `prefersEphemeralWebBrowserSession = false`), Keychain `AfterFirstUnlockThisDeviceOnly`, `OA-Device-Proof` refresh.
     - macOS Desktop: Same flows + `switchOrganization()` with `0600` file storage under `~/.openagents/desktop/`.
     - Android: Magic Auth + Custom Tabs PKCE + verified App Link (`https://openagents.com/app/auth/callback`) + Keystore AES-256-GCM `0600` storage + `OA-Device-Proof`.
     - CLI: `openagents login --workos --origin https://staging.openagents.com` (RFC 8628 device flow), `openagents whoami`, refresh rotation, `openagents logout`, alongside existing NIP-98 key auth.
3. **Backup & Restore Drill (#14102):**
   - Local drill passes: `coax/web/api/tests/workos_restore_drill_test.sh` (verifying both full-DB and 6-hourly `auth:workos` dumps restore with exact row counts for `auth.workos_identities`, `auth.workos_pending`, and all 7 `workos.*` tables + `workos.users_live` generated columns).
   - Cloud Run backup jobs redeployed with `OA_BACKUP_SCHEMA=auth:workos`:
     ```bash
     coax/web/api/backup/deploy.sh --run dump-auth
     coax/web/api/backup/deploy.sh --run drill
     ```
   - Latest drill JSON in `gs://openagentsgemini-db-backups/drills/` reports `"ok":true,"restore_errors":0`.

---

## 2. Production Cutover (`openagents.com`)

1. **Deploy `oa-api` and register the 1-minute `workos-events` mirror poller:**
   ```bash
   coax/web/api/deploy.sh production
   coax/web/api/schedules.sh workos-events
   ```
   (`deploy.sh` runs `oa-api-migrate-production`, binds the 4 `oa-workos-*` secrets, and preserves `OA_WORKOS_ENABLED` across deploys.)
2. **Verify WorkOS environment before enabling UI traffic:**
   ```bash
   # Trigger one mirror sync and confirm cursor advances
   # Flip OA_WORKOS_ENABLED=1 on oa-api:
   gcloud run services update oa-api \
     --project=openagentsgemini --region=us-central1 \
     --update-env-vars="OA_WORKOS_ENABLED=1"
   coax/web/api/workos-env-check.sh --production
   ```
3. **Post-enablement smoke checks (within 5 minutes):**
   - Load `https://openagents.com/login` in a fresh browser profile: confirm Custom Coax GPU UI displays Magic Auth, Google, GitHub, Apple, Enterprise SSO, Passkey, and Nostr options with zero console/Wasm errors.
   - Sign in with a test account via Magic Auth and Google OAuth; confirm `workos.users`, `workos.sessions`, `workos.events`, and `workos.sync_cursor` populate in `oa_analytics` via `oa_readonly`.
   - Confirm `https://gamebuilder.org` continues using existing email-code auth untouched (`workos-env-check.sh` verifies `gamebuilder.org` isolation).
   - Confirm existing Nostr, Passkey, Spark wallet backup (`L2-fresh`), Lightning Address, and agent routes show zero regressions.

---

## 3. Instant Rollback (Zero Data Loss)

Because OpenAgents owns the canonical `acct_` spine and Option A1 sessions (`__Host-oa_session`, `oa_at_`/`oa_rt_`) locally in Postgres, rollback is a single env-var toggle with **zero session loss** for already-signed-in users:

```bash
gcloud run services update oa-api \
  --project=openagentsgemini --region=us-central1 \
  --update-env-vars="OA_WORKOS_ENABLED=0"
```

- `/api/auth/workos/config` immediately returns `{"enabled":false,...}`.
- `apps/openagents-web/login.coax`, iOS, macOS, Android, and CLI immediately fall back to first-party email codes, Passkeys, and Nostr keys.
- Existing `auth.workos_identities` links and `workos.*` mirror rows stay safely preserved in Postgres; no migration rollback is needed.

---

## 4. B2B Enterprise SSO & In-App Organization Switcher Verification

| Surface | Multi-org login picker (`need-org`) | Signed-in org switcher (`GET /orgs` + `POST /orgs/switch`) | Enterprise SSO domain discovery (`POST /sso/discover`) |
| --- | --- | --- | --- |
| **Web (`apps/openagents-web/login.coax`)** | `LG_ORG_SELECT` panel + `/login#wos_org=...` callback handoff | `H_SWITCH_ORG` in signed-in view | Email domain -> WorkOS `GET /connections?domain=...` -> PKCE redirect |
| **macOS (`apps/openagents-desktop/Account.swift`)** | `pickWorkOSOrg()` modal picker | `switchOrganization()` modal picker | `showWorkOSProviders` -> Enterprise SSO -> `ASWebAuthenticationSession` |
| **iOS (`apps/shared/native/NativeLogin.swift` + `screen.coax`)** | `handleWorkOSResult(.needOrg)` action sheet | `AppAuth.workosOrgs()` / `workosSwitchOrg()` | `login.sso` GPU link -> `workosDiscoverSSO` -> `ASWebAuthenticationSession` |
| **Android (`apps/shared/app-login-flow.coax` + `app-login.c`)** | `ALF_ORG` state + `OA_LOGIN_WORKOS_ORG` | `AppAuth` / API `POST /api/auth/workos/orgs/switch` | `OA_LOGIN_WORKOS_SSO` -> Custom Tabs PKCE + App Link callback |

---

## 5. Follow-Up: `gamebuilder.org` Cutover & Legacy Email-Code Retirement

Perform only after `openagents.com` has run stably on WorkOS for at least one full release cycle:

1. **Add `gamebuilder.org` redirect URI in WorkOS Dashboard:**
   - `https://gamebuilder.org/api/auth/workos/callback` (or route via `https://openagents.com/api/auth/workos/callback` with listed-site handoff).
2. **Enable WorkOS for `gamebuilder.org`:**
   ```bash
   gcloud run services update oa-api \
     --project=openagentsgemini --region=us-central1 \
     --update-env-vars="OA_WORKOS_GAMEBUILDER_ENABLED=1"
   coax/web/api/workos-env-check.sh --production   # with expect_gb=1
   ```
3. **Retire legacy first-party Resend login-code routes (after 14-day soak with zero legacy code starts):**
   - Verify `SELECT count(*) FROM auth.audit WHERE action = 'email.start' AND at > now() - interval '14 days'` is `0`.
   - Remove `/api/auth/email/start`, `/api/auth/email/verify`, `/api/auth/email/link` from `coax/web/api/auth.c` (`RULES[]` and handlers) and remove the legacy email-code fallback branch from `apps/openagents-web/login.coax`, `AppAuth.swift`, and `app-login-flow.coax`.
   - **KEEP Resend (`email.c` / `OA_RESEND_KEY`) strictly for first-party security notices** (`auth_notice` for `workos.link`, account freeze, key link/unlink, recovery start/cancel, and 30-day delete warnings). Never route security notices through the auth provider.
