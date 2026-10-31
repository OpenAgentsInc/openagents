# Build your own realm

The step after the Haunted House. When the house's quest is done, BUILD YOUR OWN REALM
(`realm.coax`, `BTN_OWN`) opens https://openagents.com/realms/new. The visitor says or types
the world they want, the page shows it as a short board with a checkbox per feature, and
MAKE IT makes a realm from only the checked features at `/realms/<slug>`, with no deploy,
and takes them into it.

## The flow

0. **Setup.** Before the prompt, a few questions one at a time, each with big tap choices (or the
   keys 1 to 6), SAY IT (TALK as a toggle; the words go to `POST /api/playtest/realms/pick
   {field, words}`, where the model picks one of that question's words under a schema of just
   those words, or "none") and SKIP (the model picks from the prompt):
   - 2D (side-on) or 3D;
   - the camera, 3D only: third person, first person or top-down. The owner asked for "first,
     second or third person"; second person isn't a game camera, so it's top-down instead;
   - the art style: low poly, painterly or flat;
   - the ground: meadow, forest, desert, snow, stone, beach;
   - the time of day: day, dawn, dusk, night.
   What you do there (explore, build, quest) isn't asked: there are no quests yet, and building is
   the board's own VISITORS CAN row. The answers go with every plan and MAKE IT as fixed fields
   (`setup {camera, style, ground, time}`; 2D is the side camera, 3D with no camera chosen is
   third person). The model is told they are fixed, and its board is held to them. The board shows
   the setup in a line with CHANGE, which goes through the questions again with its answers shown.
1. **Prompt.** TALK is a toggle (a tap or Space): it shows LISTENING and the words as they are
   heard; the next tap closes the microphone and sends them as one prompt. A pause between
   sentences doesn't send; four quiet seconds after words do, and 25 s with nothing heard closes
   it. The microphone is open only while TALK is on (`coax/web/voice.js` "on"/"end"; other realms
   keep push to talk). The page's `data-voice="haunted-house"` reuses that route of the
   voice host, facing "Realm builder" so the host leaves the turn to the page). The heard words, or
   typed ones (TYPE IT), go to `POST /api/playtest/realms/plan?as=lines`.
2. **Board.** The server asks Gemini (`OA_BUILDER_MODEL`, default `gemini-2.5-flash` with its thinking off (0060: a board in 1 to 2 s; a try cut off at 3.5 s is tried once more), Vertex first,
   like the NPCs) under a fixed JSON schema built from the catalogue. It keeps only catalogue
   features with catalogue values and writes each row's words itself (`verse.builder_label`), so the
   board says only what the engine will build. No keyword matching: the model picks, the schema and
   the catalogue bound it.
