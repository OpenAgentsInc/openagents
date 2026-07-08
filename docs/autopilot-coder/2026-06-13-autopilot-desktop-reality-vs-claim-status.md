# Autopilot Desktop — Reality vs. Claim Status Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-13
Status: honest status snapshot. Triggered by owner observation that the running
Electrobun desktop app shows only a static shell ("🛩️ Autopilot Desktop …
Electrobun + Bun + (Foldkit next) … offline · 0 sessions") and **no real UI**,
despite roadmap claims of web/desktop/mobile parity.

> **UPDATE (2026-06-13, later) — RESOLVED.** Everything below was fixed and
> merged to `main`:
> - **Connectivity (R1):** node-home auto-discovery (`.pylon-tailnet`/`.pylon-local`,
>   walking up from cwd) — CL-45 (#4958). No more false "offline".
> - **Sidebar shell (R3 structure):** the placeholder render is gone; a real
>   sidebar + pane router ships — CL-44 (#4957). Full parity feature set landed:
>   Ask/approvals/balance/assignments/cloud + Pause/Resume + session detail
>   (CL-46..CL-52), and dedicated Sessions/Decisions/Spawn/Settings/SessionDetail
>   screens on both clients (CL-55..CL-59, on the mobile `ConnectionProvider`).
> - **Foldkit (R3 the real parity):** DONE — the desktop webview was converted
>   from hand-DOM to a Foldkit TEA app rendering the shared
>   `@openagentsinc/autopilot-ui` components (CL-53 #4966), so web + desktop
>   render from one component library. The desktop webview is **Foldkit-only —
>   no hand-DOM** going forward (see `apps/autopilot-desktop/AGENTS.md`).
>   (Compiles/bundles + reducer-tested; a local `electrobun dev` GUI smoke is
>   recommended before release.)
> - **Roadmap claim (R4):** corrected in
>   `2026-06-13-autopilot-clients-roadmap.md`.

This audit answers two questions:

1. **Why is it "offline · 0 sessions"** when a healthy local Pylon node is
   running?
2. **Where is the UI** — why does it look nothing like the web or mobile
   client, and what happened to the promised Foldkit parity?

Both have concrete, verifiable answers below. Neither is a mystery; both are
gaps between what the roadmap *claims* and what the code *does*.

---

## TL;DR

- **The node is fine.** A real Pylon node is running and healthy on
  `127.0.0.1:4716` (`{"ok":true,"schema":"openagents.pylon.control.v0.3"}`),
  cwd `/Users/christopherdavid/work/openagents`, with a valid control token at
  `openagents/.pylon-tailnet/control-token`.
- **"offline" is a token-path mismatch, not a node failure.** The desktop app
  defaults `PYLON_HOME` to `<cwd>/.pylon-local`. Launched via the new
  `autopilot` shell function (which `cd`s into `apps/autopilot-desktop`), it
  looks for `apps/autopilot-desktop/.pylon-local/control-token`, which does not
  exist. Token read returns `null` → the poll throws → it renders the
  `offlineNodeState()` fallback (`ok:false`, 0 sessions). The node is never
  actually contacted. **This is a one-line launch-env fix.**
- **There is no real desktop UI parity.** The rendered UI in
  `src/ui/main.ts` is hand-rolled `document.createElement` DOM and still
  contains the *original scaffold placeholder copy* ("the desktop client shell
  is alive", "(Foldkit next)", "Next (CL-5): connect to the local Pylon node").
  **The desktop imports zero Foldkit components.** It pulls only CSS tokens
  (`cssVars`, `darkTokens`) from `@openagentsinc/autopilot-ui`, not the shared
  Foldkit views the web app actually renders.
- **The roadmap claim "Desktop already shares the Foldkit components" is false.**
  The shared Foldkit component package (`packages/autopilot-ui`) exists and the
  **web** companion consumes it; the **desktop** never adopted it. Parity today
  lives at the *protocol / view-model / conformance* layer, **not** at the
  rendered-pixels layer.

So: the desktop is a thin, hand-DOM projection wired to a real (but
mis-addressed) control client, wearing first-commit placeholder text. It is not
"alive with the same UI as web/mobile." That work was scoped (CL-2, CL-31,
CL-42) and partially landed as shared *tokens/protocol*, but the desktop
render path was never moved off the placeholder shell onto Foldkit.

---

## Evidence

### The node is up and healthy

```
$ lsof -nP -iTCP:4716 -sTCP:LISTEN
bun  29864  …  TCP 127.0.0.1:4716 (LISTEN)

$ ps -o command= -p 29864
bun apps/pylon/src/index.ts node

$ lsof -a -p 29864 -d cwd        # working dir of the node
/Users/christopherdavid/work/openagents

$ curl -s http://127.0.0.1:4716/health
{"ok":true,"schema":"openagents.pylon.control.v0.3"}

$ find … -name control-token
/Users/christopherdavid/work/openagents/.pylon-tailnet/control-token
```

The control token the desktop needs exists — under `.pylon-tailnet/`, the home
the running node uses.

### Why the desktop reports "offline"

`src/bun/index.ts:9-11`:

```ts
const controlBaseUrl = Bun.env.PYLON_CONTROL_BASE_URL ?? "http://127.0.0.1:4716"
const pylonHome = Bun.env.PYLON_HOME ?? join(process.cwd(), ".pylon-local")
```

- `controlBaseUrl` defaults correctly to the running node's address.
- `pylonHome` defaults to `<cwd>/.pylon-local`. There is **no** `.pylon-local`
  anywhere; the live token is in `.pylon-tailnet`.

The poll's fetch wrapper reads the token *first* and throws if it is missing
(`src/bun/index.ts:55-62`):

```ts
async fetchNodeState() {
  const token = readControlToken(pylonHome)
  if (token === null) throw new Error("Pylon control token is not available")
  return fetchNodeState({ baseUrl: controlBaseUrl, token })
}
```

`pollNodeStateOnce` catches any throw and returns the offline fallback
(`src/bun/node-state-poll.ts:31-34, 78-84`):

```ts
} catch {
  return offlineNodeState(input.fallbackSchema)   // { ok:false, schema, sessions:[] }
}
```

…which `nodeStatusLine` renders as `offline · 0 sessions`
(`src/ui/session-view.ts:76-87`). So the "offline" line is produced **without
ever contacting the node** — it is purely "couldn't find the token at
`.pylon-local/`."

**Fix (operational):** launch the desktop with the env pointed at the real
home, e.g.

```sh
PYLON_HOME=/Users/christopherdavid/work/openagents/.pylon-tailnet \
PYLON_CONTROL_BASE_URL=http://127.0.0.1:4716 \
bun run dev
```

or, better, have the desktop default-discover the node home the same way the
TUI does (see Remediation). The new `~/.zshrc` `autopilot` function should set
`PYLON_HOME` so it connects out of the box.

### Why there's "no UI"

`src/ui/main.ts:50-63` is the entire above-the-fold render, built by hand:

```ts
const h = document.createElement("h1")
h.textContent = "🛩️  Autopilot Desktop"
…
a.textContent = "Electrobun + Bun + (Foldkit next) — the desktop client shell is alive."
…
c.textContent = "Next (CL-5): connect to the local Pylon node over loopback and render live sessions."
```

That copy is the **first-scaffold placeholder** (commits `a548b5c13`,
`288190782`) and was never removed, even though CL-5 was later marked
"completed" (`f7fa56c6f`). The session list (`src/ui/session-render.ts`,
`src/ui/session-view.ts`) is a separate, *minimal hand-rolled* HTML string
builder — not the shared Foldkit views.

The desktop's only dependency on the shared UI package is CSS tokens
(`src/ui/theme.ts:1`):

```ts
import { cssVars, darkTokens } from "@openagentsinc/autopilot-ui"
```

It does **not** import any of the shared Foldkit components that exist in that
package and that the web client renders:

```
packages/autopilot-ui/src/  → view.ts, node-status.ts, session-actions.ts,
  decision-actions.ts, steer-controls.ts, accounts.ts, artifacts.ts,
  earnings.ts, cloud-quota.ts, verify-status.ts, assignments.ts
```

`grep foldkit apps/autopilot-desktop/src` → **nothing.** There is no Vite build,
no `views://` Foldkit bundle; `electrobun.config.ts` just copies a static
`index.html` and bundles `src/ui/main.ts` directly. Contrast the desktop audit's
own architecture section, which promised "the same stack as
`apps/openagents.com/apps/web`" built "with Vite + `@foldkit/vite-plugin`."

### The parity claim, checked

`2026-06-13-autopilot-clients-roadmap.md:84` states:

> "Desktop already shares the Foldkit components; mobile (RN) maps the same
> tokens."

First clause is **inaccurate** for the rendered UI. What is actually shared:

| Layer | Shared across clients? | Evidence |
|---|---|---|
| Control/bridge **protocol** | ✅ yes | `@openagentsinc/autopilot-control-protocol` imported by pylon, desktop, web; mirrored by mobile |
| **View-model** helpers / **design tokens** | ✅ yes (tokens) | desktop imports `cssVars`/`darkTokens`; CL-31/CL-42 dark-mode tokens |
| Cross-client **conformance matrix** | ✅ yes | CL-33 `tests/conformance.test.ts` |
| **Rendered Foldkit components** | ❌ desktop: **no** | desktop hand-DOMs its own; only **web** imports `autopilot-ui` views (`apps/openagents.com/apps/web/src/page/clientsPreview.ts`) |

So "parity" is real at the *data/contract/token* level and absent at the
*rendered-UI* level for desktop. The owner's read — "where's the UI, I thought
we had parity with mobile and web" — is correct: the desktop never moved off its
placeholder shell.

---

## What was actually delivered (desktop)

From `git log -- apps/autopilot-desktop` the desktop got real, working plumbing:

- `a548b5c13` Electrobun scaffold (CL-3)
- `288190782` boots a native Electrobun window
- `43fa96705` / `76b04587f` loopback control client + Bun→webview node-state RPC
  + (hand-rolled) session render (CL-5 P0)
- `f7fa56c6f` live session-detail timeline (CL-5)
- `74ce8df78` node-status breakdown + verify line + accounts panel
- `ef487c8d0` required-artifact inspection
- `2af49f406` cross-client conformance matrix (CL-33)
- `6a88b577e` nest sub-agent sessions under parent
- `486a5ecc0` dark-theme CSS from shared tokens (CL-31)
- `6cecfa17b` auto-update feed chooser
- `78dcc51e5` bridge transport (CL-14)
- `40dfd12cf` OS notifications (CL-30)
- `731cef1b5` Deploy to Cloud (CL-26)
- `c1ea7c61e` distribution infra

This is genuine end-to-end wiring (control client, RPC, polling, notifier,
deploy, bridge, conformance). **But every one of these renders through the
hand-DOM shell**, and the connect path defaults to a home (`.pylon-local`) that
doesn't match the running node's home (`.pylon-tailnet`) — so on this machine,
launched via the `autopilot` alias, it shows the placeholder + offline.

---

## Root causes (two, independent)

1. **Connectivity: home-path default mismatch.** `PYLON_HOME` defaults to
   `<cwd>/.pylon-local`, but the node runs with `.pylon-tailnet`. Result:
   token-not-found → offline fallback. *Operational/config bug, trivially
   fixable; no node problem.*

2. **UI: placeholder shell was never replaced with Foldkit.** The render path is
   hand-DOM with stale scaffold copy; the shared Foldkit component package
   (`autopilot-ui`) exists and is consumed by **web only**. Desktop adopted the
   *tokens* but not the *components*, so there is no visual parity, and the
   roadmap's "Desktop already shares the Foldkit components" overstates it.
   *Real product gap.*

---

## Remediation plan

### Now (unblocks the owner's "where's the UI" instantly)

- **R1 — Make `autopilot` connect by default.** Set `PYLON_HOME` in the launcher
  so loopback works out of the box. Either (a) export
  `PYLON_HOME=…/openagents/.pylon-tailnet` in the `~/.zshrc` `autopilot`
  function, or (b) better, have `src/bun/index.ts` auto-discover the node home:
  probe `.pylon-tailnet` then `.pylon-local` under the openagents repo root, and
  fall back to `PYLON_HOME`. This is the single change that turns "offline · 0
  sessions" into the live session list that already exists.
- **R2 — Delete the placeholder copy.** Remove the "shell is alive / (Foldkit
  next) / Next (CL-5)" header from `src/ui/main.ts:50-63`. CL-5 shipped; the
  text is misleading on every launch.

### Next (the actual parity work — the real gap)

- **R3 — Move the desktop render path onto Foldkit + `autopilot-ui`.** Stand up
  the Vite + `@foldkit/vite-plugin` build the desktop audit specified, bundle to
  `views://autopilot-desktop/…`, and render the **shared** components
  (`view.ts`, `node-status`, `session-actions`, `decision-actions`,
  `steer-controls`, `accounts`, `artifacts`, `verify-status`, `cloud-quota`)
  instead of the hand-DOM in `session-view.ts`/`session-render.ts`. This is the
  work that produces genuine web/desktop pixel parity. (Re-open / scope under
  CL-2 + CL-31; the package is ready, only the desktop consumer is missing.)
- **R4 — Correct the roadmap claim.** Change `…-clients-roadmap.md:84` from
  "Desktop already shares the Foldkit components" to the accurate state:
  desktop shares **protocol + tokens + conformance**; Foldkit **component**
  adoption is pending (R3). Don't let the doc assert parity the code hasn't
  shipped.
- **R5 — Conformance should cover render adoption.** The CL-33 matrix asserts
  data/contract parity; add a check (or explicit caveat) that the desktop
  renders via the shared component package, so this divergence can't silently
  recur.

### Verification

After R1–R3, the acceptance is the roadmap's own M1 bar
(`…-clients-roadmap.md:45`): "Desktop *and* mobile-in-emulator both show the
**same live Autopilot Coder session** running on this Mac" — rendered through
the **same** Foldkit components on desktop and web, with mobile mapping the same
tokens.

---

## Honest one-paragraph status

The Autopilot Desktop app is real, end-to-end-wired plumbing (Electrobun shell,
loopback control client, node-state RPC + polling, notifier, deploy, bridge,
conformance matrix) that today renders through a hand-built DOM shell still
wearing its first-commit "shell is alive / Foldkit next" placeholder, and — on
this machine, launched via the `autopilot` alias — fails to connect only because
its default Pylon home (`.pylon-local`) doesn't match the running node's home
(`.pylon-tailnet`), so it shows "offline · 0 sessions" without ever reaching the
healthy node. There is no rendered-UI parity with web/mobile yet: the shared
Foldkit component package exists and the web client uses it, but the desktop
adopted only the design tokens, not the components. Two fixes close the gap a
launcher/discovery change for connectivity (R1), and moving the desktop render
path onto Foldkit + `autopilot-ui` (R3) for the parity the roadmap already
promised.
