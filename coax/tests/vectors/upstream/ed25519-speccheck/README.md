# ed25519-speccheck

This repository generates and uses test vectors for Ed25519 signature scheme to check edge cases
in the implementation of the algorithm. Namely, we test bounds checks on points
and scalars involved in a signature, along with cofactored vs. cofactorless verification.

We hope this helps outline the measures needed to implement the FIPS 186-5 and
RFC 8032 standards rigorously. For more information, read our [paper])(https://eprint.iacr.org/2020/1244, published
[at SSR'20](https://ssr2020.mozilla.org/accepted-papers).

You can run this utility with `RUST_LOG=debug cargo run` to get a list of the
test vectors and their inteded test conditions.

# Usage

To print out details on the test cases, use `RUST_LOG=debug cargo run`.

To generate files with test cases, `cases.json` and `cases.txt`, use `cargo run`.

To run the scripts on the connected libraries, execute the `./run.sh` script at
the root of the project (some additional installations of the associated libraries might be required).

## Condition table

Those are the cases we considered, with the index of the test vectors when applicable:

```
 ------------------------------------------------------------------------------------------------------------
|  |    msg |    sig |  S        | A ord | R ord | cof-ed | cof-less |        comment                        |
|------------------------------------------------------------------------------------------------------------|
| 0| ..22b6 | ..0000 | S = 0     | small | small |    V   |    V     | small A and R                         |
| 1| ..2e79 | ..ac04 | 0 < S < L | small | mixed |    V   |    V     | small A only                          |
| 2| ..b9ab | ..260e | 0 < S < L | mixed | small |    V   |    V     | small R only                          |
| 3| ..2e79 | ..d009 | 0 < S < L | mixed | mixed |    V   |    V     | succeeds unless full-order is checked |
| 4| ..f56c | ..1a09 | 0 < S < L | mixed | mixed |    V   |    X     |                                       |
| 5| ..f56c | ..7405 | 0 < S < L | mixed |   L   |    V*  |    X     | fails cofactored iff (8h) prereduced  |
| 6| ..ec40 | ..a514 | S > L     |   L   |   L   |    V   |    V     | S out of bounds                       |
| 7| ..ec40 | ..8c22 | S >> L    |   L   |   L   |    V   |    V     | S out of bounds                       |
| 8| ..8b41 | ..5f0f | 0 < S < L | mixed | small*|    V   |    V     | non-canonical R, reduced for hash     |
| 9| ..8b41 | ..4908 | 0 < S < L | mixed | small*|    V   |    V     | non-canonical R, not reduced for hash |
|10| ..155b | ..ac04 | 0 < S < L | small*| mixed |    V   |    V     | non-canonical A, reduced for hash     |
|11| ..c06f | ..ac04 | 0 < S < L | small*| mixed |    V   |    V     | non-canonical A, not reduced for hash |
 ------------------------------------------------------------------------------------------------------------
```

Here "mixed" means with a strictly positive torsion component but not small,
i.e. "mixed" and "small" are mutually exclusive. Out of the eight test cases
above, only some are concretely testable:

Vectors 0-2 have either small A or small R, or both.

Vector 3 has A and R mixed and succeeds in both cofactored and cofactorless.

Vector 4 has A and R mixed, succeeds in cofactored and fails cofactorless. This vector is the main indicator for a cofactored verification equation.

Besides small components, we also test:

- a large S > L (prepared to pass cofactorless and cofactored) (vectors 6, 7,
  where vector 7 contains an S so large it can't have a canonical serialization
  with a null high bit).
- a "pre-reduced" scalar (vector 5), namely one that fails if the verification equation is
  `[8] R + [8 k] A = [8 s] B` rather than the recommended `[8] (R + k A) = [8] sB`.
  (which passes cofactored, without pre-reduction).
- a non-canonical representation of R (vectors 8 & 9).
- a non-canonical representation of A (vectors 10 & 11).

For a total of 12 test vectors.

## Verified libraries

- [Apple CryptoKit](https://developer.apple.com/documentation/cryptokit) : in `scripts/ed25519-ios`
- BoringSSL, through [Ring](https://github.com/briansmith/ring) : in unit tests
- [Bouncy Castle (Java)](https://www.bouncycastle.org/java.html) version 1.66 : in `scripts/ed25519-java`
- [Dalek](https://github.com/dalek-cryptography/ed25519-dalek) : in unit tests
- [ed25519-donna from Signal](https://github.com/signalapp/libsignal-protocol-c.git): in `scripts/ed25519-signal-donna`
- [ed25519-java](https://github.com/str4d/ed25519-java) version 0.3.0 : in `scripts/ed25519-java`
- [Go-ed25519](https://golang.org/pkg/crypto/ed25519/) : in `scripts/ed25519_test.go`
- [libra-crypto](https://github.com/libra/libra/tree/master/crypto/crypto) : in unit tests
- LibSodium, through [pynacl](https://github.com/pyca/pynacl) : in `scripts/pynacl_test.py`
- nCipher's ed25519, by Rob Starkey
- [npm's ed25519](https://www.npmjs.com/package/ed25519) : in `scripts/eddsa_test`
- [OpenSSL](https://github.com/openssl/openssl) : in `scripts openssl_3/test_script.sh`
- [Pyca](https://cryptography.io/en/latest/) using OpenSSL 1.1.1g as default backend : in `scripts/pyca-openssl`
- [python-ed25519](https://github.com/warner/python-ed25519)) : in `scripts/python-ed25519`
- [ref10 from SUPERCOP through Python bindings](https://github.com/warner/python-ed25519) : in `scripts/python-ed25519.py`
- [tweetnacl](https://www.npmjs.com/package/tweetnacl) version 1.0.3 : in `scripts/tweetnacl`
- [Zebra](https://github.com/ZcashFoundation/ed25519-zebra) : in unit tests

## Results

```
 ---------------------------------------------------------------
|Library        | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10| 11|
|---------------+---+---+---+---+---+---+---+---+---+---+---+---|
|BoringSSL      | V | V | V | V | X | X | X | X | X | X | X | V |
|BouncyCastle   | V | V | V | V | X | X | X | X | X | X | X | X |
|CryptoKit      | V | V | V | V | X | X | X | X | X | X | X | V |
|Dalek          | V | V | V | V | X | X | X | X | X | X | X | V |
|Dalek strict   | X | X | X | V | X | X | X | X | X | X | X | X |
|ed25519-donna  | V | V | V | V | X | X | V | X | X | X | X | V |
|ed25519-java   | V | V | V | V | X | X | V | V | X | X | V | X |
|Go             | V | V | V | V | X | X | X | X | X | X | X | V |
|libra-crypto   | X | X | X | V | X | X | X | X | X | X | X | X |
|LibSodium      | X | X | X | V | X | X | X | X | X | X | X | X |
|npm            | V | V | V | V | X | X | X | X | X | X | X | V |
|OpenSSL-3.0    | V | V | V | V | X | X | X | X | X | X | X | V |
|PyCA           | V | V | V | V | X | X | X | X | X | X | X | V |
|python-ed25519 | V | V | V | V | X | X | V | V | X | X | X | V |
|ref10          | V | V | V | V | X | X | V | X | X | X | X | V |
|TweetNaCl-js   | V | V | V | V | X | X | V | V | X | X | X | V |
|Zebra          | V | V | V | V | V | V | X | X | X | V | V | V |
 ---------------------------------------------------------------
```

Contributors
------------

The authors of this code are Kostas Chalkias ([@kchalkias](https://github.com/kchalkias)), François Garillot ([@huitseeker](https://github.com/huitseeker)) and Valeria Nikolaenko ([@valerini](https://github.com/valerini)).  To learn more about contributing to this project, [see this document](./CONTRIBUTING.md).

#### Acknowledgments

Special thanks go to Yolan Romailler, Rajath Shanbag and Rob Starkey for contributing test
vector results.


License
-------

This project is [Apache 2.0 licensed](./LICENSE).
