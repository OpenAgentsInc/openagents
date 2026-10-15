# Export and share a chat

Every Coder chat is saved on your computer as you go. You can save one to
a file, open it again later or on another computer, or share it with a
link. This guide covers all three, and saving a web chat.

## Save a chat to a file

Inside Coder, in the chat you want:

```text
/export
```

Coder saves the chat to `~/.openagents/exports/`, shows the file's path,
and copies it when it can. To choose the place, give a path:

```text
/export ~/Desktop/login-fix.json
```

The file is in ATIF, an open format for agent chats: every message,
step, and result, with the tokens, cost, and time of each model call.

## Save an earlier chat

From a shell, list your saved chats with their IDs, then export one:

```sh
openagents coder sessions list
openagents coder export ID --output chat.json
```

To open an earlier chat in Coder instead, type `/resume` and pick it, or
`/resume ID`.

## Open a chat file

```sh
openagents coder import chat.json
```

It opens the chat to read or continue, on this computer or another one.

## Share a chat with a link

A trace is a chat uploaded to your openagents.com account. Log in once
with `coder login`, then:

```sh
coder trace upload --last --share
```

Coder takes out passwords, keys, your home folder's name, and email
addresses before anything leaves your computer, tells you how many it took
out, and prints the trace's address. Anyone with the link can open it.

Without `--share`, the trace is private to you. To share it later, or
stop sharing it, open it from [Settings → Traces](https://openagents.com/settings/traces) on
openagents.com and choose **Share** or **Stop sharing**.
**Delete** removes it and its link. More in [Traces](/docs/traces).

## Save a web chat

In a chat on openagents.com, type `/export` in the message box and send
it. The chat downloads as a file, and its usual place in your Downloads
folder is copied.

## Download everything

```sh
coder export --account
```

Saves everything on your account to one file in this folder: your chats
(each with a Markdown copy), projects, traces, computers, and settings.
Keys, tokens, and passwords are never in it.

Next: [Connect a computer](/docs/connect-a-computer).
