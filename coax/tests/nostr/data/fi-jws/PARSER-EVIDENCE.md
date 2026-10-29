# FI parser and JWS-signature prototypes (2026-10-28)

These unintegrated checks are not FI claim or identity authority. Compiler
binary SHA-256 `56f87661b9f1f105598132bf5c45f13e07fea46f4f23249921a22111d58f3ee4`;
see `../rsa/CURRENT-EVIDENCE.md` for its observed source revision and the
separate RSA evidence. All mutation runs use default budgets and no waivers.

| Source | SHA-256 | Coax checks | Mutants killed |
| --- | --- | --- | --- |
| `fi-base64url.coax` | `be749bd53ce734f6170847a791b3d0894b2e584e72ca4d7065b6a336f30a3d3b` | 5/5 | 202/202 |
| `fi-compact.coax` | `33cba54530fec124bf5187e931a02ff3c5f1a0a29f1d7f5b9bf01a77ee4f4427` | 8/8 | 267/267 |
| `fi-protected.coax` | `b5230480ce459eba2f0720c6bb59a782d2ee0c12ed1bbecf2a06bfe45b9c6970` | 23/23 | 1063/1063 |
| `fi-jwks-parse.coax` | `cf8f4409579654820ef5b43745a794bcd0da37a9054220d52f1db82f29ed1973` | 29/29 | 1395/1395 |
| `fi-jws-signature.coax` | `66102a4d7aec8ba3d0330d59e925647f1ca84fbe70ddec0c9b7f34a1361f5de0` | 85/85 | 2364/2364 |

The prior JWKS revision killed 1393/1393 mutants; the current revision preserves `key_ops` presence and killed 1395/1395 mutants.

The preceding JWS revision killed 2363/2364 mutants; the sole survivor was a combined decoder-status/length predicate. The current revision directly checks both conditions and adds changed-payload and oversize-header tests; its full default-budget mutation run killed 2364/2364 mutants with no exemptions.

All source paths above are under `coax/lib/nostr/block/`. Independent Python
oracles exercise base64url (520 cases), protected headers (506 cases) and JWKS
(392 cases) in both checked and release native and actual wasm32-wasip1. The
compact parser has Coax checks and mutation evidence, but no separate compact
oracle. A new OpenSSL-generated public-only fixture signs a `nip-fi+jwt`
protected header and arbitrary non-authoritative payload. The private test key
was discarded. The observed OpenSSL was 4.1.0 at `/opt/homebrew/bin/openssl`, binary SHA-256 `7687ff498451e22381f19ad10576f43257cdf9e4de5b9d219cfba5d7c9461526`. Native used Apple clang 21.0.0; Wasm used Homebrew clang 23.1.4 and Node 26.12.1 (V8 14.6.202.34-node.40). OpenSSL independently verifies the original fixture; the JWS
harness compares nine original/tampered/invalid cases against OpenSSL plus a
separate closed-header oracle in native and Wasm, both modes. Altering
semantically similar payload bytes refuses, exercising the original signing
span. The official RFC 7515 appendix A.2 RS256 example also passes the raw RSA primitive in each native/Wasm mode, while the FI header gate correctly refuses it for missing `typ` and `kid`. Its public-only extracted fixture `rfc7515-a2.json` has SHA-256 `aa767928f06159eb7fe6050ebee2d59f3d87492323303d9eec82ce1d3b155892`, from `https://www.rfc-editor.org/rfc/rfc7515.txt` SHA-256 `ecbf84674118e62526511c1237e1deaa86aada86d25b247623a2a7e04ae62e40`. These are finite vectors, not an audit or JOSE conformance claim. Checked and release ASan/UBSan also passed a native original, modified signature, invalid header, short token, short modulus and malformed modulus harness. Its source `fi_jws_sanitized.py` SHA-256 is `563416ac7a3c68ff4204b200695ed9dc874ccb9bdc9a429d353368a531fe345b`; checked executable SHA-256 was `72783d9e6b4fa9a2f63ddd2cfd09a0e48b9d34bcb611d9f47d97b718c818b521` and release executable SHA-256 was `5c208afd8b6299e5d6f37231b786a00cd3bb456fa157d4ede5f8fac20f8721aa`.

Test harness source hashes:

- `fi_base64url.py`: `5c70358e7c53ee751b39deac1bb5d2f638263bcd43f2ae122945cdf5a7d3993e`
- `fi_protected.py`: `b4cb2104fea771a33080e1b0eb0392691c1ef0c664278bb1ccd9e14dbcc0683a`
- `fi_jwks_parse.py`: `2c76b95c2fff69702aaca45135d5491dea1dc2c96481266685b73f082c537633`
- `fi_jws_signature.py`: `ce37231007e20dcbacb2897ff27ba5749a8b652c49911f3017fb530f8242c76c`

All test paths above are under `coax/tests/nostr/`. Fixture hashes:
`token.txt` `b923626e39cdf11a8f6dd2a799c487e21bd49cd196947be5394031e47b100eb0`;
`public.pem` `acd5948b81c890ad7a2bf04f9ac63a333e2bfb9435341acc37e4c75573bdfdb8`;
`modulus.hex` `d543077d28c8d9ea1cdef8920399335e0fdd4f9352d6690baccedc099e09a148`.

