# OpenAgents 1.0 launch

Drafts and the launch-day plan for 1.0 (#11104, #11105). Nothing here has
been posted or deployed. Every draft waits on owner approval.

The launch leads with the iPhone app (owner, 2026-10-17; epic #11977): the
desktop app and the terminal make your computer's power available, and the
phone is where you see it. The phone works in the first minute with no
computer, and sync to the web, desktop, and terminal is free. The plan is
[mobile-first-launch.md](../../mobile/2026-10-17-mobile-first-launch.md).
Launch day is Monday 2026-10-19.

| File | What it is | State |
| --- | --- | --- |
| [release-notes.md](release-notes.md) | Release notes for the phone, the web, and the terminal | Phone section added 2026-10-17 (#11985); waiting on owner approval |
| [post.md](post.md) | X post (245 characters), a 9-part thread or blog version, and the posting checklist (AI labels, we/I voice, no fake personas; #11768) | Leads with the phone, 2026-10-17 (#11985); waiting on owner approval |
| [claims.md](claims.md) | Every number and comparison in the launch copy, with its evidence or what was cut (#11129) | Checked 2026-10-11; the landing page, pricing, film and posts on 2026-10-15 (#11764); phone claims added 2026-10-17, evidence pending |
| [`whats-new-1.0.md`](../../../crates/openagents-web/content/docs/whats-new-1.0.md) | Docs page "What's new in 1.0" | Revised 2026-10-11; waiting on owner approval; not linked into the site |
| [operations.md](operations.md) | Rollback per platform, log checks, launch-day checklist, watch rota | Ready; the watch rota is for the owner to fill |

## What has to be true first

- **The phone works in the first minute with no computer.** On a fresh
  install, the app opens on a chat, a signed-out person gets a few free
  messages on our Gemini, then signs in in one tap, and a coding task runs
  on Our cloud with the free cloud credit (#11978, #11979, #11980, #11981).
  Each phone claim has its evidence in [claims.md](claims.md#phone-claims).
- **The phone build people install has passed that check.** The build is in
  the external TestFlight group with its public link, and Beta App Review
  has passed (#11986). The screenshots come from that build (#11984).
- **Coder 1.0.0 is built from `main`** on Monday 2026-10-19, after the
  release gate (#11091; [big-bang brief](../../audits/2026-10-16-coder-1.0-big-bang.md)).
  The withdrawn `coder-v1.0.0` build and the rc builds don't count.
- **The free daily allowance is live in production** (#11631). Strike the
  line if that changes before launch day.
- **The web release has passed the smoke test** (#11094 is done; #11102 is
  still open).
- Strike any line a build doesn't do. Each release-notes line names its
  issue or commit to make that quick.
- Any new number or comparison goes in [claims.md](claims.md) with its
  evidence first.

## Owner steps on launch day

1. Read and approve [release-notes.md](release-notes.md), [post.md](post.md),
   [claims.md](claims.md#phone-claims),
   and the [docs page](../../../crates/openagents-web/content/docs/whats-new-1.0.md).
   Strike anything the release builds don't do.
2. Link the docs page: add
   `("whats-new-1.0", include_str!("../../content/docs/whats-new-1.0.md"))`
   to `DOCS` in `crates/openagents-web/src/pages/content.rs` and change the
   array length from 33 to 34. It ships with the next web deploy.
3. Post the X post and the thread from the OpenAgents and owner accounts.

## Left out on purpose

- The desktop and Android apps. Android doesn't exist yet; say "iPhone
  first" plainly.
- The Pro plan and environments, agent payments and the wallet, and the
  public inference API (not in production).
- Web chat files, memory, scheduled prompts, export everything, background
  agents, and `coder issue-run`. They are on `main`, but the v1 spec
  (`docs/kitchen-sink/README.md`) puts them after the launch, and none has
  passed the smoke test.
- Cloud agents that keep running with your laptop closed (Pro, not on
  sale yet).
- Plugins (the sample plugins are being replaced, #11096).
- Episode 289 promised stable 1.0 "next week"; no video is drafted here.
