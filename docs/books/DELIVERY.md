# Supported routes and publication evidence

The [route catalog](routes.json) and [reader paths](paths.md) define four
bounded native profiles. Their entry commands run existing Coax examples and
one local C/protobuf boundary. They require no GPU, dataset download, user key,
network service or visible browser. They do not complete the broader
[application milestones](completion.md).

## A clean checkout

Start with the normal repository checkout and an existing qualified Coax
executable. [Coax's build instructions](../../coax/README.md) describe obtaining
one; compiler bootstrap and qualification are separate from the route's time
budget. A compiler version string alone is not a qualification record.

```shell
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
scripts/check-books.sh --check
scripts/check-books.sh --route coax-current-ffi \
  --coax /absolute/path/to/qualified/coax --receipt /tmp/coax-route.json
```

`--check` reads catalog and book metadata, registries, excerpts and local links.
It invokes no compiler, route example, network or training job. The release
workflow in `coax/tests/run.sh` includes catalog validation and the focused
runner/publication tests. Qualify one named route per command, so its complete
execution stays within the declared bound.

The other route IDs are `aethyr-core-replay`, `psionic-public-models` and
`nostr-event-client`. [Their profiles](routes.json) record the precise scope,
prerequisites, output fixtures and remaining limitations. The Psionic route
actually trains/resumes the existing tiny synthetic module; its MLP/CNN steps
are dataset-free API examples. No route repeats the frozen MNIST experiments.
The Aethyr route proves CPU scheduling/input/replay, and the Nostr client route
uses scripted transport bytes. Browser rendering and live relay transport need
their own evidence.

## Check only what changed

Use a new receipt filename even after a failure:

```shell
python3 scripts/book_routes.py --route coax-current-ffi --plan \
  --coax /absolute/path/to/qualified/coax --previous /tmp/coax-route.json
python3 scripts/book_routes.py --route coax-current-ffi \
  --coax /absolute/path/to/qualified/coax --previous /tmp/coax-route.json \
  --receipt /tmp/coax-route-next.json
```

Planning performs source discovery when needed, which can invoke Coax's
front-end checks; it does not execute the route examples. Unchanged valid
closure information avoids rediscovery. The metadata-only `--check` needs no
compiler at all.

A retained step must have the same recipe, mode/options, effective environment,
compiler and host C tool identities, host target and all input hashes. Coax's
dependency report supplies actual source/descriptor inputs. Host commands must
declare their script and other authored inputs; this is an explicit reviewed
contract, not automatic tracing of every host file read. Transitive source
changes invalidate their readers; unrelated route edits do not require another
whole-book execution.

Raw stdout and stderr are retained beside the receipt. Their bytes, hashes and
exit status must match the expected fixture; missing or changed logs cannot
supply a retained pass. Passed prior observations keep their origin receipt
hash and observed Git revision. They are not relabeled as freshly executed or
published. Before final success, the runner rechecks earlier inputs, tools,
runner, environment and outputs. Failure/interruption preserves the current
receipt and any captured output. Each subprocess is a bounded process group;
the whole route defaults to 810 seconds and accepts at most 840, leaving room
for cleanup within the 15-minute command limit.

Receipts include the Git HEAD observed at execution, but a dirty worktree is
not called that commit's source snapshot: the recorded file hashes identify
the actual inputs. Reuse relies on trusted historical receipts. Integrity
checksums detect damaged records; they are not signatures or execution
attestation. Tool/host identity does not hash every operating-system/SDK
internal or establish compiler correctness.

## Keep publication identity separate

Publishing still requires a clean, pushed checkout and the existing local
GitBook credential setup. The publisher checks source and book structure before
reading credentials or changing the remote draft. For a real publication:

```shell
python3 scripts/publish-coax-book.py --book psionic \
  --receipt /tmp/psionic-publication.json
python3 scripts/publish-coax-book.py --book psionic --check-receipt \
  --receipt /tmp/psionic-publication.json
python3 scripts/publish-coax-book.py --book psionic --check-live --require-receipt \
  --receipt /tmp/psionic-publication.json
```

The default receipt location is `docs/books/publications/BOOK.json`. A successful
publish atomically records the source Git commit, SUMMARY and ordered page
hashes, submitted Markdown hashes, change-request identity, merged revision,
observed live page identities/order, timestamp and public URL. It writes no
success record after a failed merge or inventory mismatch. Retain the receipt
in version control after publication; changing only this evidence file does
not require publishing unchanged book pages again.

`--check-receipt` is local and needs no credentials. Strict `--check-live`
first checks the local source receipt, then compares its exact recorded live
revision and page identities. Inventory-only checking remains available when
there is no receipt, but says so explicitly. Matching titles alone do not mint
source evidence. Submitted Markdown hashes are provenance of the request;
independent live document-body parity is not claimed.

Implementation and offline publisher tests do not establish that today's live
books were republished. [Publication status](publications/README.md) stays
unrecorded until an actual authenticated publication supplies a receipt.
