# 1.0 launch claims check

Every number, comparison, and "every / all / never" claim in the 1.0 launch
copy, with the public evidence for it or what we did instead (#11129).
Checked against the code on 2026-10-09, and again against `main` at
`35e41fd528` on 2026-10-11.

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
| Coder 1.0.0 | Release notes | Built from `main` on Monday 2026-10-19 after the release gate ([big-bang brief](../../audits/2026-10-16-coder-1.0-big-bang.md)); the 2026-10-15 build is withdrawn | Holds once that build ships (#11091); strike it otherwise |
| Desktop: checks for updates every six hours; macOS 13 or later | Release notes; `/download` desktop section | [`CHECK_INTERVAL`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/openagents-desktop/src/update.rs) is six hours | Cut: desktop is out of this launch (owner, #11104). `/download` hides the desktop section while `DESKTOP_RELEASED` is false |
| Desktop, iPhone, and Android lines | Release notes, post, What's new | Owner, 2026-10-09: web and terminal only; changed 2026-10-17: the launch leads with the iPhone | iPhone back in (see [Phone claims](#phone-claims)); desktop and Android stay out |
| `auto` is free up to about $1 of model use a day for a signed-in account; starts again at midnight UTC | Release notes, What's new | `881f380e0b`: `free_allowance.usd_micros_per_day` defaults to 1,000,000 (`GATEWAY_FREE_ALLOWANCE_MICROS`, `deploy/staging/gateway.sh`); one allowance per account per UTC day; [Pricing](https://github.com/OpenAgentsInc/openagents/blob/main/crates/openagents-web/content/docs/pricing.md) says the same | Keep, once the production gateway deploy carries it; staging only today. Left out of the post |
| Every model call's tokens, cost, and time are recorded when it ends, and `/export` carries them | Release notes, post 6, What's new | `6ab491f16a` (`crates/coder-new/src/provider.rs`, `trajectory.rs`); `/export` is in `slash.rs` and in rc.7's command list | Keep, for a 1.0.0 build at or after that commit |
| Coder says it is Coder, made by OpenAgents, never another product | Release notes | `crates/coder-new/src/identity.rs` opens every route with that note; tests check each route (#11264) | Keep, for a 1.0.0 build at or after `57157f1c62` |
| Run won't run git commands that throw away uncommitted work | Release notes | `crates/coder-new/src/git_guard.rs` refuses `stash` (but `list`/`show`), `reset --hard/--merge/--keep`, `checkout --`/`.`/`-f`, `restore` without `--staged`, and `clean -f` (`9dbcba33e6`) | Keep; worded "such as" because the list is long |
| Tab switches between `auto` and `flash`; `flash` needs your own OpenRouter key | Release notes, post 6, What's new | `99eed810ce`: `toggle_flash` in `crates/coder-new/src/lib.rs`; without a key it says "flash needs an OpenRouter API key. Add one with F2." | Keep. The model behind `flash` is not named in the copy |
| "Saved Claude keys are encrypted" | Release notes (earlier draft) | No public page states it, and it names a vendor | Cut |
| X post length | Post | Counted: 245 characters with the link at its real length, which X also counts as 23; recounted 2026-10-11 | Keep |

## Phone claims

Added 2026-10-17 (#11985) for the phone-first launch (#11977). None has its
evidence yet. Each stays out of the posted copy, or is cut, until the
evidence below exists, and no number goes in without a measurement.

| Claim | Where | Evidence that will prove it | Decision |
| --- | --- | --- | --- |
| Works with no computer: a new person gets a real answer and starts a coding task with no computer connected | Post 1, 2; X post; release notes | A run of the installed external TestFlight build on a fresh test account with no paired computer (#11979, #11981, #11986), captured by the screenshot test (#11984) | Pending |
| A few free messages before sign-in | Post 2; X post; release notes | `MESSAGES = 10` per visitor (`crates/openagents-web/src/visitor_api.rs`, `c40a1a4019`), plus the app sending them signed out on the installed build (#11978) | Pending; say "a few", not "ten", unless the owner wants the number |
| A few dollars of free cloud credit | Post 2; release notes | The credit amount in the Computers service config and a fresh account running a task on Our cloud without a card (#11981) | Pending; name a dollar amount only from that config |
| An answer in the first minute (time to first answer) | Post 1 | The first-minute counts (install to first answer streamed) from the server for fresh installs (#11983), or a timed run on the installed build | Pending; cut "in the first minute" if not measured by launch |
| Our cloud starts in N seconds | Not in the copy | First-turn time on a fresh account (#11981); the freeze doc measured about 77 seconds | Left out; add only with a measurement |
| Sync is free | Post 4; release notes | Owner decision 2026-10-17; no sync charge in any plan file ([docs/pro](../../pro/README.md) lists sync as never gated) | Keep |
| Sign in with one tap, with GitHub or Apple | Post 2; release notes | Sign in with Apple and the in-app sign-in sheet on the installed build (#11980) | Pending |
| Drive your computer from the phone: send, queue, steer, stop | Post 3; release notes | #11849 (closed); a test-account run on the installed build | Keep once the run passes |

## Landing, pricing, film and posts (#11764)

Every figure on `/` (`crates/openagents-web/src/pages/landing.rs`),
`/pricing` (`pages/pricing.rs`), [`/docs/pricing`](../../../crates/openagents-web/content/docs/pricing.md),
the launch film (the captions in `scripts/film/system1-routing.py`,
`coder-segments.py`, `revenue-sharing.py`, `docs/verse/film/launch.json`,
`cut.json`, and the Episode 290 script `docs/transcripts/290.md`), and the
queued posts. Checked 2026-10-15 against `main` at `62cae1223e`.

The queued posts are [post.md](post.md) (checked above) and the Episode 290
show notes in `290.md`. Sarah's old tweet queue (`docs/sarah/SARAH_TWEET_QUEUE.md`)
was removed from the repository with the rest of `docs/sarah/`; nothing else
is queued.

No x401-style line ("agents prove what they're authorized to do", "offline
verification") is in any of this copy.

### The study numbers

All from [briefed-agent-ab.md](../../inference/briefed-agent-ab.md) (#11211):
21 closed issues, 3 runs each, the same model (`claude-opus-5-5`) both ways;
bare Claude Code against the briefed agent.

| Claim | Where | Evidence | Decision |
| --- | --- | --- | --- |
| $1.84 instead of $4.00 per accepted fix | `/` hero card and stats; film (routing, savings, revenue sharing); 290.md | "$ per accepted": 4.00 (A) and 1.84 (B0) | Keep |
| 121 s instead of 284 s (median time) | `/` stats; film; 290.md | "Median s": 284 (A) and 121 (B0) | Keep |
| 21 real issues, same model both ways; 3 runs each | `/` hero pill and stats; film small print | Headline and Setup | Keep |
| 19 of 63 fixes accepted both ways | Film (savings, revenue sharing); 290.md | "Accepted": 19/63 each | Keep |
| Half the cost per fix | `/` hero pill; film; 290.md | 53.9% lower per accepted change, 95% CI [44.3%, 60.4%] | Keep |
| 2× cheaper, 2× / 2.3× faster, "more than twice as fast" | Film (routing, savings); 290.md | $4.00 / $1.84 = 2.17; 284 s / 121 s = 2.35 | Keep |
| $2.16 saved per fix | Film (revenue sharing) | $4.00 − $1.84, at list price | Keep |
| "and nothing worse" | Film (savings caption); 290.md section 5 | The study doesn't rule out a quality cost: the judge accepted 0.889 vs 0.794, CI [-0.254, +0.048], and calls "equal or better" inconclusive | Rewritten: "and as many fixes accepted" (film), "Half the cost, and more than twice as fast." (290.md) |
| "Same model, same success rate" | Film (revenue sharing small print) | Equal point estimates only; the success difference is inconclusive | Rewritten: "Same model, 19 of 63 accepted both ways." |
| "for work a small model does just as well" | `/` problem section | No study compares a small model's quality; the measured arm used the same big model | Rewritten: "for work a small model can do" |

The measured saving comes from the brief and the `verify` tool in front of
the same model, not from small models: no small model ran in the study.
Copy may put these numbers beside "Coder works out what each step needs" (the
brief is a System 1 step), but must not say small models produced them.

### Other figures

| Claim | Where | Evidence | Decision |
| --- | --- | --- | --- |
| Six agents: Claude Code, Codex, OpenCode, Grok Build, Devin CLI, Cursor | `/`, `/pricing`, `/docs/pricing`, film | `crates/coder-new/src/harness_pick.rs` (Claude Code, Codex, OpenCode, then the ACP agents Devin, Cursor, Grok Build) | Keep; 290.md's recording checklist drops any harness the release gate skips |
| Free is $0 | `/`, `/pricing`, `/docs/pricing` | No charge for Free in any plan file | Keep |
| A free daily model allowance, about $1 a day, from midnight UTC | `/`, `/pricing`, `/docs/pricing` | `free_allowance.usd_micros_per_day` 1,000,000 per account per UTC day (`881f380e0b`); live in production since #11631 (`421c30d494`), whose test account hit the $1 cap there | Keep |
| Pro is $20 a month | `/`, `/pricing`, `/docs/pricing` | `crates/retail-cloud/fixtures/environment-plan.json` `price_usd_micros` 20,000,000; [docs/pro](../../pro/README.md) | Keep. The plan is `proposed`, `/docs/pricing` says Pro isn't on sale yet |
| 3-day free trial, a reminder the day before it ends | `/pricing`, the CAD Lab gate | [docs/pro](../../pro/README.md) "The 3-day free trial" (Stripe `trial_period_days = 3`, `trial_will_end`); not built yet | Holds once the trial ships. `/pricing` is served only with `OPENAGENTS_PRICING_PAGE=1` until then (`pricing.rs` `served`); don't turn it on before the trial works |
| 6,000 minutes (100 machine-hours) a month; 2 vCPU, 8 GB; 2 at once; 20 GB, up to 10 saved versions; $0.18 an extra hour; hours don't roll over | `/pricing`, `/docs/pricing` | `environment-plan.json`: `included_machine_hours` 100, `machine` 2 vCPU / 8 GB, `machines_at_once` 2, `storage_gb` 20, `saved_versions` 10, `extra_hour_usd_micros` 180,000, `rollover` false | Keep; `/pricing` reads them from the plan |
| We answer sales within one business day | `/docs/pricing`, `/contact` | The contact page's own promise (`pages/contact.rs`) | Keep |
| "Ask for a 40 mm fan bracket" | `/` CAD Lab card | An example request, not a measured figure; CAD Lab is gated to Pro (#11693) | Keep |
| Mock-up numbers ("+3", "1 failed", "12 passed") | `/` hero and transcript illustrations | Illustrations of one turn, not measurements | Keep |
| "Plugin authors paid in Bitcoin" | `/` footer; film | Pay-ledger rule v1 (`crates/pay-ledger/rules/v1.toml`): the author gets the plugin's whole fee; the 402 quote ran on production (`docs/payments/2026-10-03-end-to-end-demo.md`) | Keep as how payment works. The first real paid call is the owner's step (`NEEDS_OWNER.md`); until then the film says "demo payments" |
| 15 sats a call: 5 to OpenAgents, a 10-sat author fee; a 1,000-sat first paid call bonus | Film (revenue sharing); 290.md | The 2026-10-03 production quote (demo doc above); rule v1 `first_paid_call_msat` 1,000,000 | Keep, with "demo payments" on screen |
| Payouts reach your wallet within a day | 290.md section 8 | [Central receive and splits](../../payments/2026-10-02-central-receive-and-splits.md), "How payouts go out": once a day for anything above 1 sat | Keep; on 290.md's recording checklist |
| 99.7% and 99.8% of the prompt read from the cache | Film (delegation) | #11511 closing comment: 0.9965 with a delegation, 0.9977 without, from Claude Code's usage records | Keep |
| Episode 1 on Nov 7, 2023; "three years" | Film (devday); 290.md | [Episode 1](../../transcripts/001.md): upload date 2023-11-07 | Keep |
| "A six-letter code signs Coder in" | `docs/verse/film/launch.json` caption | The sign-in code is eight letters, `XXXX-XXXX` (`crates/tenancy/src/sessions/device.rs` `USER_CODE_LEN`) | Fixed: "An eight-letter code" |

### Competitor claims removed

| Claim | Where | Decision |
| --- | --- | --- |
| "They promised developers revenue sharing", "OpenAI DevDay: revenue sharing promised", "Shared with developers: zero" | Film (`devday` segment); 290.md section 8 | Cut. The segment is now Episode 1 to Coder 1.0, then "We're paying the people." The v2 cut predates this; `290-cut.md` lists the re-render |
| "Most coding agents send every step to one big model" | `/` problem section; film (routing caption) | Rewritten about an agent on its own (the study's bare arm), not about other products: "On its own, a coding agent sends most steps to one big model" |

The bare Claude Code arm stays in the film ("Claude Code on its own"): it is
the measured setup, with its public page and scripts in
[briefed-agent-ab.md](../../inference/briefed-agent-ab.md).

## Not changed here

- `/download` still lists the phone apps when their links are set. The page
  isn't a launch announcement, so it's left to the download-page issue.
- Ledger K4 says "All of it is open source" as Live. The Pro door's private
  source makes that a little too broad; worth a look when the ledger is next
  reviewed.
