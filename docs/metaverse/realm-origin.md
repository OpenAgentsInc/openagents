# The realm origin: other people's realms in a sandboxed frame

Issue [#13432](https://github.com/OpenAgentsInc/openagents/issues/13432).
From the Spawn analysis ([what to adapt](../research/spawn-what-to-adapt.md),
N4). It is required before code from the browser (E12, #13421) opens to
anyone but the owner.

**Status: shelved (owner, 2026-11-13).** Realm code is written by us, or
submitted as a pull request to this repo and reviewed before it ships. Nobody
else's code runs on the site, so no second origin is deployed and none is
planned. The code below stays in the tree unused. Bring this back only if
unreviewed code is ever allowed to run for visitors.

## The rule

A realm that runs someone else's code is never served from the site's
origin. It is served from the realm origin, in a frame, and three things hold
there:

1. **It has no origin.** Every answer from the realm origin carries
   `Content-Security-Policy: sandbox allow-scripts allow-pointer-lock
   allow-orientation-lock`, so the document's origin is opaque however it is
   opened: in the site's frame or in a tab of its own. It has no cookies and
   no storage. The frame's own `sandbox` attribute says the same, and never
   `allow-same-origin`.
2. **It cannot touch the site.** The session cookie and our own auth (#13196)
   are the site's. The realm origin serves only a realm's page and what a
   page loads (`/worlds`, `/realms`, `/build`, `/coaxial`, a page's shader).
   It answers 404 for the API, the login, the chat, settings and the live
   channel. The frame is `credentialless` where the browser has that.
3. **It speaks only typed messages.** The site, the editor's shell and the
   chat reach the realm through `window.postMessage`, with the messages of
   `realm-frame.v1` and nothing else.

The frame stays cross-origin isolated: the realm origin answers with
`Cross-Origin-Embedder-Policy: require-corp` and
`Cross-Origin-Resource-Policy: cross-origin`, and the frame has
`allow="cross-origin-isolated"`.

## The messages

`metaverse-engine/host/realm-frame.coax` is the one codec (`#authority`).
`proto/verse/realm-frame-v1.json` pins the bytes.

| Message | From | Means |
| --- | --- | --- |
| `realm-frame.v1 hello` | site | Speak. The realm answers `ready` |
| `realm-frame.v1 ask SEQ COMMAND` | site | A driver's command: `tree`, `why ID`, `hit X Y` |
| `realm-frame.v1 ready APP I O` | realm | Its app; `I` is 1 when the frame is cross-origin isolated, `O` is 1 when it has no origin |
| `realm-frame.v1 answer SEQ` then the body | realm | The command's answer |
| `realm-frame.v1 refused REASON` | realm | The text was not a message |

A site trusts a realm only after `ready APP 1 1`. A line is at most 512
bytes and a body 32,768. A text that is too long, has a control character,
another version or another kind is refused whole.

The glue carries text and decides nothing:

- On the site: `coax/web/realm-frame.js`, `openRealmFrame(holder, origin,
  path, onText)`. It refuses the site's own origin. It knows the frame by its
  window, because a sandboxed frame's origin is `null`.
- In the frame: `host.js` hears only its parent window, only the origin named
  in `?frame=ORIGIN`, and only when that origin is listed in the page's
  `data-frame-origins` (on this computer's own addresses, another of them).
  It hands the text to the module as message 720 and posts the module's
  answer back to that origin alone.
- A realm's module reads with `rf_read` and writes with `rf_ready`,
  `rf_answer` or `rf_refused`. Splice Bots is the first (`splice_framed`).

## Check it

```sh
coax/web/serve.sh --help >/dev/null            # builds coax/web/build/serve
coax/web/tool.sh realm-frame-check             # headless Chrome: the site frames Splice Bots from the realm origin
coax/web/tool.sh metaverse-engine/tools/realm-frame-profile.coax proto/verse/realm-frame-v1.json
```

The check starts one dev server with `--realm-host localhost:PORT`. The site
is `http://127.0.0.1:PORT` and the realm origin `http://localhost:PORT`: two
origins and two sites to the browser, told apart by the request's `Host`.

## What works in the frame, and what doesn't yet

Measured in headless Chrome on Splice Bots: WebGPU, a secure context, cross-origin
isolation and the module's fetches all work with no origin.

- **Storage.** There is none. A realm that keeps a save needs the site to
  keep it, through a message this version doesn't have.
- **Workers.** A page with no origin can't start a worker from a URL. The
  offline room and the pack worker need a blob URL before a realm that uses
  them is framed.
- **Rooms.** A realm with a room seat needs its seat ticket handed in by the
  site through a message, since it has no cookie.

## Deploying it

Nothing here is deployed. The code side is done; these steps are the
coordinator's.

1. **Pick the realm origin.** It must be another site than `openagents.com`,
   not a subdomain of it: a subdomain shares the site and can be sent cookies
   scoped to `.openagents.com`. A separate registrable domain is the safe
   choice.
2. **DNS and a certificate** for that host, on the same load balancer.
3. **Load balancer** (`coax/web/api/lb.sh`): a host rule for the realm
   origin that sends only `/worlds/*`, `/realms/*`, `/build/*` and
   `/coaxial/*` to the site's bucket and answers 404 for everything else
   (never the API backend). On every answer from that host it adds the
   headers of `REALM_ORIGIN_HEADERS` in `coax/web/realm-origin.coax`:
   `Content-Security-Policy: sandbox allow-scripts allow-pointer-lock
   allow-orientation-lock`, `Cross-Origin-Resource-Policy: cross-origin`,
   `Access-Control-Allow-Origin: *` and `Referrer-Policy: no-referrer`,
   beside the COOP and COEP headers the bucket's answers already carry.
4. **The pages.** A realm that may be framed lists the site in its script
   tag: `data-frame-origins="https://openagents.com"`.
5. **Smoke** (`smoke.sh`): the realm origin answers `/worlds/splice-bots` with the
   sandbox header and `/api/...` with 404; then `realm-frame-check` against
   staging.

Until step 3 is live, no realm made by someone else is shown to another
person.
