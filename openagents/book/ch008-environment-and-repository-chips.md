# 8. Environment and repository chips

> **Status: partly implemented.** The shared Coax environment and repository
> selector ([`apps/shared/app/environment/select.coax`](../../apps/shared/app/environment/select.coax))
> runs on `cpu` with its runnable book example
> [`openagents/examples/env-repo-chips.coax`](../examples/env-repo-chips.coax)
> and committed debug/release transcript
> [`openagents/examples/env-repo-chips.out`](../examples/env-repo-chips.out),
> exercising logged-out login prompts, catalog and station reachability fetches,
> selecting an online computer and one of its repositories, refusing an offline
> (`503`) computer without changing the active environment, and per-draft
> isolation. Remaining for native hosts: GPU chip picker UI tests and
> `openagents/examples/cases/chips.ops`.

## Premise

Chapter 7's conversations all run in one place you can't see. This chapter
adds two chips under the composer: the environment, which says which
computer a conversation's work runs on, and the repository it works in. You
choose both before you send, and you can see both on every message after.

## Draft

A chip is a small component with a label and a menu: `composer.env` shows
the environment, and `composer.repo` shows the repository. Tapping one
opens its picker as a card over the chat page. A chip shows only choices
that work: an environment that is offline is drawn as offline and can't be
chosen, and a repository the environment can't reach is not listed.

An environment is a computer that runs agent work for you: this account's
hosted computer, or one of your own that you enrolled, such as a Mac or a
CoderOS machine. The
[environment application profile](../../docs/protocol/environments-claude-and-world-editing.md)
owns how a phone reaches it. The environment's runtime contract is
[environment-runtime-v1alpha1](../../docs/protocol/environment-runtime-v1alpha1.md).
The list of environments is a resource loaded by an effect, as chapter 7's
history is.

The chips set where a conversation's turns run, and the choice is part of
the conversation. A conversation started on one environment stays on it;
choosing another starts a new conversation, so one turn never runs on two
computers. That is chapter 37's rule, one execution owner per task, as it
first appears in the app. The phone only presents the turn; it never runs
the tool calls itself.

The repository picker lists the repositories the chosen environment has.
The phone never clones or reads a repository. It sends the environment's
answer back as the conversation's setting.

## What we build

A plan for what this chapter ships once the draft is approved:

- The chips `composer.env` and `composer.repo` under the composer, each
  with its picker card.
- The environment and repository lists as resources loaded by effects.
- The choice stored with the conversation, and a new conversation when you
  change the environment.

It builds on what exists today:

- [`apps/shared/app/environment/select.coax`](../../apps/shared/app/environment/select.coax)
- [`docs/protocol/environments-claude-and-world-editing.md`](../../docs/protocol/environments-claude-and-world-editing.md)
- [`docs/protocol/environment-runtime-v1alpha1.md`](../../docs/protocol/environment-runtime-v1alpha1.md)
- [`apps/shared/environment-app.coax`](../../apps/shared/environment-app.coax)
- [`apps/openagents-desktop/page/environments.coax`](../../apps/openagents-desktop/page/environments.coax)
- [`apps/openagents-desktop/Window+Environments.swift`](../../apps/openagents-desktop/Window+Environments.swift)
- [`components/kit/composer.coax`](../../components/kit/composer.coax)
- [`components/kit/chip.coax`](../../components/kit/chip.coax)

### The chapter's check

- **Source:** [`apps/shared/app/environment/select.coax`](../../apps/shared/app/environment/select.coax),
  [`openagents/examples/env-repo-chips.coax`](../examples/env-repo-chips.coax),
  and the committed stdout transcript [`openagents/examples/env-repo-chips.out`](../examples/env-repo-chips.out)
  (remaining: `openagents/examples/cases/chips.ops` and native GPU picker UI checks).
- **Trace:** pressing `composer.environment` while logged out asks for login;
  after login and catalog/station delivery, `coderos` shows `Ready`, `Studio`
  (`503`) shows `Not reachable`, selecting `coderos` and `env.repo.1` binds
  `OpenAgentsInc/everglade`, and switching drafts keeps `Default` isolated.
- **Refusal:** pressing the offline `503` computer returns `pick_offline=false`
  and leaves the active environment pinned to `coderos`.
- **Independent check:** `coax/bin/coax run openagents/examples/env-repo-chips.coax`
  and `--release` produce identical transcripts matching
  [`openagents/examples/env-repo-chips.out`](../examples/env-repo-chips.out),
  beside `coax/bin/coax test apps/shared/app/environment/select.coax`.

