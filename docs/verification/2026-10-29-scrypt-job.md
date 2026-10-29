# Experimental scrypt job composition

The internal Coax job combines the existing resumable NIP-49 scrypt function
with cleanup/publication policy. It computes into a staged key and will not
publish it until scratch and internal state have been acknowledged as erased.
A failure or cancellation adds the staged key to the cleanup obligations.
Cancel after computation but before publication still blocks publication and
requires a staged-key wipe. The first failure or cancellation remains sticky.
A successfully published copy and the retained staged key must be erased when
the caller no longer needs them. This is not an automatic lifetime guarantee.

The test host performs actual observable erasure before acknowledgments. It
compares a complete scrypt result with an independently computed hashlib vector
and exercises success, cancellation after computation, bad-view failure, output
guards and a real two-chunk 2 MiB cancellation wipe after ROMix has written a block. Checked and release native
runs use ASan/UBSan; Wasm runs the actual generated module. The complete fixture also executed in arm64 iOS 26.5 Simulator in both
checked and release profiles. This is not a physical-device test.
These focused examples are not a proof of every underlying effect or call path.

This is not a public pointer-admission wrapper or production-secret approval.
The caller must zero-initialize the session, serialize every call and result
access, bind the same owned scratch allocation and length for its lifetime,
validate addresses and all aliases, keep state separate from wiped buffers,
quiesce writers before wiping, and report only successful real erase effects.
The input password and any exported result have their own caller-owned lifetimes.
A failed start may still leave cleanup pending; check `sj_next` after any attempted
start. `sj_cancel` means the request was processed, not that cancellation replaced
an earlier failure. Boolean acknowledgments and a forged state are not evidence
of real erasure. Scratch indexing is password-dependent; compiler stack/register
copies, snapshots, process crashes and hostile mutation remain outside the model.
NFKC, validated host integration, target security analysis, and physical-device
execution remain open before the NIP-49 task can be completed.

The complete loaded graph passed 75/75 Coax checks and killed all 1,726/1,726 mutants. The scheduler ran 21,410 full checks without exclusions (borrowed/string fallbacks were active). An earlier full pass killed 1,673/1,699 and prompted stronger guard and failure tests; no equivalence waivers were used. `sj_next` distinguishes invalid, running, complete and published states from wipe-region requests; callers must not acknowledge a nonpositive status. The integration
runner uses Python only as local test tooling, not as a build/runtime dependency.

```sh
coax/bin/coax test coax/lib/crypto/scrypt_job.coax
coax/bin/coax mutate coax/lib/crypto/scrypt_job.coax --mutation-stats
COAX_IOS_SIMULATOR=<booted-simulator-udid> COAX_IOS_LARGE=1 python3 coax/tests/nostr/scrypt_job.py
```

The runner used coax0 0.22.0
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.
Checked generated C SHA-256 `338760ce1b58df3363d4980c1f238ed068d6a102f6ec0ea86a084c4875ed10d6`,
Wasm `ffc5b2ce1c3394593b2086d3359e69644b4655b8298084224bb5ca9df16414ab`;
release C `8ff801d7a31a5b12b25a90b5b7cc20e339b8b108e869009a43392ca115bc7c8d`,
Wasm `64a6fca2fd888823e90c23c5456aec92dfcdcce1d1a9084cfde6380099873565`.

Independent source review covered the job logic before final test-only additions, source
`03c61d564628f7639c614fe0774d4350ae8f68146e4fa1de894d018616810496`
and fixture `19c84b726311da76dd09a0c456bfb05c0aa732b69127fda3b69e504636279722`.
It found no publication-before-cleanup bypass within the stated trusted-host
contract, and prompted the explicit failed-start seam and populated large-scratch
cancellation test. The seam exercises an injected false result, not a claim
that a valid in-profile start naturally fails. Review is not security approval.

The checked iOS Simulator fixture binary SHA-256 was `8d7f0624d6dad6d055862b828d1cc3df23629bad47fdb4cc1af5c8f685202ed6`, and the release fixture was `4c62dc604a21794a680aaaf95451b69f211599947856d92b59a8bf14904a4aa8`. Both were executed in a dedicated iPhone 17 Pro simulator with iOS 26.5. Without the environment variable, the runner compiles the generated Coax code for the simulator but does not execute it.

The separate 64 MiB iOS Simulator fixture (`coax/tests/nostr/scrypt_job_large_ios.c`, SHA-256 `042aa6a806594b15a532ddd2a5b7b6ed9a13522870e4900c4b856955ce2c8a11`) ran N=65536, r=8, p=1 to completion against an independent hashlib vector. In each profile it wiped all 64 chunks, checked every scratch byte, kept guard bytes intact, verified the state wipe and checked the key, output and password after their explicit wipes. Checked simulator executable SHA-256 `5cc321a2ad4c9ccaae06bc7ce6e64b7cb4650929f51cc95631c011f1fc81413c`; release `e3187ab32db1e3cf61fec1f446549d5a128332bdee83bb79c302c9b22df9f7f7`. This is a public-test success path; the smaller fixture covers cancellation/failure. Physical-device and hidden-copy guarantees remain open.

The final job source SHA-256 is `7c0b0b08bf5246c59884a6f7bf539b6a8edb2cdaf25ce74b168538ed2472f2ed`.
