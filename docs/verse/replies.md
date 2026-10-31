# Reply with a realm (ev-reply-realm)

The talk's "3D reply guy" (Episode 290): reply to a post with a realm.

1. In Everglade press **REPLY** (top right, by MAP), or open
   `https://openagents.com/realms/everglade#reply`.
2. Paste a post's link or type a line, then **PUT IT UP** (Enter). The words
   go on a sign on the house on the road in by the South Gate
   (`verse/everglade/plan.coax` EG_BUILDING 12), and you stand across the
   street from it.
3. **SMASH IT** (Enter): a Meteor Swarm (the spell bar's own, SRD 5e) comes
   down on its roof, its dice seeded for this reply.
4. Once the dust settles the reply is kept and its link shows:
   **COPY LINK**, **OPEN IT**, **AGAIN**. Share the link back as your reply.

The link, `/realms/everglade/reply/<id>`, opens Everglade with the reply:
the same sign, the same place to stand and the same swarm from the same
seed, a few seconds after the town is in; then **WATCH AGAIN** and
**REPLY WITH YOUR OWN**. Its preview card is the moment of impact: when the
second meteor lands the realm's HUD goes off for a few frames but for the
sign, `coax/web/card.js` (startMoment) holds the next frame drawn, and once
the reply has its id that frame goes to it as its card, under the words.
If that upload didn't happen, the reply's page asks its maker for it
(`data-card`, `data-card-on="impact"`) and takes it at the replay's impact.

## What is kept

Only what the player typed or pasted: the line (1 to 140 characters) or the
link (an `http://` or `https://` address of 280 characters at most, with no
spaces or quotes), as one line of plain characters (no control characters
or bars; runs of space become one), the seed, the house and the player who
made it. A link is text: it is never fetched, unfurled or followed, on the
server or on the page, and nothing of a post's author is kept. The page
shows it as words, never as a link. Every word is escaped on the page
(HTML) and in its text (`index.md`), and drawn by the realm's own stroke
font in the scene.

## Routes (`coax/web/api/replies.c`, `migrations/0076_realm_replies.sql`)

| Route | What |
| --- | --- |
| `POST /api/playtest/replies {realm, text \| link, seed}` | 200 `{id, path, page}`; 400 (the limits), 401 (no player key), 403 (another site), 413, 429 (40 a player a day, `OA_REPLIES_PER_DAY`) |
| `GET /api/playtest/replies/<id>` | text: `id=`, `realm=`, `house=`, `seed=`, `kind=`, `text=` (what the realm replays) |
| `GET /api/playtest/replies/<id>/card` | `want` (its maker, no card yet) or `no` |
| `POST /api/playtest/replies/<id>/card` | its maker only: a 1200x630 JPEG under 300 KB, into the site's bucket |

The page, its bare path and `index.md` are written into the site's bucket at
`realms/everglade/reply/<id>` with every preview tag (`og:title` the words in
quotes, `og:image` its card, else Everglade's), `noindex`. A release keeps
them (`coax/web/upload.py`). Checks: `coax/web/api/test_reply_realm.py`
(`coax/web/api/test.sh`) and the `reply:` tests in
`apps/openagents-web/everglade-reply.coax`.
