# Coder terminal mockup

A standalone Ratatui screen preview for the new Coder terminal. The default
view shows a sample conversation, compact tool calls, a diff, and a composer.
Press Tab to switch to the welcome view.

```sh
cargo run -p coder-new
cargo run -p coder-new -- --welcome
```

Type a draft, move with Left/Right or Home/End, and edit with Backspace/Delete.
Alt+Enter adds a newline. Enter appends a local preview message. Bracketed paste
inserts text without sending it. PageUp/PageDown scroll the conversation;
Ctrl+C quits. The preview makes no network requests, runs no tools, and saves
no messages. All conversation, plugin, and wallet values are sample data.

All colors are exact 24-bit RGB values from Grok Build's default Grok Night
theme, including the focused composer border. Use a truecolor terminal to
display them exactly. [NOTICE](NOTICE) records the source commit, `SOURCE_REV`,
upstream source file, and license. The reference clone is at
`/workspace/grok-build` in this cloud workspace.

Export the actual Ratatui buffer without an interactive terminal:

```sh
cargo run -p coder-new -- --snapshot > docs/coder-new/mockup.svg
cargo run -p coder-new -- --welcome --snapshot > docs/coder-new/welcome.svg
```

The [research index](../../docs/coder-new/README.md) contains the history and
architecture inputs for the future specification.
