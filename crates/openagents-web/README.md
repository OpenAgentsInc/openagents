# OpenAgents web

This Rust server is the OpenAgents website: the public pages that
openagents.com serves, the terms of service and the privacy policy, the one
install page, and the landing page for the pairing QR code. It also serves a
local, read-only task browser at `/app`.

The site is drawn in four intensities of white on
near-black (`src/palette.rs`). Only the homepage runs a script, its
terminal (`static/ask.js`), under a policy that allows that one same-site
script and requests to `/ask`; every other response carries a content
security policy that allows none.

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

A development server needs no secrets and makes no network requests:
everything it serves is compiled in or read from this repository.

## Pages

| Route | Source | Development server |
| --- | --- | --- |
| `/` | What OpenAgents is, one `[ Install OpenAgents ]` link, and the **Ask OpenAgents** terminal | Renders. |
| `/install` | Everything OpenAgents is launching, in order: the notarized OpenAgents for Mac `.dmg` in `openagentsgemini-oa-updates`, the iPhone app on TestFlight, pairing by QR code, and signing in to Codex or Claude Code on the Mac so the phone can run Coder | Renders. |
| `/desktop` | Permanent redirect to `/install` | Redirects. |
| `POST /ask` | The homepage terminal's questions (`src/ask.rs`, #10106): a NIP-CJ job to the OpenAgents chat worker through `relay.openagents.com`, surface `web`, signed with a key derived from the visitor's `oa_visitor` cookie and the server's secret (`OPENAGENTS_WEB_ASK_SALT`, random per process when unset). The worker answers about OpenAgents only and never offers Coder, a computer, a command, or a screen. One question at a time and 6 a minute per visitor, 32 waiting at once for everyone, besides the worker's quotas. Streams newline-delimited JSON | Answers from the live chat worker. |
| `/docs`, `/docs/{slug}` | `content/docs/*.md`, short guides in reading order, compiled in: what OpenAgents is, install, connecting a computer, chat, Coder, plugins (what they are, writing, testing, publishing and sharing), the Verse, the Grid (with its screenshot, `static/verse-grid.jpg`, captured from the live relay with `crates/verse/examples/overlook_capture.rs`), privacy and security, and help | Renders. |
| `/terms`, `/privacy` | `content/legal/*.md`, the published text (2026-09-03), compiled in | Renders. |
| `/connect` | Landing page for `https://openagents.com/connect#<code>` | Renders; no script, no referrer. |
| `/.well-known/apple-app-site-association`, `/.well-known/assetlinks.json` | Universal link and App Link claims for `/connect` | Serves. |
| `/u/{login}` | `Backend::profile` | Says the backend isn't connected. |
| `/app`, `/app/tasks/{id}` | The local task store | Reads the store; local hosts only. |

The header links Install and Docs; the footer links the terms and the
privacy policy.

The Forum, Gym, Traces, Earn, Weights, and QA sections of the old site are
not served and not linked (owner-directed, 2026-09-29). Neither is anything
for the old Coder Terminal product, which is not connected to OpenAgents:
the `/releases/{name}` proxy to its release bucket, the
`/install-terminal.sh` and `/install-terminal.ps1` install commands, and the
old Docs (`/doc`) and Blog (`/blog`) sections, whose every document was
about it (owner-directed, 2026-09-29). Today's `/docs` is new: guides to the
apps we launch and to plugins, none about Coder Terminal.

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
no script except the homepage terminal's; the homepage's single install
link and its terminal; The Grid guide's screenshot; `/ask`'s stream, cookie,
bounds, and limits, against an in-process door;
the install page and the `/desktop` redirect; that the legal pages carry the
published text; that every color in the stylesheet is a gray and that the
text steps meet WCAG AA; the `/connect` page's policy and the association
files; that the removed sections, the release proxy, the Terminal install
commands, the old `/doc` pages, and the blog answer `404` and are never
linked; that the docs list every guide and every site link in a guide
resolves; that the plugin guides name a plugin's five parts and no guide
says *tool*; that no page but the published legal text names Coder
Terminal or its install command; a connected test backend's pages and escaping; and the task
browser.
