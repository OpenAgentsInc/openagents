# FORK a realm: your own copy with your own rules

Status: built October 30 (ev-fork-realm, Episode 290). Our Everglade stays as it is; anyone can
fork it, change its rules, and keep the fork in their account ("Doomglade": always night, and the
watch comes in waves).

## In the realm

FORK stands by Everglade's title plate (drawn in Coax, `apps/openagents-web/everglade-fork.coax`).
It opens a pane: a row for each rule set Everglade's code implements, ON or OFF by a tap, then
FORK IT and CLOSE. FORK IT makes the fork and the page goes there. On a fork the button reads
FORK OF EVERGLADE; its pane names the fork, says what it was forked from and who made that
(FORKED FROM EVERGLADE · MADE BY OPENAGENTS), and has ORIGINAL (back to what it forked), PUBLISH
for its owner, and FORK IT to fork it again.

## The rule sets

`verse.fork_bases.rules` (migration `0075_realm_forks.sql`) lists what each base's code really
switches; Everglade's:

| Rule | What Everglade does |
| --- | --- |
| `night` | Always night: the clock held at midnight |
| `waves` | Hostile waves: the town watch sent in from the gate nearest you after 20 s and every 45 s, a WANTED level higher each time (the watch's own escalation, `verse/everglade/watch.coax`) |
| `nodestroy` | No destruction: spells and the hammer break nothing (every piece's hit points held above any blow; whatever came down stands back up) |
| `lowgrav` | Low gravity: a third of the fall (the face gives the rest back while you're in the air) |

The page facts carry them first (`rules=night,waves`, `fork=<slug>`; host.js from the page's
`data-rules` and `data-fork`); once the shared edit layer has the fork's spec, the fork's board
decides, so a switch takes effect at once.

## What a fork is

A made realm ([realm-builder.md](realm-builder.md)): a `verse.made_realms` row with world
`made-<slug>`, owned by the player key (or the account, once they log in), whose `base` is the
built-in realm whose module plays it and `forked_from` what it was forked from (the base, or
another fork). Its board is the base's rows as they are now (Everglade's flock, floats,
undergrowth, day cycle) and its rules, each a row feature keyed by its id, so the shared edit
layer ([realm-edit.md](realm-edit.md)) edits it: SWITCH, and TYPE IT revises it under a schema
whose enum is the fork's row ids. Its city starts as the base's published city and its edits by
hand or by voice (HOLD TO TALK, `POST /api/playtest/realm/<fork>/edit` with the base's catalogue)
go to its own world, never Everglade's. The original never changes.

Its page at `/realms/<slug>` is written into the site's bucket as a made realm's is: the preview
tags and card (0071), "Forked from Everglade, made by OpenAgents" with a link, and a script that
loads Everglade's page's module with the fork's slot (`made-<slug>`), slug and rules. Until its
owner publishes it the page is `noindex` and left off `/og`'s rows; published, it is listed with
its own line (written from its rules when its owner writes none).

## The routes

| | |
| --- | --- |
| `GET /api/playtest/realms/<slug>/fork` | text: `base`, `title`, `fork 0/1`, `from SLUG\|NAME`, `maker`, `name`, `owner 0/1`, `published 0/1`, `rule ID 0/1 LABEL`; 404 not forkable |
| `POST /api/playtest/realms/<slug>/fork {rules, name?}` | 200 `{slug, path, name, base, from, rules, page}` (`?as=lines`: `slug`, `path`, `page`); 400 a rule the base doesn't implement or a name of no plain words, 401 no player, 404, 429 today's realms (`OA_BUILDER_PER_DAY`) |
| `POST /api/playtest/realms/<fork>/publish {published?, description?}` | its owner or a site admin; 200 `{published, description}`; 403, 404 |

Names are one line of at most 40 characters with markup characters taken out; pages escape
them again and realms draw them as Coax glyphs. A new base takes a row in `verse.fork_bases`
(and its rules in its code) in a migration.

## Files

`coax/web/api/migrations/0075_realm_forks.sql`, `realms.c` (fork, publish), `npc.c` (a fork's
voice edits), `test_realm_fork.py`; `coax/web/host.js` (the `fork` and `rules` facts);
`apps/openagents-web/everglade-fork.coax` (its `#test`s), `everglade.coax`, `everglade-edit.coax`,
`everglade-face.c`.
