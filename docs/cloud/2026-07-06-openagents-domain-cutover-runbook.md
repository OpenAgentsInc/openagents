# openagents.com + auth.openagents.com domain cutover runbook (CFG-10, #8525)

Cut both hostnames over from the frozen Cloudflare Worker
(`openagents-autopilot`, Workers Paid cancelled — deploys frozen, still
serving) to the CFG-9 (#8524) `openagents-monolith` Cloud Run service, fronted
by a pre-staged Google Global External Application Load Balancer.

Status at time of writing: **everything below except the DNS changes is
already built and applied** (Terraform `infra/prod`, modules
`openagents_monolith` + `openagents_lb`, plan is a no-op). The flip itself is
a pure DNS change at Cloudflare.

## Why an LB instead of Cloud Run domain mappings

- Domain mappings are still preview-status, region-limited, and — decisive —
  cannot pre-provision TLS: cert issuance starts only after the domain already
  points at Google, guaranteeing a TLS outage window during cutover.
- The LB gives one static anycast IP (`136.68.142.56`) for both hostnames,
  first-class WebSocket support (sync live), and a later attachment point for
  Cloud Armor and Cloud CDN.
- With Certificate Manager DNS authorizations, the Google-managed cert reaches
  `ACTIVE` while traffic still flows to Cloudflare, so the flip is
  zero-TLS-downtime.

## Pre-staged inventory (Terraform: `infra/prod`, module `openagents_lb`)

| Thing | Value / name |
| --- | --- |
| Static global IP (the flip target) | `136.68.142.56` (`openagents-lb-ip`) |
| Managed cert (Certificate Manager) | `openagents-cert`, SANs `openagents.com` + `auth.openagents.com` |
| DNS authorizations | `openagents-dnsauth-openagents-com`, `openagents-dnsauth-auth-openagents-com` |
| Cert map on the HTTPS proxy | `openagents-cert-map` |
| Serverless NEG → Cloud Run | `openagents-neg` → service `openagents-monolith` (us-central1) |
| Backend service (logging on) | `openagents-backend` |
| URL map (host rule for both domains) | `openagents-url-map`; port 80 → 301 via `openagents-http-redirect` |
| Forwarding rules | `openagents-https` (443), `openagents-http` (80) |
| Cloud Run shell for CFG-9 | `openagents-monolith` pre-created (placeholder image); CFG-9's `gcloud run deploy openagents-monolith` becomes its next revision |

The LB IP receives no traffic until DNS moves — nothing here touches the live
site.

## Phase 0 — NOW (no traffic impact): cert pre-provisioning

Add these two CNAME records at Cloudflare DNS (also queued in
`~/work/NEEDS_OWNER.md`). They are validation-only; they can sit there
forever with zero effect on live traffic:

```
_acme-challenge.openagents.com       CNAME  3aab6c27-b595-45d3-b2e2-26f8bac05e6d.5.authorize.certificatemanager.goog.  (DNS only / grey cloud)
_acme-challenge.auth.openagents.com  CNAME  8f915fcb-8262-45b0-b70c-ab9d31dff96a.2.authorize.certificatemanager.goog.  (DNS only / grey cloud)
```

Note: the machine-local Cloudflare API token (`.secrets/cloudflare-openagents.env`)
and the wrangler OAuth grant are both **zone-read only** — a live write probe
on 2026-07-06 confirmed DNS record writes are denied. DNS changes are
owner-gated until a DNS-edit token exists.

Then watch the cert go `ACTIVE` (minutes after the CNAMEs resolve):

```sh
gcloud certificate-manager certificates describe openagents-cert \
  --project openagentsgemini \
  --format='value(managed.state)'   # PROVISIONING -> ACTIVE
```

## Pre-flip checklist (all must be true)

1. **CFG-9 staging smokes green** (#8524 acceptance: full mobile MVP flow —
   OpenAuth login, sync live WS, turn with resumable stream, credits,
   agent-computer session — against the staging Cloud Run service).
2. **Prod `openagents-monolith` deployed** with the real image
   (min-instances >= 1, WS enabled) — i.e. CFG-9's prod deploy has replaced
   the placeholder revision on the pre-created shell.
3. **Cert `ACTIVE`** (command above; both `authorizationAttemptInfo` entries
   `AUTHORIZED`).
4. **Full smoke against the LB without touching DNS** — this is the big
   de-risker the pre-staged cert enables. Real hostname, real TLS, forced
   resolution to the LB IP:

   ```sh
   curl -sv --resolve openagents.com:443:136.68.142.56 https://openagents.com/ -o /dev/null
   curl -sv --resolve auth.openagents.com:443:136.68.142.56 \
     https://auth.openagents.com/.well-known/oauth-authorization-server -o /dev/null
   ```

   Both must return monolith responses with the Google-managed cert. Run the
   seam smokes the same way (`--resolve` works for WS clients that honor it;
   otherwise a hosts-file entry gives the same effect for a device test).
5. Cloudflare zone DNS TTLs on the two hostnames set to Auto/300 (fast
   rollback).

## The flip (Cloudflare DNS, free tier)

Current state (public DNS, 2026-07-06): both hostnames resolve to Cloudflare
proxy IPs (`104.18.14.36` / `104.18.15.36`) — proxied (orange-cloud) custom
domains of the `openagents-autopilot` Worker.

Change to:

```
openagents.com       A  136.68.142.56   TTL 300, Proxy status: DNS only (grey cloud)
auth.openagents.com  A  136.68.142.56   TTL 300, Proxy status: DNS only (grey cloud)
```

Implementation notes:

- The Worker is attached via **custom domains**; Cloudflare represents those
  as managed DNS records. Deleting/replacing the custom-domain records with
  plain A records is the flip. If the dashboard refuses to edit a
  custom-domain record directly, remove the Worker custom domain for that
  hostname first (Workers & Pages → openagents-autopilot → Settings →
  Domains & Routes), then create the A record — do this one hostname at a
  time and verify before the next.
- Also delete/adjust any `AAAA` records for these hostnames (the proxied
  setup publishes IPv6; the LB pre-stage is IPv4-only). Leaving a stale
  proxied AAAA would keep routing v6 clients to Cloudflare.
- **Recommend `DNS only` (grey) initially.** Google's LB then terminates TLS
  with the pre-provisioned cert; behavior is exactly what the pre-flip
  `--resolve` smokes verified. Tradeoff: Cloudflare's WAF/DDoS shield is off
  for these hostnames until we either add Cloud Armor or later re-enable the
  proxy (orange works fine later — origin serves a valid public cert, use SSL
  mode Full (strict) — but do NOT re-enable it while any Worker
  route/custom-domain still exists for these hostnames, or traffic snaps back
  to the frozen Worker).
- `sites.openagents.com` (Sites/WfP program, postponed) is explicitly out of
  scope; leave its records alone.

## Post-flip verification smokes

Propagation: `dig +short openagents.com A` returns `136.68.142.56` (and no
Cloudflare 104.18.x). Then:

1. **TLS/origin**: `curl -sv https://openagents.com/ -o /dev/null` — Google
   cert, monolith response headers. Same for `auth.openagents.com`.
2. **Login chain (web)**: browser GitHub OAuth via `auth.openagents.com` →
   callback → session on `openagents.com`.
3. **Mobile session**: Khala mobile sign-in (PKCE, redirect `khala://auth`)
   on a real device — no app update expected (domains unchanged; builds
   hardcode the hostnames).
4. **Sync live**: WebSocket connect + live rows (LiveHub path).
5. **Inference**: one full turn with resumable stream; verify resume.
6. **Credits/counters**: `curl -fsS
   https://openagents.com/api/public/khala-tokens-served` moves with exact
   `token_usage_events` rows; credits balance visible after login.
7. **HTTP redirect**: `curl -s -o /dev/null -w '%{http_code} %{redirect_url}'
   http://openagents.com/` → `301 https://openagents.com/`.
8. Watch LB logs (backend logging is enabled at sample rate 1.0) and Cloud
   Run 5xx/latency for the first hour.

## Rollback (minutes)

Repoint DNS back at Cloudflare: re-add the Worker custom domains for
`openagents.com` + `auth.openagents.com` (Workers & Pages → Domains & Routes),
which restores the proxied records → frozen-but-serving Worker. Do not delete
the Worker or its bindings until the cleanup gate below. The LB stack stays
up and costs pennies; a second flip attempt is another DNS change.

Constraint to remember: Workers Paid is cancelled, so the Worker **cannot be
redeployed** — rollback restores the last frozen deployment as-is. Any fix
must land on the Cloud Run side.

## Post-cutover cleanup (after 48h green — NOT before)

1. Delete the Worker custom domains/routes for `openagents.com` and
   `auth.openagents.com` (`apps/openagents.com/workers/api/wrangler.jsonc`
   `routes` entries become historical; the `openagents-autopilot` Worker can
   linger disabled).
2. Delete the Workers-for-Platforms dispatch namespace
   `openagents-sites-production` (now unblocked; Sites program is postponed
   and its tenants archived — audit doc row 14).
3. Delete the `openagents-aiur` Worker + its `aiur.openagents.com` custom
   domain (admin panel moves behind the monolith or is retired; its hostname
   remains in the issuer redirect allowlist harmlessly).
4. Reconcile the audit doc
   (`docs/cloud/2026-07-06-cloudflare-to-google-consolidation-audit.md`) and
   close #8525.
5. Later, optional: re-enable the Cloudflare proxy (orange) for WAF, or
   attach Cloud Armor to `openagents-backend` — one or the other, decided
   deliberately.

## Auth-origin invariants (verified 2026-07-06 — survive the origin swap)

The cutover changes only the origin *behind* the domains; every auth
invariant is host-based, so nothing needs to change:

- `getIssuerOrigin` (`apps/openagents.com/workers/api/src/index.ts:2454`)
  returns `openauth.issuerOrigin`, which config
  (`src/config.ts:1295,1350`) derives as `new URL(env.OPENAUTH_ISSUER_URL).origin`
  — pure URL math on an env var the monolith carries over unchanged
  (`https://auth.openagents.com`).
- `authIssuerAllowsWebRedirectHostname`
  (`src/auth/mobile-session.ts:26`) is a literal hostname allowlist
  (`openagents.com`, `auth.openagents.com`, staging, `aiur.openagents.com`,
  localhost) — no origin/IP/runtime dependence.
- `authIssuerAllowsRedirect` (`src/auth/mobile-session.ts:58`) checks the
  web allowlist for the web client and `khala://auth` (scheme+host of the
  custom URI) + PKCE S256 for the mobile client — both independent of what
  serves the domain.
- `makeIssuerAwareFetch` (`src/index.ts:3938`) short-circuits any request
  whose hostname equals the issuer host to the in-process
  `routeAuthIssuerRequest` — issuer verification never leaves the process,
  so it cannot even observe the origin swap.
- Mobile builds hardcode the domains → **no app update needed**.

Only requirement: the monolith's environment sets `OPENAUTH_ISSUER_URL` to
the same `https://auth.openagents.com` value the Worker used (CFG-9 env
parity item).
