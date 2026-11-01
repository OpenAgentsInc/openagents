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

## Keep chapter status in the registry

Each book keeps `chapters.json` beside its `SUMMARY.md`: one record per
chapter with its number, title, file, status (`implemented`, `partly
implemented`, `design draft` or `planned`), profile, dependencies, implemented
source, qualified targets and adoption. The registry check in
`coax/book/check-structure.py` compares it with each chapter's heading and
status marker, the contents labels, the README's list or status table and, for
Psionic, the plan.
`check-structure.py` and `psionic/tests/book.py` run that comparison, so a
status change must update all of them in one commit. To check all four
registries and print their status counts:

```sh
python3 coax/book/check-structure.py --registry
```

Each book's README states the four things a draft needs before it becomes a
teaching chapter: a worked calculation or trace, runnable source, a broken
case with its diagnostic, and an independent check.

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
book's structure/claim gate before reading credentials or contacting GitBook.

## Publish the books

One publisher serves the four books written in Coax. Each book has its own
GitBook space and site under openagents.gitbook.io:

| `--book` | Source | Space | Public book | Gate |
|---|---|---|---|---|
| `coax` (default) | `coax/book/` | `sZhQ0yptSOKMneBDWMti` | <https://openagents.gitbook.io/the-coax-programming-language> | `scripts/coax-release.sh --check` and `coax/book/check-structure.py` |
| `psionic` | `psionic/book/` | `Sha9csfX5lAIHvjiIheN` | <https://openagents.gitbook.io/psionic-ml-from-scratch> | `psionic/tests/book.py` |
| `aethyr` | `aethyr/book/` | `QxVdkZGgQYnLsXWZlkTh` | <https://openagents.gitbook.io/building-aethyr-world-engine> | `psionic/tests/book.py --book aethyr` |
| `nostr` | `nostr/book/` | `FJJZf8YunJx2R1toVsuv` | <https://openagents.gitbook.io/nostr-from-scratch> | `psionic/tests/book.py --book nostr` |

Commit the reviewed changes, rebase on `origin/main`, run any checks affected
by the rebase and push. The existing GitBook space can import the pushed
repository without setting up Git Sync or creating a second site:

```sh
python3 scripts/publish-coax-book.py --token-file /absolute/path/to/gitbook.env
python3 scripts/publish-coax-book.py --book psionic --token-file /absolute/path/to/gitbook.env
python3 scripts/publish-coax-book.py --book aethyr --token-file /absolute/path/to/gitbook.env
python3 scripts/publish-coax-book.py --book nostr --token-file /absolute/path/to/gitbook.env
```

Alternatively set `GITBOOK_TOKEN` in the process environment. Never commit a
token, put it in the command line or paste it into logs. The env file needs
one `GITBOOK_TOKEN=...` assignment; the script does not execute the file.

The publisher checks that the worktree is clean and HEAD equals remote main,
checks the existing space's identity, creates a GitBook draft and publishes
the SUMMARY page inventory in one draft that is merged only after it checks.
Existing page IDs and URLs remain; new sections (`## X` or `**X**` lines)
become overview pages because the authoring API does not create Git Sync's
navigation-only groups. Missing pages are inserted first, so every relative
book link becomes a stable page reference; pages then move into SUMMARY
order. A section renamed only by dropping ` (planned)` keeps its page and
URL. For every book, the publisher then deletes each page that SUMMARY no
longer lists, including stale pages inside a current section, so a renamed
chapter gets a new URL. After the merge it adds a permanent site redirect from
each deleted page's URL to the current page with the same file name without
its number (`ch20-inside-coax0` reaches `ch27-inside-coax0`), or to the
book's first page when there is none. A link to another book's README goes to
that book's public URL. Any other repository link goes to the file at the
published commit on GitHub. The script verifies the draft inventory and the
merged revision, then reads the live page tree and fails if it has an extra,
missing, duplicate, retitled or out-of-order page compared with SUMMARY. It
does not change visibility, create a site or export content back into Git.

To run only the post-publication comparison, which changes nothing:

```sh
python3 scripts/publish-coax-book.py --book coax --check-live --token-file /absolute/path/to/gitbook.env
```

An API error or timeout is not success. Inspect the printed draft before
retrying; `--change-request ID` resumes that draft for the same source commit.
The legacy repository-import endpoint can accept a request without advancing
an unconnected space; the publisher therefore uses the draft API.

Finally inspect the public book and its new pages. A merged API revision
is useful evidence, but a public render and working crosslinks verify the
reader's path too. Existing page URLs should remain stable.
