# OpenAgents 1.0 launch post (draft)

Draft for the owner (#11104). Nothing here is posted. Post only after every
platform named in it is live; if one slips, take it out of the post rather
than saying "soon".

## X (one post, 275 characters)

```text
OpenAgents 1.0 is out.

One agent you can chat with on the web, in your terminal, on your desktop (Mac, Linux, Windows), and on iPhone and Android.

Sign in on the web and your chats follow you. Turn on sync and your terminal chats show up there too.

openagents.com/download
```

## Thread or blog version

**1.** OpenAgents 1.0 is out. Web, terminal, desktop (Mac, Linux, Windows),
iPhone, and Android. Get it at openagents.com/download.

**2.** OpenAgents is a composable general agent. You chat with one agent. It
answers from our knowledge base. In the terminal or the desktop app, ask it
to work on your code and it starts Coder in your project.

**3.** On the web, sign in with GitHub. Your chats belong to your account now,
not a browser cookie. Pin, rename, archive, search, and delete them. Delete
means delete.

**4.** Connect your GitHub repositories at openagents.com/projects. You pick
which ones. Chats group by project in the sidebar.

**5.** In the terminal, install Coder:

`curl -fsSL https://openagents.com/cli/install.sh | bash`

Then `coder login` and `/sync on`. Your terminal chats show up in the web
sidebar, live. While Coder is open, you can reply to them from the website.

**6.** Sync is off until you turn it on. Before a message leaves your
computer, Coder checks it for keys and passwords and leaves out anything that
looks like one.

**7.** The desktop app follows your system's light or dark setting. Windows
builds aren't signed yet, so SmartScreen will ask you to confirm. Updates are
still checked against our key.

**8.** On iPhone it's TestFlight for now (link). On Android it's an APK on
the download page. We cut the phone app down to what works today.

**9.** We ask our model providers not to train on your chats and not to keep
them, and we delete usage records after 30 days.

**10.** Everything is open source:
github.com/OpenAgentsInc/openagents. Tell us what breaks.

## Notes for the owner

- Character count of the X post was counted on the text block
  above (275, under 280; X counts the link as 23 characters, its real length).
- The thread says nothing about the Pro plan, environments, agent payments,
  the wallet, or the inference API.
- Item 8 needs the TestFlight public link. If the iPhone or Android build
  isn't out on launch day, cut item 8 and the phones from item 1 and the X
  post.
