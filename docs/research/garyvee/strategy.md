# Strategy: many honest short videos, scored by signups and Pro

Written 2026-10-15. Status: **plan**, for the owner to review. Nothing here
is built unless it says so.

Inputs: Gary Vaynerchuk's virtual-influencer clip ([transcript.md](transcript.md))
and the "100 AI Instagram pages" agency post ([100-ai-instagram-pages.md](100-ai-instagram-pages.md)).
This page takes the operating discipline from both and drops the deception.

## The idea in one paragraph

Gary's point is that everyone will be in "the IP business": a character or
a show that keeps producing is an asset. The post's point is that volume
only matters if every piece of content is scored by what it sells. For us
that means: cut many short, real videos a day from things that actually
happened in our product (Coder runs, traces, film shots, Verse/CAD demos),
write their hooks from real users' words, publish each through a link that
says which channel it came from, and every week keep what turns into
signups and Pro and kill what only gets views. The people and agents in the
videos are who they say they are.

## What we sell (the thing each video points at)

| Product | One line | Where it lands |
| --- | --- | --- |
| Coder | One Coder in front of every coding agent; small models for small steps | [landing page](../../../crates/openagents-web/src/pages/landing.rs), `/download` |
| OpenAgents web and mobile | One conversation, everywhere | openagents.com |
| Pro | $20 a month | `/pricing` |
| Enterprise | Sales-led | `/contact?topic=sales` |

## What we take, and what we change

