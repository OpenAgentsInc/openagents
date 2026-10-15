# Slash commands

Commands you type in Coder's input. Type `/` to see them above the input;
keep typing to filter, `Tab` to complete, and `Enter` to run one.

## Chats

| Command | Does |
| --- | --- |
| `/resume` | Open an earlier chat from your 100 most recent. `/resume 3` picks from the last list; `/resume ID` opens one by its ID. |
| `/rewind` | Undo a turn's file changes and go back to its message, to edit and send again. Lists your last 20 messages. |
| `/export` | Save this chat to a file. `/export PATH` chooses where. |
| `/compact` | Summarize the older part of the chat so the model reads less. |

## Who answers

| Command | Does |
| --- | --- |
| `/models` | Choose the model and reasoning level of whoever answers, before the chat's first message. |
| `/preferences` | How much Auto favors cost, speed, or intelligence, and your saved choices of agent. |
| `/plugins` | Turn plugins on and off, and add keys. |

## Your account

| Command | Does |
| --- | --- |
| `/login` | Log in to your openagents.com account. |
| `/logout` | Log out of it. |
| `/sync on` | Save every chat on this computer to your account, earlier ones too, and keep saving. `/sync all` does the same. |
| `/sync off` | Stop saving. Chats already on your account stay there. |
| `/sync delete` | Remove this computer's chats from your account. |

## Work in the background

| Command | Does |
| --- | --- |
| `/agent` | Start a background agent in its own worktree: `/agent codex fix the flaky login test`. Alone, lists the agents you can start. |
| `/agents` | Background agents: open, stop, message, or resume one. |
| `/loop` | Repeat a prompt while Coder is open: `/loop 5m check the deploy`. `/loop stop 1` or `/loop stop all` ends them. |
| `/schedule` | Run a prompt on this computer on a schedule, even while Coder is closed: `/schedule weekdays 09:00 triage new issues`. `/schedule remove ID` deletes one. |
| `/shells` | Commands running in the background. `/shells stop ID` stops one. |

## Everything else

| Command | Does |
| --- | --- |
| `/memory` | What Coder remembers. `/memory forget NAME` deletes one note. |
| `/appearance` | Use your terminal's own background color instead of Coder's. |
| `/brainstorm` | Public profile lookups, when the Brainstorm plugin is on. |
| `/help` | Every command and key. |

## In the web chat

| Command | Does |
| --- | --- |
| `/export` | Save the chat to a file in your Downloads folder. |

More on each: [Use Coder in the terminal](/docs/coder-terminal),
[Choose who answers](/docs/coder-agents), and
[Sync, share, and export](/docs/coder-sync).

Next: [Command reference](/docs/command-reference).
