# Release notes

*Release notes* turns the commits between two releases into grouped,
user-facing release notes (breaking changes, features, fixes, and the
rest), each line citing its commit. It's one of the
[example plugins](README.md): copy it to make your own.

```text
You:   Turn these commits into release notes for v1.5.0:
       4f2a9c1 feat(export): export invoices as CSV (#212)
       8b7c6d5 Merge pull request #211 from acme/dark-theme
       a7b8c9d feat(api)!: drop the v1 export endpoint
       ...
Coder: ## Release notes: v1.5.0
       ### Breaking changes
       - **api:** Drop the v1 export endpoint (`a7b8c9d`)
       ### Features
       - **export:** Export invoices as CSV (#212) (`4f2a9c1`)
       ...
       5 commits grouped, 1 merge commit left out. …
```

## What's in it

| Part | File | What it does |
| --- | --- | --- |
| Wasm | [`src/lib.rs`](../../../crates/plugin-release-notes/src/lib.rs) | The `notes` operation: reads a git log, groups its commits, and renders the notes. |
| Workflow | [`programs/release-notes.json`](../../../crates/plugin-release-notes/programs/release-notes.json) | One `module` step that hands the Wasm the request (`"request": "text"`) and lets it read the files the request names (`"read_named": true`). |
| Plugin record | [`package.json`](../../../crates/plugin-release-notes/package.json) | The name, the summary, the publisher, and the workflow's digest. |
| Tests | [`evals/`](../../../crates/plugin-release-notes/evals/) | Four tests where it should help and two where it should stay out of the way. |

## What it reads

Wasm can't run `git`, so the commits come from text: a log you paste,
or a file the request names that holds one, such as the output of
`git log v2.0.0..v2.1.0 > release/commits.txt`. It reads `git log`'s
default format (with or without `--decorate`), `--oneline`, `--graph`,
and `--format="%h %s"`.

The range: a `FROM..TO` token in the request names it and titles the
notes. Without one, a decorated log stops at the previous tag, so pasting
your whole `git log --decorate --oneline` gives the notes since the last
release, and the result says how many older commits it left out.

## How it groups

1. A Conventional Commit (`feat(api)!: …`) goes by its type: `feat` to
   **Features**, `fix` to **Fixes**, `perf` to **Performance**, `docs` to
   **Documentation**, `revert` to **Reverts**, and `refactor`, `test`,
   `ci`, `build`, `chore`, and `style` to **Internal**. Its scope is shown
   in bold.
2. A `!` after the type, or a `BREAKING CHANGE:` footer in the body, puts
   it under **Breaking changes**, with the footer's note under it.
3. Any other commit goes by its first word: `Add` and `Support` are
   features, `Fix` and `Prevent` are fixes, `Remove` and `Drop` are
   **Removed**, and so on.
4. A commit whose first word says nothing goes under **Other changes**,
   never into a guessed group.
5. Merge commits are counted and left out. A trailing `(#123)` is kept as
   the pull request.

Every line cites its commit's short hash, and the footer says which rule
grouped the commits.

## Copy it

The Wasm is a text transform: it needs no file but the log. To make a
plugin that writes something else from the same log (a changelog entry in
your project's style, a summary for a status update), copy the crate,
replace `render` and the groups, and keep `commits`, which reads every
`git log` format above. The steps are the same as for
[Explain this error](explain-this-error.md#copy-it).

## Read-only run on a real repository

On 2026-10-01, `git log --oneline 4467f24eb9..1c095e50d0` of the owner's
`openagents` repository (read with `git log`; nothing was written to the
repository) was saved to a scratch folder, and `openagents plugin run
crates/plugin-release-notes --in SCRATCH --request "Write release notes
for 4467f24eb9..1c095e50d0 from commits.txt"` grouped its 313 commits and
left out its one merge: 12 features, 5 fixes, 5 removals, 1 revert, 4
documentation changes, 11 internal changes, and 275 under **Other
changes**. That last number is the honest finding: this repository doesn't
use Conventional Commits, and most of its subjects open with a noun
("Phone Run Coder asks…", "Coder starts in…") that says nothing about the
kind of change, so the plugin doesn't guess.
