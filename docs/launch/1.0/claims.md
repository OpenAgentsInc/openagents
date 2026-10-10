# 1.0 launch claims check

Every number, comparison, and "every / all / never" claim in the 1.0 launch
copy, with the public evidence for it or what we did instead (#11129).
Checked against the code on 2026-10-09.

Copy checked: [release-notes.md](release-notes.md), [post.md](post.md),
[What's new in 1.0](../../../crates/openagents-web/content/docs/whats-new-1.0.md),
the homepage (`crates/openagents-web/src/pages/home.rs`, its cards in
`crates/openagents-chat/src/home_cards.rs`, its starter questions in
`crates/openagents-chat/src/suggestions.rs`), and `/download`
(`crates/openagents-web/src/pages/download.rs`).

## Competitive claims

None. The launch copy names no competitor and makes no "cheaper", "faster",
or "more than" claim. The comparisons in three years of posts ("~45% cheaper
than Claude Code at the same success rate", "paid more developers than any
lab", "faster than Ollama"; `docs/kitchen-sink/twitter.md`) stay out of it
until each has a public page with the setup, the cost, and a script anyone
can re-run (ledger G2, G8; #11131). The one comparison page we have,
[/efficiency](https://openagents.com/efficiency), is computed from committed
study rows and isn't in the launch copy.

## Claims table

| Claim | Where | Evidence | Decision |
| --- | --- | --- | --- |
| `/sync all` adds your 150 newest earlier chats | Release notes, What's new | [`EARLIER = 150`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/coder-new/src/account_sync.rs) | Keep |
| Usage records are deleted after 30 days | Release notes, post 7 | [Privacy and security](https://openagents.com/docs/privacy-and-security); [`DEFAULT_KEEP_DAYS = 30`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/coder/src/relay/usage.rs) (#11042) | Keep |
| The privacy policy was updated on October 9 | Release notes | [/privacy](https://openagents.com/privacy): "Last updated: 2026-10-09" | Keep |
| We ask model providers not to train on your chats and not to keep them | Release notes, post 7, What's new | [Privacy and security](https://openagents.com/docs/privacy-and-security); the chat worker asks for zero retention by default (`CODER_PROVIDER_PRIVACY`, [`generate.rs`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/coder/src/generate.rs), #11040) | Keep |
| "Deleted means deleted" / "Delete means delete" | Release notes, post 3 | [Privacy and security](https://openagents.com/docs/privacy-and-security) says our storage provider may keep a copy we can recover for up to 7 days | Rewritten in the release notes to say so; cut from the post |
| "We went through every page and took out the system talk" | Release notes | The copy check runs on the pages in the site's page test (`crates/openagents-web/src/copy_guard.rs`, #11031), not on every page | Rewritten: "We took the system talk out of the pages, and a check in the site's tests keeps it out." |
| "Everything is open source" / "Everything we build is open source" | Post 8, homepage card | The website, Coder, and the apps are in the [public repository](https://github.com/OpenAgentsInc/openagents) under [Apache 2.0](https://github.com/OpenAgentsInc/openagents/blob/main/LICENSE). Some services are not: the Pro inference door's source is a private repository (`docs/gateway/README.md`) | Rewritten: post 8 says the website and Coder; the homepage card (and the same line in the `/ui` catalog) says "This site and our apps are open source." |
| Coder 1.0.0 | Release notes | `/download` serves 1.0.0-rc.5 today (`CODER_VERSION`) | Holds once the Coder 1.0.0 build ships (#11091); strike it otherwise |
| Desktop: checks for updates every six hours; macOS 13 or later | Release notes; `/download` desktop section | [`CHECK_INTERVAL`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/openagents-desktop/src/update.rs) is six hours | Cut: desktop is out of this launch (owner, #11104). `/download` hides the desktop section while `DESKTOP_RELEASED` is false |
| Desktop, iPhone, and Android lines | Release notes, post 1, 7, 8, X post, What's new | Owner, 2026-10-09: the launch is web and terminal only | Cut from all three drafts |
| X post length | Post | Counted: 245 characters with the link at its real length, which X also counts as 23 | Fixed: the draft said 279 in the heading and 275 in the notes |

## Not changed here

- `/download` still lists the phone apps when their links are set. The page
  isn't a launch announcement, so it's left to the download-page issue.
- Ledger K4 says "All of it is open source" as Live. The Pro door's private
  source makes that a little too broad; worth a look when the ledger is next
  reviewed.
