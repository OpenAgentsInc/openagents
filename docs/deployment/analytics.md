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
