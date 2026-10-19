# Manage computers

## On the website

[Settings](https://openagents.com/settings) lists the computers logged in
to your account, each with **Remove**, which signs it out. To add one, run `coder login` on it;
see [Connect a computer](/docs/connect-a-computer).

## On your phone

In [OpenAgents for iPhone](/docs/iphone), **Settings → Devices** lists
your computers, each marked **Online** or **Offline**, and
**OpenAgents Cloud**, the cloud computer we run for you.

- **Connect a computer** opens the scanner. See
  [Connect a computer](/docs/connect-a-computer).
- **Start OpenAgents Cloud** starts your cloud computer, and
  **Stop OpenAgents Cloud** stops it.

## What your phone sees of a computer

- **Its sessions.** The computer's sessions show in the phone's sessions
  list. You can open one and keep going; the computer answers.
- **Its voice conversation**, as **Voice transcript**, when you use
  voice through that computer.

## On the computer

`openagents connect devices` lists the phones that can reach the
computer, and `openagents connect remove DEVICE` removes one at once.

## Tailscale

You don't need Tailscale. If you already use it, it can be another route
to your computer; see [Connect a computer](/docs/connect-a-computer).
Access only ever comes from pairing.

Next: [Plugins](/docs/plugins).
