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
