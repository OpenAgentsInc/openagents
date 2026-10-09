# OpenAgents 1.0 release notes (draft)

Draft for the owner (#11104). Nothing here is posted. Each line names the
issue it comes from so it can be checked against the build before posting.
Before publishing a platform's notes, open that platform's release build and
strike any line it doesn't do. Drop the issue numbers from the public copy if
you want it shorter.

Left out on purpose: the Pro plan and environments (Pro is next week), agent
payments and the wallet (later), the public inference API (built on main, not
deployed yet), plugins (the sample plugins are being replaced, #11096).

## Web (openagents.com)

Ships with the web deploy after the owner's local test (#11094).

- **Sign in with GitHub.** Click **Log in** at the top right. (#11039)
- **Your chats follow your account**, on any browser. Chats you started
  before signing in move to your account when you sign in. (#11039)
- **Organize chats.** Each chat's **…** menu has Pin, Rename, Archive, and
  Delete. Search is at the top of the sidebar. Archived chats are under
  **Archived chats** at the bottom of the sidebar. (#11036, #11038)
- **Delete everything.** Settings has **Delete all chats**. Deleted means
  deleted. (#11038)
- **Projects.** Open [/projects](https://openagents.com/projects) and click
  **Connect GitHub**. You pick which repositories, through a GitHub App. Chats
  are grouped by project in the sidebar. (#11034, #11056)
- **Your terminal chats on the web.** Sign in from Coder and turn on sync
  (see Terminal below), and those chats show in the sidebar with live status.
  While Coder is open on that computer, you can reply from the website.
  (#11046, #11047, #11048)
- **Signed-in computers.** Settings > Computers lists every computer signed
  in to your account. (#11045)
- **Status on chat rows.** A row shows a spinner while an answer is coming.
  (#11033, #11035)
- **Slow answers no longer fail.** When the model is slow to start, you see
  that it's still working instead of an error. (#11087)
- **Plain words.** We went through every page and took out the system talk.
  (#11031)
- **Privacy.** We ask the model providers not to train on your chats and not
  to keep them. Usage records are deleted after 30 days. Saved Claude keys
  are encrypted. The [privacy policy](https://openagents.com/privacy) was
  updated on October 9; read it. (#11040, #11041, #11042, #11044)

## Terminal (Coder 1.0.0)

Ships with the Coder 1.0.0 release on the stable channel (#11091).

Install or update, on macOS and Linux:

```sh
curl -fsSL https://openagents.com/cli/install.sh | bash
```

On Windows, in PowerShell:

```powershell
irm https://openagents.com/cli/install.ps1 | iex
```

- **Sign in to your OpenAgents account.** Run `coder login` (or `/login`
  inside Coder). It shows a code and opens
  [openagents.com/device](https://openagents.com/device); approve it there.
  `coder logout` (or `/logout`) signs out. (#11045)
- **Sync your chats.** `/sync on` saves new and changed chats to your
  account. `/sync all` also adds your 150 newest earlier chats. `/sync off`
  stops. `/sync delete` removes this computer's chats from your account.
  `/sync` shows where you are. Sync is off until you turn it on. (#11046)
- **Keys stay home.** Every message is checked for keys and passwords before
  it leaves your computer and again on our side. A message that looks like
  one isn't sent. (#11046)
- **Deletes go both ways.** Delete a chat on the website and Coder deletes it
  too. (#11046)
- **Plain error messages** instead of internal error text. (#11091)

## Desktop (Mac, Linux, Windows, 1.0.0)

Ships with the desktop 1.0.0 release (#11092). Get it from
[openagents.com/download](https://openagents.com/download).

- **Mac:** `.dmg`. **Linux:** AppImage or `.deb`. **Windows:** `.msi` or
  `.zip`.
- **Windows builds aren't signed yet.** Windows SmartScreen asks you to
  confirm: click **More info**, then **Run anyway**. Updates are still checked
  against our signing key before they install.
- **Light and dark.** The app follows your system. Pick System, Light, or
  Dark in Settings > Appearance. (UI-14)
- **Plain words** throughout. (#11031)
- **Updates.** The app checks for a new version when it starts and every six
  hours, and Settings offers **Restart to update**.

## iPhone (TestFlight)

Ships with the TestFlight build for the external testers group (#11090,
#11093). Join: `<TestFlight public link, from the owner>`.

- **Only what works.** We trimmed the app to chat with OpenAgents, your
  earlier chats and your computers' chats, Run Coder, Connect a computer,
  Report a problem, and Appearance. (#11090)
- **Light and dark.** Account > Appearance: System, Light, or Dark. (UI-14)
- **Plain words** throughout. (#11031)

## Android (APK)

Ships as a signed APK on the download page (#11093). Get it from
[openagents.com/download](https://openagents.com/download). There's no Play
Store listing yet.

- Same app as iPhone: chat with OpenAgents, earlier chats and your
  computers' chats, Run Coder, Connect a computer, Report a problem, and
  Appearance. (#11090)
- **Light and dark** follows your phone, or pick one in Account > Appearance.
  (UI-14)
- **Plain words** throughout. (#11031)
