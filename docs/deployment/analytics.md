# Website analytics (#11153)

openagents.com counts its own use: no cookies, no third-party script, no
IP addresses, accounts, or message text. The code and the exact list of
what is counted are in `crates/openagents-web/src/analytics/mod.rs`; the
public description is the "What we count on the website" section of
`content/docs/privacy-and-security.md` and section 5 of the privacy policy.

## Where the counts live

| | Staging | Production |
|---|---|---|
| Bucket (private) | `openagentsgemini-web-analytics-staging` | `openagentsgemini-web-analytics-prod` |
| Dashboard key secret | `openagents-web-analytics-key-staging` | `openagents-web-analytics-key` |
| Runtime account with object admin | `oa-vertex-inference@` | `157437760789-compute@` |

Each instance rewrites `raw/YYYY-MM-DD/HH/INSTANCE.json` (its hourly
totals) every minute and once more on shutdown; any instance remakes
`daily/YYYY-MM-DD.json` for the last two days every hour. Bucket lifecycle
rules delete `raw/` after 30 days and `daily/` after 400 days (13 months).

The web container reads `OPENAGENTS_WEB_ANALYTICS_BUCKET` and
`OPENAGENTS_WEB_ANALYTICS_KEY`; `deploy/staging/render.py` and
`scripts/deploy/web.sh promote` add both. A development server can use
`OPENAGENTS_WEB_ANALYTICS_DIR=DIR` instead of a bucket.

## Opening the dashboard

Sign in on <https://openagents.com> with GitHub as a site admin (an
`invite_only` entry marked `admin`, `docs/auth/github.md`; in production,
the owner) and choose **Analytics** in the account menu at the bottom of
the left panel, or open <https://openagents.com/admin/analytics>. Anyone
else, signed out or signed in without admin, gets the site's plain 404.

Scripts can send the dashboard key as a bearer instead:

    curl -s -H "Authorization: Bearer $(gcloud secrets versions access latest --secret openagents-web-analytics-key --project openagentsgemini)" https://openagents.com/admin/analytics

A wrong key also gets the 404. To change the key, add a new secret
version and deploy a new revision.

## Affiliate and campaign links (#11698)

A separate, disclosed feature beside the cookieless counts: `/go/{code}`
(our own campaign codes, the checked-in list in
`crates/openagents-web/src/attribution/codes.rs`) and `/r/{token}[/{series}]`
(an affiliate's referral link from the referral system). The code and the
exact rules are in `crates/openagents-web/src/attribution/mod.rs`; the
public description is section 6 of the privacy policy and "Links that
credit who sent you" in `content/docs/privacy-and-security.md`. The general
counts above still set nothing in the browser and keep no identifier; the
two features share only the bucket.

- **Cookie:** `oa_ref`, signed (HMAC-SHA256 with a key derived from
  `OPENAGENTS_WEB_ASK_SALT`), `HttpOnly`, `SameSite=Lax`, 30 days, last
  click wins, set only by `/go/` and `/r/`, and only for a person's click
  (never an agent or a crawler), never with `Sec-GPC: 1` or `DNT: 1`, and
  only after consent where it's asked. Cleared at the next GitHub sign-in.
- **Consent:** `OPENAGENTS_WEB_COUNTRY_HEADER` names the request header
  holding the visitor's two-letter country. openagents.com is a Cloud Run
  domain mapping (checked 2026-10-15: `gcloud beta run domain-mappings
  list` maps `openagents.com` to the `coder` service; no load balancer, no
  backend service with custom request headers), and Cloud Run sends no
  country header, so it is unset and every visitor is asked first. Behind
  a Google external load balancer, add the custom request header
  `X-Client-Geo-Location: {client_region}` on the backend service and set
  `OPENAGENTS_WEB_COUNTRY_HEADER=x-client-geo-location`; visitors from the
  EEA, the UK and Switzerland, or with no header, are still asked.
- **Records:** each instance rewrites
  `attribution/raw/YYYY-MM-DD/INSTANCE.json` (that day's clicks, sign-ups
  and counted-only totals) every minute and on shutdown. A click record is
  a random click id, the code (an affiliate as `aff-` and 16 hex digits of
  the token's SHA-256, never the token), series, time, destination and
  traffic class. Keep these 13 months: add a bucket lifecycle rule
  deleting `attribution/raw/` objects after 400 days (the `raw/` rule
  doesn't match this prefix).
- **Sign-up:** the GitHub callback sends the click with the sign-in
  (`acquisition` and `campaign` in `POST /v1/sessions/github`); the
  account service captures it with the new account in the same write
  (`tenancy::accounts::referrals::Source::campaign`) and, for an affiliate
  under a published attribution policy, records an accepted
  `CapturedSource` attribution with the click as evidence.
- **Sales:** the gateway's Stripe webhook writes a named conversion for
  each paid month (start or renewal) of an account that came through a
  link, and a reversal for a full refund or lost dispute, to
  `attribution-conversions.jsonl` in its state directory
  (`crates/gateway/src/conversions.rs`). Commission on those sales follows
  the published affiliate terms (<https://openagents.com/affiliates>,
  `tenancy::accounts::referrals::affiliate`).
- **Scoreboard:** <https://openagents.com/admin/attribution>, for the same
  people and key as the analytics dashboard: clicks, sign-ups and
  sign-ups per click per code, affiliate and series, per week, for 8
  weeks. Under a site admin's sign-in (not the key) it also shows each
  affiliate's commission (earned, waiting, ready to pay, paid, taken back)
  and the payouts due, from `GET /v1/admin/affiliates`. Nothing is sent
  from the page: each monthly payout needs the owner's approval.
- **Affiliate's own numbers:** Settings shows "Your affiliate earnings" to
  an account with an affiliate link (`GET /v1/account/affiliate`, plus its
  links' clicks and sign-ups for the last 90 days).
