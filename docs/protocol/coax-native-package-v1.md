# Coax Native package v1

The installed iOS starter speaks `coax.native.package.v1`. Its one codec is
[`updates-wire.coax`](../../coax/native/updates-wire.coax); the
[golden manifest](../../proto/coax-native/package-v1.json) is checked by
`coax/web/tool.sh coax/native/profile.coax`. This profile is immutable.

The envelope is ASCII `CNU1`, four little-endian bytes giving the manifest's
byte length, those exact UTF-8 JSON bytes, then a 64-byte Ed25519 signature.
The signature covers UTF-8 `Coax Native Package v1` followed by a zero byte
and the exact manifest bytes. Verify before parsing. Maximum manifest size
is 16,384 bytes; no trailing envelope bytes are accepted.

The manifest has exactly `schema`, `app`, `runtime`, `generation`, `files`.
`app` is the installed bundle identifier. `runtime` is a lowercase 64-digit
SHA-256 fingerprint of the installed service implementation and build
contract. `generation` is an integer 1 through 2,147,483,647. Each file has
exactly `path`, `sha256`, `bytes`. Paths are unique, relative ASCII paths
with only letters, digits, dot, hyphen, underscore and slash; empty, `.` and
`..` segments are refused. `index.html` is required. Limits: 24 files, 8 MiB
per file, 32 MiB total. Artifact digests are lowercase SHA-256 hex.

The static feed serves `latest.cnu` and immutable `objects/<sha256>`.
The publisher uploads and verifies all artifacts before promoting the signed
envelope. Transport cache or redirection cannot authorize another payload.
The client records a verified high-water envelope and refuses lower or equal
generations for staging. A deliberate rollback is a new, higher generation
containing old compatible code. Locally it may boot a previous healthy
generation for recovery without reducing the high-water mark.

The private key never belongs on the public service. The installed public
key and runtime can change only in another native build. This first profile
has no expiry, delegated keys, rollout cohorts or deltas; those parts of the
broader Updates proposal remain future work.
