# Dependency check

*Dependency check* reads your manifests and lockfiles offline and flags
three things: packages a lockfile holds at more than one version,
dependencies a manifest leaves loose (any version, no upper bound, or a
Git branch instead of a commit), and licenses your declared policy
doesn't allow. It's one of the [example plugins](README.md): copy it to
make your own.

```text
You:   Do any of our npm dependencies have a license our policy does not allow?
Coder: ## Dependency check
       Read `package.json`, `package-lock.json` (4 packages). Policy:
       `dependency-policy.toml` (allows 5 licenses, denies 2); 5 licenses
       checked.
       ### Licenses the policy doesn't allow (2)
       - pdf-render@1.2.4 is licensed AGPL-3.0-or-later, which the policy denies
       - font-pack@0.9.1 is licensed OFL-1.1, which the policy doesn't allow
```

## What's in it

| Part | File | What it does |
| --- | --- | --- |
| Wasm | [`src/lib.rs`](../../../crates/plugin-dependency-check/src/lib.rs) | The `check` operation: reads the policy, the manifests, then the lockfiles, and reports what it found and what it couldn't check. |
| Workflow | [`programs/dependency-check.json`](../../../crates/plugin-dependency-check/programs/dependency-check.json) | One `module` step that reads the usual manifest and lockfile names at the project root when they exist (`read_present`), and any other the request names (`read_named`), such as `crates/api/Cargo.toml`. |
| Plugin record | [`package.json`](../../../crates/plugin-dependency-check/package.json) | The name, the summary, the publisher, and the workflow's digest. |
| Tests | [`evals/`](../../../crates/plugin-dependency-check/evals/) | Four tests where it should help and two where it should stay out of the way. |

## What it reads

| Ecosystem | Manifests | Lockfiles |
| --- | --- | --- |
| Cargo | `Cargo.toml` | `Cargo.lock` |
| npm, pnpm, Yarn, Bun | `package.json` | `package-lock.json`, `npm-shrinkwrap.json`, `pnpm-lock.yaml`, `yarn.lock`, `bun.lock` |
| Python | `pyproject.toml`, `requirements*.txt` | `uv.lock`, `poetry.lock` |
| Go | `go.mod` | `go.sum` |

The policy is `dependency-policy.toml`, or cargo-deny's `deny.toml` when
there is none:

```toml
[licenses]
allow = ["MIT", "Apache-2.0", "BSD-3-Clause", "ISC"]
deny = ["GPL-3.0"]

[ranges]
allow_unbounded = false   # true: a lower bound alone is fine
exact = false             # true: manifests must pin exact versions
```

Licenses are SPDX expressions: `OR` needs one acceptable side, `AND` needs
both, `WITH` reads as its license, and `GPL-3.0` in the policy matches
`GPL-3.0-only` and `GPL-3.0-or-later`.

## What it flags

- **Duplicate versions.** A package a lockfile holds at more than one
  version, most versions first. For Go, a module required at two major
  versions (`x/kit` and `x/kit/v2`).
- **Loose versions.** `*`, `latest`, an empty requirement, or a lower bound
  with no upper bound (`>=1.0`, `>2`), in any manifest. Cargo's `1.2` and
  npm's `^1.2.3` are bounded and pass, unless the policy asks for exact
  versions.
- **Unpinned sources.** A Git dependency with no `rev`, `tag`, or commit
  hash: it follows whatever the branch holds.
- **Licenses.** A package whose license the policy denies or doesn't
  allow.
- **No lockfile.** A manifest with no lockfile of its ecosystem beside it
  or at the root.

## What it doesn't check

It says so instead of guessing. Only npm lockfiles and manifests record
licenses; Cargo, pnpm, Yarn, Bun, and Python lockfiles don't, and the
result counts their packages as not checked (`cargo deny check licenses`
reads them from the registry, which an offline check can't). One call
reads at most 64 KiB in all, so a large lockfile is cut, and the result says
which file was cut and that what's past the cut isn't counted.

## Copy it

To check something else in the same files (a banned crate, an outdated
pin, a dependency your team forbids), copy the crate and add a finding
kind beside `duplicate` and `unbounded`. The small TOML reader in
`src/lib.rs` handles tables, arrays, and inline tables, which covers
manifests and lockfiles. The steps are the same as for
[Explain this error](explain-this-error.md#copy-it).

## Test result

On 2026-10-01 on our computers (3 runs per arm, Gemini Flash, live Jev,
no shell in either arm): **Better**, 6 of 6 tests with the plugin and 2 of
6 without it. Without it Coder can't read the manifests and lockfiles at
all. With it, one of three `python-requirements` runs looked at nothing:
Jev didn't choose the workflow for that wording. Published as
`49357306398ef037f72bfa7c8a98a9295a7fe0e7c59e5bf2c3c1373d2d604536` on the
test set release `a6b800a63b4e…`; details in [the measurement](../../extensions/measurements/2026-10-01-example-plugins.md).

## Read-only run on a real repository

On 2026-10-01, on scratch copies (`git archive HEAD`; the real checkouts
were only read):

- The owner's `probe` repository: it read `Cargo.toml`, `package.json`,
  `Cargo.lock` (104 packages), and `bun.lock` (28 packages), found `syn` at
  2.0.119 and 3.0.3, `@types/node` at 22.20.1 and 26.2.0, and
  `undici-types` at 6.21.0 and 8.3.0, and said licenses aren't checked
  because the repository declares no policy.
- The owner's `openagents` root (`Cargo.toml`, `Cargo.lock`, `deny.toml`):
  it read the `deny.toml` policy (nine allowed licenses), listed ten crates
  held at more than one version (`base64` at four), and said that
  `Cargo.lock` was cut at the read limit after 160 of its packages and
  records no licenses. A lockfile of 313 KB is past what one call may
  read; the result says so rather than reporting a clean tree.
