# Everglade on its own domain

Status, 2026-11-08: live at `everglade.world` (DNS done; the owner chose the name).
`/` is a standard MMORPG front door (owner: "standard mmorpg landing page then right
into the game world"): a hero capture, the EVERGLADE wordmark in CharacterCreateARKaiC
(`coax/web/everglade-wordmark.coax` draws it as SVG outlines), PLAY NOW, four features,
three screenshots and a footer. `/play` is the one address for playing (owner, 2026-11-08: the
address bar says `everglade.world/play` all the way in, never `/worlds/...` or `?via=`). It is
Everglade II with Character Create's arrival and font (`domain.create` in `realms.json`): with no
character kept in this browser (`aethyr.player.appearance.v1`) it opens Character Create in the glade
and runs you into the world on the same page and address; with one it opens the world with that look,
class and level, where you stood last (kept in this browser as `coaxial.everglade2.place`, host action
11, and told to the room as the hello's `A`, #13057; the room's character checkpoint brings back class,
level and health). The front door opens `/play?via=portal` out of sight under itself as soon as it has
painted, so PLAY NOW fades into the game in about half a second with no page load, and says `/play`.
`/worlds/character-creator` and `/worlds/everglade2` on this host 301 to `/play` with the query
dropped. Any other path shows Everglade's 404 (status 404; the bucket's 404 page writes it in on this
host). The pages are `realms/everglade2/domain/`, written by `realms.ts` from the `domain`
block in `realms.json`. Not built yet: host-aware login, the host field in analytics,
the release smoke and golden for the domain, and `staging.everglade.world`. The URL map's
`everglade` matcher was set by hand (below); `lb.sh everglade-domain` is still to write.

## Goal

Everglade gets a domain of its own, written `EVERGLADE_DOMAIN` below until the
owner picks the name. On that domain Everglade is its own thing: a visitor
lands in Everglade and never passes through openagents.com, its home page,
its navigation or its branding.

OpenAgents still hosts it. It's the same build, the same bucket, the same
release, the same rooms and the same API that serve
`openagents.com/worlds/...` today. One release updates both addresses at once.
There's no second deployment, no fork of the code and no new load balancer.

"Everglade" here means Everglade II (`/worlds/everglade2`), which is becoming Everglade:
the portal arrival, then Character Create in the glade, then the forest path,
all in one multiplayer zone.

## What a visitor sees

