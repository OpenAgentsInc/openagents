# OpenAgents Sites: your realm on your domain, and yours to take

Status: spec, 2026-11-02. Nothing in this doc is built unless a line says
**built**. It replaces the retired 2026 Sites program (below) and builds on
Aethyrforge, the realm builder ([realm-builder.md](../verse/realm-builder.md),
[realm-edit.md](../verse/realm-edit.md)) and the `openagents` CLI
([2026-11-01-openagents-cli-v1.md](../cli/2026-11-01-openagents-cli-v1.md)).

## What you get

You make a realm by talking to the builder at `openagents.com/realms/new` or
with `openagents realm new`. Sites adds four things:

1. **Test before it's live.** You and the people you invite try a revision
   at a preview link before anyone else sees it. Publishing makes that exact
   revision live, and you can roll back.
2. **Your own address.** The realm answers at `name.openagents.site` with no
   setup, or at a domain you own, such as `play.acme-agency.com` or
   `www.client.com`.
3. **Your brand, or your client's.** You set the title, icon and link card,
   and you can turn off the OpenAgents chrome.
4. **Take it with you.** One EXPORT gives you everything needed to host the
   realm yourself, or to hand it to a coding agent to rebuild it in Three.js
   or any other engine. Exports are free, always.

Who it's for: a creator making a world for themselves, and an agency or
freelancer making one for a business that wants it on its own site.

## What we had before, and what we keep

OpenAgents Sites ran from June 4 to July 14, 2026. Customers ordered a site,
an agent built it, and an operator deployed it to
`sites.openagents.com/<slug>`. It ran on Cloudflare Workers for Platforms,
D1 and R2. Custom domains used Cloudflare for SaaS custom hostnames but never
went live: the API token was never set. It was removed in the move from
Cloudflare to Google Cloud (6154556111, "Sites returns later redesigned on
owned primitives"), and its money surfaces were retired on July 14
(c87ce7caad and the decommission receipt at cf642cafc2). Read its docs with
`git show d613b8ea22^:apps/openagents.com/docs/sites.md`.

We keep these ideas from it:

- Save, then deploy, then roll back, with a pointer to the live version.
- A URL for every version.
- An ownership token in DNS.
- Export receipts: what was exported, at which version, and its hash.
- Resolve the `Host` header to a tenant only after the reserved first-party hosts.

We drop the Cloudflare stack, operator-run deploys, and arbitrary customer
code. Sites v1 hosts realms, not arbitrary HTML or JavaScript.

## Concepts

| Term | Meaning |
| --- | --- |
| Realm | A made realm: a versioned document (`aethyr.realm` v1) with revisions, built (migration 0093). |
| Revision | One saved state of the document. Every edit makes the next one (built). |
| Published revision | The revision visitors get (built, `POST .../publish {rev}`). |
| Site | A realm, plus its addresses, its branding, and the build it's pinned to. One realm has at most one site. |
| Address | A hostname that serves the site: its free `*.openagents.site` name, and any custom domains. |
| Preview | A revision opened by a secret link, before or instead of publishing. |
| Release | What one publish freezes: the revision, the engine build, and the asset hashes. |

## 1. Test, preview and publish

Today (built) a realm has three states: moves not yet saved (in your browser
only), saved revisions (you see them, marked "not published"), and the
published revision (visitors see it). Visibility is `private`, `unlisted` or
`public` (migration 0099).

What's missing for "test before it goes live":

- **Preview links.** `POST /api/playtest/realms/<slug>/preview {rev}`
  returns a link with a 128-bit token: `https://<address>/?preview=<token>`.
  Anyone with the link sees that revision, even when the realm is private.
  A preview never counts as a visit, is `noindex`, and expires after 7 days
  or when you revoke it. On the page: SHARE A PREVIEW in the owner's bar. In
  the CLI: `openagents realm preview SLUG [--rev N]`.
- **Pinned releases.** Publishing freezes the revision today, but visitors
  play it with whatever engine build is current. A release must also record
  the engine build and the asset hashes it played with, and serve those, so
  shipping a new engine never changes a client's live site. This is #12629's
  "current gap". Upgrading a site to the new engine is an explicit step,
  previewed first: UPDATE ENGINE.
- **Rollback.** `POST .../publish {rev}` refuses a revision that isn't the
  newest (409). Add `POST .../rollback {release}`, which makes an earlier
  release live again without changing the document. CLI:
  `openagents realm releases SLUG`, `openagents realm rollback SLUG N`.

There is no separate staging server per creator. A preview runs the same
pages and API as production with a different revision, and that's the test
environment. `staging.openagents.com` stays ours for testing OpenAgents
itself.

## 2. Addresses

### Free subdomain

Every site gets `<name>.openagents.site` when it's first published. It's
served by one wildcard certificate, so it needs no setup.

User content gets a separate registrable domain, not `*.openagents.com`. A
subdomain of `openagents.com` shares cookie scope and reputation with our
login and API. A separate domain also lets us block one abusive site without
touching `openagents.com`. **Owner decision:** which domain to buy
(`openagents.site` is a placeholder).

### Custom domain

What the creator does, on the realm's SETTINGS pane under DOMAIN, or with
`openagents site domain add SLUG play.example.com`:

1. Type the domain.
2. Add the two DNS records the pane shows, with a COPY button on each:
   - `play.example.com CNAME sites.openagents.site`. For an apex domain
     (`example.com`), an `A` record to the load balancer's IP instead.
   - `_acme-challenge.play.example.com CNAME <id>.authorize.certificatemanager.goog`.
     This proves you control the domain and lets us get its certificate
     before traffic moves.
3. The pane checks the records every minute and says which one is missing
   or wrong. When the certificate is active it shows LIVE, and the domain
   serves the published release.

How we serve it, on today's stack (the Coaxial load balancer, see
`coax/web/release.sh` and `coax/web/cdn.sh`):

