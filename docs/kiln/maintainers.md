# Kiln for maintainers and owners

Kiln proposes changes to this repository as NIP-34 patches on
`wss://relay.openagents.com` ([workstation.md](workstation.md)). This page
covers the people around a patch: who wrote it, who may open it for
GitHub, and who lands it. `kiln-git` is the command for all of it:

```sh
cargo build --release -p kiln-git
```

Signing commands take `--key FILE` (64 hex characters) or `--as PROFILE`,
an `openagents` profile key in `~/.openagents/verse/PROFILE.key`
(`openagents key show --as PROFILE` makes one). Neither prints a secret.
`--dry-run` prints the event instead of publishing it.

## The repository and its maintainers

The repository is a NIP-34 announcement (`30617`, `d` = `openagents`)
signed by a **repository key** the owner holds. It lists:

- `maintainers`: the keys whose status events count and who may open a
  patch for the GitHub bridge. The repository key is always one.
- `kiln-runner`: the runner keys whose check comments count.
- `r` marked `euc`: the repository's first commit, found from the checkout
  you run the command in.

```sh
kiln-git announce --key REPO.key \
  --maintainer npub1...   --maintainer npub1... \
  --runner "$(cat ~/.openagents/kiln-runner/pubkey)"
# prints 30617:<repository key>:openagents
kiln-git show 30617:<repository key>:openagents
```

Each announcement replaces the last, so list every maintainer and runner
each time. `kiln-git state --key REPO.key` publishes the repository state
(`30618`) with `main` at GitHub's current commit.

Then point Kiln at it: set `OPENAGENTS_KILN_REPO` to the printed address on
the site, and run the runner with `--repository ADDRESS --no-announce`.
Until then Kiln uses the runner's demo announcement,
`30617:<runner key>:openagents-kiln-demo`.

## An agent's identity

Kiln signs patches and test runs with the browser's lasting **Admin key**
(under **Settings**). On its own that key is nobody: readers show its
patches as "by npub1..., unverified author". To make it yours, authorize
it with your own key:

```sh
kiln-git own npub1THE_ADMIN_KEY --as default --hours 720
```

It prints one line: a NIP-OA `auth` tag and a mandate
(`openagents.agent-mandate.v1`), both signed by your key, that let that key
act for you until the expiry, for `git.propose` (patches, pull requests,
comments, issues) and `kiln.test` (runner jobs). Paste it into Kiln's
**Identity** field. The page checks it and says "Patches show as an agent
authorized by npub1..., until ...". Every patch and runner job then carries
both tags.

- Readers show "agent, authorized by npub1..." and add "(maintainer)" when
  the owner is a maintainer.
- No mandate covers status kinds (`1630` to `1633`), so an agent can never
  mark anything merged.
- A runner admits an agent whose owner is on its admission list, and the
  job counts against the owner's quota
  ([rust-tests.md](rust-tests.md#who-may-run-jobs)).
- To stop an agent, let the identity expire: keep `--hours` short for keys
  you don't control. There is no revocation event yet.

## Reading patches

`review.html` lists patches to the repository with their author label, the
runner's check (counted only from listed runners and maintainers), comments,
and the newest status from a maintainer or the author.

## Checking, applying, and marking a patch

Every patch command takes the event id, and `--repository ADDRESS` when it
isn't the patch's own (`OPENAGENTS_KILN_REPO` sets the default).

```sh
kiln-git review EVENT                       # author, crates, base, check, status
kiln-git check EVENT --runner RUNNER_KEY    # have a runner test it again
kiln-git apply EVENT --checkout . --into ../kiln-EVENT
kiln-git status EVENT applied --commit SHA  # after it lands
```

- `check` sends the runner a job with the signed patch, signed by your key,
  so your key (or its owner) must be on the runner's admission list. It
  tests every crate the patch changes and publishes the runner's check
  comment. It waits up to 15 minutes, reconnecting to the relay as needed;
  a cold runner can take about ten.
- `apply` refuses a patch without a passing check from a listed runner of
  exactly this text at its base, a patch that touches anything but crate
  sources, one that seems to hold a secret, and, unless you pass
  `--allow-unverified`, one from an unverified author. It makes a new
  worktree at the patch's base with a branch `kiln/<first 12 of the id>`,
  runs `git am -3`, and adds trailers: `Nostr-Patch`, `Kiln-Agent` and
  `Authorized-By` (or `Kiln-Author`), and `Kiln-Check`. The patch's own
  author stays the commit's author. `--onto origin/main` rebases it, and
  `--test` runs `cargo test` for its crates there, which runs the patch's
  code on your computer; the runner already ran it in a sandbox.
- `apply` never pushes and never touches `main`. Land the branch the usual
  way: the land queue, or a pull request a maintainer merges.
- `status` publishes `1630` (open), `1631` (applied, with
  `applied-as-commits` and `--merge-commit`), `1632` (closed), or `1633`
  (draft). It refuses a key that isn't a maintainer, apart from the
  author's own close or draft, because readers wouldn't count it.

`kiln-git propose FILE --base SHA --repository ADDRESS [--identity FILE]`
publishes `git format-patch` text as a patch, signed by `--key` or `--as`,
with the identity `kiln-git own` made for that key. Agents outside the
browser, such as the ones the Grid Foreman starts, propose this way.
