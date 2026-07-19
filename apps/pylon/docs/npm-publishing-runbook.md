# npm Publishing Runbook (Pylon + workspace packages)

> Status: canonical publishing runbook, established 2026-06-12 during the
> first @openagentsinc workspace-dependency publishes and the 0.3.0-rc2
> publish. Read this before publishing ANY package from this monorepo.
> Owner decisions recorded here: the npm scope is `@openagentsinc`
> (npm username `openagentsinc`). 0.3.0 Ships as an **rc dist-tag only**
> until the owner decides stable.

## Scope truth

- Every publishable package in this repo is scoped `@openagentsinc/`.
  The repo-wide rename from `@openagents/` landed 2026-06-12 (commit
  059ba3f62, 344 files). Never publish under `@openagents/` — we do not
  own that scope.
- Published set as of 2026-06-12: `@openagentsinc/pylon` (0.2.5 latest),
  `@openagentsinc/agent-runtime-schema` 0.1.0,
  `@openagentsinc/nip90` 0.1.0, `@openagentsinc/tassadar-executor` 0.1.0.

## Auth

- The account has 2FA. Interactive `npm publish` fails with `EOTP`
  unless `--otp=CODE` is passed.
- The unattended path is a **granular automation token** stored at
  workspace `.secrets/npm-publish.env` (`NPM_PUBLISH_TOKEN`) and in
  `~/.npmrc` as `//registry.npmjs.org/:_authToken=...`. Never print the
  token into tracked files, commit messages, issue comments, or logs.
- `npm whoami` must return `openagentsinc` before publishing.

## The bun/npm split (load-bearing)

- **Do NOT use `bun publish`**: its auth handshake against
  registry.npmjs.org fails (404 on its `/-/v1/done?authId=` web-login
  flow) even with a valid `~/.npmrc` token (observed 2026-06-12).
- **Do NOT use `npm pack`/`npm publish` directly from the package dir**:
  npm does not rewrite `workspace:*` / `catalog:` dependency protocols,
  so the published manifest would be uninstallable.
- **The working recipe**: `pnpm pack` (which DOES rewrite
  `workspace:*` and `catalog:` to concrete resolved versions — verified
  by inspecting the packed package.json), then
  `npm publish <tarball> --access public`.

## Publish order

Leaf dependencies first, then Pylon:

```sh
cd packages/agent-runtime-schema   && pnpm pack && npm publish ./openagentsinc-agent-runtime-schema-<v>.tgz --access public && rm -f ./*.tgz
cd ../provider-account-schema       && pnpm pack && npm publish ./openagentsinc-provider-account-schema-<v>.tgz --access public && rm -f ./*.tgz
cd ../blueprint-contracts           && pnpm pack && npm publish ./openagentsinc-blueprint-contracts-<v>.tgz --access public && rm -f ./*.tgz
cd ../nip90               && pnpm pack && npm publish ./openagentsinc-nip90-<v>.tgz --access public && rm -f ./*.tgz
cd ../tassadar-executor   && pnpm pack && npm publish ./openagentsinc-tassadar-executor-<v>.tgz --access public && rm -f ./*.tgz
cd ../../apps/pylon       && pnpm run release:gate   # must pass first
pnpm pack && npm publish ./openagentsinc-pylon-<v>.tgz --tag rc --access public && rm -f ./*.tgz
```

- `@openagentsinc/provider-account-schema` and `@openagentsinc/blueprint-contracts`
  are the canonical security-contract packages (one authority for
  `ProviderSecretRef` + the secret-safety predicates, and the `IsPrivateDataSafe`
  private-data-safety predicate). Pylon's bundled runtime imports them, so they
  are `workspace:*` deps of `@openagentsinc/pylon` and must be published as leaf
  deps BEFORE Pylon (same pattern as `agent-runtime-schema`). `pnpm pack`
  rewrites the `workspace:*` deps to their concrete versions in the packed Pylon
  manifest.

- `--tag rc` is load-bearing for any pre-stable Pylon publish: omitting
  it moves `latest` and every plain `npm install @openagentsinc/pylon`
  onto the RC. `latest` stays 0.2.5 until the owner tags a stable.
- Verify after: `npm view @openagentsinc/pylon dist-tags`.

## Propagation gotchas (will look like failure, are not)

- Right after publish, `npm view` and direct installs can 404 for
  several minutes. The FULL registry doc
  (`GET registry.npmjs.org/@openagentsinc%2f<name>`) goes 200 before the
  **abbreviated/corgi manifest**
  (`Accept: application/vnd.npm.install-v1+json`) does — and bun
  installs use the corgi endpoint. A passing curl with no Accept header
  does NOT mean `pnpm install` will resolve yet.
- `bun pm cache rm` does not fix this. It is registry-CDN-side. Wait
  and poll the corgi endpoint until 200 for every just-published
  package, then re-run the gate.

## Gates

- `pnpm run release:gate` in apps/pylon runs unit/runtime tests, the
  bootstrap/status/inventory/operator smokes, dashboard smoke, package
  dry-run, and `scripts/smoke-local-package-install.sh` (packs pylon +
  nip90 + tassadar-executor locally. Resolves agent-runtime-schema from
  the registry — so that package must be published and corgi-propagated
  for the gate to pass).
- Tarball filename patterns in the install smoke derive from the scope
  without the `@` (`openagentsinc-<name>-<v>.tgz`). If a package is
  renamed, update the awk patterns in
  `scripts/smoke-local-package-install.sh`.
- Known date-bomb class: tests can sign NIP-98 events with an injected fixed
  epoch. If they verify against wall time, they pass only during the wide
  `maxSkewSeconds` window. Then, they fail permanently. Verify with the same
  injected `now`. This defect occurred before and was corrected.

## Consumer install must work without bun (git-dep prepare hazard)

`@openagentsinc/nip90` pulls `nostr-effect` as a **git dependency**. npm runs
the git dependency's `prepare` lifecycle script during consumer installation.
Registry tarballs do not run this script. Thus, a `prepare` script that needs
Bun breaks `npx @openagentsinc/pylon` and `npm install` on a clean Node system.
The failure is `code 127 (git dep preparation failed)`. `pnpm install` and
`pnpm exec` can still work because Bun blocks lifecycle scripts by default.

This defect caused the 2026-06-18 launch failure. The `nostr-effect` `prepare`
script now uses a Node-only guard. `nip90` now pins
`nostr-effect#4c52847`. Before publishing, run the **npm + no-bun** consumer
smoke in `release-install-smokes.md`. Repinning a git dep inside a published
package (e.g. nip90) requires republishing that package AND pylon.

## After publishing

1. Update `apps/pylon/README.md` "Launch Package And Version Truth"
   with the new published truth (verified `npm view` output, dated).
2. Registry ride-along: any product-promise copy that names package
   versions moves only with a registry version bump + deploy
   (`product-promises.ts`), receipts first.
3. The packaged-binary proof lanes (e.g. the Claude bounded-task repeat,
   #4859) install from the rc dist-tag:
   `npm install -g @openagentsinc/pylon@rc`.

Authored by Fable (claude-fable-5) for the 2026-06-12 publish campaign
(#4858).
