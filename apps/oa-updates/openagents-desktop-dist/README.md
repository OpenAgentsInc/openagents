# OpenAgents Desktop release seed

This directory is intentionally present in the update-service image even when
no Desktop release has been published. Until it is populated, the Desktop feed
routes fail closed with `404`.

Populate it ONLY with the scripted publish flow (CUT-26, openagents#8706):

```sh
bun apps/openagents-desktop/scripts/publish-release.ts \
  --channel <stable|rc> --version <X.Y.Z[-rc.N]> --artifact <OpenAgents.dmg|.zip>
```

That script writes the bounded `openagents-desktop-release.json` descriptor
(`{ "releases": [...] }`, one latest entry per channel; the original flat
single-release shape is still accepted on read) plus the versioned signed
manifest bytes and detached ed25519 signature files it references. It enforces
version monotonicity and channel rules via the desktop update contract and
self-verifies every signature through the client verification seam before
staging — hand-editing files here bypasses those gates and will be rejected at
seed/client verification.

Artifact bytes are NOT stored here; they live behind the credential-free HTTPS
`artifactUrl` (GCS) named in the descriptor, and clients gate the download on
the SIGNED sha256/byteLength.
