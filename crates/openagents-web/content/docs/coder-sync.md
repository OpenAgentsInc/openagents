# Sync, share, and export

Coder keeps your chats on your computer. Logged in (`coder login`), it
can keep them on your openagents.com account too, so you can read and
answer them on the web and on your phone.

## Log in

| To | Run |
| --- | --- |
| Log in from the terminal | `coder login`, then approve the code at [openagents.com/device](https://openagents.com/device) |
| Log in from inside Coder | `/login` |
| Log in with a key, without a browser | `coder login --key`, then paste a login key from your account's Settings (or set `OPENAGENTS_TOKEN`) |
| Sign this computer out | `coder logout`, or `/logout` in Coder |

The website's [Connect your terminal](https://openagents.com/settings/terminal)
page walks through the same steps: it shows the install command and a
`coder login --pair` command, and you approve the login on that page.

## Turn sync on

After you log in, Coder asks once where this computer's chats should
live: **Sync all my chats** or **Keep chats on this computer**. Answer in
the terminal or on the website. Change it any time:

| Command | Does |
| --- | --- |
| `/sync on` | Save every chat on this computer to your account, earlier ones too, and keep saving. Memory notes sync too. |
| `/sync off` | Stop saving. Chats already on your account stay there. |
| `/sync delete` | Remove this computer's chats from your account. |
| `coder sync` | Send every chat here now, background agents' chats included (sync must be on). |

Sync is off until you choose it. A Coder run with its own folder
(`--state DIR`) doesn't sync until you turn sync on for that folder. Messages that look like they hold a
password or key are never sent. Deleting a synced chat in one place
deletes it in the other.

## Read and reply on the web and phone

Synced chats appear in the sidebar on [openagents.com](/), marked with
this computer's name, and in the phone app's chat list.

While Coder is open on that computer, you can reply there: Coder picks up
your reply as soon as it's free, shows it in the chat as your message,
and answers with this computer's files and commands. The answer appears
on the web and phone as usual. When Coder isn't open, the page says so
and offers no reply box.

To start a new chat inside one of your projects on openagents.com from
the terminal, ask Coder ("make a new chat in my finances project") or run
`openagents chat send "MESSAGE" --project finances`. It's an
openagents.com chat, so it shows in that project on the website and phone.

A chat Coder ran in a checkout of a GitHub repository goes under your
project for that repository (Projects in the sidebar, and on your phone).
Move a chat to another project, or out of one, from its menu on the
website or from the terminal:

| Command | Does |
| --- | --- |
| `openagents projects move CHAT PROJECT` | Move a chat into a project. CHAT is its id, the start of its id, or its title; PROJECT its name, repository, or id. |
| `openagents projects move CHAT --none` | Take a chat out of its project. |

A chat you moved stays where you put it.

With sync on, prompts you set up with `/schedule` also show on your
account, where you can pause or delete them.

## Save a chat to a file

| To | Run |
| --- | --- |
| Save this chat | `/export`, or `/export PATH` |
| Save a saved chat by its ID | `openagents coder export ID --output chat.json` |
| List saved chats and their IDs | `openagents coder sessions list` |
| Open a chat file to read or continue it | `openagents coder import chat.json` |

Exports use ATIF, an open format for agent chats: every message, step,
and result, with tokens, cost, and time for each model call. Coder
shows the file's path when it's done and copies it when it can.

## Share a chat as a trace

A trace is a chat uploaded to your account, which you can look through on
the website and, if you choose, share with a link.

```sh
coder trace upload --last            # your latest chat
coder trace upload --last --share    # with a public link
coder trace list                     # your traces
```

Passwords, keys, your home folder's name, and email addresses are taken
out first. See [Traces](/docs/traces).

## Download everything

```sh
coder export --account
```

Saves everything on your account to one file in this folder: your chats
(each with a Markdown copy), projects, traces, computers, and settings.
Keys, tokens, and passwords are never in it.

Next: [Coder from scripts](/docs/coder-scripts).