Planned paths: `openagents/examples/cases/chips.ops`. The app is
[`apps/openagents-ios`](../../apps/openagents-ios).

## Sources

- **Episodes:**
  - [Episode 275, *Coder*](../../docs/transcripts/275.md): one suite across computers, cloud and mobile
  - [Episode 281, *Coder Mobile*](../../docs/transcripts/281.md): one chat across terminal, Android and iOS
- **Also:**
  - [The environment application profile](../../docs/protocol/environments-claude-and-world-editing.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P3

## Open questions for the owner

1. Which environments does the first build list: this account's hosted
   computer, your enrolled computers, or both?

## Source listing

Run the Chapter 8 example in [`openagents/examples/env-repo-chips.coax`](../examples/env-repo-chips.coax) (committed transcript [`openagents/examples/env-repo-chips.out`](../examples/env-repo-chips.out)):

<!-- source: openagents/examples/env-repo-chips.coax env-repo-chips -->
```jai
erc_run :: () -> bool {
  s := [1] EnvSelect.[es_new()];

  // Logged out: pressing a chip asks for login and opens nothing.
  _ := es_press(s[..], "composer.environment", 0);
  need_login := es_take_login(s[..]);
  print("logged_out_asks_login=% sheet=% env=%\n", need_login, s[0].sheet, es_env_value(s[..]));

  // Log in, enable manual environment + repo selection, open picker, deliver catalog & stations.
  es_capabilities(s[..], true, true);
  es_login(s[..], true);
  es_open_env(s[..], 1000);
  e: [1] EsEffect;
  _ = es_take(s[..], e[..]);
  _ = es_catalog_done(s[..], e[0].op, e[0].generation, ES_O_DONE, 200, ERC_CATALOG, 1000);
  ops: [3] int;
  for k: 0..<3 { _ = es_take(s[..], e[..]); ops[k] = e[0].op; }
  _ = es_station_done(s[..], ops[0], s[0].generation, ES_O_DONE, 200, erc_fact(ERC_A, 1000, "OpenAgentsInc/openagents", "OpenAgentsInc/coax"), 1000);
  _ = es_station_done(s[..], ops[1], s[0].generation, ES_O_DONE, 200, erc_fact(ERC_B, 1000, "OpenAgentsInc/openagents", "OpenAgentsInc/everglade"), 1000);
  _ = es_station_done(s[..], ops[2], s[0].generation, ES_O_DONE, 503, EsStationFact.{}, 1000);
  print("catalog=% online_b=% offline_c=%\n", s[0].catalog[0].n, es_reach_words(s[0].computers[1].reach), es_reach_words(s[0].computers[2].reach));

  // Select online computer B ("coderos") and one of its repositories.
  b: [ES_EL_ID] u8;
  pick_b := es_press(s[..], es_computer_id(s[..], 1, b[..]), 1000);
  l: [1] EsRepoList;
  es_repo_list(s[..], l[..]);
  _ = es_press(s[..], "composer.repo", 1000);
  pick_repo := es_press(s[..], "env.repo.1", 1000);
  print("pick_b=% env=% repos=% pick_repo=% repo=%\n", pick_b, es_env_value(s[..]), l[0].n, pick_repo, es_repo_name(s[..]));

  // Refusal: pressing offline computer C (503) is refused and keeps "coderos".
  _ = es_press(s[..], "composer.environment", 1000);
  pick_offline := es_press(s[..], es_computer_id(s[..], 2, b[..]), 1000);
  print("pick_offline=% env_kept=%\n", pick_offline, es_env_value(s[..]));

  // Per-draft isolation: a second conversation keeps Default until explicitly changed.
  es_draft(s[..], "conv_other");
  other_env := es_env_value(s[..]);
  es_draft(s[..], "");
  print("other_draft_env=% original_draft_env=%\n", other_env, es_env_value(s[..]));

  return need_login && pick_b && pick_repo && !pick_offline && es_env_value(s[..]) == "coderos" && other_env == "Default";
}
```

Output (`coax/bin/coax run openagents/examples/env-repo-chips.coax`, identical in `--release`):

```text
logged_out_asks_login=true sheet=0 env=Default
catalog=3 online_b=Ready offline_c=Not reachable
pick_b=true env=coderos repos=2 pick_repo=true repo=OpenAgentsInc/everglade
pick_offline=false env_kept=coderos
other_draft_env=Default original_draft_env=coderos
ok=true
```
