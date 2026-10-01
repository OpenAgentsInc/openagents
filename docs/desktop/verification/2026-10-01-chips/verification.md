# Desktop follow-up chips above the composer (2026-10-01)

Issue: [#10075](https://github.com/OpenAgentsInc/openagents/issues/10075).
The owner reported from the Mac build at `65f0a50e77` that a reply's
follow-up suggestions drew as full-width buttons stacked in the transcript.

## What changed

| Request | Now | Test |
| --- | --- | --- |
| Follow-ups as small chips above the composer | The latest reply's follow-ups (`coder-followup-N`) leave the transcript. The footer draws them in a wrapping row (`chat-followups`) directly above the composer card: pill buttons with the `ask` glyph, 13-point text, sized to their words, on the dark theme's `SELECTED` fill with `TEXT` labels and a `MUTED` glyph. The row sits in the footer, so it shows the same way whether the composer is docked or centered. | `followups_are_chips_above_the_composer_for_the_latest_reply_only` (default 1200×840 and minimum 760×540, 2x) |
| Wrap at the minimum window | Chips flow onto more rows, each inside the column and as wide as its words. | `followup_chips_wrap_at_the_minimum_window` |
| A tap sends the suggestion | A chip is the same card action as before: it sends its words and records the follow-up as used. The chips leave while the reply to one is coming. | `followups_are_chips_above_the_composer_for_the_latest_reply_only`, `cards_mount_paint_and_admit_only_their_current_buttons` |
| Only the latest reply | Follow-ups come from `projection::actionable`, which reads only the last completed reply. An earlier reply's follow-up is neither a chip nor a transcript row. | `followups_are_chips_above_the_composer_for_the_latest_reply_only` |
| Keyboard and screen readers (#10024) | Each chip is a button named by its words, which admits focus and click, and reads before the composer. A screen reader's click sends it. | `a_screen_reader_reaches_and_presses_a_followup_chip` |
| Other card buttons against the phone | In transcript rows, a `pill` button (Run Coder, Connect a computer, Open Wallet, a command's Run, Try again) or a button with `intrinsic_width` is now as wide as its label, as on the phone. A Gym card's outlined buttons and chips, and a Gym sheet's other choices and close control, are content-sized. A Gym card's or sheet's one primary button (START THE TEST) still spans the card, as the phone's does. Stop Coder is the composer's icon control and was already compact. | `pill_and_intrinsic_buttons_hug_their_labels` (`crates/rust-native`); the Gym card and Coder offer captures below |

## Captures (headless, real shell)

Written by the tests with `OPENAGENTS_CHIPS_CAPTURE_DIR`,
`OPENAGENTS_HANDOFF_CAPTURE_DIR`, and `OPENAGENTS_GYM_CAPTURE_DIR` set:

- `chips-default-1200x840.png`, `chips-minimum-760x540.png`: the owner's
  three follow-ups above the composer.
- `chips-wrap-default-1200x840.png`, `chips-wrap-minimum-760x540.png`: five
  follow-ups, wrapping at the minimum window.
- `run-coder-default-1200x840.png`, `run-coder-minimum-760x540.png`: Run
  Coder as a content-sized chip under its reply.
- `gym-card-default-1200x840.png`, `gym-sheet-default-1200x840.png`: a Gym
  card with its full-width START THE TEST and content-sized choices, and a
  Gym sheet.

## Checks

- `cargo test -p openagents-desktop -p openagents-chat-app -p rust-native -p rust-native-desktop`
- `cargo clippy -p openagents-desktop -p openagents-chat-app -p rust-native -p rust-native-desktop --all-targets -- -D warnings`
- `cargo fmt --check` on the changed packages

The phone's chat draws its chips outside its transcript and is unchanged.
The shared row layout change applies to `pill` and `intrinsic_width`
buttons inside any transcript.
