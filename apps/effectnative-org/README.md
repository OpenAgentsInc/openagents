# effectnative-org — Cloud Run hosting for effectnative.org

Deploy wrapper for the Effect Native framework website (openagents #8571,
epic #8566). The site itself lives in the public repo
[OpenAgentsInc/effect-native](https://github.com/OpenAgentsInc/effect-native)
(issue effect-native#19) and builds fully static with `bun run site:build`
(`dist/site/`, gallery included at `/components`). This directory holds only
the serving container and the operator redeploy script — no site source.

## Redeploy (one command)

```sh
CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config \
  apps/effectnative-org/scripts/deploy-cloudrun.sh
```

The script clones latest `effect-native@main` (or uses `EFFECT_NATIVE_DIR`
if set), runs `bun run site:build`, stages `dist/site/` into `./site/`
(gitignored), deploys via `gcloud run deploy --source`, and runs smokes
(root prerendered HTML, `/docs/`, `/components/` gallery deep link,
`/sitemap.xml`, 404).

## Infra shape

- Service: `effectnative-org`, project `openagentsgemini`, `us-central1`,
  public, nginx static container on port 8080.
- Separate service from the openagents.com monolith and from
  `effect-native-gallery`: different product, different domain, independent
  deploy cadence.
- HTTPS: Cloud Run **domain mapping** for `effectnative.org` and
  `www.effectnative.org` (www 301s to apex in nginx.conf). Chosen over a
  load balancer because every other custom domain in this project already
  uses domain mappings and a static site needs no LB features. DNS is at
  the registrar (GoDaddy), already pointed at Google's documented
  A/AAAA + `ghs.googlehosted.com` records; the mapping create is gated on
  Google domain verification for the deploying account (owner step — see
  workspace `NEEDS_OWNER.md` and #8571). Once verified:

  ```sh
  CLOUDSDK_CONFIG=... gcloud beta run domain-mappings create \
    --service effectnative-org --domain effectnative.org \
    --region us-central1 --project openagentsgemini
  CLOUDSDK_CONFIG=... gcloud beta run domain-mappings create \
    --service effectnative-org --domain www.effectnative.org \
    --region us-central1 --project openagentsgemini
  ```

No hosted CI: redeploys are operator/agent-run per standing platform policy.