- **Certificate.** One Certificate Manager DNS authorization and one
  certificate per custom host, added as an entry on the load balancer's
  certificate map. `cdn.sh` already does this for our own hosts.
- **Routing.** The URL map gets one default rule for unknown hosts, sending
  them to a `site-serve` route in `oa-api`. It resolves `Host` to a site
  (reserved first-party hosts first, then `sites.addresses`) and serves the
  realm page for that site's release, with the slug and branding filled in.
  Adding a domain then needs no URL-map change, only a certificate-map entry.
  Today `oa-api` reads `Host` only for analytics, so this route is new.
- **Records.** `sites.addresses (host, site, kind, state, cert_name,
  dns_auth_name, verified_at, last_check)`, with states `pending_dns`,
  `issuing`, `live`, `failing` and `removed`. A host belongs to one site.
  Removing it deletes the certificate-map entry and the certificate.
- **Checks.** A scheduled job re-checks every address daily. A domain whose
  CNAME has moved away for 7 days goes to `failing`, and the owner is told.
  We never serve a host whose DNS no longer points at us.
- **Limits.** Certificate Manager and certificate-map quotas cap the number
  of custom domains per project. Check the current quotas before launch and
  set a per-account limit well under them.

Custom domains are a paid feature, priced in sats over our Bitcoin rails
(Lightning). **Owner decision:** the price, and whether one domain comes
free with a creator invite.

## 3. White label

A site's branding settings (SETTINGS, then BRAND, or
`openagents site brand SLUG --title ... --icon FILE`):

- title, description, icon and link card (`og:image`), with the realm's shot
  as the default card;
- OPENAGENTS CHROME on or off: the REALMS button, the OpenAgents name and
  the way back to `/realms`. Off by default on a custom domain;
- a small "Made with OpenAgents" mark, removable on the paid tier.

No custom HTML or scripts in v1. A site is a realm the engine draws, so
branding is data and can't break or hijack the page.

## 4. Export

