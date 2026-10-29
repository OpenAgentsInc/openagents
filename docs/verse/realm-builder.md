# Build your own realm

The step after the Haunted House. When the house's quest is done, BUILD YOUR OWN REALM
(`realm.coax`, `BTN_OWN`) opens https://openagents.com/realms/new. The visitor says or types
the world they want, the page shows it as a short board with a checkbox per feature, and
MAKE IT makes a realm from only the checked features at `/realms/<slug>`, with no deploy,
and takes them into it.

## The flow

1. **Prompt.** HOLD TO TALK (or hold Space): the microphone is open only while it is held
   (`coax/web/voice.js` push to talk; the page's `data-voice="haunted-house"` reuses that route of the
   voice host, facing "Realm builder" so the host leaves the turn to the page). The heard words, or
   typed ones (TYPE IT), go to `POST /api/playtest/realms/plan?as=lines`.
2. **Board.** The server asks Gemini (`OA_BUILDER_MODEL`, default `gemini-3.8-flash`, Vertex first,
   like the NPCs) under a fixed JSON schema built from the catalogue. It keeps only catalogue
   features with catalogue values and writes each row's words itself (`verse.builder_label`), so the
   board says only what the engine will build. No keyword matching: the model picks, the schema and
   the catalogue bound it.
3. **Toggles.** A tap checks a row on or off. Talking again sends the words, the plan and the
   switches: the model revises that board.
4. **Make.** MAKE IT sends `{plan, on: [...]}` to `POST /api/playtest/realms?as=lines`. The server
   takes the plan's stored board (the page never sends features, only switches by index), keeps the
   checked rows, makes the world `made-<slug>` owned by the player key (or the account), and writes
   `realms/<slug>`, `realms/<slug>/index.html`, `index.md` and `realms/made-slugs.txt` into the
   site's bucket (`drops_site_write`). The page then navigates there.
5. **In the realm.** `/realms/<slug>` loads `/realms/built`'s module (`myrealm.coax`) for the slug,
   which reads `GET /api/playtest/realms/<slug>` and lays the realm out from the slug.

## The catalogue (what the engine builds today)

`verse.builder_catalogue()` in `coax/web/api/migrations/0048_realm_builder.sql`:

| Feature | Values | Built from |
|---|---|---|
| water | small, medium, large | a pond drawn by `myrealm.wgsl` |
| trees | round, pine, dead, mixed; 3 to 20 | the fixture pack's tree (dead: no leaves, tinted) |
| rocks | 2 to 16 | the fixture pack's rock |
| plants | 4 to 24 | the fixture pack's fern |
| lanterns | gold, orange, red, green, blue, violet, white; 4 to 16 | lights on posts (strokes) |
| flock | crow, bat; 3 to 12 | strokes circling whoever walks |
| props | crates, barrels, wagon, fence, camp (up to 5) | the fixture pack's props |
| character | a name and a role (up to 4) | a figure with its name and role |
| tool | build, fly, jump, map, talk, voice | what visitors may do (below) |

Grounds: meadow, forest, desert, snow, stone, beach. Times: day, dawn, dusk, night. Styles:
lowpoly and painterly (photoreal is not built yet, so it is not offered).

## Ownership and settings

- The owner is the player key that made it (`verse.worlds.owner_player`), or the account once the
  guest logs in: signing in moves a key's worlds to the account, so a guest claims their realm by
  logging in.
- Every rule is in SQL. `GET .../realms/<slug>` says `owner 1` only to the owner.
- SETTINGS (owner only, one pane): the tools visitors get. `POST .../<slug>/settings {tools}`; anyone
  else gets 403. The realm shows a tool's button only while it is on.
- BUILD: the owner's thing goes in at once. A visitor's (only while build is on) becomes a proposal.
  The owner sees N WAITING, and each card shows who, what, where and the count before and after,
  with ACCEPT and DECLINE (`POST .../proposals/<id>/accept|decline`; decided once).
- Talk and voice are owner settings in v1. Characters show their name and role; talking to them
  comes next.

## Limits

`OA_BUILDER_DAILY_CAP` model calls a day site-wide (3000), one plan per player every 2 s,
`OA_BUILDER_PER_DAY` realms a player a day (12), 200 placed things a realm, 50 waiting cards.
Made-realm slugs never take a built-in realm's address (`verse.builder_reserved`).

## Analytics

The builder's page is `realm-builder`: `builder-prompt` (via voice or typed, its length only,
revise), `builder-board` (rows, rows on), `builder-toggle` (feature, on), `builder-make`,
`realm-made` (slug, page). The made realm's page is `realm-made`: `realm-enter` (slug, owner),
`realm-setting`, `realm-place`, `realm-decide`. The team and admins are filtered out as on every
playtest page.

## Releases

Made realms live only in the bucket. `upload.py` never removes `realms/made-slugs.txt` or a listed
realm, and `release.sh` never copies staging's made realms over production's. `/realms/built`
(with no slug, a small realm of its own) and `/realms/new` have goldens.

## Files

`apps/openagents-web/builder.coax`, `builder-face.c`, `builder.wgsl`, `realms/new/`;
`myrealm.coax`, `myrealm-face.c`, `myrealm.wgsl`, `realms/built/`; `coax/web/api/realms.c`,
`migrations/0048_realm_builder.sql`, `test_realm_builder.py`.
