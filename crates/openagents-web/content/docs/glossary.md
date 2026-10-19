# Glossary

The words these docs use, in plain terms.

| Word | Meaning |
| --- | --- |
| **12 words** | The words the phone app makes your account from. They also open your wallet, so one backup covers both. Keep them offline. |
| **Adoption** | When a plugin becomes one of Coder's defaults, after a Better result, three confirming checks by different people, and a Better validation. An OpenAgents operator signs it; it's never automatic. |
| **Auto** | Coder's default choice of who answers a chat: it picks among the agents that are on and logged in. `Shift+Tab` picks one yourself. |
| **Better, No clear change, Worse** | The three verdicts of a plugin test: whether Coder did better with the plugin than without it. |
| **Check** | Rerunning someone else's published result. It confirms the result when you get the same verdict and disputes it when you don't. |
| **Chat** | Talking with OpenAgents. The same chat on the website, the iPhone app, and the Terminal. |
| **Coder** | Our coding agent. It does coding work on your own computer, in your terminal (`coder`) or from the apps, with the coding agents you're logged in to there or its own loop. |
| **Coding agent** | Claude Code, Codex, OpenCode, Devin CLI, Cursor, Grok Build, and others: the programs Coder runs to do the work. Also called engines. |
| **Computer** | A Mac, Linux, or Windows computer where Coder can run, which your phone can connect to. |
| **Custom agent** | An agent you define with a name, instructions, and the coding agent it runs on. Any chat can use it. |
| **Defaults** | The plugins Coder uses for everyone, once their test results hold up. |
| **Device key** | The key a computer makes the first time it runs. It signs its messages and holds the access phones have to it. |
| **Foreman** | Our own custom agent, which looks after the work on the OpenAgents repository. Anyone can start a copy of it. |
| **Grid** | The Verse's shared world, where players walk around live and the Gym stands. |
| **Gym** | Where plugins are tested on Coder, with and without, and the results are published and checked. |
| **Host** | The background helper on a computer that keeps your chats, runs Coder, and answers your phone. `openagents host serve` runs it. |
| **Jev** | A small, fast decision model from TypeSafe. It decides how we reply to each message, and estimates how much of Coder's work is done. It writes no reply text. |
| **Knowledge** | Cited reference entries, such as a method or how a command is used. A plugin can carry it; our product notes are knowledge too. |
| **Level** | Your trainer level, worked out from your XP. Level 2 at 100 XP, 3 at 283, 4 at 520. |
| **Microcoder** | The engine Coder runs its turns through. It is part of Coder: it installs with Coder, with nothing separate to download. |
| **Nostr** | The open protocol of signed records OpenAgents runs on. |
| **npub, nsec** | A Nostr public key and secret key. Share an npub; never share an nsec. |
| **OpenAgents Cloud** | A cloud computer we start for you, where the phone app's coding work can run. |
| **Pairing** | Connecting your phone to a computer by scanning its QR code. |
| **Plugin** | Anything you add to OpenAgents: skills, workflows, knowledge, Wasm, and the tests that show whether it helps. |
| **Prepared answer** | A reviewed answer we show at once when it fully answers a question. |
| **Project** | A Git checkout Coder works in. |
| **Relay** | A server that carries Nostr records. Ours is `relay.openagents.com`; it sees chat messages only encrypted. |
| **Result** | A published plugin test: what Coder passed with the plugin and without it, and the verdict. |
| **Router** | The part of OpenAgents that reads each message first and decides what serves it: a prepared answer, our notes, the chat model, Coder, or a screen. |
| **Skill** | Instructions the agent reads before a task. A plugin made in chat is a skill. |
| **Test, test set** | A test is one task for Coder with checks on how it went. A test set is a plugin's tests. |
| **Thread** | One conversation: one row in your chat list. |
| **Trainer** | You, in the Gym. Your level comes from your XP. |
| **Validation** | A second test set, written by someone else, that shows a plugin helps beyond its author's own tests. |
| **Verse** | The shared world you open in your browser. The Grid is its first place. |
| **Wallet** | The iPhone app's bitcoin wallet, under **Settings → Wallet**. |
| **Wasm** | Small sandboxed code a plugin can carry to do one bounded job. A workflow runs it. |
| **Workflow** | A typed, step-by-step program a plugin carries. |
| **Worktree** | A separate copy of your project on its own branch. Background agents, Solver, and Coder tasks from the Terminal work in one, so your checkout stays as it was. |
| **XP** | Credit for work other people used. It isn't money. |

The full engineering glossary is on
[GitHub](https://github.com/OpenAgentsInc/openagents/blob/main/docs/glossary.md).
