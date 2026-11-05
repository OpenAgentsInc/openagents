# Portable canonical CPU image-cache component

The reusable Coax authority and effects-only C SDK are described in
`aethyr/render/host-image-cache.md`. This component introduces neither a
platform codec fallback nor a GPU readiness claim. The caller still owns
physical source routing/containment and texture/ledger effects.

The actual SDK passes the same complete 18-field receipt normally and with
AddressSanitizer/UndefinedBehaviorSanitizer:
`[7,64512,64512,1,1,1,1,1,1,112,1,0,8,134217728,1,1,0,1]`.

- All seven original compressed hotbar images produce exactly 64,512 bytes
  matching the frozen canonical bank, with natural 48×48 dimensions.
- A repeated identity retains its bytes and performs no new source read.
- Missing, malformed-before-plan, malformed-during-decode, short-read,
  negative-read and oversized sources refuse without retained decoded bytes.
  Missing/malformed observations produce the exact 607 `URL 0 0` text.
  A later valid image still decodes successfully.
- Exactly 112 immutable source identities fit; the next creates no retained
  entry and performs no source effect.
- Eight actual 2048×2048 decoded buffers fill 134,217,728 bytes. The ninth
  image is failed/not-ready before pixel allocation; an existing owner
  still reads successfully without another source effect.
- Close frees physical buffers and reports the actual post-release Coax
  residency as zero, measured before destroying the ownership state.
  The source, URL map, canonical outputs and executable remain unchanged.

The admission gate kills 111/111 root mutants, and the independent complete
receipt authority kills 74/74, without survivors or equivalents. Initial
mutation results exposed two missing URL boundary checks and redundant
allocation predicates; these were sharpened or removed, with no equivalent
annotations. Initial builder failures (duplicate foreign declaration and a
string instead of a byte-view FFI) occurred before execution and are retained.
Earlier receipts lacking the decoder-late-failure/post-free observation are
kept separately; the published receipts include both.

The pressure JPEG is a test-only SDK-produced 2048×2048 resampling of the
original meteor-swarm JPEG. Its exact bytes are frozen and SHA-pinned; it
does not replace an authored asset or establish pixel parity by itself.
The seven original files and full canonical outputs are the byte oracle.
Generation used `/usr/bin/sips --resampleHeightWidth 2048 2048 SOURCE --out
PRESSURE`, with the source and log retained. No generation SDK is needed by
the shipped cache or Linux fixture consumer.

The decoder/ownership C emission has an actual compiler profile and 27
source-table entries, all matching the separate frozen source closure.
The unchanged admitted b587 Mac 0.50 compiler is verified using its original
receipt in the genuine 73b60ebdf5 root. Later compiler notice changes are
not relabelled as admitted by that receipt. This is a small component
emission, not a primary realm or compiler release qualification.

Complete raw source/profile/C/header, build argv, runtime source fence,
normal/instrumented output banks, earlier failures and pressure fixture are
under `~/.openagents/aethyrforge/metal-host-12868/image-cache/`.
The final SDKs are `published-{normal,asan}-sdk/`; final receipts and outputs
are `published-{normal,asan}/`. This record commits both receipts, exact
source join and artifact manifest. The Mac sanitizer run disables framework
leak detection and reports no ASan/UBSan findings; it is not an LSan claim.

This checkpoint qualifies the portable component. Subsequent Metal adapter
adoption and matching GPU uploads/607/recovery are recorded separately in
`image-cache-mac.md`; full current realm packaging remains a separate gate.
WebKit stays the default and both issues remain open.
