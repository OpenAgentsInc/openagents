# OpenAgents mobile: first-class Antigravity over Tailscale

Date: 2026-11-22. Scope: the iOS app (`apps/openagents-ios`, build 1415), the shared
core it builds (`apps/shared/app`, `apps/shared/native`), the computer's phone service
(`coder/tui/host-*.coax`), the Antigravity worker (`coder/tui/agyworker.coax`) and the
Tailscale link (`AppTailnet*.swift`, `apps/openagents-desktop/Tailnet.swift`).

This audit comes from reading code. Nothing was run on a device. Items that need a
live check are marked **unverified**.

## Verdict

The phone can already run an Antigravity chat on a paired Mac, and requests already
travel straight over Tailscale when both ends are up. But Antigravity is a thin text
pipe today, not a first-class agent:

- You can't choose Antigravity from the phone. The Mac picks the agent, and it prefers
  Microcoder.
- You can't choose an Antigravity model or thinking level, at the start or later.
- Antigravity never asks the phone for approval. A phone run allows every tool call
  with full disk access.
- Every Antigravity tool call shows as "Bash". Plans, task lists, artifacts, diffs and
  images don't reach the phone.
- The reply doesn't stream. The phone polls every 1.5 s, only in the foreground.
- The phone has no notifications, so you don't learn that a turn finished.
- Usage, sign-in state and quota are Claude only.
- Chats that Antigravity or the `agy` CLI started on their own aren't listed.
- The Tailscale path only works while the desktop app runs, and the v1.0 release build
  of the desktop app ships neither pairing nor the Antigravity worker.

The fix list in [Work plan](#work-plan) has 9 blockers, then the rest in order.

## What works today

| Area | State | Evidence |
| --- | --- | --- |
| Zero-config pairing over Tailscale | Works. The Mac advertises `_openagents-coder._tcp`; the phone reads its tailnet address once and keeps it. The Mac admits only an untagged device of its own Tailscale user (`tailscale whois`). | `apps/shared/native/AppTailnet.swift:100-121`, `AppTailnetLink.swift:74-143`, `apps/openagents-desktop/Tailnet.swift:195-239` |
| Direct transport over the tailnet | Works. Each sealed request goes to the Mac's tailnet listener first (port 47109) and falls back to the relay after 4 s. | `AppDeviceLink.swift:297-304`, `AppTailnetLink.swift:155-176`, `Tailnet.swift:242-258` |
| Grant renewal away from home | Works over the saved tailnet address when the grant has under a day left. | `AppTailnet.swift:293-297` |
| Start an Antigravity chat | Works when the Mac picks it. The model ID `antigravity` names the engine. | `coax/lib/agent/environment/phone-agent-policy.coax:21-28`, `coder/tui/host-phone.coax:94-112` |
| Follow-ups, steer, one queued message, Stop | Work. | `apps/shared/app/device/lane.coax:259-262, 338-355`, `coder/tui/agyworker.coax:1316-1334` |
| Transcript | Thoughts, replies, commands and file reads show. | `coder/tui/host-desktop.coax:470-515` |
| Desktop-started Antigravity chats | Listed on the phone when the daemon owns the run and its folder is in the grant's workspace. | `host-desktop.coax:349-363` |
| Engine label | "Antigravity" shows for a phone-started chat. | `apps/shared/app/view/page.coax:149-157` |

## Gaps

### A. Choosing Antigravity

**A1. The phone can't pick the agent.** The request has `model` and `effort` fields
and no harness field (`coax/lib/agent/environment/wire.coax:44-50`). The engine comes
from the model ID. The phone always sends the first model the Mac offers
(`apps/shared/environment-app.coax:157-163`), and the Mac's order is Microcoder,
Antigravity, Claude (`phone-agent-policy.coax:22`). So a phone chat runs on Antigravity
only when Microcoder has no Gemini sign-in. The picker logic exists in the shared
reducer (`environment-app.coax:265-268`) but nothing on iOS draws it, and a UI test
asserts the chip is absent (`apps/openagents-ios/UITests/SidebarTests.swift:30`). The
picker was hidden by owner choice; first-class Antigravity needs it back, or at least a
per-computer default agent in Settings.

**A2. No Antigravity model or thinking level.** The Mac offers one entry,
`{id: "antigravity"}`, with no effort ladder (`host-phone.coax:103`). The task line
forces `model = ""` and `effort = ""` for Antigravity (`host-phone.coax:156-161`). The
worker itself supports both, including a change mid-session
(`agyworker.coax:201-208, 1153-1174`), and can list agy's models (`--probe`,
`agyworker.coax:843-851`). The phone path uses none of it.

