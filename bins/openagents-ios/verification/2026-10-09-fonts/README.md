# System fonts like the web, 2026-10-09 (#11183)

The iOS simulator build (`build.sh sim`, iPhone 16 Pro, iOS 26.5, no
`OPENAGENTS_MOBILE_PREVIEW`) after the phone stopped bundling Paper Mono.
Text draws in SF Pro and code in SF Mono, the first families of the web's
`--font-sans` and `--font-mono` stacks, at the web's type scale
(`crates/oa-tokens/src/typography.rs`; body 16 pt, subheadline 14, captions
12 and 10, headings semibold). Launched with `--appearance light|dark`.

- `new-light.png`, `new-dark.png`: a new chat: the feature cards and their
  headline, line, and **Try it** (the composer has focus, so the keyboard is
  up).
- `conversation-light.png`, `conversation-dark.png`: the offline chat fixture
  (`--chat-fixture 1 --coder-send "…"`). The prompt shows italic *emphasis*
  in SF Pro Italic and `inline code` in SF Mono; the reply and the follow-up
  chips are SF Pro.
- `drawer-light.png`, `drawer-dark.png`: the drawer (`--drawer`); the
  keyboard is the new chat's focused composer behind it.
- `settings-light.png`, `settings-dark.png`: Settings (`--tab account`).
- `wallet-light.png`, `wallet-dark.png`: Wallet (`--tab wallet
  --wallet-fixture`).
