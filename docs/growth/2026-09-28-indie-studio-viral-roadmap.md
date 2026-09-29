# Going viral like an indie game studio: the OpenAgents growth roadmap

Written 2026-09-28, the night before the open playtest launch. Status:
**plan**. Nothing here is built unless it says so, and every target is a
target we set, not a prediction. The [launch roadmap](../roadmap/2026-09-29-launch-roadmap.md)
owns what ships, the [playtesting program](../game/playtesting.md) owns how
we run season 1, and the [app wireframe](../product/2026-09-28-app-wireframe.md)
owns what the player sees. This page owns how we show all of it to the world,
and how the world turns into playtesters.

We are building the best coding agent in the world by using network effects:
an agent collective. Coder is the first agent. The Verse is where agents
connect, communicate, and transact, so people stay in the loop. The Gym is
where people make agents better through tools (plugins). We market that the
way an indie game studio markets a game in early access: the product is the
marketing, the players are the story, and we ship in public several times a
day.

## Contents

- [The one strategy: the loop](#the-one-strategy-the-loop)
- [What the research says, and what we take from it](#what-the-research-says-and-what-we-take-from-it)
- [Our hook and our anchor](#our-hook-and-our-anchor)
- [What we can show today, honestly](#what-we-can-show-today-honestly)
- [Formats and cadences](#formats-and-cadences)
- [The playtest funnel](#the-playtest-funnel)
- [Creator and streamer seeding](#creator-and-streamer-seeding)
- [The next 7 days: the midweek push](#the-next-7-days-the-midweek-push)
- [30, 60, and 90 days](#30-60-and-90-days)
- [Weekly content calendar](#weekly-content-calendar)
- [Metrics and targets](#metrics-and-targets)
- [Asset checklists](#asset-checklists)
- [Rules we don't break](#rules-we-dont-break)
- [Owner decisions](#owner-decisions)
- [Sources](#sources)

## The one strategy: the loop

Everything on this page is one loop. Each turn of it makes the next turn
bigger, because each player we celebrate is also a reason for their friends
to join.

```
      +-------------------------------------------------------------+
      |                                                             |
      v                                                             |
   1 SHIP ------> 2 SHOW ------> 3 INVITE ------> 4 PLAYTEST        |
   a TestFlight   a build card,  one link: iOS,   players chat,     |
   build and APK  a clip, a      Android, and     walk the Grid,    |
   several times  scoreboard     the community    report problems,  |
   a day                                          and (soon) test   |
                                                  tools in chat     |
                                                        |           |
                                                        v           |
   6 SHIP <------------------ 5 CELEBRATE PLAYERS' RESULTS          |
   the fix or the tool        "Coder passed 7 of 8 tests with       |
   they found, credited       Trainer 7KQ's tool, 5 without", "3FA  |
   by trainer name            found the bug fixed in build 22"      |
      |                                                             |
      +-------------------------------------------------------------+
```

| Step | What we do | The asset | The number it moves |
| --- | --- | --- | --- |
| 1 Ship | Ship a build with a player-visible change and a "What to test" line in the Account **Changelog**. | The build and its Changelog line | Builds per day with a player-visible change |
| 2 Show | Post a build card, a clip, or a scoreboard the same hour. | [Build card](#version-update-graphics-build-cards), [clips](#short-form-clips), [scoreboards](#scoreboards) | Impressions, video views, profile visits |
| 3 Invite | Every post ends with one link and one sentence. | The [join link](#the-playtest-funnel) | Link clicks, installs |
| 4 Playtest | New players get to their first useful thing with zero explanation (IDIOT PROOF). | The app, the day-0 brief | Activation, D1 and D7 return |
| 5 Celebrate | Post the players' results, by trainer name, with their consent. | [Spotlights](#player-spotlights-and-leaderboard-moments) | Share rate, k-factor |
| 6 Ship | Ship the fix or the tool they found, and credit them in the Changelog and the build card. | The next build | Time from report to build (target: 2 days, from the playtesting program) |

The loop is the strategy. The rest of this page is cadence, formats, and
measurement for each step. When a choice comes up that this page doesn't
cover, pick the option that makes a player's result more visible.

## What the research says, and what we take from it

We ground each decision in published indie-game practice. The
[sources](#sources) list links every claim.

| Finding | Evidence | What we do with it |
| --- | --- | --- |
| Virality can't be scheduled; repeatable channels can. | Chris Zukowski's first lesson from *Among Us*: viral phenomena happen "once or twice" a year, and you shouldn't base a business on one. *Among Us* shipped 15+ Steam patches and 35+ devlog entries over two years before its 2020 spike. | "Viral by midweek" is a push with a measurable bar, not a promise. The cadences run every week whether or not a post breaks out. |
| The game has to be good, and people have to stay. | Zukowski's second lesson (players stayed 4+ hours); Andrew Chen: "the highest retention products have empirically shown to be the most viral." | Retention is a growth metric here. D1 and D7 sit next to installs on the dashboard. A spike into a leaky first run is wasted. |
| Creators trade up. | *Among Us* was carried by mid-sized streamers for years (a Korean streamer drove about half its Steam sales) before Sodapoppin; a platform feature (itch.io front page) started it. | Seed many small, relevant creators first. Ask platforms for features (TestFlight has none, so: newsletters, Hacker News, dev communities). |
| Creators work, and YouTube outlasts Twitch. | Zukowski's developer survey ranked streamers and YouTubers third for wishlists, with YouTube ahead of Twitch because videos have a longer tail. | YouTube is our long-form home. Live streams are for launch days and raids. |
| You're not contacting enough creators. | Game Marketing Intel, citing former Devolver strategist Clara Sia: first-time developers see 5 to 10 percent coverage from creator outreach, experienced marketers 25 to 40 percent; she targets thousands of contacts. | Contact 200 creators in 30 days to expect 10 to 20 videos, and grow the list every week. |
| The hook must land in seconds. | Derek Lieu: the first 6 seconds decide whether people keep watching, and on TikTok the first 1 to 3 seconds. His structure is genre, hook, content. Zukowski's TikTok advice: explain the hook within 3 to 10 seconds. | Every clip opens on the hook (a score going up) and not on a logo. |
| The product is the clip. | *Lethal Company*'s proximity chat made moments people clipped; one clip had 1.6 million views on X within a day. Zukowski: make "a game with moments people want to TikTok about." | We design shareable moments into the app: the **Result** screen's before-and-after, the level-up, the title appearing on a name tag. |
| Short-form works in waves. | Zukowski's TikTok case studies: *The Matriarch* 4.5M views and 10,000 wishlists; *FREERIDE* 715k views and 13,000 wishlists; "TikTok works in waves... each wave grows from the last." | Post short clips daily across TikTok, YouTube Shorts, Instagram Reels, and X, and reply to comments with video. |
| A weekly devlog builds trust for years. | Factorio Friday Facts ran weekly from 2013, started because players asked whether the game was still in development, and became a channel between developers and players. | A weekly devlog on Fridays, **Gym Notes**, starting 2026-10-02. |
| Screenshot Saturday is a standing weekly stage. | `#screenshotsaturday` is a long-running weekly hashtag where developers post progress, and publishers and players browse it. | Post a Grid clip every Saturday with the hashtag. |
| Playtest in waves, from warm leads, with a community channel. | Steam Playtest admits players in batches; guides recommend a first wave of 25 to 50 players, a Discord channel for signups and reports, and role automation for players who actually played. | Waves of invites to creators and communities, and one community space with a playtester role that comes from an accepted contribution. |
| Awareness compounds before a big moment. | Zukowski's Next Fest benchmarks: the wishlists a game already holds going in are the strongest predictor of what it gains there (Spearman r of 0.825), and games with more than 2,000 have more room to break out. | Build the following continuously so every later beat (Gym trials, season 2) lands on an audience that is already there. |
| k-factor is invites times conversion. | Andrew Chen: k = i × c. Growth with k above 1 is exponential but rarely lasts; retention sustains it. | Measure k per player per week. Target k well below 1 at first and grow it through retention and share moments, not tricks. |

## Our hook and our anchor

Zukowski splits a pitch into the **hook** (what only your game does) and the
**anchor** (what makes it familiar and safe). Both appear in every asset.

- **Hook:** *You make an AI agent measurably better, and everyone's agent
  gets the upgrade, with your name on it.* No other coding agent lets a
  player test a tool on it, see it pass 7 of 8 tests with the tool
  instead of 5 without, and ship that tool to everyone, all by chatting.
  When other trainers check your result, you earn XP.
- **Anchors:** a game (levels, XP, titles, a season, raids, a main menu that
  says **ENTER THE GYM**), a coding agent (Coder, like the ones people
  already use), and a chat you can try with no setup (**CHAT WITH
  OPENAGENTS**).
- **One-line pitch:** "Train the AI that writes code. Your wins upgrade
  everyone's agent."
- **The look:** black and white, the power emblem, the white wireframe Grid.
  It reads as a game in one frame and doesn't look like any other AI product.

## What we can show today, honestly

The hook's full form, a player testing a tool from chat and seeing Coder
pass more tests with it, isn't built yet: the hosted runner, the result
card, and player publication are marked **NEW** in the
[wireframe](../product/2026-09-28-app-wireframe.md#card-04-result-card)
and planned for build 21 (milestone M10).
We don't show a player result that didn't happen. What we can show this week:

| Asset | Status | Where it comes from |
| --- | --- | --- |
| **Chat with OpenAgents**, no setup, instant prepared answers | Ships (builds 17 to 20) | [Launch roadmap](../roadmap/2026-09-29-launch-roadmap.md#chat-with-openagents-builds-17-to-20) |
| Run Coder on your own computer from your phone | Ships | Same |
| The Grid: other players, name tags with trainer levels, the ball, the stack, the dominoes, the reset pillar | Ships | [Launch roadmap](../roadmap/2026-09-29-launch-roadmap.md#verse-the-grid) |
| The Gym's **RESULTS** board and trace replay on the phone | Ships | [Gym leaderboard](../verse/gym-leaderboard.md) |
| Real scoreboards: one shared fact took Coder from 0 of 10 to 4 of 4 on `gsea-proteomics`; 30 confirmed wins at a 2.9 percent median pass cost | Published, with labels (in-sample, knowledge-assisted) | [Beating Fable together](../coder/beat-fable-together.md), [Cheapest verified passes](../coder/cheapest-verified-passes.md) |
| Trainer levels, six tutorial quests, playtest titles (**PLAYTESTER**, the season-1 founding ring) | Ships; titles need the playtest referee key | [Playtesting rewards](../game/playtesting.md#rewards) |
| Builds shipping several times a day | True: builds 1 to 15 in two days, then 16 to 19 | [Launch roadmap](../roadmap/2026-09-29-launch-roadmap.md) |
| The intro cinematic `CIN-01` | Specified, not built (no scripted camera path yet) | [Wireframe](../product/2026-09-28-app-wireframe.md#cin-01-intro-cinematic) |

The scoreboard line, "Coder passed 7 of 8 tests with Trainer 7KQ's tool,
5 without, checked by 3 trainers", is the format we post as soon as the
first real eval result is checked (the Gym moved from benchmark scores to
evals on 2026-09-28; see [extension evaluation](../extensions/evaluation.md)
and milestone M10 in the launch roadmap). Until then, scoreboards use our published boards, and spotlights
use accepted playtest contributions.

## Formats and cadences

### Version-update graphics (build cards)

A build card is the patch-notes graphic for one TestFlight build or APK. It
is the most frequent post we make and the cheapest proof that we ship.

**When to post.** Builds ship several times a day, and a feed of eight cards
a day reads as noise. So:

- Post a card for a build with a **player-visible change**, within an hour of
  the build reaching the public TestFlight link (not when it's uploaded:
  external builds can wait on beta review).
- Fold other builds into one **Today's builds** card at 18:00 Central.
- Cap: three cards a day, including the daily roll-up.
- Always post a card for a build that ships a fix a player reported, and name
  the player (with consent).

**Template spec.**

| Field | Spec |
| --- | --- |
| Sizes | 1080 × 1350 (4:5) for X, Nostr, and Instagram; 1080 × 1920 (9:16) for Stories and Shorts; 1600 × 900 for the devlog |
| Background | Solid black. No gradients, no color. White and grays only, like the app |
| Top left | The white power emblem and **OPENAGENTS** wordmark |
| Top right | `BUILD 21` in bold condensed uppercase, with `iOS · ANDROID` under it for the platforms it's on |
| Headline | One line, bold condensed uppercase, what changed for the player, in screen words: **PREPARED ANSWERS IN CHAT**, not "chat router v1" |
| Hero | One phone screenshot or a 2 to 4 second screen recording (for the video version) of the change, in a plain black phone frame |
| Bullets | Up to three, in the app's words (the [Words on screen](../product/2026-09-28-app-wireframe.md#words-on-screen) table): "Ask what Coder can do and get an answer at once." |
| What to test | One line from the Changelog's "What to test": "Ask us three questions. Tap **Wrong answer** if one's wrong." |
| Credit | "Found by Trainer 3FA" when a player's report drove the change (with consent) |
| Footer | `PLAYTEST SEASON 1 · ENDS OCT 26` and the join link |
| Banned | The words the wireframe bans on primary surfaces (npub, relay, Nostr, benchmark, sats, and the rest), in the graphic. Captions may add the technical term for developers. |

**Where.** X (@OpenAgentsInc, where the episode archive lives), Nostr (the
OpenAgents profile), the community space's `#builds` channel, and, for the
daily roll-up, Instagram Stories. The caption repeats the headline, adds one
technical sentence for developers, and ends with the join link.

**Source of truth.** The Account **Changelog** line for the build. The card
never claims more than the Changelog says. A later task renders the card
from the Changelog entry with a script, so a build card costs zero design
time.

### Weekly devlog: Gym Notes

Factorio's Friday Facts is our model: weekly, on the same day, written by the
people who build it.

- **When:** every Friday, 10:00 Central, starting 2026-10-02.
- **Where:** a post on X and Nostr with the full text, the community space,
  and a copy in the repository's history later if the owner wants one.
- **Length:** 600 to 1,200 words, three to six images or clips.
- **Structure:**
  1. The week's number: one scoreboard or one player result, with labels.
  2. What shipped: the week's build cards in a grid.
  3. What players found: accepted playtest issues, credited by trainer
     name, and what we changed.
  4. One deep dive: how one thing works (the chat router, the Grid's
     physics, how a result gets checked by other trainers).
  5. Next week: what to test, and the next beat (a raid, a new tool).
- **Voice:** "we", plain words, every number linked to its record.

### Founder interviews and YouTube

The owner already has 288 recorded episodes, most posted to X, many over two
hours long ([transcript archive](../transcripts/README.md)). That's a large
catalog nobody new will watch whole. YouTube's long tail favors searchable,
titled, 10 to 20 minute videos, so we cut and add to it.

| Format | Length | Cadence | Who | Topics |
| --- | --- | --- | --- | --- |
| **The interview** | 12 to 20 min | Weekly, published Tuesday | Christopher (CEO), with a host asking questions: a creator, a playtester, or a team member. Two cameras or a phone and a screen capture. | Why an agent collective beats one lab's agent; how a tool makes Coder better and how we measure it; why the app is a game; what playtesters found this week; the night we shipped builds 1 to 19 |
| **Build live** | 60 to 120 min | Launch days and one other day a week | Christopher, streaming on YouTube Live and X | Shipping a build players asked for, start to finish, with chat choosing what to fix |
| **Episode cut-downs** | 8 to 15 min | Two a week from the archive and new episodes | Edited from the long episodes | One idea per video, titled as a question ("Can one shared fact beat a frontier agent?") |
| **Shorts** | 20 to 45 s | Daily | Clipped from all of the above | One claim and its proof on screen |

Rules for every video:

- The first 6 seconds show the hook, not an intro: a score going up, the Grid
  full of players, or Christopher saying the one-line pitch.
- The title and thumbnail name one outcome. Thumbnails are black and white
  with the emblem and one number in white ("0 → 4 OF 4").
- The description's first line is the join link.
- Every number on screen carries its label ("in-sample", "prepared answer").

### Short-form clips

Daily, cross-posted to TikTok, YouTube Shorts, Instagram Reels, and X, with
no platform watermark on the copies. Each clip opens on its hook in the first
3 seconds. Reply to good comments with video replies, which the platforms
pin and show.

| Clip | Hook in the first 3 seconds | Status |
| --- | --- | --- |
| **Score goes up** | A scoreboard animating 0 → 4 of 4, then who taught it | Now, from published boards |
| **The Grid** | A wide shot of other players' name tags, then the ball knocking the dominoes | Now |
| **Chat, no setup** | Open the app, type "What can you do?", answer in under a second | Now (build 20) |
| **Coder from your phone** | A phone starts a task, the computer on the desk runs it | Now |
| **Title unlocked** | **PLAYTESTER** appearing under a real player's name tag | When the playtest referee key exists and the first award is signed |
| **The cinematic** | `CIN-01.S05`: many lights stream into one emblem | Animatic now (Grid footage, subtitles, the narration script); the real cinematic when built |
| **Player's result** | "Coder: 5 of 8 → 7 of 8 tests with Project map. Test set by Trainer 7KQ." | When the hosted runner ships (build 21) |
| **Made in chat** | A screen recording: "Help me make a tool that…", the draft card, **Try it once**, the result card | When the chat's eval routes ship (build 21) |
| **Shipped in an hour** | A player's report on the left, the build card with their name on the right | Every time it happens |

### Scoreboards

A scoreboard post shows one agent, one tool, the tests passed without and
with it, and who made the difference. It follows the
[Gym leaderboard](../verse/gym-leaderboard.md)'s rule that no screenshot
separates a number from its caveats, and the eval rule that results from
different test sets are never compared.

- **Format until evals ship:** `CODER · 0 OF 10 → 4 OF 4` in white, the
  task in gray, the label (`IN-SAMPLE · ONE SHARED FACT`) under it, and the
  credit line.
- **Eval version (build 21 on):** `CODER · 5 OF 8 → 7 OF 8 TESTS · PROJECT
  MAP · TEST SET BY TRAINER 7KQ · CHECKED BY 3 TRAINERS`. Post only after
  other trainers' checks confirm it; an unchecked result is labeled
  **PENDING**, and a disputed one is not posted.
- **Cadence:** Wednesdays, plus any day a confirmed result lands.
- **Honesty line:** negative results get posted too ("Code finder: no clear
  change. Now everyone knows."), because the wireframe's **Result** screen
  treats them as useful, and posting them makes the positive ones credible.

### Player spotlights and leaderboard moments

The celebrate step. Every spotlight is opt-in: we ask the player first, as
the [leveling spec's privacy rule](../verse/agent-trainer-leveling.md#privacy)
makes boards and tags opt-in.

- **Spotlight (Thursday):** one player, their trainer name, what they did
  (an accepted bug, a verified fix, later a confirmed tool), the build that
  shipped it, and a 15-second clip of their name tag in the Grid.
- **Firsts (whenever they happen):** the first outside **PLAYTESTER** title,
  the first founding-playtester ring, the first player-confirmed tool, the
  first raid. Each first gets its own post the same day.
- **Leaderboard recap (Sunday):** the week's top trainers by counted XP, the
  bug hunters, and the fix verifiers, from signed awards only.
- **Scarcity that's real:** the founding-playtester ring exists only for
  counted contributions in season 1, which ends 2026-10-26. We say so, with
  the date, in every Sunday recap.

### Playtest waves

Waves turn the open link into moments. The link stays open all season; a
wave is a dated call to a named group, with a task list and a raid.

| Wave | Dates | Who we invite | The ask |
| --- | --- | --- | --- |
| 0 | 2026-09-29 | Everyone following OpenAgents on X and Nostr, confidants | Install, chat, walk the Grid, report one problem |
| 1 | 2026-09-30 to 10-02 | Creators (first 50), coding-agent communities, Lightning wallet users | The day-0 task list; creators get a 3-minute brief and assets |
| 2 | 2026-10-06 to 10-12 | Strangers from week 1's posts; gamers who use coding agents | Unaided first run; wallet sessions (moderated) |
| 3 | 2026-10-13 to 10-19 | Guilds and groups of 5 to 8 friends | Raids in the Grid, the paper trainer loop |
| 4 | 2026-10-20 to 10-26 | Everyone who played | Verify fixes, questionnaire, earn the founding ring before it closes |

These line up with the [season 1 weeks](../game/playtesting.md#season-1-week-by-week).

## The playtest funnel

```
  post / clip / creator video
             |
             v
  JOIN LINK  (one short link per channel, so we can count sources)
             |
             v
  join page: the one-line pitch, a 20 s clip, three buttons
    [ iPhone: TestFlight ]  [ Android: APK ]  [ Join the community ]
             |                      |
             v                      v
  first open: CHAT WITH OPENAGENTS answers in under a second
             |
             v
  the Grid, the Gym's RESULTS board, a tutorial quest
             |
             v
  Report a problem  -->  accepted  -->  PLAYTESTER title, playtest XP
             |
             v
  spotlight post  -->  their friends click the join link  (k-factor)
```

- **The join page.** Web work is on hold (M5), so this is an
  [owner decision](#owner-decisions): the smallest possible static page on
  openagents.com, or a link-in-bio page, or linking straight to TestFlight
  and the GitHub release. Without a page, source tracking falls back to
  per-channel short links.
- **iOS.** The public TestFlight link. Its tester cap is set by the owner, up
  to Apple's 10,000; raise it before the first push so a spike doesn't hit a
  full group.
- **Android.** The signed APK from the GitHub release, with its SHA-256 and a
  two-line sideload guide on the join page.
- **Community.** One space where playtesters talk, get build cards, and file
  reports. [Owner decision](#owner-decisions): Discord (where game
  communities already are, with role automation) or a Nostr NIP-29 room
  (on-brand, and the Verse already speaks it, but no chat in the Grid on the
  phone yet). Either way, the **playtester** role comes from an accepted
  contribution, never from joining, matching the program's rewards.
- **Email.** The playtest email address on the join page collects addresses
  for wave invites and season-2 news. Zukowski's reading of indie post-mortems found
  festivals, creators, and mailing lists worked well for the successful
  games, and email is the only list we own outright.

## Creator and streamer seeding

- **Who, in order:** (1) developers who post about coding agents on YouTube
  and X (they already make agent comparison videos); (2) small and mid-sized
  game creators who cover early-access and weird indie games; (3) Bitcoin
  and Nostr creators (the Wallet and the Verse speak their language); (4)
  AI newsletter writers.
- **Tiers:** Tier 1 (10,000+ views a video), tier 2 (1,000 to 10,000), tier 3
  (under 1,000 but a close match). Start with tiers 2 and 3: *Among Us* was
  carried by mid-sized creators for years before the big ones found it.
- **How many:** 50 in wave 1 (by 2026-09-30), 200 by day 30, 500 by day 90.
  At the 5 to 10 percent first-timer coverage rate, 200 contacts is 10 to 20
  videos.
- **The kit** (see [asset checklists](#asset-checklists)): a 3-minute brief,
  the join link, the known issues, raw Grid and chat footage, logos, and a
  "try this" list of the five most clippable things.
- **The ask:** "Play for 20 minutes, report one thing, make whatever you
  want." No scripts, no required claims, and no payment in season 1. If a
  creator's report is accepted, they get the same title and XP as anyone.
- **Follow-up:** reply to every video, spotlight creators' players, and
  invite the creators who made something to the next raid.

## The next 7 days: the midweek push

"Viral by midweek" means a measurable bar by the end of **Thursday
2026-10-01**, not a hope. We set three levels:

| By end of 2026-10-01 | Floor | Target | Stretch |
| --- | --- | --- | --- |
| Installs (TestFlight installs plus APK downloads) | 100 | 500 | 2,000 |
| Best single post (views or impressions) | 50,000 | 250,000 | 1,000,000 |
| Accepted outside playtest reports | 5 | 15 | 40 |
| Creator videos or posts about us | 2 | 5 | 15 |

| Day | Date | Ship | Show | Invite and celebrate | Owner |
| --- | --- | --- | --- | --- | --- |
| D-1 | Mon 09-28 (tonight) | Build 20 (chat router) to the public link | Record 3 clips: Grid wide shot, chat answering in under a second, Coder from the phone. Make the build-card template. Draft the launch thread. | Create the join links. Pick the community space. List the first 50 creators. Raise the TestFlight tester cap. | Owner: public link, APK release, triage and referee keys |
| D0 | Tue 09-29 (launch) | Morning build with overnight fixes | 09:00 Central: launch thread on X and Nostr (20 s hook video, one-line pitch, links, known issues). Build cards through the day. 12:00: **Build live** stream, 90 minutes. | Reply to every reply for the first 3 hours. Wave 0. Triage all channels. | Christopher on stream |
| D1 | Wed 09-30 (**midweek push**) | Fix build for day-0 reports; credit reporters on the card | 09:00: scoreboard post, "One shared fact: Coder 0 of 10 → 4 of 4", with its labels and the record link. 14:00: short clip of the Grid full of players. | Wave 1: creator kit to 50 creators by noon. Post a Show HN (owner decision) and share in the developer communities that allow it. First "shipped in an hour" post if a report lands. | Christopher replies in threads all day |
| D2 | Thu 10-01 | Build with the week's P1 fixes | Spotlight #1 (first accepted outside report). Clip: "Coder from your phone". Record interview #1. | Invite creators who replied to a Saturday Grid meetup. Read D1 return. Score the push against the table above. | Christopher: interview |
| D3 | Fri 10-02 | Build | **Gym Notes #1** devlog. Interview #1 on YouTube, plus 3 Shorts cut from it. | Email everyone on the list: devlog link and the Saturday meetup. | |
| D4 | Sat 10-03 | Quiet build day unless a P0 | `#screenshotsaturday` Grid clip. Grid meetup at 13:00 Central: stack the blocks, push the ball, meet at the RESULTS board, streamed. | Clip the meetup. Thank every player by trainer name (with consent). | Christopher in the Grid |
| D5 | Sun 10-04 | None | Week-1 recap: installs, reports accepted, builds shipped, top bug hunters, the founding-ring deadline. | Ask the week's best players for spotlights. | |
| D6 | Mon 10-05 | Week-1 fix build | Episode cut-down from launch week. | Retro: which post, clip, and creator drove installs (by join link); double what worked, cut what didn't. Check the playtesting program's week-1 exit. | Owner: review this page's numbers |

If a post breaks out, drop the plan for that day: reply to everyone, pin the
post, add the join link as the first reply, ship a build that fixes what the
new players hit, and post its card with "for everyone who joined today".

## 30, 60, and 90 days

### Day 30 (to 2026-10-28): season 1 and the habit

- Every cadence running: build cards, daily clips, Wednesday scoreboards,
  Thursday spotlights, Friday **Gym Notes** (#1 to #4), Saturday
  `#screenshotsaturday`, Sunday recaps, and weekly interviews (#1 to #4).
- Waves 0 to 4 done, matching the season-1 weeks; two public raids.
- 200 creators contacted; the creator kit updated each week.
- The first signed **PLAYTESTER** and founding-ring awards posted as firsts.
- Season 1 closes 2026-10-26 with a recap video and the questionnaire.
- Build-card rendering from the Changelog automated (an issue to file).
- Targets: 2,500 installs, D7 return 20 percent, 30 accepted playtest
  issues (the playtesting program's number), k-factor 0.2.

### Day 60 (to 2026-11-27): the Gym opens, the real scoreboard

- The hook becomes real when evals in chat ship (build 21): the first
  checked player result gets a launch-sized beat (thread, video, stream,
  creator wave).
- Scoreboards switch to eval results: "Coder passed 7 of 8 tests with
  Project map, 5 without. Test set by Trainer 7KQ, checked by 3 trainers."
- **Share outside the app** on the Result screen and **Share what you
  made** on the credit card send an image card and the
  join link, with the trainer's name on it (the k-factor engine).
- The intro cinematic `CIN-01` ships; its 37-second cut becomes the trailer.
- Season 2 announced with its own title and a new cosmetic.
- Targets: 6,000 installs, D7 25 percent, share rate 15 percent of results,
  k-factor 0.35.

### Day 90 (to 2026-12-27): the collective as the story

- A monthly "state of the collective" video: how many tools players
  confirmed, how much better Coder got because of them, and who did it.
- Creator program: repeat creators get early builds and a raid of their own
  in the Grid.
- Guild raids: groups compete to confirm results.
- Targets: 10,000 installs (the TestFlight external cap, so the owner
  decides by day 60 whether to move to an App Store release), D7 30 percent,
  k-factor 0.5.

## Weekly content calendar

Times are Central. "Build cards" run every day as builds ship (at most
three).

| Day | 09:00 | 12:00 to 14:00 | 18:00 | Long form |
| --- | --- | --- | --- | --- |
| Monday | Episode cut-down | Short clip | Today's builds card | Retro and plan the week |
| Tuesday | Interview on YouTube | 3 Shorts from the interview | Today's builds card | **Build live** stream |
| Wednesday | Scoreboard | Short clip | Today's builds card | |
| Thursday | Player spotlight | Short clip | Today's builds card | Record next week's interview |
| Friday | **Gym Notes** devlog | Devlog clip | Today's builds card | Email to the list |
| Saturday | `#screenshotsaturday` Grid clip | Raid or Grid meetup | Meetup clips | |
| Sunday | Leaderboard recap | | | |

## Metrics and targets

Every target below is ours, set here, and reviewed on Mondays. Benchmarks
are cited for context only: playtesters self-select, so we aim above them.

| Metric | How we measure it | Week 1 | Day 30 | Day 90 | Context |
| --- | --- | --- | --- | --- | --- |
| Installs | App Store Connect TestFlight tester installs plus GitHub release APK downloads | 500 | 2,500 | 10,000 | TestFlight's external cap is 10,000 |
| Join-link clicks by source | Per-channel short links | Tracked | Tracked | Tracked | Tells us which posts and creators work |
| Activation | Distinct keys that get a reply from the OpenAgents chat worker (a count, no content) | 60 percent of installs | 70 percent | 80 percent | IDIOT PROOF: no setup before the first win |
| D1 return | TestFlight sessions per tester, and distinct keys chatting again the next day | 35 percent | 40 percent | 45 percent | GameAnalytics: top-quartile mobile games reach about 30 to 33 percent D1 |
| D7 return | As above, on day 7 | n/a | 20 percent | 30 percent | Top-quartile mobile games sit around 6 to 7 percent D7 |
| Runs per player | Chats per active player per day now; test runs and checks per player per week once evals ship | 3 chats | 5 chats | 3 test runs or checks a week | |
| Accepted playtest issues | The triage log | 15 | 30 | 80 | Season-1 target from the playtesting program |
| Report to build | Median time from an accepted P0 or P1 to a public build with the fix | 2 days | 2 days | 1 day | Playtesting program target |
| Share rate | Shares from **Share outside the app** and trainer card exports, per result or card shown | n/a | 10 percent | 20 percent | Needs `SCR-05.E09` |
| k-factor | New installs from players' shared links, per active player, per week (k = invites × conversion) | Estimate | 0.2 | 0.5 | Andrew Chen; k above 1 isn't the goal |
| Creator coverage | Videos or posts per creator contacted | 5 percent | 7 percent | 10 percent | 5 to 10 percent for first-timers (Clara Sia) |
| Followers | X, Nostr, YouTube, community members | Tracked | +5,000 total | +20,000 total | |

Measurement that needs building: per-channel join links, a count-only
activation and return counter on the chat worker, and a referral code on
shared links. None of them may record message content, keys beyond a
count, or anything the playtest log's [invariants](../game/playtesting.md#playtest-logging-in-a-release)
forbid.

## Asset checklists

**Launch kit (by D0 09:00):**

- [ ] 20-second hook video: score going up, the Grid, chat answering, the
      join link (9:16 and 16:9).
- [ ] Launch thread text: pitch, what works, honest limits, links.
- [ ] Build-card template in all three sizes.
- [ ] Join links per channel (X, Nostr, YouTube, community, email, each
      creator tier).
- [ ] Known-issues list (from the [day-0 checklist](../game/playtesting.md#known-issues-on-day-0)).
- [ ] Pinned post on X and Nostr.
- [ ] TestFlight tester cap raised; APK release public with its SHA-256.

**Creator kit (by D1 noon):**

- [ ] 3-minute brief: what OpenAgents is, what to try, what's broken.
- [ ] "Five clippable things": chat in under a second, the dominoes, the
      reset pillar, the RESULTS board replay, Coder running on your computer
      from the phone.
- [ ] Raw footage: 10 minutes of the Grid with players, 5 minutes of chat,
      a Coder task end to end.
- [ ] Logos: the power emblem and wordmark, white on black and black on
      white, PNG and SVG.
- [ ] Contact: the playtest email and the community link.

**Every build card:** the build number, one headline in screen words, one
screenshot, up to three bullets, the "What to test" line, credit with
consent, the footer, and the join link in the caption.

**Every scoreboard:** the number, its denominator, its label, the record
link, and the credit.

**Every spotlight:** written consent, the trainer name as they want it shown,
the accepted issue or confirmed result link, and a Grid clip.

**Every video:** the hook in the first 6 seconds, the join link first in the
description, a black and white thumbnail with one number, and captions on.

## Rules we don't break

- **No number without its label.** In-sample stays in-sample. A pending
  result says **PENDING**.
- **No player result we didn't measure.** The "7 of 8 with, 5 without"
  format waits for real, checked eval results.
- **No promised money.** Nothing pays testers or creators in season 1, and
  we don't narrate a bitcoin reward the app can't give (the same rule as
  `CIN-01.S06`).
- **No player's name or clip without consent.**
- **No XP for joining,** and no fake scarcity. The founding ring's deadline
  is the season's real end date.
- **No banned jargon on graphics.** Captions can carry the technical term.
- **No bought installs, bought followers, or engagement pods.** They break
  the metrics we steer by.

## Owner decisions

| # | Decision | Options | Our recommendation | Needed by |
| --- | --- | --- | --- | --- |
| 1 | The join page | A minimal static page on openagents.com (despite the web hold); a link-in-bio page; direct links only | A one-screen static page: it's the only way to count sources and show both platforms | D-1 |
| 2 | The community space | Discord; a Nostr NIP-29 room; both | Discord now for reach and roles, with a Nostr room mirrored for the Verse-native crowd | D-1 |
| 3 | TestFlight tester cap | Current cap; 10,000 | 10,000 before the D0 thread | D-1 |
| 4 | Who is on camera | Christopher only; Christopher plus a host | Christopher plus a rotating host for interviews | D2 |
| 5 | YouTube channel | Use the existing OpenAgents channel; create one | Use or create one channel named OpenAgents, with the episode archive as playlists | D3 |
| 6 | Show HN and developer communities | Post D1; wait for the Gym | Post D1 with the chat and the Grid, and again when the Gym's player runs ship | D1 |
| 7 | Spotlight consent flow | Ask by DM; a toggle in the app later | DM now; an in-app **Feature me** toggle later | D2 |
| 8 | Who makes build cards before automation | Christopher; an agent from the Changelog; a designer | An agent drafts from the Changelog, Christopher approves in one tap | D0 |
| 9 | App Store release | Stay on TestFlight; release to the App Store | Decide by day 60, before the 10,000 cap binds | Day 60 |

## Sources

- Chris Zukowski, [Among Us: the lessons of their viral success](https://howtomarketagame.com/2020/09/14/among-us-the-4-lessons-of-their-viral-success/), How To Market A Game, 2020.
- Chris Zukowski, [How to go viral on TikTok](https://howtomarketagame.com/2022/02/14/how-to-go-viral-on-tiktok/), How To Market A Game, 2022.
- Chris Zukowski, [Know your game's anchor](https://howtomarketagame.com/2019/12/23/know-your-games-anchor/), How To Market A Game, 2019.
- Chris Zukowski, [Benchmarks: how many wishlists can I get from Steam Next Fest](https://howtomarketagame.com/2025/03/26/benchmarks-how-many-wishlists-can-i-get-from-steam-next-fest/), How To Market A Game, 2025.
- Game Marketing Intel, [How many streamers do you actually need to contact](https://gamemarketing.substack.com/p/how-many-streamers-do-you-actually), 2026, citing Clara Sia.
- Game Developer, [Practical indie marketing advice with Chris Zukowski](https://www.gamedeveloper.com/marketing/game-developer-podcast-36-indie-marketing-advice-from-chris-zukowski) (the survey ranking streamers and YouTubers).
- Derek Lieu, [How to hook the audience and how quickly to do it](https://www.derek-lieu.com/blog/2022/10/24/how-to-hook-the-audience-and-how-quickly-to-do-it) and [Game trailer structure: genre, hook, content](https://www.derek-lieu.com/blog/2021/4/12/game-trailer-structure-genre-hook-content).
- Simon Carless, [How to get to your game's hook, quick](https://newsletter.gamediscover.co/p/how-to-get-to-your-games-hook-quick), GameDiscoverCo.
- Wube Software, [Friday Facts #1](https://factorio.com/blog/post/fff-1), 2013, and William Spies, [Seven years of Factorio Friday Facts](https://spieswl.github.io/blog/2020/seven-years-of-factorio-friday-facts).
- Tubefilter, [YouTubers, Twitch streamers, and Among Us are driving record downloads](https://www.tubefilter.com/2020/09/28/youtubers-twitch-streamers-among-us-record-downloads-players/), 2020.
- Wikipedia, [Lethal Company](https://en.wikipedia.org/wiki/Lethal_Company), and IGN on X, [the proximity chat clip](https://x.com/IGN/status/1727344157994435067), 2023.
- Uberstrategist, [How to win #screenshotsaturday](https://uberstrategist.com/how-to-win-screenshotsaturday/).
- Valve, [Steam Playtest](https://partner.steamgames.com/doc/features/playtest), Steamworks documentation, and LaunchLens, [How to run a Steam playtest that actually improves your game](https://www.launchlens.io/blog/steam-playtest-guide) (waves of 25 to 50, Discord roles).
- Andrew Chen, [Why the best way to drive viral growth is to increase retention](https://andrewchen.com/more-retention-more-viral-growth/), and the k = i × c definition summarized in [Understanding viral growth in SaaS](https://medium.com/point-nine-news/understanding-viral-growth-in-saas-45eea50d8900).
- GameAnalytics, [2026 mobile and PC gaming benchmarks](https://www.gameanalytics.com/reports/2026-mobile-pc-gaming-benchmarks) (D1 and D7 retention quartiles).
- Levels.io, [Building a startup in public: Hoodmaps](https://levels.io/hoodmaps) (founder-led, built live on stream).

## Maintaining this page

Update the targets table every Monday with the week's actuals, and move a
format from "later" to "now" in [What we can show today](#what-we-can-show-today-honestly)
when its feature ships. When an owner decision is made, record it in the
table with the date.
