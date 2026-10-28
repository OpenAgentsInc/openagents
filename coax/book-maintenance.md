# Maintaining the Coax book

The book's source is `coax/book/`; `SUMMARY.md` defines its navigation.
Keep implemented features distinct from planned syntax. Fix the compiler
walkthrough when implementation ownership changes. Do not present old source
counts or timing samples as current app measurements.

## Verify

From the repository root:

```sh
coax/book/check-examples.sh
```

The script checks exact output and exit status in debug and release, runs
checks in authored runnable examples, exercises the Metal/library/native-host
examples, kills the request example's mutants, and checks navigation, local
links and opted-in source excerpts. Deliberate traps declare a `.status`;
mode-specific behavior uses `.release.out` and `.release.status`.

For native sanitizer and official protobuf evidence:

```sh
PROTOC=/absolute/path/to/pinned/protoc BOOK_SANITIZE=1 coax/book/check-examples.sh
```

`check-structure.py` is book-only Python tooling, not a build dependency.
An excerpt marked `<!-- checked-source: examples/file.coax -->` must occur
exactly in that source. This is opt-in: illustrative fragments, expected
errors and elided listings are not all standalone compilable programs.

## Publish the existing book

Commit the reviewed changes, rebase on `origin/main`, run any checks affected
by the rebase and push. The existing GitBook space can import the pushed
repository without setting up Git Sync or creating a second site:

```sh
python3 scripts/publish-coax-book.py --token-file /absolute/path/to/gitbook.env
```

Alternatively set `GITBOOK_TOKEN` in the process environment. Never commit a
token, put it in the command line or paste it into logs. The env file needs
one `GITBOOK_TOKEN=...` assignment; the script does not execute the file.

The publisher checks that the worktree is clean and HEAD equals remote main,
checks the existing space's identity, requests an import of `coax/book`, then
verifies the imported commit and complete SUMMARY page inventory. It does not
change visibility, create a site or export GitBook content back into Git.
An API error or timeout is not success; inspect the space before retrying
an import with an unknown outcome.

Finally inspect the public book and its new pages. An imported API revision
is useful evidence, but a public render and working crosslinks verify the
reader's path too. Existing page URLs should remain stable.
