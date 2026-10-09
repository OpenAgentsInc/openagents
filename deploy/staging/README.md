# Web staging: openagents-web-1-staging

The 1.0 web stack on staging (#11094), wired the way
`scripts/dev/full-local.sh` wires it locally: one Cloud Run service,
`openagents-web-1-staging` (us-central1, openagentsgemini), separate from
the production service `coder`, with three containers in one instance:

| Container | Image | What it is |
| --- | --- | --- |
| `web` (port 8080) | `openagents/openagents-web` (`crates/openagents-web/cloudbuild.yaml`) | The site, started by `web.sh`: chats in the staging bucket `openagentsgemini-web-chats-stage`, sign-in through the gateway, Settings > Claude (own keys sealed with the staging keyring), `/api/v1` to the gateway. |
| `gateway` (127.0.0.1:8791) | `openagents/openagents-stack` (this directory's `Dockerfile`) | The account service (GitHub sign-in on the staging OAuth App, sessions, `/device`, projects) and the inference gateway, started by `gateway.sh`. |
| `worker` | the same stack image | The chat worker, every model call through the gateway sidecar, on its own staging key over `wss://relay.openagents.com`, started by `worker.sh`. |

URL: <https://staging.openagents.com> (a Cloud Run domain mapping in openagentsgemini;
the DNS CNAME is `ghs.googlehosted.com`), also <https://openagents-web-1-staging-157437760789.us-central1.run.app>
and `openagents-web-1-staging-ezxz4mgdsq-uc.a.run.app`. It is one
instance (min and max 1). Accounts, sessions, API keys, sealed GitHub
tokens, saved provider keys and saved own-Claude keys are on an NFS disk
on `oa-accounts-nfs-staging` (10.42.26.2, reached over Direct VPC egress
on the subnet `openagents-web-staging`), backed up by hourly snapshots,
so they outlive the instance; chats are in the staging bucket. See
[account storage](../../docs/deployment/account-storage.md), which also
explains the gateway handoff a deploy goes through.

The staging GitHub OAuth App must list
`https://staging.openagents.com/auth/github/callback`
as its callback URL for sign-in to come back.

## Secrets (Secret Manager, staging only)

`openagents-web-1-staging-{github-oauth,byo-keys,ask-salt,csrf-key}`,
`openagents-gateway-staging-{admin-token,openrouter-key,ai-gateway-key,typesafe-key,vertex-sa,smoke-signup-token,byok-keyring,store-key}`,
`openagents-chat-worker-staging-secret`. Each grants
`roles/secretmanager.secretAccessor` to the runtime account
`oa-vertex-inference@openagentsgemini.iam.gserviceaccount.com` (the
automation account is refused `actAs` on the default compute account), which
also holds `roles/storage.objectAdmin` on the staging chat bucket. Never
point this service at a production secret.

The gateway opens the public inference API with a free tier of 20 requests
a day on `google/gemini-2.5-flash-lite`, so the smoke suite can make one
`/v1/responses` call with a fresh account's key.

Open sign-up (`POST /v1/accounts` without GitHub) is off, as in production.
The gateway's `operator_signup_token_env` names `SMOKE_SIGNUP_TOKEN`
(`openagents-gateway-staging-smoke-signup-token`): the smoke suite sends it
to make its one test account.

## Smoke

`scripts/smoke/staging.sh` (default base: this service) runs every 1.0 web
check: the homepage's four questions answered, docs and breadcrumbs,
`/download`, the agent documents, GitHub sign-in up to github.com, the
signed-in pages, the gateway, and the hosted installer into a scratch HOME.
`--restart` adds the durable-accounts check: it makes an account, an API
key, a saved provider key and a saved own-Claude key, forces a new
revision, and checks they all still work (`--only durable --restart` runs
just that).

## Deploy

`scripts/deploy/web.sh stage [REF]` does all of this from a clean export
of a commit (both builds in parallel, skipped when already built, then the
smoke) and prints the web digest to promote to production with
`scripts/deploy/web.sh promote DIGEST`. `stage --keep-spec` builds only
the web image and swaps it into the live staging spec, leaving the rest
(for example storage being tried for #11127) as it is. By hand:

```sh
export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config
SA=projects/openagentsgemini/serviceAccounts/oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com
T=$(git rev-parse --short=10 HEAD)
gcloud builds submit --project openagentsgemini --service-account $SA \
  --config crates/openagents-web/cloudbuild.yaml \
  --ignore-file crates/openagents-web/web.gcloudignore --substitutions _TAG=stg-$T .
gcloud builds submit --project openagentsgemini --service-account $SA \
  --config deploy/staging/cloudbuild.yaml \
  --ignore-file deploy/staging/stack.gcloudignore --substitutions _TAG=$T .
# Digests of both images, then:
python3 deploy/staging/render.py --revision openagents-web-1-staging-$T \
  --web-image .../openagents-web@sha256:... --stack-image .../openagents-stack@sha256:... \
  --worker-pubkey be4c57cadade24f6a3dd95a6e5a7414cbeab2d0df219c31157b850d4b6718b9c > /tmp/staging-service.json
gcloud run services replace /tmp/staging-service.json --region us-central1 --project openagentsgemini
```

The worker's public key is the first line of the `worker` container's log
(`worker <64 hex>`, be4c57ca…); it is derived from
`openagents-chat-worker-staging-secret` and does not change between deploys.

Roll back by deploying the previous image digests as a new revision
(`render.py` with a new `--revision` name). Moving traffic back to an old
revision with `update-traffic` does not work: that revision's instance
has handed the account store to the newer one and runs without a gateway.

Logs: `gcloud logging read 'resource.type="cloud_run_revision" AND resource.labels.service_name="openagents-web-1-staging"' --project openagentsgemini --freshness=30m`.
