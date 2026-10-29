# Experimental BIP-340 nonce-point preparation

`snp_prepare` computes the tagged nonce hash for caller-supplied normalized key
and matching public x, reduces it, uses scalar one for fixed-work fallback when
the nonce is zero, and derives the nonce point. The nonce nonzero bit is kept
separate from the publishable validity bit until finalization also requires a
finite point. Metadata refusal leaves existing state untouched; after admission,
failed steps clear validity. A successful function return is not permission to
publish: the caller must check validity and complete signature verification.

The source SHA-256 is
`08513e024253cd076f1bbf620562e23a32ae7836899b2d97aa1dfb6f1f9ecfad`.
All 2,914 full authority mutants were killed. The independent harness checked
107 tagged hashes and pinned-libsecp nonce points in checked/release native and
Wasm builds, including a 1 MiB message; the iOS 26.5 Simulator exercised the
BIP-340 fixture and refusal. Independent review covered the validity transition
and documented prerequisites.

The caller must bind the normalized key to the supplied public key and provide
live, stable, disjoint views. Caller-owned work requires an actual wipe. This
slice does not erase the caller's key, by-value temporaries, compiler/ABI or
register copies, and does not establish target timing or production approval.
