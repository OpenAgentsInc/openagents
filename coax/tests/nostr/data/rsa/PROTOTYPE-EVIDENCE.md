# RSA prototype evidence (2026-10-28)

This is incomplete work for #bd06, not an approved FI verifier. No claim, key
source, JOSE or possession authorization uses this prototype. The authority
mutation gate is **not passing**: 893 of 936 mutants killed, 43 survived.
No waiver or equivalence claim has been applied. The high-budget positive
check is separate because it uses 232,573,461 interpreter steps and the
ordinary mutation runner does not expose a steps option. The next worker must
strengthen the authored checks and obtain zero survivors without exemptions,
then independently review the implementation and finish every task gate.

The original prototype source and Python harness are retained as `prototype-v1.coax.txt` and `prototype-v1-test.py.txt`; subsequent work may change the live files.

Working tree base: `d5fc7e58328986e6e16a5b62609fc181d5c439b7`. Compiler used:
`/Users/christopherdavid/.codex/worktrees/coax-nostr-budget-compat/openagents/coax/bin/coax`,
SHA-256 `56f87661b9f1f105598132bf5c45f13e07fea46f4f23249921a22111d58f3ee4`.
That compiler worktree was clean at revision
`ed02790f1ef8712b14172e9846ecb2ccb490ec10`; its `coax/src/main.cpp` SHA-256 is
`cf01ca5c7346ee45d445d079eea4a66f5b7837b1bfee19036b299d286a9c8918`.
This records observed artifacts, not proof of the binary's build provenance.

Observed source and generated C hashes:

| Artifact | SHA-256 |
| --- | --- |
| `coax/lib/crypto/rsa_verify.coax` | `d95676ca81b82a5253563d30cfc72946471622d00db65056dc04dc76ac4aa16d` |
| `coax/tests/nostr/rsa_positive.coax` | `462c00066d0b469460a1133c6a2766ae3af83260ccd50a845bfc41dcaafaf8ad` |
| `coax/tests/nostr/rsa_verify.py` | `bb8503ea501a190b029e4edaed50b346d5e8b89080ea9c4bc1c0881333211971` |
| `coax/tests/nostr/rsa_verify_wasm.c` | `f8178499327bb662af63088081e0edbaf7d2dc81e2348a9358f73a36c27c15db` |
| `coax/tests/nostr/rsa_verify_wasm.cjs` | `3cf3936a598375989844d5bff89058e68acab02363427e722ca7dea203e4f582` |
| checked generated C | `4623d5f0cf5f99aa7e9d364d34c0355f321e3cf39719a5c0d9a92803b9387ed1` |
| release generated C | `cc6f8addeb5981f7d0cc0c77b15f823a9987dc3834493e7a193762f691d58b0a` |

The independently sourced test files are the exact C2SP Wycheproof files at
commit `d17cc89fa5c8f4f4d378b5385ebf75da5d0926c9`, in `testvectors_v1/`:

- `rsa_signature_2048_sha256_test.json`: `94a917b01ff50fb874cfc05bf29b4af44868d944a6558201cf18380da93fb393`
- `rsa_signature_3072_sha256_test.json`: `0f5f18cabfaad3e2792e82f7e9882f8999049b456714de924b8a5e202f61ca43`
- `rsa_signature_4096_sha256_test.json`: `957aca128e30bd02c8982f8ca482d6521d16683f4bbc3f193cdfd4c8f04bc667`

The Python harness checks these hashes, selects e=65537, builds checked and
release native code, and compares 773 cases per mode to independently computed
`pow(signature, 65537, modulus)` and exact PKCS#1/SHA-256 encoding. Each
Wycheproof valid/invalid label is also checked against that oracle. It passed
both native modes and both Wasm modes (773 cases each), using Node WASI. The ordinary Coax file check passed 43/43 loaded checks;
`coax test coax/tests/nostr/rsa_positive.coax --steps 1000000000` passed 44/44.
`coax mutate coax/lib/crypto/rsa_verify.coax` failed with 43 survivors.

Only public arithmetic is intended. The fixed exponent is an interface
assumption; callers must separately require JWK e=65537 and the other profile
parameters. Message hashing inherits the SHA-256 input cap. No side-channel,
security review, target deployment, NIST, JOSE, browser-network, device, or
end-to-end FI assertion claim follows from this evidence. The other downloaded
RSA and JWS files in this directory are retained from the previous worker and
are not used by these results.