| From the post or clip | What we do instead |
| --- | --- |
| 3,000 videos a month from avatars | 3-6 short videos a day cut from real product footage: Coder runs, `/trace` pages, launch-film shots, Verse/CAD demos |
| Hook vault from Amazon reviews | Hook vault from **our** users' words: GitHub issues, the `/contact` form, feedback, replies on X. Quoted only with consent or paraphrased without a name |
| One tool per job, run by a person on Discord | One agent per job, each a step in a pipeline we own, with a person approving before anything posts |
| Named Amazon Attribution per page | One link per channel, series and creator; signups and Pro sales credited by name (see "Attribution" below, #11698) |
| Kill/clone weekly on sales | Kill/clone weekly on signups and Pro starts per channel code, not views |
| Claims lock (approved supplement wording) | The claims list: every figure in a video must already be in [claims.md](../../launch/1.0/claims.md) with its evidence, e.g. **$1.84 instead of $4.00 per accepted fix** ([evidence](../../inference/briefed-agent-ab.md)) |
| QA gate (lip-sync, labels, off-claim lines) | QA gate: claims check, copy check (no system talk), voice check, AI label check, consent check |
| "Create humans that promote your shit" (Gary) | Characters, yes; fake humans, no. Coder and Draconia appear as what they are: an agent and an original animated character, labeled |

## What we reject, explicitly

- **Fake people.** No AI persona presented as a real person, no "creator"
  who never used Coder, no testimonial that didn't happen. It breaks our
  honesty rules, Meta's inauthentic-behavior rules, and the FTC Endorsement
  Guides and consumer-reviews rule
  ([details](100-ai-instagram-pages.md#critical-read)).
- **Account farms.** One official account per platform (plus real people's
  own accounts, by their choice). No networks of pages, no bought followers,
  no engagement pods (already a rule in the
  [growth roadmap](../../growth/2026-09-28-indie-studio-viral-roadmap.md#rules-we-dont-break)).
- **Undisclosed AI.** Any AI-generated or AI-altered video, voice or
  character carries the platform's AI label and says so in the caption.
- **Undisclosed pay.** If we ever pay a creator, it is disclosed with the
  platform's paid-partnership tool, and it is paid in bitcoin.
- **Non-Bitcoin rails.** Anything we pay or are paid goes over Bitcoin
  (Lightning, Bitcoin-based stablecoins). No card-only affiliate networks,
  no USDC/EVM payouts.
- **Wrong voice.** OpenAgents speaks as "we" (site, captions, replies).
  Coder and other agents speak as "I" in their own clips.
- **Raw Draconia.** The Draconia MetaHuman stays in the private pipeline
  ([private-assets.md](../../verse/private-assets.md#metahuman-characters-draconia)):
  rendered shots only, never raw assets, never as AI training data.

## The pipeline: one agent, one job

| Step | Owner | Input | Output |
| --- | --- | --- | --- |
| 1. Find moments | Clip-finder agent | New `/trace` pages, episode transcripts ([archive](../../transcripts/README.md)), film shots ([film](../../verse/film.md)), Coder run recordings | 10-20 candidate moments a day, each with its source link |
| 2. Hook vault | Hook agent | Issues, `/contact` requests, X replies, feedback | A vault of real phrasings, tagged by pain (cost, speed, safety, "which model") |
| 3. Script | Script agent | A moment plus a hook | 15-45 s script; first 3 s is the problem in the user's words; one CTA with the channel link |
| 4. Claims check | Claims agent | The script | Pass, or the line that has no entry in claims.md |
| 5. Render | Film pipeline | Script plus footage | Vertical 1080p cut with captions and an AI label where needed |
| 6. Approve | Christopher | The queue | Yes or no in one tap |
| 7. Post | Sarah (company comms, tweet queue) | Approved cuts | Posted per channel with its code |
| 8. Score | Scoreboard agent | Analytics rows | Weekly table: per channel and series, signups, Pro starts, cost |

Owners are roles. "Agent" means a Coder-run job in our repo; nothing posts
without step 6.

## Attribution: named, like an affiliate program

Owner decision (2026-10-15): per-channel totals aren't enough; every
signup and Pro sale is credited by name to the link, series or creator
that brought it, as Amazon Attribution does. The design is
[#11698](https://github.com/OpenAgentsInc/openagents/issues/11698):

- `openagents.com/go/{code}` for our own channel and series codes
  (`yt-cost`, `x-cost`, ...) and `openagents.com/r/{handle}` for affiliates.
- A click sets a signed, first-party `oa_ref` cookie (last click wins, 30
  days); signup records the account's attribution in our existing referral
  system; each paid Pro month is a named conversion, reversed on refund.
- Affiliates earn commission only on named, paid, unrefunded sales, under
  published terms, paid over Lightning.
- It's disclosed in the privacy policy, needs consent where the law asks
  for it, and sets nothing for `Sec-GPC`/`DNT`. General site analytics
  stays cookieless.

## Channels and cadence

| Channel | Format | Cadence (days 1-30) | Notes |
| --- | --- | --- | --- |
| X | Clip + one line + link; threads for launches | 2-3 a day | Through Sarah's tweet queue |
| YouTube Shorts | 30-45 s vertical | 1 a day | Same cut as TikTok; full episodes stay on the main channel |
| TikTok | 15-30 s vertical | 1 a day | Official account only |
| LinkedIn | 45-60 s, cost and team angle | 3 a week | Enterprise and Pro buyers |
| Instagram Reels | 15-30 s vertical | 1 a day | AI label on any generated shot |

## Metrics

| Metric | Why |
| --- | --- |
| Signups per channel code per week | The main score |
| Pro starts per channel code per week | The money score |
| `/contact?topic=sales` requests per channel | Enterprise score |
| 3-second hold rate and completion | Hook quality (diagnostic only) |
| Views | Reported, never steered by |
| Cost per video (model + render time) | Keeps the pipeline honest |
| Claims-check failures | Should trend to zero |

## The 30-day plan

| Days | What happens | Owner |
| --- | --- | --- |
| 1-3 | Build the hook vault from the last 90 days of issues and `/contact`; list 30 moments from traces and the launch film | Hook agent, clip-finder agent |
| 1-5 | Ship `/go/{code}` with a closed code list and aggregate counting | A Coder run plus review |
| 4-7 | **First experiment** (below) | Script agent, film pipeline, Christopher, Sarah |
| 8-14 | Scale to the full cadence on all five channels | Same |
| 15 | First kill/clone review | Scoreboard agent, Christopher |
| 15-28 | Clone the two best series; kill the two worst; add one Verse/CAD series | All |
| 29-30 | Write the 30-day result into this folder: what converted, what didn't, cost per signup | Scoreboard agent |

## First experiment: the cost hook

**Question.** Does the cost hook ($1.84 instead of $4.00 per accepted fix)
bring more signups per click than a capability hook ("one Coder in front of
every coding agent")?

**Setup.** 10 clips: 5 cost-hook, 5 capability-hook, cut from the same real
Coder runs. Posted on X, YouTube Shorts and TikTok over 4 days, alternating
order. Each series has its own `/go/` code.

**Acceptance checks.**

- [ ] Every figure in all 10 clips is in [claims.md](../../launch/1.0/claims.md) with evidence.
- [ ] Every clip shows a real run; the trace or recording is linked in the
      caption or a reply.
- [ ] Every AI-generated shot carries the platform's AI label.
- [ ] No person appears or is quoted without consent.
- [ ] Captions use "we" for OpenAgents and "I" for Coder.
- [ ] `/go/` counts are visible in the analytics dashboard within an hour.
- [ ] After 7 days we have signups per click for both series, and we keep
      the winner only if it leads by a margin we name before we start.

## Related docs

- [1.0 launch](../../launch/1.0/README.md), [claims](../../launch/1.0/claims.md), [post drafts](../../launch/1.0/post.md), [operations](../../launch/1.0/operations.md)
- [Growth roadmap: going viral like an indie studio](../../growth/2026-09-28-indie-studio-viral-roadmap.md)
- [Film engine](../../verse/film.md) and [film shots](../../verse/film/)
- [Kitchen Sink: the X archive](../../kitchen-sink/twitter.md) and [promise ledger](../../kitchen-sink/ledger.md)
- [Episode 290 transcript](../../transcripts/290.md), [Sarah, episode 260](../../transcripts/260.md)
- [Sales floor](../../sales/agent-sales-floor.md)
