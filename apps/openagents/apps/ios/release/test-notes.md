# OpenAgents 1.0.0 (1015)

An installation that was signed in before 1014 stays on its account when the
server can't add the new key yet, instead of coming up signed out.

Also in this build:
- Voice can send a request to any saved agent and chat you choose.
- Each computer's voice chat is listed as "Voice transcript".
- A saved agent's stuck turn stops and says why at the next check-in.

Check that an installation updated from 1014 opens on the same account and its
chats, that a chat answers, and that a connected computer shows its sessions.

OpenAgents 1.0.0 (1014)

No sign-in. The app makes your account the first time it opens, from 12 words
it keeps on this phone. The same 12 words open your wallet, so one backup
covers both: Settings > Back up your account shows them after Face ID, and
Restore from 12 words brings an account back on a new phone. A wallet you
already had keeps its own words and balance.

Connect a computer by scanning the code OpenAgents shows on it (Settings >
Connect a computer). The computer signs in as you.

Also in this build:
- Cloud chats have the same ... menu as computer chats: Pin, Copy Transcript,
  and Archive.
- A chat that runs a custom agent, such as Foreman, says so under its title
  and in the composer.
- Run on offers OpenAgents Cloud when a project is picked.
- Working shows while a saved account chat answers.
- The voice screen shows the computer's Codex usage and your saved voice, and
  a chat links back to a live call.
- Settings > About shows the version and opens an About page.
- The demo is no longer offered.

Check a fresh install (no sign-in, a chat answers), Back up your account and
Restore from 12 words with a dedicated test install, that an existing wallet
still shows its balance after updating over your installation, scanning a
computer's code, and the cloud chat menu. Check light and dark appearance.

OpenAgents 1.0.0 (1013)

The wallet now uses separate screens for receiving bitcoin, sending, payment
review, activity, and wallet information. The balance page keeps Send and
Receive together, with recent payments and a backup reminder. Buttons, text,
spacing, and payment codes follow the Bitcoin Design wallet patterns.

This build changes the presentation of existing wallet features. It adds no
payment methods or wallet services. Existing wallets and recovery words stay
in place.

Check each receive method, entering a Lightning request amount, copying its
payment request, entering a destination, reviewing the amount and fee, editing
before sending, and returning to activity. Check light and dark appearance,
large text, recovery words after device authentication, and updating over your
existing installation. Use a dedicated test wallet for any payment checks.

Sign-in changes already on main are included: use the existing sign-in sheet;
Sign in with Apple is no longer offered.

OpenAgents 1.0.0 (1012)

The phone works with no computer. It opens on a chat: type a question and send
it, and the answer streams in, with no sign-in for your first few messages.
When it asks you to sign in, use Sign in with Apple or the sign-in sheet; the
chats you had move into your account.

Signed in, new chats run on our servers. Ask for a coding task and it starts a
cloud computer for it, covered by free cloud time. After a good answer, the
"Run this on your own computer" card shows the install command and the
download link, and goes away once a computer joins. Saved agents and the app
show the OpenAgents power mark.

Check the first message signed out, reaching the message limit, Sign in with
Apple, the sign-in sheet closing by itself, a cloud chat on a new account, the
own-computer card's copy and link, and running out of free cloud time.

OpenAgents 1.0.0 (1011)

The tab bar is gone. Tap the menu button at the top left, or swipe in from the
left edge, to open the drawer: your sessions with search, plus Home, Settings
and New session in its bottom bar. Tap a session to open it as the page; tap
or swipe the dimmed page to close the drawer. New session also stays at the
bottom of Home and Settings.

Long press a session in the drawer and drag it, as on the desktop: drop it
among the pins to pin it there (or reorder pins), onto a section to move it
in, or onto Recent to take it out of Pinned and sections. With nothing
pinned yet, Pinned appears at the top while you drag.

Check opening and closing the drawer during a reply, search in the drawer,
the left-edge swipe on Home, a session, and Settings (deeper Settings pages
still swipe back), VoiceOver (the drawer reads on its own; escape closes it),
and voice calls from the New session bar. On iPad the sidebar is unchanged;
in Slide Over the iPhone drawer is used.

