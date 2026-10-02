# Facts the judge checks replies against

Ground truth for the simulated-user QA run (docs/qa/simulated-users.md).
Keep it short and current; a reply that contradicts a line here is a
finding. When the product changes, change this file in the same commit.

## What OpenAgents is

- OpenAgents is an app you chat with on your phone and your computer. When
  something needs a computer, it sends Coder, our coding agent, to do the
  work on one of your own computers.
- Coder runs on your own computer, in projects you picked, with that
  computer's Git and GitHub sign-in. It drives Codex, Claude Code, or Grok
  Build: whichever one is signed in on that computer with capacity.
- Chatting needs no computer. From the chat alone, OpenAgents can't read
  your files, run code, or reach your computer.
- There is no OpenAgents account or password. Each phone or computer makes
  its own key the first time it opens. A reply that tells a person to
  "sign in" or "create an account" with OpenAgents is false. (Signing in to
  Codex, Claude Code, or Grok Build on the computer is real and needed for
  Coder.)
- The phone app has a Bitcoin wallet (Spark and Lightning through Breez);
  its seed stays on the phone, and payments can carry network fees.
- The Verse is a shared 3D world; the Gym is where plugins are tested and
  results published; XP is never money.

## How to get it (openagents.com/download)

- OpenAgents for Mac 1.0.0-rc.2: a `.dmg` for macOS 13 or later, Apple
  silicon and Intel. Open it and drag OpenAgents onto Applications.
- OpenAgents Terminal 1.0.0-rc.2: on macOS and Linux,
  `curl -fsSL https://storage.googleapis.com/openagentsgemini-cli-releases/openagents/install.sh | sh`;
  on Windows, an `irm … install.ps1 | iex` PowerShell line.
- The iPhone and Android apps and OpenAgents for Linux and Windows are not
  published for download; they are built from source at
  github.com/OpenAgentsInc/openagents. (iPhone builds go to TestFlight
  testers by invitation.)
- A phone connects to a Mac by scanning the QR code OpenAgents for Mac
  shows (or approving it on the Mac's screen).

## Limits and privacy

- There is no usage limit, quota, or per-minute cap on chat. Any reply or
  screen that mentions a limit, quota, credits running out, or "try again
  later because of usage" is a finding.
- Chat messages are encrypted before they leave the device; the relay sees
  only the encrypted form and keeps nothing. The chat service stores no
  message text. To answer, it sends the conversation to Space Bunny Alpha
  through OpenRouter (whose provider may keep it, not to train on), and to
  Google's Gemini through the Vercel AI Gateway when Space Bunny can't
  answer.
- Coder sends what it reads to the model provider you signed in to (as
  Codex or Claude Code do on their own). Your code stays on your computer.

## Surfaces

- The website's Ask box answers questions about OpenAgents only and points
  to /download for what the apps do. It never starts Coder.
- OpenAgents Terminal (`openagents`, bare or `openagents terminal`) has
  slash commands: /new, /threads (Ctrl+T), /stop (Esc), /export,
  /settings, /connect, /plugins, /expand (Ctrl+O), /help, /quit.
- Plugins: published ones include Project map, Code finder, and Test
  reader; a person can ask to test one, or make their own by chatting.
