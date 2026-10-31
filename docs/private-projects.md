# Private projects

A private project is a page at `https://openagents.com/projects/<slug>` that
only its members, and agents they register, can see. Members are Nostr keys
named in the project's document. There is no WorkOS and no password: a member
signs in with a one-time link minted by another member, signs requests
with their key, or signs in with the OpenAgents phone app once the phone is
linked to them ([phone-sign-in.md](phone-sign-in.md)).

The mechanism is public (this repo). Each project's contents are not: the
document lives in a private repo and is served by oa-api from Cloud SQL, only
to members. Nothing of a private project goes on the public Nostr relay or in
the public site bucket.

## Pieces

| Piece | Where |
| --- | --- |
| Storage and rules | `coax/web/api/migrations/0007_private_projects.sql` (schema `projects`, function `projects.api`); the project's agent in `0013_project_agent.sql` |
| HTTP routes | `coax/web/api/private.c` |
| Page | `apps/openagents-web/private.coax`, `private-face.c`, `private.wgsl`, the generic locked page `apps/openagents-web/projects/private/` |
| Tool | `coax/web/api/private.sh` |
| Routing | `coax/web/api/lb.sh private-route SLUG`, `lb.sh private-pages` |
| Checks | `coax/web/api/test_private.py` and `test_private_agent.py` (run by `coax/web/api/test.sh`), `coax test apps/openagents-web/private.coax`, `coax/web/smoke.sh` |
| Threat model | `docs/private-projects-threat-model.md` |

## The document

`openagents.private-project.v1`, JSON, kept in the project's private repo:

- `slug`, `title`, `summary`, `rules` (strings)
- `members`: `[{name, pubkey}]`, hex pubkeys
- `goals`: `[{id, text, status: agreed|proposed, agreed_by: [names]}]`; an
  agreed goal names the members who agreed in `agreed_by`
- `days`: `[{date: YYYY-MM-DD, label, milestones: [{id, text, owner, status: todo|doing|done, note}]}]`, Chicago dates
- `checklists`: `[{id, title, items: [{id, text, status, evidence}]}]`
- `links`: `[{label, url}]`; a link whose label names analytics or a dashboard
  is shown in the ANALYTICS panel
- Sprints, all optional (`0087_project_sprints.sql`):
  - `sprints`: `[{id, goal, start, end, status: planned|active|closed, agreed_by: [names], review, retro}]`;
    one goal each, agreed by members, one active at a time
  - `backlog`: `[{id, text, status: todo|doing|done, evidence, stretch, carried_from, proposed_by}]`,
    in priority order; a done item names its evidence; `stretch` marks a stretch
    goal and `carried_from` the date an unfinished item came from
  - `wip_limit`: at most this many backlog items are `doing` (1 to 20)
  - `priority_changes`: `[{at, what, why, proposed_by, agreed_by: [names]}]`, the
    log the next digest reads; a change with no `agreed_by` is a proposal

Every push stores a new version and keeps the old one in `projects.history`.
A push names the version it was based on and is refused (409) when someone
pushed in between.

## Who may do what

| | member | agent | the project's agent (runner) | anyone else |
| --- | --- | --- | --- | --- |
| read the state | yes | yes | yes | 404 |
| push the document | yes | only milestone/item/backlog status and notes, new *proposed* goals, backlog items it proposes (`proposed_by`) and proposed priority changes | anything but the title, the members, agreed goals and sprint goals, only inside a running turn | 404 |
| change a sprint, the backlog's order, the WIP limit or an agreed priority change | yes | 403 | sprint goals 403; the rest inside a turn | 404 |
| change or remove agreed goals, members, title | yes | 403 | 403 | 404 |
| post notes and plans, messages and replies | yes | yes | inside a turn | 404 |
| set a milestone's or item's status | yes | yes | inside a turn | 404 |
| mint sign-in links, register agents | yes | 403 | 403 | 404 |
| talk to the project's agent, stop a turn, UNDO TURN, agree to a proposed goal | yes (approved projects) | 404 | 403 | 404 |

The project's agent ([project-agent.md](project-agent.md)) is a key a member
registers with `private.sh agent SLUG PUBKEY NAME --runner`. Only projects in
oa-api's server config (`OA_AGENT_PROJECTS`, from Secret Manager at deploy
time) have one; for any other project its routes answer the same 404 as a
project that doesn't exist. Every turn records the member message that caused
it, each document edit as a diff under it, and its commit; UNDO TURN restores
the version from before the turn as a new version (refused while a later
change sits on top) and has the runner push `git revert` of the turn's commit.
A proposed goal becomes agreed when every member has pressed AGREE on its
current text.

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