3. **Toggles.** A tap checks a row on or off. Talking again sends the words, the plan and the
   switches: the model revises that board.
   Every revision says so: "Saved. The board shows it now.", or "Saved. The board already had all
   of that." when the board came back the same.
   Nothing asked for is dropped (0079): the planner names every thing asked for in `asked`, each
   with the feature that makes it or none, and each with none stays on the board as a `want THING`
   line, under ASKED FOR, NOT IN IT YET, marked NOT APPLIED YET (at most 24, carried through every
   revision). MAKE IT puts each on the realm's feature list as a waiting ask
   (`docs/verse/realm-edit.md`, "What you asked for"), where an agent can build it.
   OR DROP A DESIGN DOCUMENT (`coax/web/docs.js`, action 31) sends a .md, .txt, .pdf or .docx to
   `POST /api/playtest/realms/plan/doc?as=lines {name, data, setup}` (migrations/0080): the server
   takes its text out (a .docx's `word/document.xml`, inflated; a PDF the model reads itself) and
   the planner plans the board it describes, the same way.
4. **Make.** MAKE IT makes exactly what the board shows: words still typed are sent first, and
   while words said or typed are on their way to the planner it waits for that board, then makes
   it ("Making it once your last words are on the board."). MAKE IT sends `{plan, on: [...]}` to `POST /api/playtest/realms?as=lines`. The server
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
| lanterns | gold, orange, red, green, blue, violet, white; 4 to 16 | lights in cages on dark posts, a pool of light each after dark |
| flock | crow, bat; 3 to 12 | dark wings circling low ahead of whoever walks |
| props | crates, barrels, wagon, fence, camp (up to 5) | the fixture pack's props |
| character | a name and a role (up to 4) | a figure (the pack's barrel and crate) with its name and role |
| house | white, red, blue, green, yellow, brown, grey; small to large; 1 to 5 | walls, a gabled roof, a door and lit windows (0065) |
| herd | bison, deer, horse; 3 to 10 | animals roaming their own patches of the land and stopping to graze (0079) |
| dwellings | tepee, tent, hut; 1 to 8 | a camp, doors to its middle: tepees of hide on crossed poles, ridge tents, round thatched huts |
| village | 3 to 8 people | its people walking about the dwellings or the house: plain figures in everyday colours, never a caricature |
| tool | build, fly, jump, map, talk, voice | what visitors may do (below) |

Cameras (`verse.builder_cameras()`, 0049; the spec's `camera` line, `myrealm.coax`):

| Camera | What you see | Controls |
|---|---|---|
| third | behind you and above (the first one built) | W/S walk, A/D turn, a drag turns |
| first | at eye height (1.6 m), your figure unseen | W/S walk, A/D turn, a drag looks round, up and down; on a phone the left thumb walks and the right looks |
| top | 17 m up and 10 m south of you, a fixed angle, north up | W/A/S/D, a thumb or a drag walk that way |
| side | 2D: side-on, orthographic (12 m tall, tilted 8° so the ground shows), a lane through the realm | A/D walk, W or Space jumps (always); trees and characters stop you, and you jump onto rocks, crates and barrels |

Palettes (0079, the spec's `palette` line): natural, pastel (pale and soft), vivid (bolder) and
muted (greyed); `myrealm.coax` tints every piece, the sky, the horizon and the ground with it, and
a pastel realm's textures are lifted pale in `myrealm.wgsl` (`paletted`) so wood and hide read soft.
An ask about colours, ground, time, style or camera is the setup (`asked` feature `setup`, 0081),
never a line still wanted.
A realm's revision in edit mode sets it too, and an ask's DELETE puts the one before back.

Grounds: meadow, forest, desert, snow, stone, beach. Times: day, dawn, dusk, night. Styles:
lowpoly, painterly and flat (photoreal is not built yet, so it is not offered). A painterly or flat
realm draws with `verse/style-preset.coax`: painterly is `STYLE_PAINTERLY`, flat is
`verse_style_flat(FLAT_GARDEN, tier)`, each at the realm's time of day (`verse_style_at`) and the
phone tier under 600 CSS pixels wide; `myrealm.wgsl` carries `verse/style.coax`'s functions
(`build.sh` splices them). Low poly draws as before.

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
- TALK: E by a character (within 3.5 m), or a tap on one, opens the talk: type and press Enter,
  BYE or Esc ends it, as walking 8 m away does. The question goes to the character chat Everglade's
  people use (`POST /api/playtest/npc/ask` with `realm` the slug; its memory of the player across
  visits), and the character knows the realm (its ground, time, features, the other characters) and
  the lore its owner wrote in EDIT. With TALK off a visitor gets no talk, and the server refuses them
  too (403, `migrations/0061_realm_talk.sql`); the owner always talks.
- VOICE (with TALK): HOLD TO TALK in the talk. The page's `data-voice="made-realm"` (realms.json's
  built, copied onto every made realm's page) is the voice host's route; holding faces the
  character, so the host leaves the turn to the page, which asks the character and hands the answer
  back to be spoken (`coax/web/voice.js` actions 7, 8 and 9).

## Limits

`OA_BUILDER_DAILY_CAP` model calls a day site-wide (3000), one plan per player every 2 s,
`OA_BUILDER_PER_DAY` realms a player a day (12), 200 placed things a realm, 50 waiting cards.
Made-realm slugs never take a built-in realm's address (`verse.builder_reserved`).

## Analytics

The builder's page is `realm-builder`: `builder-prompt` (via voice or typed, its length only,
revise), `builder-board` (rows, rows on), `builder-toggle` (feature, on), `builder-describe` (chars), `builder-make`,
`realm-made` (slug, page). The made realm's page is `realm-made`: `realm-enter` (slug, owner), `realm-talk-open` (owner), `realm-talk` (owner, voice),
`realm-setting`, `realm-place`, `realm-decide`. The team and admins are filtered out as on every
playtest page.

## Releases

Made realms live only in the bucket. `upload.py` never removes `realms/made-slugs.txt` or a listed
realm, and `release.sh` never copies staging's made realms over production's. `/realms/built`
(with no slug, a small realm of its own) and `/realms/new` have goldens.

## Files

`apps/openagents-web/builder.coax`, `builder-face.c`, `builder.wgsl`, `realms/new/`;
`myrealm.coax`, `myrealm-face.c`, `myrealm.wgsl`, `realms/built/`; `coax/web/api/realms.c`,
`migrations/0048_realm_builder.sql`, `0049_realm_builder_setup.sql`, `test_realm_builder.py`.
