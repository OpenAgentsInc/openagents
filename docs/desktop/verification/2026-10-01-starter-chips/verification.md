# Desktop new chat: starter chips above the centered composer (#10097)

A new, empty chat shows the phone's starter suggestions
(`openagents_chat_app::first_run::SUGGESTIONS`, the first
`SUGGESTIONS_SHOWN` not yet used: Who are you?, What can you do?, What's
new in the Gym?, Test a plugin) as pill chips in a wrapping row directly
above the centered composer: the same row, 8pt spacing, and style as the
latest reply's follow-up chips above the docked composer (#10075). The
chips and composer are centered together. A tap sends the suggestion's
words. Once the chat has a message the starters go, and the latest reply's
follow-ups take the row.

Debug builds only; nothing packaged, signed, or installed.

## Headless captures (`cargo test -p openagents-desktop -- starter`)

- `starters-default-1200x840.png`: one row above the centered composer.
- `starters-minimum-760x540.png`: the row wraps onto two lines, inside the
  column.

Tests: `shell::card_fixtures::starters_are_chips_above_the_centered_composer_until_a_message`
(placement, order, size, wrap, tap sends, chips leave, follow-ups take the
row), `shell::access_tests::a_screen_reader_reaches_and_presses_a_starter_chip`
(AccessKit button named by its words, read before the composer, focusable,
pressable), and `empty_chat_centers_the_draft_and_a_real_reply_docks_it`
(the chips plus composer stay centered).

## Acceptance gate (`ui-starter-chips`)

`scripts/release/acceptance.sh --bin-dir <debug> --no-engines --only ui-starter-chips,ui-chips,ui-placeholder`:

| Scenario | Result | Evidence |
| --- | --- | --- |
| ui-placeholder | PASS | centered composer paints its placeholder (1452 faint pixels) |
| ui-starter-chips | PASS | 4 starters (Who are you?, What can you do?, What's new in the Gym?, Test a plugin) directly above the centered composer, on 1 line(s) at 1200x840 and 2 at 760x540; a tap sent "Who are you?"; after the reply none remain and 3 follow-up chips sit above the docked composer |
| ui-chips | PASS | 3 chips in a row 8pt above the composer; none in the transcript |

Gate captures: `gate-starters-default-1200x840.png`,
`gate-starters-minimum-760x540.png`, `gate-after-reply-default-1200x840.png`.
