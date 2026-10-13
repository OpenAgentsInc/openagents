# Phone launch, 2026-10-13 (#11557)

The owner's review of the Episode 290 launch film draft (0:04.2): "I don't
want the mobile app to open to an empty chat. That carousel thing looks
nonsensical." The simulator build (`build.sh sim`, iPhone 17 Pro, no
`OPENAGENTS_MOBILE_PREVIEW`, `--chat-fixture 1 --appearance dark`).

- `before-film.png`: the phone in the film draft: an empty chat under a
  carousel of three icon cards, **See the roadmap**, and **Try it**.
- `welcome-dark.png`: a fresh install, no chats: **Welcome to OpenAgents**,
  one line, and one action, **Start a chat**. The Coder / Verse switch
  stays.
- `new-chat-dark.png`: **Start a chat** (or **New chat** anywhere): a clean
  composer, **Ask OpenAgents**, ready to type. No cards.
- `relaunch-dark.png`: after a chat (`--coder-tap
  "shell-welcome-start,send:What is OpenAgents?"`), a plain relaunch opens
  that chat where it was left.

A relaunch with chats but none open (the last screen was a new chat or the
chats list) opens every chat; `openagents-mobile`'s
`launch_with_no_chat_open_opens_the_chats_list` covers it.