OpenAgents 1.0.0 (1010)

Open Agent Lab from the agent picker in a new conversation. Choose a connected
computer and project, edit an agent and a small set of tests, then save and run.
Inspect the results, open a trial's conversation, stop a run, and try a saved
agent as a comparison. Use agent saves your chosen instructions to the picker.
Try the Foreman template to create an agent for coordinating issue work.

The computer needs this update too. Tests run in separate project copies with
the selected runtime's normal access. Codex, Claude Code, and installed Rust
runners use the same conversations and assignment service as other agents.

Try reopening an experiment on another device, opening an earlier saved
version, explicit human review, missing computers, and a connection loss during
save. Check large text, VoiceOver, sign-out, and switching accounts. The login
persistence fixes from build 1006 and dashboards from build 1005 are included.

When editing an agent, use Done above the keyboard to return to the form.
Agent and project menus stay open while model choices update.

OpenAgents 1.0.0 (1009)

Foreman is a ready-made agent that keeps track of every open issue and every
agent working on them. Open Agent Lab from the agent picker and choose Start from
Foreman. Choose your computer and a project, then Save and Use agent. Foreman
then appears in the agent picker.

Pick Foreman and ask "What isn't being worked on?" or "Who is on #11812?". Check
that it names issues, chats and evidence. The computer needs the updated desktop
app, with Claude Code and the GitHub CLI signed in.

OpenAgents 1.0.0 (1008)

Custom agents now use the selected computer's saved model choices and refresh
from that computer. Opus 5.5 also appears in the default Claude choices.
Saved model selections stay selected while the list refreshes.

Create or edit an agent, choose Claude Code, and select Opus 5.5. Check the model
choices with an online computer and while offline. Switching computers should
show the new computer's choices. Existing agents keep their saved model.

OpenAgents 1.0.0 (1007)

Home now uses Coder Noir and Coder Light, with compact issue totals and current
agent tasks. Edit widgets from the toolbar; manage saved dashboards and shared
layouts from the menu. Sessions stays visible as you scroll, and the Sessions
shortcut at the top opens the original list with its folders and history.

Check Home in dark and light appearance, open Sessions, and reopen a conversation.
Try editing and canceling, saving a dashboard, and reopening the app. Check large
text and VoiceOver on your phone.

OpenAgents 1.0.0 (1006)

Your login stays saved when you update the app through TestFlight. Existing
saved logins carry over, and choosing Sign Out keeps you signed out.

Install this build over your signed-in app without deleting it. Open the app,
quit and reopen it, and check that your account and projects are still there.
Then sign out and reopen it to check that sign-out stays in effect.

OpenAgents 1.0.0 (1005)

Home opens first and shows open issues, issues assigned to you, and work across
agents and computers. Choose a project, add widgets, change their size, and
rearrange them with drag or Move up/Move down. Save several dashboards and choose
a default. Your layout returns after restarting the app.

Layouts can be exported, imported, saved to your account, and shared through the
layout directory. Review a shared layout and choose your own projects before
saving it. Publishing is a separate choice; private is the default. Updates
preserve your widget settings, order, and removals for review.

Try Home with several active agents, an unavailable computer, and GitHub issues.
Open an issue or agent from its widget, then return Home. Check large text,
VoiceOver, background and foreground, sign-out, and sign-in to another account.
GitHub must be connected to the same OpenAgents account for issue counts.

OpenAgents 1.0.0 (1004)

Create your own agents from the agent picker when starting a conversation.
Choose a name, instructions, and Claude Code, Codex, or an installed SDK runner.
Your saved agents appear at the top of the picker and are available after reopening the app.

Try these steps:
- Connect an updated computer with your chosen runtime signed in.
- Open a new conversation, tap the agent picker, and choose Create agent.
- Save the agent, choose a computer and project, and send a task.
- Follow up, stop a running task, and reopen the conversation.
- Edit the agent and start another conversation to try the new instructions.

SDK runners must be installed on the computer first. Creating an agent does not
install software or start work. Shared projects and invitations from build 62
are included.
