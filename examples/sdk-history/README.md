# Standalone account and history example

This independent Cargo workspace uses the public Rust SDK at Git revision
`4b5b70d72b1f1d142170417c283937876dc5734d`. Its lockfile pins the dependency
resolution. Copy this directory outside the repository; it needs no sibling
checkout, server library, GUI, or private patch.

```sh
cargo run --locked --manifest-path /path/to/sdk-history/Cargo.toml
```

Approve the displayed code on the website. The example discovers your account,
loads every owned summary page (including archived chats), reads the first chat
from beginning to end, downloads its available saved files and captures into
memory, and signs out. Pass `--chat ID` to open any authorized existing chat
without finding it in the sidebar first. Pass `--project ID` to list an
available shared project; the default output includes shared project IDs.
`--no-browser` prints the approval link without opening it.

The caller owns browser opening and credential storage. This example stores its
login only in memory, restores it into a second client, then clears it and
revokes an issued app session. It does not save a login across process restarts.
The native desktop uses its separate macOS Keychain store. An explicitly
imported sign-in key is forgotten locally on sign-out, not revoked.

For another deployment, provide both `--api https://HOST/v1` and
`--website https://HOST`. To use a dedicated test key, put it in an environment
variable and pass `--token-env VARIABLE_NAME`; never put a token on the command
line. `--verify` checks three disposable smoke fixtures without printing their
contents. The SDK does not read environment variables automatically.

The [public auth](../../docs/sdk/README.md) and
[history contract](../../docs/sdk/chat-history.md) describe the matching
[application schema](../../docs/sdk/openapi.app.json). Qualification records the
schema hash, deployed server, native application, and cleanup in
[the delivery evidence](../../docs/coder-desktop/delivery-evidence.json).

Ctrl+C cancels polling and page reads. Network failures do not trigger writes or
execution. This reader starts no engine, task, computer, or reply. Pages and saved
file sizes follow the SDK limits. The terminal example prints requested saved
content; use the dedicated test account for automated qualification.
