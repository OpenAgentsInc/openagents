# OpenAgents web

This Rust server is the OpenAgents website: the public pages that
openagents.com serves, the terms of service and the privacy policy, the
desktop download, the landing page for the pairing QR code, and the release
proxy that the install command reads. It also serves a local, read-only
task browser at `/app`.

The site is a terminal in a browser, drawn in four intensities of white on
near-black (`src/palette.rs`). No page runs a script, and every response
carries a content security policy that allows none.

The pages follow the private Coder service's site (`bins/coder-serve` in
the `coder` repository), reimplemented here. openagents.com still deploys
from that repository; see
[the port status](../../docs/deployment/openagents-web.md) for what remains
before this server replaces it.

## Run it

From the monorepo root:

```sh
cargo run -p openagents-web -- --store "$HOME/.openagents/tasks"
```

Open `http://127.0.0.1:4300`. The server binds to loopback and answers only
the `Host` headers `127.0.0.1:4300` and `localhost:4300`.

| Option | Default | Meaning |
| --- | --- | --- |
| `--store DIRECTORY` | `~/.openagents/tasks` | The task store `/app` reads. It is never created. |
| `--listen ADDRESS` | `127.0.0.1:4300` | The address to bind. |
| `--public-host HOST` | none | Another `Host` header the public pages answer, such as `openagents.com`. Repeatable. The task browser still answers only the local hosts. |
| `--releases-url URL` | `https://storage.googleapis.com/openagentsgemini-cli-releases` | The public bucket `/releases/{name}` proxies. |

A development server needs no secrets. The release proxy and the install
page's channel rows read the public release bucket; everything else is
compiled in or read from this repository.

## Pages

| Route | Source | Development server |
| --- | --- | --- |
| `/` | Welcome card, install commands, desktop link, ask box | Renders. |
| `/ask?q=` | Commands (`download`, `desktop`, `blog`, `docs`, `help`, `clear`) and questions | Commands answer; a question says chat isn't connected. |
| `/terms`, `/privacy` | `content/legal/*.md`, the published text (2026-09-03), compiled in | Renders. |
| `/docs`, `/docs/{slug}` | `content/docs/*.md`, compiled in; `/doc` redirects | Renders; the install page reads the channel pointers. |
| `/blog`, `/blog/{slug}` | `content/blog/*.md`, compiled in | Renders. |
| `/desktop` | The notarized `.dmg` in `openagentsgemini-oa-updates` | Renders. |
| `/connect` | Landing page for `https://openagents.com/connect#<code>` | Renders; no script, no referrer. |
| `/.well-known/apple-app-site-association`, `/.well-known/assetlinks.json` | Universal link and App Link claims for `/connect` | Serves. |
| `/releases/{name}` | Proxy to the release bucket, with ranges | Proxies the public bucket. |
| `/install-terminal.sh`, `/install-terminal.ps1` | Redirect under `/releases/` | Redirects. |
| `/u/{login}` | `Backend::profile` | Says the backend isn't connected. |
| `/app`, `/app/tasks/{id}` | The local task store | Reads the store; local hosts only. |

The Forum, Gym, Traces, Earn, Weights, and QA sections of the old site are
not served and not linked (owner-directed, 2026-09-29).

Pages that read accounts go through the
`Backend` trait in `src/backend.rs`. The development backend is connected to
nothing: those pages render, say that their data needs the production
backend, and show no records. A production backend answers `connected()`
with `true`; then a missing profile answers `404`.

## Task browser

The browser reads the same durable task store and paged ATIF view as
`coder task list` and `coder task view`. It doesn't submit work or grant
execution rights. The task store contains private prompts, workspace paths,
and trace content, so the browser answers only a local `Host`, even when
`--public-host` is set. Do not expose it through a public reverse proxy. If
you have not created a task store, the browser shows an empty state without
creating one. Reload to read newer task state.

## Tests

```sh
cargo test -p openagents-web
```

The tests check that every public page answers `200` on a development
server with the header, the footer's links to the terms and the policy, and
no script; that the legal pages carry the published text; that every color
in the stylesheet is a gray and that the text steps meet WCAG AA; the
`/connect` page's policy and the association files; the release proxy's
allowlist and ranges against a stand-in bucket; that the removed sections
answer `404` and are never linked; a connected test backend's pages and
escaping; and the task browser.
