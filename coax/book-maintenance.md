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

## Review capability claims

Both books use `coax/book/capability-claims.json` for selected current-scope
statements. A `capability-claim` block is bound to its exact reviewed prose,
selected feature objects from the existing Coax catalogue, and retained
source/evidence files. The catalogue remains authored in `coax/release.coax`;
the ledger does not define another feature status or establish semantic proof.

The normal Coax and Psionic structure checks validate these bindings. A changed
capability, statement or evidence file requires reviewing the claim's scope
before updating its digest. Version-only changes and unrelated catalogue rows
do not invalidate it. Negative delivery claims also bind the relevant book
inventory so a new delivered chapter prompts review. Preserve dated hardware,
numerical and mutation limits when reviewing a claim.

```sh
python3 coax/tests/book_claims.py
python3 coax/book/check-structure.py
python3 psionic/tests/book.py
```

These are maintenance checks, not compiler/app runtime dependencies. They never
rewrite prose or bless new evidence automatically. The publisher runs the
Coax structure/claim gate before reading credentials or contacting GitBook.

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
checks the existing space's identity, creates a GitBook draft and publishes
the SUMMARY page inventory atomically. Existing page IDs and URLs remain; new
sections become overview pages because the authoring API does not create
Git Sync's navigation-only groups. Relative book links become stable page
references, including links to newly inserted pages. The script verifies the
draft inventory and merged revision. It does not change visibility, create a
site or export content back into Git.

An API error or timeout is not success. Inspect the printed draft before
retrying; `--change-request ID` resumes that draft for the same source commit.
The legacy repository-import endpoint can accept a request without advancing
an unconnected space; the publisher therefore uses the draft API.

Finally inspect the public book and its new pages. A merged API revision
is useful evidence, but a public render and working crosslinks verify the
reader's path too. Existing page URLs should remain stable.