**A3. No plan mode.** The worker has `auto`, `default` and `plan` modes
(`agyworker.coax:174-179`). The phone's worker spec sets only `PATH`
(`host-phone.coax:164-177`), so every phone run is `auto`.

**A4. The phone run uses a different agy profile than the desktop.** The desktop sets
`CODER_AGY_PROFILE_DIR` to an isolated profile (`Worker.swift:777, 888-895`). The
phone's spec doesn't, so a phone chat runs under agy's default profile: possibly a
different sign-in, settings and history. **Unverified** whether the daemon's own
environment supplies it.

**A5. Antigravity may not be offered at all from the login-service daemon.**
`ph_worker` looks for `agyworker` beside its own binary or in `$CODER_WORKER_DIR`
(`host-phone.coax:65-71`). The resident copy installs only `coder-host`, and nothing
sets `CODER_WORKER_DIR`. If that copy answers, `agy` reads as unavailable.
**Unverified**: check on the owner's Mac with `DescribeEnvironment`.

### B. Approvals and questions

**B1. A phone run asks for nothing.** The task is written with `no_ask = true` and
`sandbox = "danger-full-access"` (`host-phone.coax:158`), and the worker runs without
`CODER_AGY_INTERACTIVE`, so it allows every tool call except paths with `..`
(`agyworker.coax:500-503`). There is no way to turn approvals on from the phone.

**B2. The Mac refuses the phone's answer.** `RespondInteraction` on a run returns
`unsupported_profile` (`host-desktop.coax:830`). The worker's permission lines
(`MWW_PERMIT`, with a question and options) are never put in the session the phone
reads (`host-desktop.coax:470-515`).

**B3. A desktop-started Antigravity chat can hang.** The desktop sets
`CODER_AGY_INTERACTIVE=1`. If it asks for approval while you're on the phone, the
daemon answers only `MWW_ASK`, the phone sees nothing and can't answer. **Unverified**
by running.

**B4. The phone's prompt is Allow or Deny only.** "Allow this?" with a boolean
(`page.coax:231-236`). Antigravity's permission requests carry several options (allow
once, always allow, deny), and its questions carry free choices. The phone has no
question panel.

### C. What the transcript shows

**C1. Every tool call is "Bash".** The Mac rewrites each Antigravity call to `Bash`,
each view to `Read` and each delegation to `Task`, and carries the real label as the
argument (`host-desktop.coax:481-496, 644-645`). The phone's `tk_of_name` knows only
Claude and Codex tool names (`coax/lib/agent/session/kinds.coax:24-35`). Edits, web
fetches, searches, browser actions and MCP calls all read "Ran ‹label›".

**C2. No diffs.** The page draws a diff only when an item carries `path` and `diff`
(`page.coax:290-295`). The run builder never sets them for Antigravity.

**C3. No task list.** The run builder drops `MWW_TODOS` (`host-desktop.coax:420`). The
phone can only show "Updated todos" and a count.

**C4. No plans or artifacts.** Antigravity's plan events are answered inside the worker
(`agyworker.coax:1258`) and never forwarded. Implementation plans, walkthroughs,
screenshots and browser recordings have no item role on the wire: the roles are user,
assistant, tool, notice and thought (`wire.coax:95, 114`).

**C5. No usage, attempt or stats lines.** `ATTEMPT` and `STATS` are dropped
(`host-desktop.coax:420`). The effective model never shows. `IOSThreadProof`
(`AppCore.swift:104-140`) holds requested and effective model but nothing fills it for a
computer chat.

**C6. A signed-out agy shows a generic error.** An Antigravity error ending becomes
`N_ERROR` (`host-desktop.coax:513-514`), not the `login` notice, so the phone can't
tell you to sign in.

**C7. Desktop-started chats carry no engine label.** The summary sets model and effort
only for phone runs (`host-desktop.coax:304-308`).

**C8. Row caps.** An activity group holds 16 rows and a transcript 384
(`docs/audits/2026-11-20-mobile-ui-gap-analysis-zeron.md:97, 188`). Long Antigravity
turns pass both.

### D. Composer

**D1. No images or files.** The request has no attachment field and refuses unknown
keys (`wire.coax:155-174`). The worker sends no image blocks. The app has no photo or
camera permission strings (`project.yml:95-104`). Antigravity is multimodal, so this
matters more than for a text agent.

