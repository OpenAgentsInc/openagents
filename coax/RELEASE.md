# Release and implementation status

[`release.coax`](release.coax) is the single authored catalogue for the current
compiler version and feature status. It renders the CLI metadata, JSON inventory,
accepted-feature reference and book status. [`release-model.coax`](release-model.coax)
checks its identities, fields, status states and historical versions. The model
is an authority unit with checks and a zero-survivor mutation gate.

## Three version domains

- `IMPLEMENTATION_VERSION` identifies the compiler release. The current format
  is three numeric components, each 0–999, without leading zeroes or prerelease
  suffixes. Extending that format needs a model and schema review.
- `STATUS_SCHEMA_VERSION` identifies the JSON inventory shape, currently 1.
  A compiler release does not increment it automatically. It is unrelated to
  protobuf, reflection or signed protocol schema versions.
- A feature's `introduced` records its historical implementation version.
  It does not change when the compiler version changes. `null` in JSON, or an
  em dash in the tables, means unrecorded or unimplemented; it does not mean
  the feature arrived in the current release.

The stable feature `id` identifies a reviewed claim. `works` is an implemented
bounded feature, `partial` states its remaining limits, `reserved` is recognized
and refused, and `designed` is planned. Updating a status requires reviewing the
actual implementation and its evidence. Generation proves consistency of these
claims across surfaces; it does not prove the claims themselves, general compiler
correctness or authority to execute a decoded request.

## Updating a release

From the repository root:

```sh
# Edit the version or feature records in coax/release.coax first.
scripts/coax-release.sh --write
coax/bin/coax test coax/release-model.coax
coax/bin/coax mutate coax/release-model.coax
coax/build.sh
scripts/coax-release.sh --check --binary coax/bin/coax
python3 -m unittest discover -s coax/tests -p release.py
```

Run the model's checks/mutation when its policy changes. The normal bootstrap
already runs them; an unchanged model does not need an additional manual run.
Build the compiler once when its embedded metadata changes. Ordinary prose
edits outside managed regions need only the relevant documentation checks.

The Coax generator emits these checked-in snapshots:

| Output | Consumer |
| --- | --- |
| `src/release.h` | `coax version`, `coax status` and `coax status --json` |
| `release-status.json` | Tools consuming inventory schema 1 |
| `STATUS.md` | Current human-readable status |
| `book/appendix-a-keywords.md` | Published accepted-feature reference |
| `generated/current.md` | Bounded current-version regions in README/spec/roadmap/book |
| `generated/version-example.md` | The book's CLI version example |

The shell adapter compiles/runs the Coax generator, transports its fixed output
bundle and compares or writes files. Metadata validation and rendering live in
Coax. Normal generation, bootstrap and app generation need only the Coax/native
toolchain, shell and standard file utilities. Python is optional test tooling;
the separate GitBook API publisher also uses Python.

Every managed region is validated before writes begin. On macOS the catalogue
is compiled for the Mac host, even when Xcode passes an iPhone/Simulator SDK;
`COAX_HOST_TARGET` supplies an explicit host triple when needed. The tool must
execute its generator locally, so an inherited cross-target SDK is unsuitable.

`--write` updates only the listed snapshots and explicitly marked regions. It
does not replace version numbers in historical prose or generate the error-code
catalogue, grammar or every explanatory paragraph. Keep those accurate through
review. Add a managed region explicitly if another document needs current status.

## Bootstrap, application and publication gates

The snapshots are committed so a clean checkout can start from its reviewed
frontend C seeds. When no compiler exists, `coax-release.sh` builds a temporary
stage0 generator frontend using those seeds and the checked-in metadata header.
It does not publish that temporary compiler. `COAX_RELEASE_COMPILER` can select
an already trusted compiler, including for recovery of stale generated files.

`coax/build.sh` validates the model and compares generated files before the
expensive compiler gates. Before publishing the final compiler, it compares
that binary's version, Markdown and JSON with fresh generator output. A matching
version string alone cannot hide stale feature data. The native frontend,
generator, shell adapter and toolchain remain trust inputs.

The iOS generator performs the same source/snapshot/binary drift check before
emitting application C. The book publisher does so before reading credentials
or creating a remote draft; its existing clean, pushed-main and page checks
still apply. A drift failure is fixed locally with the canonical source and
`--write`, followed by rebuilding only if embedded binary metadata changed.
Neither app nor book publication silently repairs source files.

The [focused evidence](../docs/verification/coax-release-status-2026-10-28.md)
records the actual tests and their limits. [R0](ROADMAP.md#3-r0--establish-the-adoption-gate-now)
remains open for device/frame-tail measurements, complete development-loop
attribution and independent review enforcement.
