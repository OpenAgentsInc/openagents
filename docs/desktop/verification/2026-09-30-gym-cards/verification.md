# Desktop Gym and evals through chat (#10020)

Part of #10003. A desktop chat whose reply carries the router's Gym cards
and `start_eval` / `open_screen` offers now shows the same cards and sheets
as the phone, and its buttons do the same thing, from shared Rust.

## What is shared

- `openagents_chat_app::gym::Gym::tap`: what a Gym button does, moved out of
  the phone's `CoderTab::gym_tap`. The phone and the desktop both call it;
  they differ only in how they carry out the returned `gym::Effect`.
- `Gym::open_screen`: the `GymResult` / `GymPublish` / `GymTestSet`
  offers open the same sheet on both.
- `openagents_chat_app::cards::sheet`: mounts the phone's `SheetView`
  (`SCR-05`, `SCR-06`, `SCR-11`, `SCR-20`, `SCR-21`, stop confirmation)
  as a card under the reply. `Cards::rows_with` adds it after the Gym cards.
- `Session::card_action` sends Gym buttons through `Gym::tap` (even while a
  reply streams) and maps the effect: send in this chat, new chat, compose,
  Computers, the Verse Gym (the read-only Watch), or a Coder handoff.

Routing is typed: button IDs minted by the last view map to
`eval_cards::Action`, and offers are `router::Offer`. No text matching.

## What the desktop does

`crates/openagents-desktop/src/chat.rs` carries out the Coder effects
additively: a run on the ready computer starts as this chat's local Coder
run with the phone's own `openagents ext eval` prompt (`Gym::on_computer`
records it), Stop and Add to the Gym go to that run, and Open Coder selects
its chat.

## Checks

- `cargo test -p openagents-chat-app gym::tests::desktop`: the phone
  (`CoderTab`) and the desktop (`Session`) show the same tool, run, and
  draft card values and the same actions per button; Start with no runner
  and no computer leaves the same refused run card; Connect a computer goes
  to Computers on both; with a ready computer the desktop hands Coder the
  same prompt as the phone; Stop opens the same sheet and confirms with a
  stop to that run; See every test opens the same `SCR-21` sheet, and
  Looks good sends "Looks good" in the chat.
- `cargo test -p openagents-desktop --bin openagents-desktop gym_card_tests`:
  headless window, real painted clicks on START THE TEST, Connect a
  computer, See every test, Close, and LOOKS GOOD.
- Screenshots (`OPENAGENTS_GYM_CAPTURE_DIR=… `, 1200×840 at 2×):
  `dsk-gym-01-tool-card.png`, `dsk-gym-02-run-card.png`,
  `dsk-gym-03-draft-card.png`, `dsk-gym-04-test-set-sheet.png`.
- Existing suites green: `openagents-chat-app` (120), `openagents-desktop`
  (113 lib, 65 bin), `openagents-mobile` (150); clippy `-D warnings` clean.

## Not covered here

- The desktop Gym has no hosted runner and no store yet (`Gym::empty()`), so
  a run with no ready computer is refused as on a phone without a runner,
  and runs do not survive a restart. Hosted runs need the phone's
  `hosted.rs` runner (in `openagents-mobile`) moved to shared code and a
  desktop trainer key.
- No live run against a real computer or Coder was made.
