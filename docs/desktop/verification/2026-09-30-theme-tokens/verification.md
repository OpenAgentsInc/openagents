# Desktop theme tokens, dark only, reduced motion

[#10022](https://github.com/OpenAgentsInc/openagents/issues/10022), part of
[#10003](https://github.com/OpenAgentsInc/openagents/issues/10003).

## What landed

- `crates/rust-native-desktop/src/theme.rs` is the one source for the token
  groups. `Theme` carries color (`background`, `text`, `muted`, `rule`,
  `button`, `button_text`, `link`, `focus`, plus `Roles` for transcript
  `ColorRole`s), radius (`card_radius`, `button_radius`, `Radii`), border
  (`Borders`), shadow (`Shadows`, flat: every level is none), opacity
  (`Opacities`), and motion (`Motion`: 150/200/300 ms and the website's
  `cubic-bezier(0.4, 0, 0.2, 1)`). `theme::ladder` holds the OpenAgents white
  ladder (ffffff, c8c8c8, 8a8a8a, 4a4a4a on 0a0a0a, raised 1a1a1a), the
  values CoderOS and the website use.
- Hard-coded values moved to tokens with identical values: the transcript's
  role palette (`transcript.rs` `ink`) and the card outline, checkbox,
  circular control, and focus ring widths, radii, and mixes in `layout.rs`.
- Dark only: the native window asks for `winit::window::Theme::Dark`
  (title bar, traffic lights, Wayland decorations, the Windows caption) and
  nothing reads the system's appearance. `Theme.appearance` is always `Dark`.
- Reduced motion: `theme::motion` holds the system's setting and follows
  the person's "Reduce motion" switch from Settings (#10021,
  `motion::follow(app.reduce_motion())`); motion is reduced when either
  asks. The system's setting comes from macOS
  `NSWorkspace.accessibilityDisplayShouldReduceMotion`, Linux's desktop
  portal `org.freedesktop.appearance` `reduced-motion` then GNOME's
  `enable-animations`, and Windows' `SPI_GETCLIENTAREAANIMATION`.
  `Motion::duration` is zero for `Need::Decorative` under reduced motion;
  the Grid backdrop, the app's only running animation, draws one still frame
  per showing (existing, now also following the Settings switch live).

## No visual change

`openagents-desktop --capture` on `origin/main` (`f2b2a5fc01`) and after the
change, 1× shell and 2× pairing captures. "Reduced" is a second after
capture (pre-rebase build) with reduced motion forced on. "Baseline rerun" is a second `origin/main`
capture: the two connect captures differ between any two runs because each
shows a new random pairing code.

| Capture | Baseline SHA-256 | After | Reduced | Baseline rerun |
| --- | --- | --- | --- | --- |
| `dsk-01-connect.png` | f5ee79b07066 | differs | differs | differs |
| `dsk-01-copied.png` | e18d27aa0991 | differs | differs | differs |
| `dsk-02-connected.png` | a81a25740572 | identical | identical | identical |
| `dsk-02-project.png` | 9593d3b07396 | identical | identical | identical |
| `dsk-03-home.png` | eddad968190e | identical | identical | identical |
| `dsk-03-remove.png` | d34b4f5c9be0 | identical | identical | identical |
| `shell-collapsed.png` | 10de6c292c5b | identical | identical | identical |
| `shell-minimum.png` | 69f945cf5d8a | identical | identical | identical |
| `shell-selected-chat.png` | ce6aa6ea015b | identical | identical | identical |
| `shell-settings.png` | 70033cd10c9d | identical | identical | identical |
| `shell-welcome.png` | c11b7fd42542 | identical | identical | identical |
| `shell-wide-sidebar.png` | 70553d251606 | identical | identical | identical |

After rebasing onto `3a7f088030` (Zeron parity and Settings landed
meanwhile), the same comparison against a fresh `3a7f088030` capture: the ten
deterministic captures are byte-identical (`shell-welcome` 426e1c06fba4,
`shell-selected-chat` 52e16e0b8818, `shell-settings` 5f4248cadb31,
`shell-wide-sidebar` 08f9a77f9d29, `shell-minimum` 7f8bdc610bdb, the rest as
above); only the two random-code connect captures differ.

Dark theme captures: [welcome](shell-welcome.png),
[selected chat](shell-selected-chat.png), [connected](dsk-02-connected.png).
Token snapshot: [tokens.txt](tokens.txt).

## Tests

- `theme::tests::the_openagents_tokens_match_their_snapshot` pins every token
  (`crates/rust-native-desktop/tests/theme_tokens.txt`).
- `theme::tests::the_theme_is_dark_whatever_the_system_says`: every theme is
  dark, the window asks for dark, and the window, Wayland, layout, paint, and
  adapter sources call no appearance API.
- `theme::tests::reduced_motion_zeroes_decoration_and_keeps_what_is_essential`,
  `the_persons_switch_or_the_system_reduces_motion`, `the_portal_reply_reads_as_reduced_or_not`,
  `the_standard_easing_starts_slow_and_lands`.
- `openagents-desktop` `tests::the_shell_stays_dark_and_the_same_under_every_motion_setting`:
  identical shell pixels under all four system/Settings-switch combinations,
  dark field, light text.
- `backdrop::tests::reduce_motion_draws_one_still_frame_per_showing` (existing).
- `cargo test -p rust-native-desktop -p openagents-desktop -p openagents-deck`,
  clippy `--all-targets` clean, `cargo check --target x86_64-pc-windows-gnu
  -p openagents-desktop` clean.

## Not verified here

- A live window on a Mac, Linux desktop, or Windows PC set to Light (dark
  title bar and app) and with the system's reduced-motion switch on (Grid
  camera still). Recorded in the workspace `NEEDS_OWNER.md`.
- The Linux build could not be cross-checked from this Mac (no Linux C
  toolchain for `ring`); the Linux change is the portal read before the
  existing `gsettings` read.
- The roles keep today's Zeron-parity values (#10029); they are not yet
  remapped onto the white ladder, which would be a visual change.