EXPORT (in the owner's bar), or `openagents realm export SLUG [--rev N]
[--out DIR]`, writes one archive with three parts. The archive's
`receipt.json` names the realm, revision, release, engine build and the
SHA-256 of every file.

### For an agent to rebuild it

`rebuild/` gives a coding agent everything it needs to remake the realm in
Three.js or any other engine, without our code:

- `realm.json`: the document (`aethyr.realm` v1) and its schema, with every
  field documented.
- `scene.gltf` and `scene.json`: the realm laid out. The engine places
  things from the document, so the document alone doesn't say where each
  tree stands (`myrealm-shapes.coax` decides). The export resolves every
  placed thing to a position, rotation, scale, mesh and material, and
  includes the ground, water, sky colours, sun and lights.
- `REBUILD.md`: a brief written from the document, never by a model: the
  camera and its controls (third person, first person, top-down or side),
  the art style and palette, the time of day, what moves and how (flocks
  circling, herds grazing), the characters with their lore and quests, and
  what visitors may do.
- `shots/`: desktop and phone screenshots from the start position
  (`realm shot`), so the agent can compare its result with the original.

Done means: give `rebuild/` to Claude Code with "rebuild this in Three.js",
and its result matches the shots on layout, camera and style. We keep that as
a check, with one realm per camera kind.

### To host it yourself

`site/` is a static folder that runs from any web server (nginx, S3, any
static host): `index.html`, `host.js`, the realm's `.wasm`, its shaders and
assets, and the release's `realm.json`. It runs offline from the release.
Features that need our servers (characters that talk, voice, visitors
placing things, analytics) are off, unless you give the export an API key so
they call `openagents.com`. The engine is Apache-2.0 like the rest of this
repository, so you can host and change the export.

### The source

`source/` holds the Coax sources the release was built from, at the commit
the release names, for anyone who wants to change the engine itself.

## 5. Agencies and handoff

- A site can have **collaborators**: accounts that can edit and preview,
  but not publish or change domains, unless the owner gives them that.
- **Transfer** gives the realm, its site and its domains to another
  account, such as the client's. Their account must accept it.
- One account can own many sites. An agency's dashboard is `/realms/mine`
  with each site's addresses and state.

## CLI and web parity

Every Sites action is one route, a row in the CLI's command table, and a
control on the page, as with realms today:

| CLI | Route | Page |
| --- | --- | --- |
| `realm preview SLUG [--rev N]` | `POST .../preview` | SHARE A PREVIEW |
| `realm releases SLUG` / `realm rollback SLUG N` | `GET .../releases`, `POST .../rollback` | SETTINGS, RELEASES |
| `realm export SLUG [--rev N]` | `POST .../export` (an archive in the bucket, a signed link) | EXPORT |
| `site domain add\|list\|check\|remove` | `/api/sites/<slug>/domains` | SETTINGS, DOMAIN |
| `site brand SLUG ...` | `POST /api/sites/<slug>/brand` | SETTINGS, BRAND |
| `site transfer SLUG ACCOUNT` | `POST /api/sites/<slug>/transfer` | SETTINGS, OWNER |

## Order of work

| Step | What | Depends on |
| --- | --- | --- |
| S0 | `realm export` with `rebuild/` only (document, laid-out scene, REBUILD.md, shots) | nothing; the parts exist |
| S1 | Preview links | nothing |
| S2 | Pinned releases and rollback | #12629 |
| S3 | `*.openagents.site` addresses and branding | S2, the domain bought |
| S4 | Custom domains | S3, the price |
| S5 | `site/` self-host export | S2 |
| S6 | Collaborators and transfer | WorkOS accounts (`w-` accounts) |

S0 and S1 can ship this week. They answer "can I test it" and "is it mine"
before any money or DNS work.

## Owner decisions

1. The domain for user sites (placeholder `openagents.site`).
2. The price of a custom domain, and whether creators get one free.
3. Whether v1 hosts realms only (recommended) or arbitrary pages too.
4. Whether the self-host export may call our API with a key, and how that
   key is metered.
