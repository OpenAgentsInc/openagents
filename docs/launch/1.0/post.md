# OpenAgents 1.0 launch post (draft)

Draft for the owner (#11104). Nothing here is posted. Post only after every
platform named in it is live; if one slips, take it out of the post rather
than saying "soon".

## X (one post, 245 characters)

```text
OpenAgents 1.0 is out.

An open network of agents you work with in one conversation, on the web and in your terminal.

Sign in on the web and your chats follow you. Turn on sync and your terminal chats show up there too.

openagents.com/download
```

## Thread or blog version

**1.** OpenAgents 1.0 is out, on the web and in the terminal. Get it at
openagents.com/download.

**2.** OpenAgents is an open network of agents you work with through one
conversation. Each message goes to whichever agent, model, or computer in
the network serves it best; questions about us are answered from our
knowledge base. In the terminal, ask for work on your code and Coder starts
in your project.

**3.** On the web, sign in with GitHub. Your chats belong to your account now,
not a browser cookie. Pin, rename, archive, search, and delete them.

**4.** Connect your GitHub repositories at openagents.com/projects. You pick
which ones. Chats group by project in the sidebar.

**5.** In the terminal, install Coder:

`curl -fsSL https://openagents.com/cli/install.sh | bash`

Then `coder login` and `/sync on`. Your terminal chats show up in the web
sidebar, live. While Coder is open, you can reply to them from the website.

**6.** Sync is off until you turn it on. Before a message leaves your
computer, Coder checks it for keys and passwords and leaves out anything that
looks like one.

**7.** We ask our model providers not to train on your chats and not to keep
them, and we delete usage records after 30 days.

**8.** The code for the website and Coder is open source:
github.com/OpenAgentsInc/openagents. Tell us what breaks.

## Notes for the owner

- The X post above is 245 characters, under 280. X counts the link as 23
  characters, which is also its real length.
- Web and terminal only (owner, 2026-10-09). No desktop, iPhone, or Android
  lines; add the iPhone back once TestFlight review passes.
- The thread says nothing about the Pro plan, environments, agent payments,
  the wallet, or the inference API.
- Every number and comparison here is checked in [claims.md](claims.md).
