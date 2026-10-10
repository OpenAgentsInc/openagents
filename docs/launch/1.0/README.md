# OpenAgents 1.0 launch

Drafts and the launch-day plan for 1.0 (#11104, #11105). Nothing here has
been posted or deployed.

| File | What it is | State |
| --- | --- | --- |
| [release-notes.md](release-notes.md) | Release notes for the web and the terminal | Draft, waiting on owner approval |
| [post.md](post.md) | X post (245 characters) and an 8-part thread or blog version | Draft, waiting on owner approval |
| [claims.md](claims.md) | Every number and comparison in the launch copy, with its evidence or what was cut (#11129) | Checked 2026-10-09 |
| [`whats-new-1.0.md`](../../../crates/openagents-web/content/docs/whats-new-1.0.md) | Docs page "What's new in 1.0" | Draft, waiting on owner approval; passes the `oa-copy` guard; not linked into the site yet |
| [operations.md](operations.md) | Rollback per platform, log checks, launch-day checklist, watch rota | Ready; the watch rota is for the owner to fill |

## Before anything is posted

- The launch is web and terminal only (owner, 2026-10-09; the
  announcement moved to Monday 2026-10-12). Each of those release builds
  has to exist and pass the smoke test (#11102): terminal #11091, web
  #11094.
- Strike any release-notes line the build doesn't do. Each line names its
  issue to make that quick.
- Any new number or comparison goes in [claims.md](claims.md) with its
  evidence first.
- To put the docs page on the site, add
  `("whats-new-1.0", include_str!("../../content/docs/whats-new-1.0.md"))` to
  `DOCS` in `crates/openagents-web/src/pages/content.rs` (and bump the array
  length). It ships with the next web deploy.

## Left out on purpose

The desktop, iPhone, and Android apps (not in this launch; the iPhone can
be mentioned once TestFlight review passes). The Pro plan and environments (next week), agent payments and the wallet
(later), the public inference API (built, not deployed), and plugins (the
sample plugins are being replaced, #11096). Episode 289 promised stable 1.0
"next week"; no video is drafted here.
