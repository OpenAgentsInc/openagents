# Ben OTEC Site Deployment Trace

Date: 2026-06-05

Public target:

- Site URL: `https://sites.openagents.com/otec`
- Public proof: `https://openagents.com/api/public/proof/otec`
- Agent challenge: `https://openagents.com/api/public/proof/otec#agent-challenges`
- Source artifact in this repo: `docs/sites/otec/index.html`

## Order

- Software order ID:
  `software_order_c34f3a52d60b41d699b71525365b6ee5`
- Customer user ID: `github:81483518`
- Customer display name from production D1: `bensilone`
- Request:
  `Website for ocean based, OTEC powered, SWAC cooled, gigawatt scale, floating datacenter.`
- Repository selected during onboarding: `bensilone/openagents`

## Implementation Choice

The current Sites runtime already supports public static deployments at
`sites.openagents.com/<slug>` through:

- `site_projects`
- `site_versions`
- `site_deployments`
- R2 artifact objects under the `ARTIFACTS` binding
- runtime route resolution in `workers/api/src/site-runtime-routes.ts`

No additional infrastructure issue is required for the first public static
OTEC deployment. Workers-for-Platforms generated app deployment, protected Site
runtime, app D1/R2 provisioning, and customer contribution APIs remain later
roadmap work.

## Public-Safe Scope

This first version is a static concept Site. It intentionally:

- uses only public/product-safe claims;
- avoids private runner logs, provider account refs, auth grants, raw Exa
  payloads, source archives, customer-private data, tokens, or secrets;
- links to the public proof and agent challenge;
- marks the build and deployment state through Site/version/deployment records;
- does not claim accepted contributions, payments, rewards, verified customer
  delivery, or settlement beyond the deployment record itself.

## Artifact

The deployed artifact is a single static HTML document:

- local source: `docs/sites/otec/index.html`
- source commit:
  `a33d366f276f849cf351c90b46f6438bfb7ffb90`
- source hash:
  `sha256:6e14d766e84bf42d9faf66b01e8c8b796ac269a0574893651cfaab1c21944ec4`
- runtime asset path: `index.html`
- R2 object key:
  `sites/otec/versions/2026-06-05T142050Z/index.html`
- content type: `text/html; charset=utf-8`
- cache control: `public, max-age=60`

Upload command:

```sh
bunx wrangler r2 object put openagents-autopilot-artifacts/sites/otec/versions/2026-06-05T142050Z/index.html --remote --file docs/sites/otec/index.html --content-type 'text/html; charset=utf-8' --cache-control 'public, max-age=60'
```

## Production Records

The deployment uses deterministic public IDs:

- Site project: `site_project_otec`
- Site version: `site_version_otec_20260605_initial`
- Site deployment: `site_deployment_otec_20260605_initial`
- Compatibility check: `site_compatibility_check_otec_20260605_initial`
- Build validation: `site_build_validation_otec_20260605_initial`

Activation record:

- SQL file: `docs/sites/2026-06-05-ben-otec-production-activation.sql`
- D1 database: `openagents-autopilot`
- Execution date: `2026-06-05`
- Result: 8 queries processed; 16 rows read; 32 rows written
- D1 bookmark:
  `00000061-0000f59c-00005081-ad181d64202d9d29d32641f13735badd`

Verified deployed public proof state after activation:

- `site.slug = "otec"`
- `site.activeUrl = "https://sites.openagents.com/otec"`
- `version.latestSavedVersionId = "site_version_otec_20260605_initial"`
- `deployment.status = "active"`
- `deployment.url = "https://sites.openagents.com/otec"`
- `buildValidation.status = "passed"`
- `agentInstructionCard.siteUrl = "https://sites.openagents.com/otec"`
- `agentChallenges[0].challengeUrl =
  "https://openagents.com/api/public/proof/otec#agent-challenges"`

## Verification

The deployed Site returned `HTTP/2 200` with `content-type:
text/html; charset=utf-8` and `cache-control: public, max-age=60`.

Positive content smoke:

```sh
curl -fsSI https://sites.openagents.com/otec
curl -fsS https://sites.openagents.com/otec | rg -n "OTEC|SWAC|Public proof|Agent challenge"
curl -fsS https://openagents.com/api/public/proof/otec | jq '{site: .site, deployment: .deployment, buildValidation: .buildValidation.status, agentSiteUrl: .agentInstructionCard.siteUrl, challengeCount: (.agentChallenges | length)}'
```

Observed proof summary:

- `site.id = "site_project_otec"`
- `site.status = "approved"`
- `site.activeUrl = "https://sites.openagents.com/otec"`
- `deployment.status = "active"`
- `deployment.url = "https://sites.openagents.com/otec"`
- `version.sourceCommitSha =
  "a33d366f276f849cf351c90b46f6438bfb7ffb90"`
- `compatibility.status = "ready"`
- `buildValidation.status = "passed"`
- `buildValidation.sourceHash =
  "sha256:6e14d766e84bf42d9faf66b01e8c8b796ac269a0574893651cfaab1c21944ec4"`
- `agentInstructionCard.siteUrl = "https://sites.openagents.com/otec"`
- `agentChallenges.length = 1`

Forbidden-material smoke:

```sh
curl -fsS https://sites.openagents.com/otec | rg -n "provider_account|auth_grant|runner_payload|callback_token|gho_|OPENAI_API_KEY|OPENCODE_AUTH_CONTENT"
curl -fsS https://openagents.com/api/public/proof/otec | rg -n "provider_account|auth_grant|runner_payload|callback_token|gho_|OPENAI_API_KEY|OPENCODE_AUTH_CONTENT"
```

Both forbidden-material checks returned no matches.