Additional test ABI sources and script hashes:

- base64url Wasm C `acab0b5a9cb37237d9ba7fdb7527ed29b8e9f211d81ab20364c4825bd13ac619`, JS `d142a446b9e3c2004397014defbe9028661379ee1bc59caaab6d19ffd3f09794`
- protected-header Wasm C `115abfcc5b0f2c6a12afebf0e5c18e8333ac4581fcf58b35c9a5c84c5f0b701a`, JS `3ccc3c3b0f9386bcfe6e6445c24bc127657cd2ef1d14ce433d9c3ae52173c4f8`
- JWKS Wasm C `a6a96a83f39692e38374dc78305d0abcd539809cc4c1eb99f06fabb52a0457b1`, JS `544991c5fe091493d780eefa1604f97a0f0d557ce535285df064e69201dcbfd2`
- JWS Wasm C `61930fc2cfaf22de16cf66a859a901015cd8cff9c4e4343b46579966731a32ea`, JS `ac6f0d9cee21a1c28bd68cb68d2cdc990d8c0abe0d804f903a597f227f1b482b`

Other observed generated-C / native-library / Wasm SHA-256 values:

- base64url checked: `5143b362d6e3c8a24939c4cb1ba2e9106050706e64ebc8afe4efb99a7bf96793`, `5972c83be4a9815b4115175d0ff949e7ca5d4f39d2efcef8bb85e9c056d58842`, `f0717414a4a99ba8efad35d16c47d2101c53741c45d1c946a080c47b3de49954`
- base64url release: `bc8c739c073a44f31735ed79a24030698316c58933c4a49dda3b51cedf18854e`, `e7d23996fc49c77b4402277a25bcd2677b9399489f1ad98c4f0888ab47387322`, `4b4e2a55f891ae953dbafde80f67f5d4122e6c422b75a6a486b833e5b9e093f2`
- protected checked: `86706d3cd2d31bf25b688e3b8d8f991251982ada15f67d7ca07e51a9c479855e`, `573b0b516495c1fab876cb4078294e557aae12e03ec6fe5104aabf392f8e102e`, `c93474f845c253eecb9aed5f876c0bf0934448ce6c3d5031150d66f525dbe3be`
- protected release: `7c3c71482828512dbfa0d09589d016bba7d349eed148c4dfcabe8dd8052957d4`, `d231567a53fcc195435a0f199047c83f90b06b8ed76efaed316fa777d4124a8f`, `6a425bf9d8a539c81dfc3f7db47af308d4701d049a7233e99d404067ea68e7c5`
- JWKS checked: `2cfece583a51399dff608c079508452e4f55e81d884071ce28120b8d12333931`, `30a5fde4efddff132d751185a1eb97f9a83fc553f08c50659c609cbc15a14cda`, `014396b070f15d6cb058026d16bd8f68723bdddedfb0edf12cb539c64aa06bd7`
- JWKS release: `54f7447b326e992abbdd99e6d8ad37342de0a5f3537c9ad4147eb1d6fe0fd093`, `0fb41486d6149ef325a160063c91d1e0966691a0dd4a9808becdf05e5995129e`, `5f3a8804ef488da94cfff895456f3d2f9e8e3c229ccf7dcd2eab4839e5a9885c`

The tested JWS generated-C / native-library / Wasm SHA-256 values were:

- checked: `c5c5d3ce798c6de10ada9368e76b775b272a9c7f67da083124c2e7425f3fcfd0`, `0d088a8a1725488ff158a3fe8dca53ccdcf07d1a6a2466b1dd6e136d265e914f`, `206ba1f39e9470210fe9f7dbdd3809e943939b9256791b8cc77fb97fb47643e5`
- release: `10553584d4902275c9e6b337e8a53a07ecfa08334123d1fc1b0f0d44878be924`, `7909f6f50798bcba524cd8bb83924f932407c1858331ca2c343825117251dc70`, `52b47b5079684c1427881fdc432f2928184949fcb55e348c63dae6999abb4cb5`

JWKS ASan/UBSan passed checked and release native builds for valid, duplicate, every truncated prefix and malformed-root cases. `fi_jwks_sanitized.py` SHA-256 is `00d2296388a09ac44ff4635edd71240f9a7458d7049f10e991ddf22f19e961ea`; checked executable SHA-256 was `24149f02163411b05aeabd93a590112a2520e029b68749aa14be6be65124f8b4`, and release executable SHA-256 was `ef60a8f77033e0e0f37c0f6cc92a439ca7f1987245c663965a8ea17f81c25e1b`.

The composed signature check currently caps the decoded protected header at 2048 bytes and hashes signing input only within the SHA-256 input cap. These are prototype bounds that must be included in any eventual policy identity. The decoder assumes disjoint source/output. The compact/header/JWKS inputs and
projections prove no trusted fetch, configured issuer, claim validity, valid
possession, policy identity, or session lifecycle. Browser network effects
still require an authenticated broker. `../rsa/FI-WORKER-HANDOFF.md` enumerates
the remaining integration and effect gates.
