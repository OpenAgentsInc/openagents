# Existing failures outside playable Grid

These failures were observed while broadening the Grid checks. Their code
was unchanged by #10038, and the affected Grid checks pass separately.

## Mobile terminal paste

`cargo test -p coder-mobile --lib` passed 106 tests, ignored 2, and failed
`terminal_live_tests::the_app_opens_a_shell_on_a_real_host_and_runs_commands`.
The fixture timed out at “a paste is sent as typed text” after approximately
48 seconds. A focused retry of the same test also timed out. It serves a
scratch host; it does not use the owner's resident host.

Reproduce with:

```sh
cargo test -p coder-mobile terminal_live_tests::the_app_opens_a_shell_on_a_real_host_and_runs_commands --lib -- --nocapture
```

The terminal path and its test were not changed. The four shared Grid surface
tests pass, as do the existing mobile presence/body/eval tests in the broad
run. Keep the terminal timeout separate from the desktop player acceptance.

## Verse amber assertion

The initial Verse library run passed 323 tests, ignored 1, and failed
`palette::tests::full_amber_is_the_terminal_amber` at `src/palette.rs:59`:
`0xb0 in linear light, got 1`.

Reproduce with:

```sh
cargo test -p verse --no-default-features --features xp-host --lib palette::tests::full_amber_is_the_terminal_amber
```

The palette implementation and assertion were not changed. The desktop Grid
uses the existing neutral palette. All other Verse library tests pass when
that assertion is excluded, and the unfiltered relay integration tests pass.
