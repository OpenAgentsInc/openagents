# Standalone Labs client

This independent Cargo workspace pins the public SDK to
`1b55b50ae17dcd47315f803a8238ab7c3c3afe82`, including its optional `labs-cad`
preview checks. Copy this directory outside the repository;
it needs no sibling checkout, server, database, or private patch. Its lockfile
pins the dependency resolution. Package publication remains a separate decision.

Use a dedicated test account with CAD access. Supply its credential through a
named environment variable and check its expected account ID. The example
rejects the owner's account. Never put a credential on the command line.
Create a private, empty folder for request records and the result:

```sh
mkdir -m 700 /path/to/private-labs-run
cargo run --locked --manifest-path /path/to/sdk-labs/Cargo.toml -- \
  --token-env OPENAGENTS_TOKEN --expected-account YOUR_TEST_ACCOUNT_ID \
  --state /path/to/private-labs-run
```

The example creates a small CAD box, previews and saves a longer version,
checks one refused edit, restores a saved job request, waits for a rebuild,
reads the GLB header through a byte range, adds a note, reads the change list,
and deletes the design. It checks that the design is no longer readable.
It runs deterministic CAD work; it starts no model call, payment, computer,
or agent session. Results contain counts and file hashes, without credentials,
document content, or signed file URLs.

The caller owns account changes, cancellation, and private storage. Ctrl+C
stops the workflow's current request; cleanup uses a fresh cancellation token.
A local wait timeout does not cancel the remote job. The example explicitly
requests job cancellation before deleting the design. If cleanup is interrupted,
read the document ID from `cleanup.json` and run again with `--cleanup ID` and
the same account options. Use an empty private folder for the cleanup result.
The SDK itself reads no environment variables and writes no files.

Production is the default. For another deployment, provide both
`--api https://HOST/v1` and `--website https://HOST`. Authentication and all
Lab references stay bound to that deployment and account. See the
[Labs contract](../../docs/sdk/labs.md) and
[SDK client](../../crates/openagents-sdk/README.md#labs) for API limits,
retry rules, streaming, and optional local checks.
