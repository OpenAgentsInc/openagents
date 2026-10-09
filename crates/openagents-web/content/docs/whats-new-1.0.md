# What's new in 1.0

OpenAgents 1.0 is out on the web, in the terminal, on Mac, Linux, and
Windows, and on iPhone and Android. Get every version from the
[download page](/download).

## On the website

- **Sign in with GitHub.** Click **Log in** at the top right, or go to
  [/login](/login). Your chats then follow you to any browser.
- **Pin, rename, archive, or delete a chat** from the **…** menu on its row.
  Search sits at the top of the sidebar. Find archived chats under
  [Archived chats](/chat/archived).
- **Delete all your chats** in [Settings](/settings).
- **Connect your GitHub repositories** at [/projects](/projects) with
  **Connect GitHub**. You choose which repositories. Chats are grouped by
  project in the sidebar.
- **See your signed-in computers** in [Settings](/settings), under
  **Computers**.

## In the terminal

Install or update Coder on macOS or Linux:

```sh
curl -fsSL https://openagents.com/cli/install.sh | bash
```

On Windows, in PowerShell:

```powershell
irm https://openagents.com/cli/install.ps1 | iex
```

- **Sign in to your account:** run `coder login`, then approve the code at
  [openagents.com/device](/device). Inside Coder, type `/login`.
- **Sync your chats to the website:** type `/sync on` in Coder. Add your 150
  newest earlier chats with `/sync all`. Stop with `/sync off`. Remove this
  computer's chats from your account with `/sync delete`.
- **Reply from the website:** open a synced chat in the sidebar and type
  your reply while Coder is open on that computer.

Sync is off until you turn it on. A message that looks like a key or a
password is never sent.

## On your desktop

Download the app for Mac, Linux, or Windows from the
[download page](/download).

- **Light or dark:** Settings > Appearance. It follows your system until you
  pick one.
- **Windows:** SmartScreen asks you to confirm the first time. Click
  **More info**, then **Run anyway**.

## On your phone

- **iPhone:** join the test from the [download page](/download).
- **Android:** download the app from the [download page](/download).
- **Light or dark:** Account > Appearance.

## Your privacy

We ask our model providers not to train on your chats and not to keep them.
Read the [privacy policy](/privacy) for the details.

Next: [Download](/docs/download).
