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
- **Where it runs, for now.** On this Mac, or on a small Google Compute
  machine started through the cloud spend guard
  (`scripts/dev/cloud-budget.py`). Either works from production, because
  both ends only need the relay. A runner started inside an agent's session
  stops when that session ends, so a lasting one needs the owner to start
  it:

  ```sh
  cargo build --release -p kiln-runner
  kiln-runner --state ~/.openagents/kiln-runner --jobs 4
  cat ~/.openagents/kiln-runner/pubkey ~/.openagents/kiln-runner/repository
  ```

- **Its key in production.** Set `OPENAGENTS_KILN_RUNNER` to the contents
  of `STATE/pubkey` and `OPENAGENTS_KILN_REPO` to `STATE/repository` on the
  production web service. These are public keys; never copy `STATE/key`
  anywhere.
- **Limits.** The runner accepts a job from any key, one at a time, for
  its listed crates only, offline inside `coder-boundary`. That's
  acceptable while only admins see the page and the key. Before Kiln is
  public, the runner needs an admission list and quotas
  ([workstation.md](../kiln/workstation.md#access-control)).
