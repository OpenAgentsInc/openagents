# Serving Kiln from a site

How a host such as `openagents.com/admin/kiln/` serves the Kiln page, and
how that page reaches a `kiln-runner` over the relay.

## What the host serves

`scripts/build-kiln.sh` writes a static directory (`$KILN_OUT`):

```text
index.html  review.html  worker.js
pkg/kiln.js  pkg/kiln_bg.wasm
tools/manifest.json  tools/<sha256>.wasm ...
```

The host serves that directory under one path prefix, plus a
`config.json` of its own:

- **Relative URLs only.** The page, the worker, the tools, and
  `config.json` are all fetched relative to the page, so any prefix works.
  The page must be served at a URL that ends in `/`
  (`/admin/kiln/`, not `/admin/kiln`); redirect the bare path.
- **Cross-origin isolation.** Send `Cross-Origin-Opener-Policy:
  same-origin` and `Cross-Origin-Embedder-Policy: require-corp` on the
  prefix, and `Cross-Origin-Resource-Policy: same-origin` on its files, as
  `kiln-dev` does. Today nothing needs `SharedArrayBuffer`, but the page
  reports isolation and later kernel work will need it. The page's
  cross-origin reads (GitHub's API, `raw.githubusercontent.com`) send CORS
  headers, so they work under `require-corp`.
- **Content types.** `application/wasm` for `.wasm` and
  `text/javascript` for `.js`; `worker.js` is a module worker.
- **`config.json`:**

  ```json
  {
    "runner": "<kiln-runner public key, hex or npub>",
    "repo": "30617:<repository key>:<repository id>",
    "relay": "wss://relay.openagents.com"
  }
  ```

  Every field is public. The page also takes `?runner=`, `?relay=`, and
  `?load=crates/NAME` in its URL. Without `config.json`, the relay defaults
  to `wss://relay.openagents.com`, and **Test runner key** is left empty
  for the person to fill in.

Admin-only access is the host's job (its sign-in in front of the prefix).
The page itself holds no secret and needs none.

## How the page reaches the runner

There's no HTTP path between them. Both are Nostr clients of the same
relay:

1. The page makes a fresh key in the tab at each load, signs the relay's
   NIP-42 challenge with it, and sends each `cargo` job as a kind `25900`
   request NIP-44 encrypted to the runner's key, tagged `p` with that key.
2. `kiln-runner` keeps a subscription for `25900` events tagged with its
   key, reopened before each connection's 115-second lifetime ends. It
   decrypts the job, checks it, runs it, and answers with `27000` feedback
   and a `26900` result encrypted to the page's key.
3. The relay sees only ciphertext and the two keys.

So a production page needs nothing from the site's backend to run tests:
only the runner's public key in `config.json`, and a runner that is up and
connected to the same relay.

## The runner's key and identity

- **Its own key.** `kiln-runner --state DIR` makes a secp256k1 key on first
  start and keeps it in `DIR/key` (mode `0600`, hex). It prints the public
  key and writes it to `DIR/pubkey`. The key is a service identity: it
  signs feedback, results, check comments, and (for the demo) the
  repository announcement. It holds no money and grants nothing.
- **Not the owner's key, and not a site key.** Don't give the runner the
  owner's personal key or a key the site uses for anything else. A host
  that wants a stable runner keeps its state directory (and so its key)
  across restarts.
- **Its repository.** By default the runner publishes a demo NIP-34
  announcement under its own key, `30617:<runner key>:openagents-kiln-demo`,
  and accepts patches only to that address (written to
  `DIR/repository`). When the owner publishes the real `openagents`
  announcement with a repository key (see
  [workstation.md](workstation.md)), the runner should run with
  `--no-announce`, accept that address, and be listed as a trusted check
  key there; that is phase 3 work.
- **Who may use it.** Today the runner accepts a job from any key, one at a
  time, for its allowed crates only. That is acceptable behind an admin
  page on a machine with nothing else on it; it isn't acceptable for a
  public page. Before Kiln is public, add an admission list (the admin
  pages' keys, or keys with an owner attestation) and per-key quotas, as
  [workstation.md](workstation.md#access-control) describes.
- **Where it runs.** Not on the owner's Mac and not on a production web
  host: on a dedicated build machine with a Rust toolchain and a warm
  Cargo cache, with `--state` on a disk that can hold a shallow clone,
  one worktree, and one target directory (about 5 GB for `memory-stream`'s
  dependencies, more for larger crates).
