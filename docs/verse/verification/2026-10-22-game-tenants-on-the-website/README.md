# Game tenants on the website, the GPUI way back, and the mic (#12485, #12486, #12487)

Checked on 2026-10-22 against this checkout's website binary
(`openagents-web`, development backend) on `127.0.0.1`, in headless Chrome
under `openagents browser run`.

## The arches open game tenants served by the website (#12486)

Setup: `scripts/deploy/games.sh deploy haunt` and `deploy icecream`, with
scratch tenant keys, a scratch store, and a local relay, then the website
with `OPENAGENTS_LIVE_GAMES=haunt=…,icecream=…` and
`OPENAGENTS_LIVE_GAMES_CHANNEL=staging` pointing at them.

- The server followed both pointers within 30 ms of starting
  (`livehost: haunt/staging now serves d09710434868`).
- `/live/games` answered `{"tenants":["haunt","icecream"]}`; `/t/haunt/` and
  `/t/icecream/` answered 200 with the game policy; `/t/openagents/`
  answered 404.
- The homepage Grid stood both arches from `/live/games` alone, without a
  `?hosted=` address, and prefetched each from in front of it.

## Back to the Grid is the chat's, not the page's (#12487)

Walking through each arch on the homepage (`?at=` in front of it, then W):

- The GPUI chat stood the tenant's page full screen with its own **Back to
  the Grid** button (`haunt-open.jpg`, `icecream-open.jpg`). The page drew
  no button or frame chrome of its own; the pylon panel and header are
  hidden while a zone is open.
- The zone took the keyboard (the active element was its frame).
- Clicking the chat's button closed the zone and stood the player 3.5 m in
  front of the arch they used (`haunt-back.jpg`).

The ICE CREAM MAN guide card in `icecream-open.jpg` is that game's own page.

## The mic in Game Lab's house (#12485)

The house shows a mic button (bottom right; also V) only when Game Lab
frames it on the Grid homepage and the browser has the Web Speech API.
What it hears goes to the homepage's `homeChatSay`, and the chat sends it as
the visitor's message in Game Lab's chat, where the model chooses a tool.

- **Button to page:** with a stand-in for the speech API (headless Chrome
  has no recognition service), clicking the button, and pressing V, sent
  "turn the music down" to the framed page, and the house said so
  (`mic-sent.jpg`).
- **Words to tools, live:** `spoken_requests_reach_the_house_tools` in
  `crates/openagents-web/src/labs.rs` (ignored; it calls Gemini 4 Argon on
  Vertex) sends four requests, worded as dictation types them, through
  Game Lab's chat with the house's tools. Each called the right tool and
  changed the house (`dictation.json`):

  | Said | Tool | Seconds |
  | --- | --- | --- |
  | turn the music down | `haunt_audio` | 9.3 |
  | make it louder | `haunt_audio` | 11.3 |
  | make the parlor glow orange like jack o lanterns | `haunt_light` | 12.6 |
  | add a werewolf in the long hall that howls when you get near | `haunt_place` | 12.5 |

## Limits

- Real speech recognition wasn't exercised: headless Chrome starts a
  recognizer but never returns words. Chrome on a desktop, with a
  microphone, is needed to hear speech; Safari and Firefox without the
  Web Speech API show no button.
- The step from `homeChatSay` to a sent Game Lab message runs in the GPUI
  chat's tick and needs a signed-in Game Lab; it wasn't driven in a browser
  here.
- Nothing is deployed: production and staging serve game tenants once
  their environment names them (`docs/deployment/openagents-web.md`,
  "Game tenants").
