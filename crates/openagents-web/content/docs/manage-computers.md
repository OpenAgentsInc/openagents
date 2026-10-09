# Manage computers

## On the website

[Settings](https://openagents.com/settings) lists the computers signed in
to your account, each with **Remove**, which signs it out. To add one, run `coder login` on it;
see [Connect a computer](/docs/connect-a-computer).

## On your phone

In [OpenAgents for iPhone](/docs/iphone), **Account → Computers** lists your computers, each with a one-word status.

- **Each computer** opens on a tap: its status, ordering work, a terminal
  on it, its access, and its recent work.
- **Its menu** switches it off or on, tries it now, opens its access, or
  forgets it. **Forget** stops this phone connecting and drops the
  computer from the list; the computer keeps the phone's access until you
  remove it there.
- **Connect a computer** opens the scanner, and **Add another way** takes a
  pasted code. See [Connect a computer](/docs/connect-a-computer).

## What your phone sees of a computer

- **Its chats.** The computer's own chats (from the Mac app, the Terminal,
  or `openagents chat`) show in the phone's earlier chats, labelled with
  the computer. Open one and keep going; the computer answers. After a
  relaunch with the computer off, they still list, marked **Last read …**.
- **Coder's work** on it, step by step, while it runs and after.
- **Its coding agents.** Ask in a chat "which coding agents are connected?"
  and the reply names the agents on your computer.
- **Its plugins.** Ask "list my plugins" and tap **Run** on the card.

## On your Mac

**Phones and computers** in the Mac app shows:

- **Online. Your phone can reach this Mac.** or **Offline.**
- The phones that can reach this Mac, each with **Remove**, which cuts it
  off at once.
- Your project for Coder (**Choose folder…**), whether each coding agent is
  signed in, and **Let my phone start Coder here**.
- Coder's recent work, and **Connect another phone**.

On a computer without the Mac app, `openagents connect devices` lists the
phones and `openagents connect remove DEVICE` removes one.

Only the computer itself can let phones start Coder on it: the Mac app's
switch, or a command run on that computer. No phone or relay message can
turn it on.

## When a computer is offline

Messages to Coder on that computer wait on your phone, encrypted, and go
out as soon as the computer answers again. A relaunch or a bad connection
never sends one twice.

## Tailscale

You don't need Tailscale. If you already use it, **Account → Tailnet** can
sign the app in to Tailscale to list your tailnet's devices. That sign-in
grants no access to any computer: access only ever comes from pairing.

Next: [Plugins](/docs/plugins).
