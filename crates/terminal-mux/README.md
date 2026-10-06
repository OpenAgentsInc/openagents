# Optional TTY multiplexer

`openagents-mux` draws explicitly named durable host terminals with ratatui.
It uses the paired-device store and the same admitted session transport as
native terminal mounts. It creates no shell, pairs no device, and reads no
default home directory. Leaving the client detaches; it never closes or
signals the host terminal.

```sh
cargo run -p terminal-mux --features host --bin openagents-mux -- \
  --store PAIRED_DEVICE_STORE --host HOST_PUBLIC_KEY \
  --reference GENERATION:TERMINAL --reference GENERATION:OTHER_TERMINAL
```

Use the retained terminal references from an admitted host session. A host
restart invalidates these references; this client does not open replacements.

Press `Ctrl+B`, then `n` or `p` to switch tabs, `%` or `"` to split the
current and next tabs, `o` to move focus, `z` to zoom, `1` through `8` to
select a tab, `k` or `j` to navigate retained command blocks, `r` to return
to live output, or `d` to detach. `t`, `u`, and `f` show typed thread, run,
and artifact fallbacks without opening resources. Press `Ctrl+B` twice to send one prefix
to nested tmux. Paste, application cursor keys, and mouse reports use the
inner terminal's modes. The host still checks the current typist and grant
on every input; unavailable input is discarded.

This optional projection requires cursor-addressed redraw. `TERM=dumb`
refuses before changing terminal modes. The client crops a larger host grid
and supports at most eight attached terminals and two visible panes. It
polls with a shared 64 KiB and 8 ms output budget and draws at about 30 Hz.
Unicode and color rendering depend on the outer terminal. Graphics,
hyperlink activation, clipboard writes, and interactive resource cards are
unavailable; use retained thread, run, or artifact clients for those resources.
Host status remains visible above retained output during transport loss.

Run `cargo test -p terminal-mux` for isolated fixtures. The outer PTY fixture
uses two scratch host PTYs, a nested full-screen program, split/tab controls,
and detach/reattach with unchanged process groups. Other fixtures check
query ownership, paste and mouse modes, refused input, bounded redraw, and
terminal-mode restoration on unwind. No owner host or real home is used.
