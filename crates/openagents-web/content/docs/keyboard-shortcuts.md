# Keyboard shortcuts

Every key in Coder, in the web chat, and in the screens they open, in one
place. Type `/help` in Coder to see the short list there.

## Coder: sending

| Key | Does |
| --- | --- |
| `Enter` | Send now, even while a reply is being written. With an empty input, sends the next queued message now. |
| `Ctrl+Enter` | Queue for later: the message runs when the reply ends, in the order you queued. With nothing running, it sends at once. |
| `Ctrl+Q`, `Ctrl+J` | Queue for later, in terminals that send `Ctrl+Enter` as a plain `Enter`. |
| `Shift+Enter`, `Alt+Enter` | A new line. |
| `Esc` with messages queued | Put the queued messages back in the input to edit or remove. |
| `!` on an empty input | Run a shell command in your folder, such as `!git status`. `Backspace` on the empty input goes back to a message. |
| `@path/to/file` | Attach a file to your message. |

## Coder: the chat

| Key | Does |
| --- | --- |
| `Esc` | Stop the reply. Closes a list or screen. Twice on a draft clears it. |
| `Shift+Tab` | Before a chat's first message: choose who answers (Auto, Claude Code, Codex, OpenCode, or another agent here). |
| `Tab` | Complete a `/` command. |
| `Up`, `Down` | Your earlier messages. `Down` from the input selects the agent rows under it. |
| `Ctrl+N` | Start a new chat in the same folder. The last one stays in `/resume`. |
| `Ctrl+O` | Show each step in full, or fold the steps back to one line each. |
| `Ctrl+P` (`Cmd+P` on macOS), `F2` | Open plugins. |
| `PageUp`, `PageDown`, mouse wheel | Scroll the chat. |
| `Ctrl+End` | Jump to the bottom of the chat. |
| `Ctrl+A`, `Ctrl+E` | Start or end of the line. |
| `Cmd+A` | Select the whole input, where the terminal passes `Cmd` keys on. In Terminal, iTerm2, and Ghostty, use `Ctrl+Shift+A`. |
| Drag | Copy text from the chat. Double-click copies a word, triple-click a line. |
| Click a message | Shows the model that wrote it for about three seconds. |
| `Ctrl+C` | Quit. The chat is saved. |

## Coder: a step that waits for you

| Key | Does |
| --- | --- |
| `Y` | Allow it, this one time. |
| `N`, `Esc` | Say no. |
| `PageUp`, `PageDown` | Read the whole command. |

## Coder: lists and screens

| Screen | Keys |
| --- | --- |
| `/models` | Type to search, `Up` and `Down` to choose, `Enter` to go on, `Esc` to step back. |
| `/plugins` | `Up` and `Down` to choose, `Space` to turn on or off, `Enter` for its settings, `Tab` between fields, `Esc` to go back. |
| `/resume` | `Up` and `Down` (or `j` and `k`) to choose, `Enter` to open, `Esc` to go back. |
| `/preferences` | `Up` and `Down` to move, `Left` and `Right` (or `-` and `+`) to change a value. |
| `/agents` | `Up` and `Down` to choose, `Enter` to open, `s` to stop, `m` to message, `r` to resume, `Esc` to close. |
| Agent rows under the input | `Down` from the input to select, `Up` and `Down` to move, `Esc` (or `Up` from the first) back to your chat. |

## The web chat

| Key | Does |
| --- | --- |
| `Enter` | Send now. While a reply is being written, stops it and sends yours. |
| `Ctrl+Enter` (`⌘+Enter` on a Mac) | Queue for later. |
| `Shift+Enter` | A new line. |

Type `/export` in the [web chat](/new)'s message box and send it to save
the chat to a file.

## The Verse in your browser

In [Everglade](/everglade), as on the Mac:

| Key | Does |
| --- | --- |
| `W`, `S` | Move forward and back. |
| `A`, `D` (or `Q`, `E`) | Step sideways. |
| `Space` | Jump. |
| `Shift` | Sprint. |
| Right mouse button | Hold to look and turn. |
| Left mouse button | Hold to orbit. |
| Mouse wheel | Zoom, into first person and back. |
| `Esc` | Close a board, then release the mouse. |

Next: [Slash commands](/docs/slash-commands).
