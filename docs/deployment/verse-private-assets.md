# Verse private assets on GCP

The resources behind [Private assets](../verse/private-assets.md), created
on October 6, 2026 in project `openagentsgemini` (us-central1) by
`chris@openagents.com` (#10770). They hold licensed content that the public
repository must never hold.

## Resources

| Resource | Settings |
| --- | --- |
| Bucket `gs://openagentsgemini-verse-private-assets` | us-central1, Standard class, uniform bucket-level access, public access prevention enforced, object versioning on, soft delete 7 days. |
| Bucket lifecycle | Delete a noncurrent version 30 days after it became noncurrent, and any noncurrent version with 3 newer ones. |
| Bucket IAM | The project's owners and editors (the default convenience bindings). The project-viewer bindings were removed, so project viewers can't read licensed content. `verse-assets@` holds `roles/storage.objectViewer` under the condition `packs-and-manifests`: only objects under `packs/` and `manifests/`, never `vendor/`. |
| Service account `verse-assets@openagentsgemini.iam.gserviceaccount.com` | No keys. It holds `roles/iam.serviceAccountTokenCreator` on itself only, so it can call `signBlob` for its own signed URLs. |
| Image `us-central1-docker.pkg.dev/openagentsgemini/openagents/verse-assets:TAG` | Built by `crates/verse-private/cloudbuild.yaml` from a GitHub commit. |
| Cloud Run service `verse-assets` | us-central1, runs as `verse-assets@`, public ingress with unauthenticated invocation (the service authenticates each request with NIP-98), 256 MiB, 1 CPU, 0 to 2 instances, concurrency 40, request timeout 30 s. Environment: `VERSE_ASSETS_PUBLIC_URL` (its own URL) and `VERSE_ASSETS_BUCKET`. |

The project's owners and editors can also read the bucket through
project-level roles. Nothing outside the project can: public access
prevention refuses `allUsers` and `allAuthenticatedUsers`, and an anonymous
read returns `403`.

## Live (October 6, 2026)

- Revision `verse-assets-00001-rmv`, image `verse-assets:c64ad66cfc` (built
  from commit `c64ad66cfc` by `oa-mvp-automation@` and deployed by
  `chris@`), serves 100% of traffic at
  `https://verse-assets-157437760789.us-central1.run.app`. It predates the
  `/health` route; the next deploy adds it.
- The registry holds one asset, `cute-asian-girl`: pack
  `5416e7f78b1a…` (2,377,473 bytes), its manifest, and its 8 vendor files
  (134 MB). Its one reader is the owner's desktop Verse key (`default`
  profile, `0b010805ac08…`).
- Checked: an unsigned grant request returns `403`; an anonymous read of
  the pack object returns `403`; the owner's desktop Verse loaded the pack
  through a signed URL and cached it as `private-cache/<digest>.vtp` (file
  0600, directory 0700); a Verse home whose profile key is not a reader got
  `403` from the broker, drew nothing, and cached nothing.

## Deploy the broker

Build from a commit on `main`, then deploy the image:

```sh
SHA=$(git rev-parse origin/main)
gcloud builds submit https://github.com/OpenAgentsInc/openagents \
  --git-source-revision="$SHA" --project openagentsgemini --region us-central1 \
  --config crates/verse-private/cloudbuild.yaml \
  --substitutions _TAG="${SHA:0:10}"
gcloud run deploy verse-assets --project openagentsgemini --region us-central1 \
  --image "us-central1-docker.pkg.dev/openagentsgemini/openagents/verse-assets:${SHA:0:10}" \
  --service-account verse-assets@openagentsgemini.iam.gserviceaccount.com \
  --allow-unauthenticated --memory 256Mi --cpu 1 --min-instances 0 --max-instances 2 \
  --concurrency 40 --timeout 30 \
  --set-env-vars VERSE_ASSETS_BUCKET=openagentsgemini-verse-private-assets,VERSE_ASSETS_PUBLIC_URL=https://verse-assets-157437760789.us-central1.run.app
```

Check it after a deploy:

```sh
curl -s -o /dev/null -w '%{http_code}\n' -X POST \
  https://verse-assets-157437760789.us-central1.run.app/v1/private/url  # 403
```

Roll back by sending traffic to the previous revision:
`gcloud run services update-traffic verse-assets --region us-central1 --to-revisions REVISION=100`.

## Operate

- **Add an asset:** `verse-private add SOURCE --name NAME --license fab-standard`,
  then `verse-private place NAME --at X,Z --yaw RADIANS --broker URL`.
- **See what's there:** `verse-private list` and `verse-private show NAME`.
- **Let another device load an asset:** run `verse-private whoami --profile P`
  where that device's Verse key lives, then `verse-private grant NAME KEY`.
- **Revoke a reader:** `verse-private revoke NAME KEY`. The next request is
  refused; a URL already signed lives at most 300 seconds.
- **Remove an asset:** `verse-private remove NAME`. Versioning keeps
  noncurrent copies for 30 days;
  `gcloud storage rm --all-versions gs://openagentsgemini-verse-private-assets/vendor/NAME/**`
  purges them now.
- **Turn everything off:** `gcloud run services delete verse-assets --region us-central1`,
  or remove the bucket binding for `verse-assets@`.
- **Rotate:** there is no user-managed key. Google rotates the service
  account's system-managed signing keys; signed URLs live 300 seconds.

## Costs

Standard storage in us-central1 costs about $0.020 per GB-month, and
internet egress about $0.12 per GB. The first asset is a 2.4 MB pack and a
134 MB vendor archive, about $0.003 a month. A computer downloads a pack
once and keeps it in its private cache, so egress stays in cents. The
broker scales to zero; each grant is one manifest read and one `signBlob`
call, well inside the free tiers.
