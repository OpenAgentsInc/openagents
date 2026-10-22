# Kiln for site admins on openagents.com

`openagents.com/admin` is the staff entrance (#12440). It links the admin
pages and Kiln, the coding agent that runs in a browser tab
([`docs/kiln/`](../kiln/README.md)). This page covers how the site serves
Kiln, how Kiln reaches a test runner in production, and what the owner has
to do.

## Who gets in

The account service's `admin` flag decides, the same flag
`/admin/analytics` reads. It belongs to the account, not to the sign-in
method, so the owner is an admin whether they sign in with GitHub, a
password, or an emailed code ([docs/auth/github.md](../auth/github.md)
covers the `invite_only` list that sets it).

| Visitor | `/admin`, `/admin/kiln/` | Kiln's files and `config.json` |
| --- | --- | --- |
| Signed out | `303` to `/login?return_to=...`; sign-in comes back | `404` |
| Signed in, not an admin | `403`, "This page is for OpenAgents staff." | `404` |
| Site admin | The hub, or Kiln's page | Served |

## What the site serves

- **The build.** The web image builds Kiln with
  `scripts/build-kiln-web.sh` in the `everglade` stage, through
  `scripts/deploy/oa-module-cache.sh` like every other browser module, and
  copies it to `/srv/kiln`. `openagents-web` finds it there, or takes
  `--kiln DIR`.
- **Isolation.** Every answer under `/admin/kiln/` carries
  `Cross-Origin-Opener-Policy: same-origin`,
  `Cross-Origin-Embedder-Policy: require-corp`, and
  `Cross-Origin-Resource-Policy: same-origin`. No other page on the site
  does. Opened in its own tab, Kiln's page is cross-origin isolated. Opened
  in the Grid's panel (an iframe), it isn't, which Kiln doesn't need today.
- **Policy.** Kiln's page allows its own scripts and WebAssembly, its
  inline module script by hash, `eval` for the worker's WASI shims, the
  relay over `wss:`, GitHub's public API and raw files, and framing by this
  site only.
- **`config.json`.** The site writes it from three environment variables.
  All three are public values:

  | Variable | Value | Default |
  | --- | --- | --- |
  | `OPENAGENTS_KILN_RUNNER` | The runner's public key, 64 hex digits or `npub1...` | Empty: Kiln asks for a key |
  | `OPENAGENTS_KILN_REPO` | The NIP-34 repository patches go to, `30617:<key>:<id>` | Empty |
  | `OPENAGENTS_KILN_RELAY` | The relay, `wss://...` | `wss://relay.openagents.com` |

  A value of the wrong shape is dropped, so the variable can't carry
  anything else into the page.

## The Grid terminal

`GET /home/me` answers `"admin": true` for a site admin, and the Grid's
loader sets `openagentsAdmin`. The terminal then shows **Kiln** beside
**DeepSeek V4.1 | Any Pylon**. It opens Kiln's page in a panel over the
Grid, with **Open in a new tab** and **Back to the Grid**. Back to the Grid
hides the panel and keeps the page loaded, so a running task continues.
Nobody else sees the control, and the server refuses the page to them
either way.

## How Kiln reaches a runner

Kiln runs `cargo test` on a `kiln-runner` over the relay, not over HTTP
([docs/kiln/rust-tests.md](../kiln/rust-tests.md)). The page sends a sealed
NIP-CJ job to the runner's public key, and the runner answers on the same
relay. The site never talks to the runner, and the runner's private key
never reaches the site or the page.

- **Which runner.** One `kiln-runner` with its own state directory. It
  makes its key on first start in `STATE/key` (mode `0600`) and writes the
  public half to `STATE/pubkey`. By default it also announces a demo
  repository, `30617:<runner key>:openagents-kiln-demo`, written to
  `STATE/repository`.
- **Where it runs.** On `oa-kiln`, a Google Compute machine that holds
  nothing else (#12454). `scripts/deploy/kiln-vm.sh` creates and manages
  it; see [The Kiln machine](#the-kiln-machine) below. Both ends only need
  the relay, so nothing on the machine listens except SSH.

- **Its key in production.** Set `OPENAGENTS_KILN_RUNNER` to the contents
  of `STATE/pubkey` and `OPENAGENTS_KILN_REPO` to `STATE/repository` on the
  production web service. These are public keys; never copy `STATE/key`
  anywhere.
- **Limits.** The runner takes jobs only from keys on its admission list,
  their NIP-OA agents, and its admins, each within a daily quota, one job
  at a time, for its listed crates only, offline inside `coder-boundary`
  ([rust-tests.md](../kiln/rust-tests.md#who-may-run-jobs)). Add each
  admin's Kiln **Admin key** `npub` (under **Self-edit**) to the admin
  file; the page signs its test runs with that key too.

## The Kiln machine

`oa-kiln` in `openagentsgemini`, `us-central1-a`: an `e2-standard-4`
(4 vCPUs, 16 GB) with a 100 GB balanced disk, Debian 12, Shielded VM, nothing
listening but SSH, labelled `purpose=kiln`. On-demand it costs about $100 a month
plus the disk (about $10). It runs, as the `kiln` user:

| Service | What | State |
| --- | --- | --- |
| `kiln-runner` | `--jobs 3 --timeout 1200 --memory 12000`, admission from `/etc/kiln/allow` and `/etc/kiln/admins`, crates from `/etc/kiln/crates`; with `/etc/kiln/repository`, patches to that announcement and no demo announcement | `/var/lib/kiln/runner` (its key, clone, worktree, target) |
| `kiln-bridge` | `kiln-git bridge`; starts only once `/etc/kiln/github-token` and `/etc/kiln/repository` exist | `/var/lib/kiln/bridge` (its key, clone, `bridge.json`) |
| `kiln-fetch.timer` | Every six hours: install main's toolchain and `cargo fetch` main's crates, so the runner's offline builds find them | `~kiln/main` |

```sh
scripts/deploy/kiln-vm.sh install [COMMIT]   # first time: toolchain, build, services
scripts/deploy/kiln-vm.sh update [COMMIT]    # rebuild at COMMIT (default: main) and restart
scripts/deploy/kiln-vm.sh keys               # runner key, repository address, bridge key
scripts/deploy/kiln-vm.sh status | logs
scripts/deploy/kiln-vm.sh delete
```

Edit the lists on the machine (`gcloud compute ssh oa-kiln`, then `sudo
-e /etc/kiln/allow`); the runner and bridge read them for every job and
round, so no restart is needed. To give the bridge its token, save it as
`/etc/kiln/github-token` (owner `root:kiln`, mode `0640`), write the
repository address to `/etc/kiln/repository`, and `sudo systemctl start
kiln-bridge`.
