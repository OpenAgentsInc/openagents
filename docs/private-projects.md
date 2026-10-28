# Private projects

A private project is a page at `https://openagents.com/projects/<slug>` that
only its members, and agents they register, can see. Members are Nostr keys
named in the project's document. There is no WorkOS and no password: a member
signs in with a one-time link minted by another member, or signs requests
with their key.

The mechanism is public (this repo). Each project's contents are not: the
document lives in a private repo and is served by oa-api from Cloud SQL, only
to members. Nothing of a private project goes on the public Nostr relay or in
the public site bucket.

## Pieces

| Piece | Where |
| --- | --- |
| Storage and rules | `coax/web/api/migrations/0007_private_projects.sql` (schema `projects`, function `projects.api`) |
| HTTP routes | `coax/web/api/private.c` |
| Page | `apps/openagents-web/private.coax`, `private-face.c`, `private.wgsl`, the generic locked page `apps/openagents-web/projects/private/` |
| Tool | `coax/web/api/private.sh` |
| Routing | `coax/web/api/lb.sh private-route SLUG`, `lb.sh private-pages` |
| Checks | `coax/web/api/test_private.py` (run by `coax/web/api/test.sh`), `coax test apps/openagents-web/private.coax`, `coax/web/smoke.sh` |
| Threat model | `docs/private-projects-threat-model.md` |

## The document

`openagents.private-project.v1`, JSON, kept in the project's private repo:

- `slug`, `title`, `summary`, `rules` (strings)
- `members`: `[{name, pubkey}]`, hex pubkeys
- `goals`: `[{id, text, status: agreed|proposed, agreed_by: [names]}]`; an
  agreed goal names every member in `agreed_by`
- `days`: `[{date: YYYY-MM-DD, label, milestones: [{id, text, owner, status: todo|doing|done, note}]}]`, Chicago dates
- `checklists`: `[{id, title, items: [{id, text, status, evidence}]}]`
- `links`: `[{label, url}]`; a link whose label names analytics or a dashboard
  is shown in the ANALYTICS panel

Every push stores a new version and keeps the old one in `projects.history`.
A push names the version it was based on and is refused (409) when someone
pushed in between.

## Who may do what

| | member | agent | anyone else |
| --- | --- | --- | --- |
| read the state | yes | yes | 404 |
| push the document | yes | only milestone/item status and notes, and new *proposed* goals | 404 |
| change or remove agreed goals, members, title | yes | 403 | 404 |
| post notes and plans, messages and replies | yes | yes | 404 |
| set a milestone's or item's status | yes | yes | 404 |
| mint sign-in links, register agents | yes | 403 | 404 |

The first push of a new slug is allowed only from a key in
`coax/web/api/admins.conf`, and must list that key as a member.

## Using it

Members, from a machine with their key (default
`~/work/.secrets/openagents-analytics-admin.key`):

```
coax/web/api/private.sh push PATH/TO/SLUG.json        # push the document
coax/web/api/private.sh open SLUG                     # sign this browser in (opens the default browser)
coax/web/api/private.sh invite SLUG MEMBER_PUBKEY     # a one-time link for another member, good 24 hours
coax/web/api/private.sh agent SLUG AGENT_PUBKEY NAME  # register an agent key (--remove to drop it)
coax/web/api/lb.sh private-route SLUG                 # once per new slug: send /projects/SLUG to oa-api
```

Agents, with their own key (default `~/work/.secrets/openagents-agent-private.key`,
or `--key FILE`), always saying who they are with `--by`:

```
coax/web/api/private.sh get SLUG                          # the state as Markdown
coax/web/api/private.sh inbox SLUG                        # members' messages nobody has answered
coax/web/api/private.sh reply SLUG ID "TEXT" --by NAME
coax/web/api/private.sh note SLUG "TEXT" --by NAME
coax/web/api/private.sh plan SLUG PLAN.md --by NAME
coax/web/api/private.sh status SLUG ID done --note "evidence" --by NAME
```

A new agent key: `coax/web/api/private.sh new-key KEY_FILE` (written chmod 600;
prints only the public key), then a member registers that pubkey with
`private.sh agent`.

## The page

`/projects/<slug>` answers from oa-api: a member's shell (200) or the locked
page (404). Both are the generic page's script under a different `data-page`,
so the project's own words come only from `/projects/<slug>/api/state`, with
the session cookie. Panels: GOALS, TODAY (a day's milestones, with the days
either side), CHECKLIST, ANALYTICS (live from `oa_analytics`, today and the
last 7 days, plus the document's dashboard link), AGENTS (newest first) and
MESSAGES (a thread and a field to write to the agents). It reads the state
every 30 seconds and when the tab is shown again; LIVE turns to OFFLINE when
the last good answer is older than 75 seconds.

## Next: end-to-end encryption

Today contents are plain text in the private repo and in Cloud SQL, readable
by whoever runs oa-api. The next step is NIP-44: the document and messages
encrypted to each member's and agent's pubkey (a per-project content key
wrapped for each), decrypted in the page with the member's key (via
`nostr-effect`, or Coax crypto when it lands) and by `private.sh` locally.
oa-api would then store and check only ciphertext, versions and signatures;
the rules it can no longer read (agents may not change agreed goals) move to
signed per-field updates checked by members' clients.