**D2. No `@` file mentions and no slash commands.** `components/app/mention.coax`
exists; the phone doesn't load it. Antigravity's workflows and rules have no entry
point.

**D3. No voice, no edit-and-resend.**

**D4. A follow-up to an idle desktop Antigravity chat may be dropped.** For desktop
runs the Mac writes queue lines (`host-desktop.coax:791-808`); `agyworker` has no queue
handling (`agyworker.coax:1310-1396`). Phone runs avoid this
(`host-phone.coax:213-214`). **Unverified** by running.

### E. Sessions and history

**E1. Antigravity's own sessions aren't listed.** Native history scans only
`~/.claude/projects` (`host-environment.coax:649-660`). A reader for agy's transcripts
exists (`coax/lib/agent/observe/antigravity.coax`) and is used by the CLI and evals,
but no `host-*.coax` file loads it. Chats from the Antigravity app or `agy` in a
terminal are invisible on the phone.

**E2. No real resume.** The engine supports `session/resume`
(`coax/lib/agent/engine/antigravity.coax:1025-1031`); the worker never calls it and
prepends recall text instead (`agyworker.coax:628-636`). A chat that restarts loses
agy's own context.

**E3. The phone ignores the session's `control` field**, so a finished desktop chat
still shows a composer.

### F. Accounts and usage

**F1. Usage is Claude only.** The wire rejects any provider but `claude-code`
(`coax/lib/agent/accounts/wire.coax:39`). The screen says "Connect Claude" and shows
Claude's windows (`apps/shared/app/usage/view.coax:8-22`). The engine already parses
agy's per-turn usage (`antigravity.coax:927`) and the desktop keeps a
`usage-antigravity.txt` cache (`Window+Usage.swift:113`).

**F2. No sign-in from the phone.** `agyworker --connect` installs agy and runs its
login with a URL and code (`agyworker.coax:859-941`). The phone can't start it or see
its state. Error words name Claude: "Sign in to Claude.", "Install Claude Code on this
computer." (`apps/shared/provider-account-text.coax:8, 17`).

### G. The Tailscale link

**G1. The desktop app must be running.** The tailnet listener and the Bonjour record
live in the desktop app process (`Tailnet.swift:135-150`), which forwards each request
to the daemon's socket (`DaemonClient.swift:260`). With the app closed, the phone falls
back to the public relay after a 4 s wait per request. The login-service daemon should
own the listener.

**G2. The v1.0 desktop build has no pairing and no Antigravity.** The minimal profile
stubs the tailnet codec (`apps/openagents-desktop/app-v1.coax:25-33`), compiles the
listener out (`AppDelegate.swift:30-32`, `OA_FULL`) and deletes `agyworker` from the
bundle (`build.sh:27-35`). Only the full build supports this feature. The v1 spec lists
"Tailscale pairing" as excluded (`docs/desktop/openagents-v1-spec.md:29, 86`).

**G3. One request, one reply, no push.** The direct connection carries one sealed
event each way per call (`Tailnet.swift:244-257`). Each call opens a new Unix socket to
the daemon with a 10 s limit. There is no stream, so:

