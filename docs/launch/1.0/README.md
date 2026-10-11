# OpenAgents 1.0 launch

Drafts and the launch-day plan for 1.0 (#11104, #11105). Nothing here has
been posted or deployed. The launch is web and terminal (Coder) only.

| File | What it is | State |
| --- | --- | --- |
| [release-notes.md](release-notes.md) | Release notes for the web and the terminal | Revised 2026-10-11 against `main`; waiting on owner approval |
| [post.md](post.md) | X post (245 characters) and a 9-part thread or blog version | Revised 2026-10-11; waiting on owner approval |
| [claims.md](claims.md) | Every number and comparison in the launch copy, with its evidence or what was cut (#11129) | Checked 2026-10-11 |
| [`whats-new-1.0.md`](../../../crates/openagents-web/content/docs/whats-new-1.0.md) | Docs page "What's new in 1.0" | Revised 2026-10-11; waiting on owner approval; not linked into the site |
| [operations.md](operations.md) | Rollback per platform, log checks, launch-day checklist, watch rota | Ready; the watch rota is for the owner to fill |

## What has to be true first

- **Coder 1.0.0 is built from `main` at `35e41fd528` or later** (#11091).
  `/download` serves 1.0.0-rc.6 (`65f83ab379`), which doesn't have the new
  Coder lines: `auto` in the input bar, Tab for `flash`, "Coder says it is
  Coder", per-call usage, the Run box changes, and the Compacted row. If the
  stable build is older, strike those lines.
- **The free daily allowance goes live with the production gateway
  deploy.** It is on staging today. Strike the line if production doesn't
  have it on launch day.
- **The web release has passed the smoke test** (#11094 is done; #11102 is
  still open).
- Strike any line a build doesn't do. Each release-notes line names its
  issue or commit to make that quick.
- Any new number or comparison goes in [claims.md](claims.md) with its
  evidence first.

## Owner steps on launch day

1. Read and approve [release-notes.md](release-notes.md), [post.md](post.md),
   and the [docs page](../../../crates/openagents-web/content/docs/whats-new-1.0.md).
   Strike anything the release builds don't do.
2. Link the docs page: add
   `("whats-new-1.0", include_str!("../../content/docs/whats-new-1.0.md"))`
   to `DOCS` in `crates/openagents-web/src/pages/content.rs` and change the
   array length from 33 to 34. It ships with the next web deploy.
3. Post the X post and the thread from the OpenAgents and owner accounts.

## Left out on purpose

- The desktop, iPhone, and Android apps (not in this launch; the iPhone can
  be mentioned once TestFlight review passes).
- The Pro plan and environments, agent payments and the wallet, and the
  public inference API (not in production).
- Web chat files, memory, scheduled prompts, export everything, background
  agents, and `coder issue-run`. They are on `main`, but the v1 spec
  (`docs/kitchen-sink/README.md`) puts them after the launch, and none has
  passed the smoke test.
- Plugins (the sample plugins are being replaced, #11096).
- Episode 289 promised stable 1.0 "next week"; no video is drafted here.
