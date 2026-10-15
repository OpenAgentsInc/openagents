# Follow a run from your phone

Start a long task in Coder on your computer, walk away, and keep up with
it from your phone: read its steps and send a reply that Coder picks up
on your computer. This works in your phone's browser, with nothing to
install.

## 1. Set up your computer once

In a terminal on the computer where Coder runs:

```sh
coder login
```

Approve the code at [openagents.com/device](https://openagents.com/device).
When Coder asks where this computer's chats should live, choose **Sync all
my chats** (`1`). If you chose to keep them on this computer before, type
this inside Coder:

```text
/sync on
```

Messages that look like they hold a password or key are never sent.

## 2. Start the work

Open Coder in your project and give it the task:

```sh
cd ~/code/my-project
coder
```

```text
Upgrade the project to the new major version of the web framework,
fix what breaks, and run the tests until they pass.
```

Leave Coder open: a reply from your phone reaches it only while it's
running on that computer.

## 3. Open it on your phone

1. On your phone, open [openagents.com](/) and log in with the same
   account.
2. Tap **Open menu** at the top left of [openagents.com](/) to see your
   chats. A chat from Coder is marked with your computer's name.
3. The chat shows Coder's steps: its edits, its commands, and their
   results.

## 4. Reply from your phone

Write your reply in the chat's message box and tap **Send** (`Enter` on a keyboard). Coder picks up your reply as soon
as it's free, shows it in the chat on your computer as your message, and
answers with that computer's files and commands. The answer shows on your
phone as usual.

When Coder isn't open on that computer, the page says so and offers no
reply box. Open Coder again in the same folder and type `/resume` to pick
the chat back up.

## Steps that wait for you

A deploy, a merge, a secret, a payment, or a DNS change waits for your
`Y` or `N` at the computer. If nobody is there to answer, the agent
finishes what it can and hands that step back to you. See
[Safety](/docs/coder-safety).

## The iPhone app

[OpenAgents for iPhone](/docs/iphone) is in beta. Once your phone is
connected to your computer, it can also start Coder there, queue messages
for the next turn, and stop a run. See
[Watch, steer, and stop Coder](/docs/following-coder).

Next: [Use Coder on Windows](/docs/coder-on-windows).
