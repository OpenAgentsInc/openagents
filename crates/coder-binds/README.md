# coder-binds

One table of desktop chords and one table of window rules, for the
CoderOS compositor, CoderQuest, and Coder Desktop.

`BINDS` holds a row per chord: modifiers, key, action, and the surfaces
the chord applies to. `hyprland_lines` renders the compositor rows as the
`bind`, `binde`, and `bindm` lines
[`os/modules/coderos/desktop.nix`](../../os/modules/coderos/desktop.nix)
writes for Hyprland, a window that handles its own chords reads the rows
for its surface with `find`, and `appendix_markdown` renders the table as
Markdown for a document that lists the chords.

A row spells the desktop modifier `Super`, and `src/modifier.rs` decides
which physical modifier that is where the row is read. The compositor and a
window under it read Super. A window on macOS reads Control and Option,
because macOS answers Command+H, Command+Q, Command+M, Command+W, and
Command+Space before the window does; a row that also holds Ctrl reads it
as Command, so Super+Ctrl+Left is Control+Option+Command+Left. Coder
Desktop stays on Command, because it is a Mac application rather than a
window manager. `coder_binds::press` is what a window calls, and the drift
test keeps the Mac reading one chord a row.

`RULES` holds a row per window rule: what it matches, on the app-id or the
title, and the effects the desktop applies. A match is a literal, the
whole string or its start, with ASCII case either exact or ignored.
`hyprland_rule_lines` renders the rows as the `windowrule` lines
`desktop.nix` writes in Hyprland 0.55's syntax, and turns each literal
into the regular expression Hyprland reads; nothing else in the repository
holds one. `matching` folds the effects of every rule a window matches,
which is what the Coder compositor applies when a window maps and when
its app-id or title changes.

| Rule | Matches | Effects | Option |
| --- | --- | --- | --- |
| Android emulator | app-id equal to `coderos.desktop.android.windowClass`, `Emulator` by default | float, keep the aspect ratio | `android` |
| Battle.net launcher | app-id `battle.net.exe`, `Battle.net.exe`, or `steam_app_battlenet` | float, center | `battlenet` |
| World of Warcraft client, by class | app-id starting with `wow` or `world of warcraft`, or a `steam_app_` app-id holding `wow`, in any case | tile, suppress fullscreen and maximize | `battlenet` |
| World of Warcraft client, by title | title starting with `World of Warcraft`, in any case | tile, suppress fullscreen and maximize | `battlenet` |
| StarCraft II client, by class | app-id starting with `sc2` or `starcraft`, or a `steam_app_` app-id holding `sc2`, in any case | tile, suppress fullscreen and maximize | `battlenet` |
| StarCraft II client, by title | title starting with `StarCraft II`, in any case | tile, suppress fullscreen and maximize | `battlenet` |
| Camera circle | title `selfie` | float, keep the aspect ratio, border 0, no shadow, pin | `camera` |
| Recording HUD | title `recording-hud` | float, border 0, no shadow, pin | `screenRecording` |

The option column names the `coderos.desktop` option that gates the
Hyprland copy of the rule. The compositor applies every rule, because a
window only matches when its launcher ran.

[`tests/binds.rs`](tests/binds.rs) checks that every CoderQuest row has a
Mac chord of its own. It also renders the bind lines and the rule lines and
fails when `desktop.nix` disagrees. Those comparisons are ignored until
`os/modules/coderos/desktop.nix` moves into this repository; run them with
`cargo test -p coder-binds -- --ignored`. Change a row here, then change the
text in `desktop.nix` to what the test prints.

Run `cargo run -p coder-binds --example gen` for the chord table as
Markdown.