| Address | What it serves |
| --- | --- |
| `https://EVERGLADE_DOMAIN/` | The Everglade entry: the portal, the single "Create your character" button, the fly-in, Character Create in the glade, then the run into Everglade II. It's the same flow as the portal lander on openagents.com, with no OpenAgents chrome. |
| `https://EVERGLADE_DOMAIN/play` | Straight into Everglade II (the returning player's way in, like `/worlds/everglade2`). |
| `https://EVERGLADE_DOMAIN/login` | Log in, on this domain, with Everglade's look. |
| `http://EVERGLADE_DOMAIN/...`, `www.EVERGLADE_DOMAIN` | 301 to `https://EVERGLADE_DOMAIN/`, path and query kept. |
| Any other page path | Everglade's own 404 page, with a link back to `/`. It never redirects to openagents.com and never shows an openagents.com page. |

`openagents.com/worlds/everglade2`, Character Create and the portal lander stay
exactly as they are. Both addresses serve the same world, and players on
either one meet in the same rooms.

"No openagents.com entry" means:
- no OpenAgents header, footer navigation, galaxy, `/worlds` list or home link;
- no "OpenAgents" in the title, description, link preview, favicon or app name;
- no link that leaves the domain, except one small "Hosted by OpenAgents" line
  in the menu, if the owner wants it (open question 2).

## How it's served

### One load balancer, one more host

openagents.com is served by the global HTTPS load balancer whose URL map is
`coaxial-openagents-web-staging` (the name is historical; it serves production
`openagents.com` and `staging.openagents.com` from IP `136.69.24.60`). Its
host rules send `openagents.com` to the `production` path matcher and
`staging.openagents.com` to `staging`. The production matcher's default is the
`coaxial-openagents-web` CDN backend bucket, with route rules for `/api/*`
(Cloud Run `oa-api`), the room shards (`/api/room/s2/`, `/api/room/s3/`) and
redirects.

Everglade's domain adds to that same load balancer:

1. **A host rule** for `EVERGLADE_DOMAIN` and `www.EVERGLADE_DOMAIN`, with
   `staging.EVERGLADE_DOMAIN` on the staging matcher's twin.
2. **A path matcher `everglade`** (and `everglade-staging`), whose rules are:
   - `/`, `/play` and `/login`: a `urlRewrite` to the Everglade pages in the
     bucket (below), so the address bar keeps the short path;
   - the asset and engine paths the pages load (the build's wasm and JS, the
     realm packs, fonts, sounds, particles; enumerate them from the Everglade
     pages' `data-assets` and script tags): the same bucket, unchanged;
   - `/api/*` and the room shards: exactly the backends `openagents.com` uses,
     so play, saves and analytics work the same;
   - `www` and plain HTTP: the 301s in the table above;
   - everything else: Everglade's 404 page, status 404.
3. **A certificate.** A Certificate Manager certificate for the apex and `www`,
   authorized by DNS (as `coax/web/cdn.sh` does), so it's issued before DNS
   moves, plus entries in the load balancer's certificate map for each hostname.

The path rules are written by a script, the way `coax/web/api/lb.sh` writes
`/api`, private and memorial routes: `coax/web/api/lb.sh everglade-domain
DOMAIN`. It makes only what's missing, leaves every other rule as it is, and
validates the map before import. A `--dry-run` prints the new map. Rolling
back removes the host rule (one import), and openagents.com is untouched in
both directions.

Cost: no new load balancer, IP or forwarding rule. The added cost is one
managed certificate plus the domain's registration. This keeps inside the GCP
monthly cap.

### The pages

`apps/openagents-web/realms.json` gains a `domain` block on the Everglade II
entry, so the realm generator (`coax/web/realms.ts`) writes the domain's pages
next to the realm's own:

```json
"domain": {
  "host": "EVERGLADE_DOMAIN",
  "title": "Everglade",
  "description": "…",
  "entry": "portal",
  "brand": "everglade"
}
```

- `realms/everglade2/domain/index.html`, `play/index.html`, `login/index.html`
  and `404.html` are generated from the same templates as the realm pages, with
  `data-brand="everglade"`.
- The pages use absolute asset paths that resolve on either host. Nothing in
  the page names `openagents.com`; the canonical URL, link preview and
  `index.md` (written by hand, as every realm page needs) use
  `https://EVERGLADE_DOMAIN/`.
- `data-brand="everglade"` makes the shared UI drop the OpenAgents chrome and
  use Everglade's name, icon, link-preview card and colours. The world, HUD,
  Character Create and the CharacterCreateARKaiC font don't change.
- `realms.ts --check` refuses a `domain` page that links to openagents.com, a
  missing `index.md`, or a host that isn't a plain domain name.

### Accounts, saves and cookies

Browsers keep cookies per host, so a session on openagents.com doesn't carry
to `EVERGLADE_DOMAIN`, and the reverse is also true.

- **Log in on the domain.** The API's login flow takes the request host into
  account: the redirect URI and session cookie are issued for
  `EVERGLADE_DOMAIN` (host-only, `Secure`, `SameSite=Lax`). The identity
  provider's allowed redirect list gains `https://EVERGLADE_DOMAIN/login/...`
  (an owner step if the dashboard needs a person).
- **One account.** It's the same user record on both hosts, keyed by the same
  identity. A character made on one address is there on the other after
  logging in.
- **Before logging in,** play works as it does today: a guest seat with a
  browser save slot. The save slot is per origin, so a guest's local save on
  openagents.com isn't visible on the domain.
- **Rooms** don't care about the host: a seat is the same whichever address
  the page came from.

### Analytics

The API already records each visit with the request's headers. It adds the
host as a field, so `/admin/analytics` and `/api/analytics/realms` can show
Everglade's own domain apart from `openagents.com/worlds/...`. Our own testing
stays excluded as it is now.

### Releases

`coax/web/release.sh` doesn't change how it ships: the domain's pages are files
in the same bucket, so every release updates them with everything else, and
`rollback.sh` rolls them back too. Two checks are added:

- the release's smoke step requests `https://EVERGLADE_DOMAIN/`, `/play` and
  one asset, and expects 200s and the Everglade title (staging uses
  `staging.EVERGLADE_DOMAIN`, or `curl --resolve` until its DNS exists);
- the golden walk captures the domain's `/` once, so a change that adds
  OpenAgents chrome to it shows up as a failed comparison.

## Owner steps (NEEDS_OWNER.md)

1. Choose and register `EVERGLADE_DOMAIN`.
2. In its DNS:
   - an `A` record for the apex (and `www`) to `136.69.24.60`, DNS only;
   - the certificate's DNS-authorization `CNAME`, which `lb.sh everglade-domain`
     prints.
3. If the identity provider needs a person to add redirect URIs, add
   `https://EVERGLADE_DOMAIN/login/callback` (the exact path is printed by the
   script).

The certificate is issued as soon as the authorization record resolves. The
host rule can go live before DNS moves, so the switch is just the `A` record.

## Build order

1. `realms.json` `domain` block, generator output, `data-brand="everglade"` in
   the shared UI, and the `--check` rules, all tested locally with
   `--resolve`.
2. `lb.sh everglade-domain`: the certificate, the host rule and the path
   matcher on staging first (`staging.EVERGLADE_DOMAIN`), then production,
   behind the owner's DNS.
3. Host-aware login, plus the host field in analytics.
4. Release smoke and golden for the domain.
5. Owner moves DNS. Check with real Chrome on desktop and phone (headless
   passes alone don't count for a hand-off).

## Open questions for the owner

1. The domain name.
2. Should the menu carry a small "Hosted by OpenAgents" line, or nothing?
3. Should `openagents.com/worlds/everglade2` stay as a second address, as this
   spec assumes, or later 301 to the domain?
4. Should the domain's `/` open on the portal (as the lander does) or go
   straight to Character Create?
