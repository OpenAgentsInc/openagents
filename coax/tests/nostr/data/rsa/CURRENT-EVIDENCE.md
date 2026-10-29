# Current public RSA prototype evidence (2026-10-28)

This is an unintegrated primitive for #bd06, not an FI verifier or an approval
of identity authority. The previous bit-serial prototype and its results are
retained separately in `PROTOTYPE-EVIDENCE.md` and the `prototype-v1` files;
`MONTGOMERY-EVIDENCE.md` describes an intermediate revision. These results
apply to the exact source bytes identified here, not to those prior revisions.

The task worktree started at `d5fc7e58328986e6e16a5b62609fc181d5c439b7`.
The tested compiler binary SHA-256 was
`56f87661b9f1f105598132bf5c45f13e07fea46f4f23249921a22111d58f3ee4`;
its observed worktree revision and compiler source hash are in
`PROTOTYPE-EVIDENCE.md` (not an attestation of binary build provenance).

| Source | SHA-256 |
| --- | --- |
| `coax/lib/crypto/rsa_verify.coax` | `9b4af0562d5b4e1364b2af316c69a7fb280820bfce2f662ea0c657673ba45c25` |
| `coax/tests/nostr/rsa_verify.py` | `2f676b016d5455efe2bcf23ea92519e9281e90fe3eb78f6ebe4ce4533dd29c7a` |
| `coax/tests/nostr/rsa_verify_wasm.c` | `f8178499327bb662af63088081e0edbaf7d2dc81e2348a9358f73a36c27c15db` |
| `coax/tests/nostr/rsa_verify_wasm.cjs` | `3cf3936a598375989844d5bff89058e68acab02363427e722ca7dea203e4f582` |

The C2SP Wycheproof license is retained in `WYCHEPROOF-LICENSE.txt` (SHA-256 `58d1e17ffe5109a7ae296caafcadfdbe6a7d176f0bc4ab01e12a689b0499d8bd`). The C2SP Wycheproof corpus commit and the 2048/3072/4096 fixture hashes are pinned in
`PROTOTYPE-EVIDENCE.md`. The additional 8192-bit fixture has SHA-256 `86df4075b4ce5954726abfa00cc19ebe8b608d6d99094d5e123d6a3a86568bd9`. The Python harness checks those hashes, compares 773 in-profile and 258 out-of-profile 8192-bit
cases per mode with the independent Python integer exponentiation and exact
PKCS#1 v1.5/SHA-256 encoding oracle, and checks the in-profile vector labels; all 8192-bit cases refuse because the selected profile caps the modulus at 4096 bits. It also
compares 300 seeded Montgomery products per native mode against Python modular
arithmetic across 1, 2, 3, 63, 64, 65, 95, 96, 127 and 128 limbs. Both native
and Wasm checked/release modes passed all 1031 profile/oracle comparisons; native additionally
passed all 300 multiplication comparisons per mode. This used Apple clang
21.0.0 for native, Homebrew clang 23.1.4 for wasm32-wasip1, and Node 26.12.1.

| Mode | Generated C | Native library | Wasm |
| --- | --- | --- | --- |
| checked | `a9e416124e1592dec5226aad21e3484d19e7c5714edf96b649de542de1b246b2` | `3e2a5e58e1744aad0e4a573b77382169e37823f797c618eb56667ded3088df42` | `39a333b5cc56258267b502112cf7536af1376a2fba112adf545cc65e179f7429` |
| release | `d7d581176820d180c16442d5644296e98e7fb95f097ac508752cd9d775849c95` | `31afb75892cafc2a8c55b61691c559fa37402cc65a9e6a9029e12a380214c391` | `40b179ca2658dbb72548c17c9741eda5109c03590a94f1f135c25f81c6afdf25` |

The ordinary default-budget `coax test` passed 49/49 loaded checks; its real
2048-bit positive signature check used 14,762,333 steps. Default `coax mutate`
killed 1018/1018 mutants, with no equivalence marks or waivers. Neither
command's budget was increased or filtered. The fixed e=65537 is an interface
assumption; an eventual caller must require the exact JWK exponent and other
policy parameters. Message hashing inherits the SHA-256 input cap.

This is bounded public arithmetic evidence, not an independent security
audit, constant-time claim, deployed-target approval, completed JOSE parser,
NIP-42 proof, authenticated network effect, or end-to-end FI authorization.