- The reply is polled every 1.5 s (`lane.coax:34`) and arrives in chunks, not tokens.
- Waiters match replies by arrival order, not by request ID
  (`AppTailnetLink.swift:251-258`). A request the Mac drops silently ("Expired or
  silent requests produce no reply", `Tailnet.swift:253`) shifts every later reply to
  the wrong waiter until the connection resets. The seal check should refuse a wrong
  reply, which then looks like a failed request. **Unverified**; worth a test.

**G4. Only Macs seen once on the same Wi-Fi.** The phone learns a Mac's tailnet
address from Bonjour on the local network (`AppTailnetLink.swift:57-88`). You can't add
a Mac by its MagicDNS name or tailnet address, and a Linux box or cloud host on your
tailnet can't be added at all: the listener is Swift in the Mac app.

**G5. No link status.** The phone shows nothing about Tailscale: not whether a chat
uses the direct path or the relay, not whether the Mac is reachable, not why admission
was refused (`tagged`, `not_owner`). There's no computers screen and no online
indicator (`2026-11-20-mobile-ui-gap-analysis-zeron.md:127-130, 148`).

**G6. The relay is still required.** Pairing and the grant go through the relay even
when the link came over Tailscale (`AppTailnet.swift:13-15`). A tailnet-only setup with
no relay access doesn't pair.

**G7. No QR pairing.** Without Tailscale on the same Wi-Fi, you open a link by hand.

**G8. Plain TCP on the tailnet.** Payloads are NIP-44 sealed and WireGuard carries
them, so content is safe. Noted for completeness; no change needed.

### H. Background and notifications

**H1. No notifications.** No UserNotifications code, no `aps-environment`, no
background modes, no Live Activity (`OpenAgents.entitlements`, `Info.plist`). You can't
learn that an Antigravity turn finished or needs you without opening the app. The Mac
has no way to send one either.

**H2. Background closes everything.** Sockets close and polling stops
(`AppDeviceLink.swift:74-81`, `lane.coax:409-413`). The run continues on the Mac and
the chat is read again on return. A write whose outcome is unknown isn't sent again
(`lane.coax:414-421`); there's no durable outbox.

### I. Tests

No fixture on the phone path uses an Antigravity tool, permission, task list or
artifact. `agy_standin.coax` can stand in for `agy acp` without a Google sign-in, but
no check runs phone request → daemon → `agyworker` → stand-in → session view. The
tailnet check (`apps/shared/host/apple/tailnet-check.swift`) covers admission rules,
not the direct transport under loss or reordering. `live-smoke.coax` is manual.

## Work plan

Blockers, in order. Each is small enough for one issue.

1. **Agent choice on the phone (A1).** Show the model chip in the composer, fed by the
   Mac's offer, and remember the last choice per computer. Remove the UI-test assertion
   that it's absent.
2. **Antigravity models and thinking levels in the offer (A2).** Have `ph_offer` list
   agy's probed models with an effort ladder; pass model and effort through `ph_task`;
   send `agyw_reconfigure` when the phone changes them mid-chat.
3. **Approvals (B1–B4).** Run phone Antigravity chats interactive by default, project
   `MWW_PERMIT` into the session as an interaction with its options, accept
   `RespondInteraction` for runs, and draw a multi-option prompt and a question panel
   on the phone. Add a per-chat "ask / don't ask" setting so full auto stays possible.
4. **Real tool kinds (C1, C2, C3).** Carry the ACP tool kind and title instead of
   `Bash`; add Antigravity names to `tk_of_name`; forward edit diffs and the task list.
5. **Notifications (H1).** APNs registration on the phone, a push target stored with
   the grant, and a send from the Mac through the account service when a turn ends or
   asks. Then a Live Activity for a running turn.
6. **Daemon-owned tailnet listener (G1, G3).** Move the listener into `coder-host`,
   keep one connection open, tag replies with the request ID, and add a server-push
   frame for session updates so the phone stops polling on the direct path.
7. **Worker and profile for the daemon (A4, A5).** Install `agyworker` with the
   resident copy or set `CODER_WORKER_DIR`, and pass the desktop's agy profile
   directory in `ph_spec`.
8. **Usage and sign-in (F1, F2, C6).** Open the accounts wire to an `antigravity`
   provider: state, quota, and the `--connect` flow with its URL and code shown on the
   phone. Map a signed-out ending to the `login` notice. Fix the Claude-only words.
9. **Ship it in a build the owner runs (G2).** Either keep this on the full desktop
   build and say so, or add `pairing` and `agyworker` to the release profile.

Then, in order of value:

10. Plans and artifacts as a new item role, with images (C4).
11. Images and files in the composer (D1).
12. Antigravity's own sessions in the list, read-only (E1), and `session/resume` (E2).
13. Computers screen: reachable or not, direct or relay, add by MagicDNS name, refusal
    reasons (G4, G5), and QR pairing (G7).
14. Plan mode toggle (A3); `@` mentions (D2); engine label on desktop chats (C7);
    effective model and usage per turn (C5).
15. Queue handling for idle desktop Antigravity chats (D4); honor `control` (E3);
    durable outbox (H2); raise the row caps (C8).
16. A phone-path check that runs the stand-in end to end, and a direct-transport check
    with dropped and reordered replies (I).

## Check first on the owner's Mac

Three cheap checks decide how much of this is already broken rather than missing:

- Pair the phone and read the offer. If `antigravity` is absent, A5 is real.
- Start a phone chat with Microcoder signed out. Confirm it lands on Antigravity and
  which agy account it uses (A4).
- Quit the desktop app and send a message. Each request should take about 4 s longer
  (G1).
