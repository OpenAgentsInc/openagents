# `coder-desk`

The desk protocol from the command line. One subcommand per verb, over
[`crates/coder-desk`](../coder-desk/README.md), so a script asks
what is on a person's screens and changes one window without naming a
compositor.

The CoderOS scripts under `os/bin/` run this command where they used to
run `hyprctl`. Fourteen of them held Hyprland's verb spellings, JSON field
names, and selector grammar, and each would have changed again when the
compositor did.

The package is `coder-desk-cli`, because `crates/coder-desk` is already
`coder-desk` in this workspace. The binary it installs is `coder-desk`.

## The verbs

| Subcommand | What it does |
| --- | --- |
| `list` | Every window the session holds, as a JSON array. |
| `screens` | Every screen the session holds, as a JSON array. |
| `focused` | The window that has the focus, as JSON, or `null`. |
| `reading` | The screens, the name of the one the focus is on, and the desks. |
| `open` | Start a program in a new window, or show a file in a new pane. |
| `focus` | Give one window the focus. |
| `place` | Move one window to a desk, without switching the screen to it. |
| `raise` | Raise one window above the others on its desk. |
| `close` | Close one window. |
| `shape` | Change how the desk draws one window. |
| `scale` | Set the scale one screen draws at. |
| `notice` | Raise a notice the operator reads. |
| `reload` | Ask the session to reload its configuration. |
| `key` | Press one chord, such as `super+t` or `return`, the way a press on the keyboard runs. |
| `type` | Type text into the focused window. |
| `click` | Press a pointer button at a point on the screens, on the window under it. |
| `move` | Move the pointer to a point on the screens, in one jump or in the steps `--steps` and `--ms` name. |
| `press` | Hold a pointer button or a key down: `left`, `right`, and `middle` are the buttons, and every other word is a key or a modifier. `--key` reads the word as a key, so `press left --key` presses the arrow. |
| `release` | Let a pointer button or a key go, by the same words. |
| `drag` | Press a button at one point, move to another, and let it go, with `--modifier` held across it. |
| `scroll` | Scroll where the pointer is, a trackpad's smooth axis or a wheel's notches with `--discrete`. |
| `shot` | Write a PNG of one screen, and answer once the file is written. |
| `status` | Which backend answers here, the socket it answers on, and whether hands drive the desk. A desk that does not know the `status` verb prints the first two alone. |

`list`, `screens`, `focused`, and `reading` print JSON on stdout, so a
script reads them with `jq`. The rest print nothing and say whether the
change went through with their exit status.

`reading` answers the two facts generation 1's `Screen` row does not carry:
which screen has the focus, and the desks with their window counts. A caller
that needs the focused screen's scale reads it there.

## Naming a window

A selector is the contract's grammar and nothing else: a handle the desk
printed, `class:<app-id>`, or `title:<title>`. A regular expression never
reaches the desk. A caller that wants one reads `list` and matches there,
which is how a script finds a program with several app-ids.

```sh
coder-desk focus class:chromium-browser
coder-desk shape title:selfie --float --size 360x360 --at 2140,60
coder-desk open --desk 3 --silent -- 'foot -e coder'
```

`--pin` shows a window on every desk. A session whose pin toggles cannot
unpin a window by name, so a caller that wants one unpinned sends `--pin`
again; the protocol refuses the `false` a name would carry.

## Exit status

| Status | What happened |
| --- | --- |
| 0 | The desk answered. |
| 2 | The command line was wrong. |
| 3 | There is no desktop session here. |
| 4 | A session was announced, and its desk did not answer. |
| 5 | The desk refused the request, and stderr says why. |
| 6 | The desk answered something this command cannot read. |

A run reached over SSH and a host with no compositor exit 3, which is the
status a script branches on when it decides whether the desktop is there at
all.

## Tests

`tests/cli.rs` runs the command against
`coder_desk::fake::Session`, which holds windows and screens in memory,
answers the protocol on a socket of its own, and records every request. Each
test reads the JSON the scripts read and the exit status they branch on.

```sh
cargo test -p coder-desk-cli
```
