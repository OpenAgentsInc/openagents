# effect-native-gallery — Cloud Run hosting for the Effect Native component gallery

Deploy wrapper for the Effect Native **component gallery** (openagents #8570,
epic #8566). The gallery itself lives in the public repo
[OpenAgentsInc/effect-native](https://github.com/OpenAgentsInc/effect-native)
(issue effect-native#18) and builds fully static with `bun run gallery:build`
(`dist/gallery/` — `index.html` + `app.js`, subpath-safe, no server runtime).
This directory holds only the serving container and the operator redeploy
script — no gallery source.

Live URL: <https://effect-native-gallery-ezxz4mgdsq-uc.a.run.app>

## Redeploy (one command)

```sh
CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config \
  apps/effect-native-gallery/scripts/deploy-cloudrun.sh
```

The script clones latest `effect-native@main` (or uses `EFFECT_NATIVE_DIR`
if set), runs `bun run gallery:build`, stages `dist/gallery/` into
`./gallery/` (gitignored), deploys via `gcloud run deploy --source`, and
runs smokes (root, `/stories/button-primary` path-shape deep link,
`?story=` query-shape deep link, `/app.js`, missing-asset 404).

## Routing contract

Per effect-native `docs/gallery.md`, the static host must rewrite
extensionless HTML requests (e.g. `/stories/button-primary`) to
`index.html` while letting missing asset requests (anything with a file
extension, e.g. `/app.js`) 404. `nginx.conf` implements exactly that:
an extension-matching location with `try_files $uri =404`, and an
extensionless fallback of `try_files $uri $uri/ /index.html`.

## Infra shape

- Service: `effect-native-gallery`, project `openagentsgemini`,
  `us-central1`, public, nginx static container on port 8080.
- Separate service from the openagents.com monolith and from
  `effectnative-org` (the framework website, which also embeds the gallery
  at `/components`): this is the raw living component library with an
  independent deploy cadence tracking framework main.
- URL: the default Cloud Run `*.run.app` address. Public component library
  with generic content — no custom domain, so no DNS/owner step.
- Mobile: the on-device gallery ships inside effect-native's Expo example
  (effect-native#18); nothing is deployed here for it.

No hosted CI: redeploys are operator/agent-run per standing platform policy.
