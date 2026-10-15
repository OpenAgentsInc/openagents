# Windows release-candidate checks

The checks each Windows RC of Coder passes before a full RC. They ran on the
owner's Windows 11 machine for rc.16 to rc.20 (#11683), with an OpenCode
model. Install the RC by naming it:
`$env:CODER_VERSION = '1.0.0-rc.N'; irm https://openagents.com/cli/install.ps1 | iex`.

**How they run:** the app is driven through a Windows pseudo console (ConPTY,
the layer Windows Terminal uses). The harness types into the installed
`coder.exe` and records every output chunk with its time. Ctrl+Enter is sent
as the win32-input-mode key event Windows Terminal sends (`VK_RETURN` with
`LEFT_CTRL`), because Coder turns that mode on. Each chat ends with
`/export`. Routing comes from `~/.openagents/coder/routing/<date>.jsonl`.

| # | Check | Passes when |
|---|---|---|
| 1 | Fresh OpenCode setup | With `~/.openagents/coder/opencode/node_modules` moved aside, the rail says "Setting up OpenCode (first time only)", OpenCode answers, and there is no fallback line. |
| 2 | `models set` | `openagents coder models set opencode` makes a new chat run on OpenCode (`rule=pin`); `models set auto` undoes it. |
| 3 | Streaming | The export's `first_token_ms` is well below `duration_ms`, and the text arrives in pieces. |
| 4 | Tools | "read README.md and tell me the first heading": each tool call appears once, with its full input (a `Bash` call has its `command`), and none fail. |
| 5 | Resume | Every turn of the chat stays in one OpenCode session. |
| 6 | Cursor | While Working, the visible cursor stays on the input row. |
| 7 | Headless | `openagents coder chat --mode opencode --json` streams `delta` events; `--session` resumes. |
| 8 | Keys | The hint reads "Enter to send, Ctrl+Enter to queue for later". Ctrl+Enter during a turn queues the message; Enter stops the turn and sends now. |
| 9 | Worktree | In a Git repo without `--in-place`, `chat` writes in `.coder/worktrees/chat-…` and resumes there. The repo's own tree is untouched. |
| 10 | Coder loop → harness | On `auto`, `hi` answers on the Coder loop; repo work after it moves to OpenCode with the conversation, and the rail says so. |
| 11 | Pinned chat | A `/models` pick before the first message holds for every turn. A pick mid-chat says "Start a new chat (Ctrl+N) to use another model". |
| 12 | Rail | The rail names the model the export shows for each turn. |
| 13 | Write guard | From a chat's worktree, `cmd /c "echo x > %CD%\..\..\..\shared.txt"` is refused with a plain reason, and no `shared.txt` appears in the repo. |

**What check 13 covers:** a chat's fence covers only the shared checkout
(owner, #11720). A write elsewhere on the disk, such as
`%TEMP%\..\..\outside.txt`, is allowed on purpose. Solver and issue-run hands
fence the whole disk.

Unit tests: `cargo test -p coder-new --lib` and `cargo test -p coder-sdk --lib`
pass on Windows. See [verification](../verification.md#building-and-testing-on-windows)
for building there.
