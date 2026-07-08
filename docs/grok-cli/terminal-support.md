# Terminal support

Upstream: https://docs.x.ai/build/cli/terminal-support

Grok draws its interface with terminal escape sequences for color, clipboard,
mouse, and full-screen control. Terminals, multiplexers, and SSH sessions
handle these differently.

**In-session diagnostics:** run `/terminal-setup` (aliases `/terminal-check`,
`/terminal-info`) to see what was detected, which clipboard routes are active,
and issues with fixes.

## Colors look wrong

Set `COLORTERM=truecolor` in your shell profile. Inside tmux, also enable
24-bit RGB:

```text
# ~/.tmux.conf
set -g default-terminal "tmux-256color"
set -as terminal-features ",*:RGB"
set -g set-clipboard on
set -g allow-passthrough on
```

The last two lines also fix clipboard and notification passthrough. Reload:

```bash
tmux source-file ~/.tmux.conf
```

## Copy does not reach my clipboard

Grok writes to:

1. Native OS clipboard
2. tmux paste buffer (inside tmux)
3. OSC 52 for remote cases (SSH, containers, Linux)

Common blockers:

| Environment | Fix |
| --- | --- |
| **iTerm2** | Settings → General → Selection → "Applications in terminal may access clipboard" |
| **Apple Terminal** | Ignores OSC 52 entirely over SSH — use `grok wrap` (below) |

### `grok wrap` (experimental)

```bash
grok wrap ssh user@host
grok wrap docker exec ...
grok wrap kubectl exec ...
```

Runs the command in a **local PTY** that intercepts OSC 52 and writes to
your local clipboard.

## Keyboard chords do not work

| Environment | Notes |
| --- | --- |
| **WezTerm** | Add `config.enable_kitty_keyboard = true` to `wezterm.lua`, restart — fixes `Ctrl+Enter` (interject) and `Shift+Enter` (newline) |
| **VS Code / Cursor / Windsurf / Zed terminals** | Cannot distinguish `Shift+Enter` from `Enter`; use `Alt+Enter` for newlines (also over SSH) |
| **Zellij** | Intercepts many Ctrl chords. On Zellij 0.41+, "Unlock-First (non-colliding)" preset (`Ctrl+O` → `c` → Change Mode Behavior); `Ctrl+G` temporarily unlocks Zellij |
| **Apple Terminal** | `Ctrl+O` interjects (no Kitty keyboard protocol for `Ctrl+Enter`) |

## No fullscreen, or mouse scrolling stops

Grok intentionally runs **inline** under Zellij and tmux control mode
(`tmux -CC`).

| Goal | How |
| --- | --- |
| Force fullscreen | `alt_screen = "always"` under `[terminal]` in `~/.grok/pager.toml` |
| Disable alt screen | `--no-alt-screen` |

If the terminal's native scrollbar takes over, mouse reporting is off:

| Terminal | Enable |
| --- | --- |
| Apple Terminal | View → Allow Mouse Reporting (`Cmd+R`) |
| iTerm2 | Settings → Profiles → Terminal → "Enable mouse reporting" |

Still stuck? Run `/feedback` in the TUI.
