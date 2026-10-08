# Cloud workspace privacy

This Rust/Wasm runtime guards the visible lifetime of private account content.
It observes server session standing; it grants no host, provider, or product
authority and performs no writes.

The page supplies `.cloud`, `#cloud-private`, `#cloud-standing`, and
`#cloud-resume`. All private content belongs inside `#cloud-private`, initially
hidden. `#cloud-standing` contains bounded session identity and membership
metadata, including a required `projection_digest` in `sha256:` plus 64
lowercase hexadecimal characters. The server digest covers the complete
rendered account and workspace projection, including unselected memberships.
This metadata contains no credentials. `#cloud-resume` contains safe navigation
to `/cloud/app` for fresh server admission and a local logout form whose signed
ticket contains only a session digest, origin, scope, and expiry.

`start()` validates initial standing, then reveals the mount only after a
current server response. It polls the same-origin session endpoint serially,
with an eight-second timeout and a
16 KiB response bound. Lost visibility, page departure, expiration, changed
authority, or an unavailable response clears private DOM and input values.
Returning to the page requires navigation; retired content never resumes.
No credentials or drafts enter browser storage.

The sign-in page supplies `.cloud` without a private mount. Its password
fields clear on visibility loss, departure, and return while the form remains
available. If the generated runtime cannot load, the loader clears password
fields and replaces the form with safe navigation to reopen the workspace.

Build with `scripts/build-coder-cloud-web.sh OUTPUT_DIRECTORY`. Generated
JavaScript is loader glue; application behavior stays in Rust.
