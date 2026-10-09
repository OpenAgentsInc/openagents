# Android shell (#11126), 2026-10-09

A debug build (x86_64, not a preview build) on the Android 15 emulator
(1080 × 2400), light and dark, from `--es appearance light|dark`. The same
shell as the iPhone app (`bins/openagents-ios/verification/2026-10-09-shell/`),
from the same Rust state.

- `new-light.png`, `new-dark.png`: a new chat in Chat mode. The top bar has
  the menu and the **Chat** / **Code** switch; the four feature cards fan
  out to swipe through, with the middle card's headline, line, and **Try
  it**, over **Ask OpenAgents**. The floating bar at the left in the light
  shot is the emulator keyboard's own toolbar, not the app.
- `code-light.png`, `code-dark.png`: Code mode with no computer: **Connect a
  computer** and where Coder works, over **Work with Coder**.
- `drawer-light.png`, `drawer-dark.png`: the drawer (`--ez drawer true`):
  search, Coder, Computers, Wallet, Settings (no Verse outside a preview
  build), the recent chats, the **Chat** pill, and the account button.
- `conversation-links-light.png`, `conversation-links-dark.png`: a live
  reply naming two links. Each link gets a card with the page's preview
  picture, title, and site. The reply scrolls under the top bar and the
  floating composer. Tapping a card opens the page in the browser.
