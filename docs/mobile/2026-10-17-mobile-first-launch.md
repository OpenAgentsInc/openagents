# Mobile first: the phone as the face of 1.0

Status: **proposal, 2026-10-17.** Written for the owner. Nothing here has
been built, posted, or decided yet; the decisions it needs are in
[Decisions for the owner](#decisions-for-the-owner).

Read with: the [1.0 launch plan](../launch/1.0/README.md), the
[big-bang release brief](../audits/2026-10-16-coder-1.0-big-bang.md), the
[master delegation](../audits/2026-10-16-master-delegation.md), the
[Pro plan proposal](../pro/README.md), and the
[current mobile app](../../apps/openagents/OPENAGENTS.md).

## The idea

Lead the 1.0 launch with the phone.

- **The story:** the desktop app and the terminal make all the power of your
  computer available; the phone is where you feel it. You start a coding
  agent on the train, watch it work, answer its question, and stop it, and
  the work happens on your Mac or on our cloud.
- **The proof:** screenshots and short clips of real things working on an
  iPhone, not diagrams.
- **The condition:** the phone has to be useful in the first minute, with
  no computer connected. Someone who installs it from a post must get a real
  answer before they're asked to install anything on a computer.
- **The bonus:** the same chats on the web, the desktop, and the terminal.
  Some of the heavier parts (cloud computers running while your laptop is
  closed) are Pro.

## Where things stand

### The launch plan says the opposite today

The [1.0 launch README](../launch/1.0/README.md) says the launch is "web and
terminal (Coder) only" and lists the desktop, iPhone, and Android apps under
"Left out on purpose". The [release notes](../launch/1.0/release-notes.md),
[post](../launch/1.0/post.md), and [claims](../launch/1.0/claims.md) were
written to that plan (last revised 2026-10-11 to 2026-10-15). They predate
two things:

- **The big-bang brief (2026-10-16).** Coder 1.0.0 now ships Monday
  2026-10-19 from `main`, "aligned with the OpenAgents desktop and mobile
  apps", with one chat list everywhere.
- **The new phone app.** The old OpenAgents phone app was deleted in #11901.
  The current one is the Zeron fork in `apps/openagents/` (UIKit over a Rust
  core), bundle `com.openagents.app`, on TestFlight for internal testers at
  1.0.0 (1011).

So moving to mobile first is a change of launch plan, and the launch files
need a pass (see [Changes to the launch files](#changes-to-the-launch-files)).

### What the phone already does (all closed on code)

The epic #11840 and its 14 children are closed on `main`; their deploy and
device checks are in the master delegation's After-close checks.

| What | Issue | Marketing value |
| --- | --- | --- |
| Sign in with your OpenAgents account | #11848 | Needed for everything below |
| See and drive your computer's live sessions (send, queue, steer, stop) | #11849 | **The core shot** |
| Your web, terminal, and phone chats in one list | #11851, #11892, #11897 | "One conversation everywhere" |
| Continue a web chat from the phone: send, stream, stop | #11893 | **Works with no computer** |
| Continue a terminal chat on a computer, as a real session | #11894 | Hand-off story |
| "Runs on": move a chat between our cloud, this Mac, another computer, a cloud computer | #11898 | Strong, unusual |
| **Our cloud**: start a session with no desktop on | #11852 | **Works with no computer** |
| Shared projects, invitations, and project chats without a computer | #11910 | Team story |
| Custom agents and Agent Lab from the agent picker; Foreman | #11902–#11921, #11926 | Power-user story |
| Labs cards (video, compare, 3D, reports) in phone transcripts | #11884, #11948 | Visual, screenshot-friendly |
| Home dashboards and widgets | #11912 | Screenshot-friendly |
| Agent visuals (`show_visual` charts, scenes) drawn natively | — | Screenshot-friendly |
| Voice calls with a Live Activity | — | Screenshot-friendly |
| Login kept across TestFlight updates | #11922 | Retention |
| Push notifications | #11853 | Closed on code; off until the App ID has the capability (`NEEDS_OWNER.md`) |

### What a new person meets today

From the code at `a6ff2c6a8a`:

1. **The first screen is a sign-in wall.** "OpenAgents", "You're signed
   out.", **Sign in with OpenAgents**, and **Try the demo**
   (`apps/ios/OpenAgents/Shell/SignInViewController.swift`).
2. **Sign-in is a device code.** The app shows a code; the person opens the
   website, signs in there, and approves the code, then comes back. On a
   phone that's a context switch to Safari and back.
3. **After sign-in, a new session needs a place to run.** The app README
   says it plainly: "No agent runs on the phone." The **Run on** menu lists
   paired computers and, when signed in, **Start Our cloud**
   (`Session/NewSessionViewController.swift`).
4. **Our cloud can refuse the first try.** Its messages include "Add your
   Claude key in Settings on openagents.com to run Claude Code on Our cloud"
   and "Our cloud isn't available for this account yet"
   (`crates/mobile/src/client_ffi/cloud_host.rs`). It also takes time to
   start ("Starting Our cloud. Choose it under Run on when it's ready.").
5. **Try the demo is offline.** It opens a fixture workspace with streaming
   replies (`DemoHost`). It's good for screenshots and for looking around,
   but nothing in it is real.

I didn't find a way to start a *new* chat on the OpenAgents cloud (the web
chat runtime, our models, the free daily allowance) from the phone with no
computer. Continuing an existing web chat works (#11893). Verify this before
acting on it; if it's missing, it's the most important gap below.

The old app (removed in #11901) did better on this one point: it needed no
sign-in at all, made a device key on first launch, and opened on a chat with
our hosted assistant. Its TestFlight review notes said "Nothing else in the
app needs a computer." That's the bar to get back to.

## The first minute we want

**Goal: from tapping the icon to a streamed answer in under 60 seconds, with
no computer and no key, and a real coding task started in under 3 minutes.**

### 1. Open on a chat, not a wall

- The first screen is a new chat with the composer focused, and a short line
  under it: "Ask anything, or start a coding task. Your computer can join
  later."
- **Recommended: sign-in comes at the first message**, not before it. Typing
  is free; sending asks for an account in one sheet. That keeps the account
  model simple (every chat has an owner, one chat list everywhere) while
  still showing the product first.
- The alternative is the old app's model: a device key and a signed-out
  allowance, with the account added later. It's a smoother first minute, but
  it means anonymous chats that later merge into an account, which the
  one-chat-list decision (#11895) doesn't cover. Not recommended for Monday.
- **Try the demo** stays, moved to a small link under the composer ("See a
  demo first"). It's how App Review and the curious look around.

### 2. Make sign-in one tap

- Replace the device-code round trip on the phone with an in-app sign-in
  sheet (`ASWebAuthenticationSession`) that opens our sign-in page and
  returns to the app on its own. The code flow stays as the fallback.
- Offer **Sign in with GitHub** first (open GitHub login shipped 2026-10-15),
  and **Sign in with Apple** if the owner wants it. App Store Review requires
  Sign in with Apple when an app offers other third-party sign-ins, so plan
  for it before an App Store submission (not needed for TestFlight).
- No invitation step. Check that no `invite_only` or `signup_disabled`
  refusal can reach a new phone account on launch day.

### 3. A new chat runs somewhere by default

- With no computer connected, a new chat **runs on OpenAgents** (our cloud
  chat runtime, `openagents/auto`, the free daily allowance). The **Run on**
  control shows it and offers the other places.
- **Coding with no computer goes to Our cloud**, started for the person when
  they ask for a coding task, with a visible "Starting a computer for this…"
  row in the chat instead of a toast and a second trip to the menu.
- **Our cloud must work without the person's Claude key.** Default it to our
  gateway models (Coder on Auto), and offer "Use your Claude or ChatGPT
  subscription" as an upgrade, not a precondition. The `needs_claude`
  refusal should never be the first thing someone sees.
- **Start time is the hardest number here.** The freeze doc measured about
  77 seconds for a cloud computer's first turn; the SPACE work (#11728,
  #11800 series) aims under 2 seconds to create. For launch, a warm pool of
  started engines, or showing the cloud chat's first answer while the
  computer boots, keeps the first minute honest.

### 4. Then: bring your computer

- After the first good answer, one card in the chat: "Run this on your own
  computer." It opens a sheet with the one command or the desktop download,
  and a QR code the desktop app scans (the advice-with-action rule: the link
  or the one command, never bare instructions).
- When the computer joins, the chat's **Run on** offers it, and the move
  is a linked continuation (#11898). The person sees the same chat keep
  going on their Mac.

### 5. Measure it

Count, on the server, without request text:

- install → first message sent → first answer streamed (time and drop-off);
- first coding task started, and where it ran;
- first computer connected (and how many days after install);
- sign-in method chosen, and where sign-in was abandoned.

These numbers decide whether "works with no computer" worked.

## The screenshots

Each shot proves one claim. Every shot is a **real run on a dedicated test
account**, never the owner's account and never the offline demo dressed up as
live; the demo can stand in only where the caption says "demo". Capture on the
largest current iPhone in light and dark.

| # | Shot | The claim | What has to work | Status |
| --- | --- | --- | --- | --- |
| 1 | First launch, a question typed, the answer streaming | Useful in the first minute, no computer | [The first minute](#the-first-minute-we-want) items 1–3 | **Gap** (sign-in wall today) |
| 2 | A coding task on Our cloud: tool rows, a diff, "Runs on Our cloud" | Real coding with no computer | Our cloud without a Claude key, fast start | Partial (#11852 closed; key refusal, start time) |
| 3 | A Claude Code session on the owner's-style Mac, driven from the phone: queue, steer, stop | Your computer in your pocket | #11849, deployed sync | Built; After-close checks |
| 4 | An agent's question answered from the Lock Screen or a notification | You don't have to sit and watch | Push (#11853) | Built; needs the App ID's push capability (`NEEDS_OWNER.md`) |
| 5 | The same chat on the phone, the desktop, and openagents.com, side by side | One conversation everywhere | #11893, #11897 | Built; After-close checks |
| 6 | "Runs on": moving a chat from Our cloud to the Mac | Start anywhere, finish anywhere | #11898 | Built; After-close checks |
| 7 | Home with widgets: open issues, agents working, computers | Your work at a glance | #11912 | Built (build 1007) |
| 8 | A Lab card or an agent chart in a transcript | Agents show, not just tell | #11884, `show_visual` | Built |
| 9 | Several agents at once (Foreman or the session list with live rows) | One person, many agents | #11926, sessions list | Built |
| 10 | A voice call with the Live Activity in the Dynamic Island | Talk to your agents | Voice extension | Built; check it still works on 1011 |

Shot 1 and shot 2 carry the whole message. If only two land by Monday, land
those two.

Capture tooling: the app already takes `-route chat:<id>`, `new`, `search`
and `-demo` launch arguments, and the UI tests save screenshots. Add one
`MarketingShotsTests` UI test that drives shots 1, 2, 5, and 6 against a
dedicated test account, so the shots can be re-taken on every build and
never drift from what the app does.

## Free and Pro

The [Pro proposal](../pro/README.md) (2026-10-13) already draws this line,
and it disagrees with one part of the idea above: it makes **sync free** and
lists "your data: export, sync" under "What is not gated, ever".

**Recommendation: keep sync free, and put the cloud behind Pro.** Sync is the
reason someone installs the desktop app after the phone, so it's the growth
loop, not the upsell. Gating it would also break "one chat list everywhere"
(#11895) for free accounts. What costs us money is the cloud, and that's what
Pro already covers:

| | Free | Pro ($20 a month, proposed) |
| --- | --- | --- |
| The phone app, sign-in, chats on our models | Yes, with the free daily allowance | More allowance (proposed $5 a month included) |
| Drive your own computers from the phone | **Yes, unlimited** | Yes |
| Same chats on web, desktop, terminal, phone | **Yes** | Yes |
| Our cloud (coding with no computer) | **A taste: proposed 1 machine-hour a month** | 100 machine-hours a month |
| Cloud agents that keep running with your laptop closed | No | Yes |
| Photos and files in chats | Proposed: photos yes, files Pro | Yes, 5 GB project storage |
| Push notifications | Yes | Yes |

The "taste" row is new and needs the owner's decision. Without it, shot 2 is
a Pro feature and the first-minute promise ("real coding with no computer")
is only true after a card is entered. With it, the free tier's cloud cost is
bounded (1 hour × free accounts) and the trial still has a reason to exist.
The Pro trial (3 days, card up front) stays as proposed.

Two things Pro needs before the phone can sell it: the Stripe side (the Pro
spec's Stripe section is unfilled), and an Apple answer. **A digital
subscription sold inside an iOS app must use In-App Purchase**, so either the
phone only links to openagents.com for Pro (allowed for "reader"-style and
US apps under the current rules, but check before relying on it) or Pro gets
an App Store subscription. For TestFlight and Monday, link out.

## Changes to the launch files

If the owner takes this direction, these need a pass before Monday (none is
made in this change):

- [`launch/1.0/README.md`](../launch/1.0/README.md): move the iPhone app out
  of "Left out on purpose"; add "the phone build has passed the first-minute
  check" to "What has to be true first".
- [`launch/1.0/post.md`](../launch/1.0/post.md): lead with the phone and shot 1
  or 2; the terminal and desktop become "and it's your computer doing the
  work".
- [`launch/1.0/claims.md`](../launch/1.0/claims.md): add each phone claim
  ("under 60 seconds", "no computer needed", "Our cloud starts in N
  seconds") with its measured evidence first.
- [`launch/1.0/release-notes.md`](../launch/1.0/release-notes.md): add the
  phone section.
- The public TestFlight link: the new app uses the same App Store Connect
  record as the old one (`com.openagents.app`), so the existing
  **OpenAgents Beta Testers** group and its public link (`dvQdns5B`) can
  carry it. Recent builds went to Internal Testers only. A build has to be
  added to the external group with new test information and review notes,
  and pass Beta App Review, before a post links to it. Review usually takes
  about a day, so the submission has to go in by Saturday night for Monday.

## The work, in order

Proposed issues. Each closes on code with its own checks; deploys and
dedicated-account runs go to After-close checks, per the current rules.

1. **Phone: start a new OpenAgents chat with no computer.** A new chat runs
   on our cloud chat runtime by default when no computer is connected, through
   the chats SDK (#11893's send/stream/stop, plus create). Verify first that
   this is really missing.
2. **Phone: first screen is a chat; sign in at the first send.** Replace the
   sign-in wall; keep Try the demo as a link.
3. **Phone: one-tap sign-in sheet** with GitHub (and Apple if chosen), code
   flow as fallback; confirm no invite refusal reaches new accounts.
4. **Our cloud without a Claude key.** Default the cloud engine to our gateway
   models; key and subscription become optional upgrades.
5. **Our cloud start time.** A warm engine pool or a cloud-chat answer while
   the computer boots; measure first-answer time on a fresh account.
6. **"Run this on your own computer" card** with the command, download, and
   desktop-scannable QR.
7. **Free cloud taste** (if the owner chooses it): the allowance, its one
   clear stop line, and the Pro link-out.
8. **First-minute counts** on the server (no request text).
9. **`MarketingShotsTests`**: re-takeable shots 1, 2, 5, 6 on a test account.
10. **Launch files pass** (the list above), after the owner approves the
    direction.
11. **External TestFlight** for `com.openagents.app`: add the build to the
    existing external group, with test information and review notes that
    say no computer is needed, and submit.

Items 1–5 decide whether the pitch is true. Items 6–11 make it visible.

## Decisions for the owner

1. **Lead the launch with the phone?** This changes the launch plan written
   2026-10-11 to 2026-10-15, which left the phone out.
2. **Sign-in at the first send (recommended), or a signed-out allowance like
   the old app?**
3. **Sync free (recommended, matches the Pro proposal), or Pro?**
4. **A free taste of Our cloud** (proposed 1 machine-hour a month), or Our
   cloud Pro-only?
5. **Sign in with Apple** now (needed before any App Store submission that
   offers GitHub sign-in), or later?
6. **A TestFlight build and external review this weekend.** The standing rule
   is no TestFlight builds until you ask; this plan needs one by Saturday night
   for a Monday link.
7. **Monday or later for the phone.** If items 1–5 aren't done and checked on
   a real phone by Sunday, the honest options are: launch Monday on web and
   terminal as planned and lead with the phone a week later, or launch Monday
   with the phone and only the shots that are true (3, 5, 6, 7, 8).

## Risks

- **A shot that isn't true.** Every screenshot must come from the build people
  install. A staged shot of a flow that fails on a fresh account is the worst
  outcome here; the claims file and `MarketingShotsTests` exist to prevent it.
- **Cloud cost from free accounts.** Bounded by the taste allowance and the
  per-account computer limit (`quota_exhausted` today).
- **Our cloud start time on launch day.** Many new accounts at once is the
  case the warm pool must cover; without it, shot 2's promise fails at scale.
- **Android.** The app is iOS only today ("Kotlin Android later"). A
  phone-first launch will get Android questions; say "iPhone first" plainly.
- **Two days.** The release gate runs Saturday night to Sunday for the Coder
  binaries. The phone work is mostly app and server work, so it doesn't block
  the binary, but it competes for the same agents and the same deploy lane.
