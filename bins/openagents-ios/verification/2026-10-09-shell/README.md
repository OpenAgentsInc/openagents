# Phone shell, 2026-10-09 (#11126)

The iOS simulator build (`build.sh sim`, iPhone 17 Pro Max, no
`OPENAGENTS_MOBILE_PREVIEW`) after the shell: no tab bar, a top bar with the
menu and the **Chat** / **Code** switch, and a drawer. Each screen was
checked beside the owner's reference screenshots, in dark and light.

- `new-dark.png`, `new-light.png`: a new chat in Chat mode: the feature
  cards (`openagents-chat` `home_cards.rs`), **Try it**, and the composer
  (**Ask OpenAgents**). Launched with `--appearance dark|light`.
- `code-dark.png`, `code-light.png`: a new chat in Code mode
  (`--shell-mode code`) on Coder's offline Computers fixture
  (`SIMCTL_CHILD_OPENAGENTS_COMPUTERS_FIXTURE=1`): the computer's recent
  Coder chats and projects, the line naming where Coder runs, and **Work
  with Coder**.
- `drawer-dark.png`, `drawer-light.png`: the drawer (`--drawer`): search,
  Coder, Computers, Wallet, Settings, recent chats, the **Chat** pill, and
  the account button.
- `chat-live-dark.png`: a conversation with the live chat worker: the
  **Worked for 6s** row before the reply and **New chat** in the top bar.
- `chat-light.png`: a conversation on the offline chat fixture
  (`--chat-fixture 1`), with the reply's follow-up chips.

The live release gate (`ReleaseGateUITests`) passed on this build: five
questions, then Wallet and every Settings row, each opened from the drawer.
