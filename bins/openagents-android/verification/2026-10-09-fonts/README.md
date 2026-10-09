# System fonts like the web, 2026-10-09 (#11183)

A debug build (x86_64, not a preview build) on the Android 15 emulator
(1080 × 2400), light and dark, from `--es appearance light|dark`, after the
app stopped bundling Paper Mono. Views draw in the system `sans-serif` face
(Roboto) and code in the system `monospace` face (Droid Sans Mono), the
web's `--font-sans` and `--font-mono` stacks on Android. The transcript is
shaped by Rust with the same two system font files
(`openagents-mobile` `android::transcripts::system_files`).

- `new-light.png`, `new-dark.png`: a new chat with the feature cards.
- `conversation-light.png`, `conversation-dark.png`: the offline chat fixture
  (`--ez chat_fixture true --es coder_send "…"`): the prompt's `inline code`
  in Droid Sans Mono, everything else in Roboto. The Roboto file has no
  italic axis, so the transcript draws *emphasis* upright.
- `drawer-light.png`, `drawer-dark.png`: the drawer (`--ez drawer true`).
- `settings-light.png`, `settings-dark.png`: Settings (`--es tab account`).
- `wallet-light.png`, `wallet-dark.png`: Wallet (`--es tab wallet --ez
  wallet_fixture true`).
