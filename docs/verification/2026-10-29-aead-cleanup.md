# Experimental AEAD cleanup policy

This standalone Coax policy models publication after erasure of an AEAD
workspace, and erasure of the staged result after failure or cancellation.
Cancellation before publication makes success nonvisible. A first failure or
cancellation is sticky. After publication, the caller owns the resulting copy
and must erase both it and the retained staging buffer when appropriate.

The policy performs no cipher operation or wipe. A trusted host must own and
protect the policy state, perform the exact ordered observable wipes, serialize
cancellation/publication and prevent writers from modifying a wiped region.
A fabricated finished or pending state and a dishonest acknowledgment are not
evidence of erasure. Input/output ownership, pointer admission, inner cipher
stack copies, crashes, target timing and production-secret use are not approved.

Three Coax checks and the full 122/122 mutation gate passed; the scheduler ran
156 checks with no exclusions. An independent finite reference model exercised
bounded valid/invalid phases, reasons, regions, acknowledgments and publication
against generated checked/release native (ASan/UBSan) and Wasm code. The C
compiled for arm64 iOS Simulator; this is not an iOS execution. The first full
policy mutation pass found seven survivors and another found two; tests were
strengthened and an unobservable redundant region check was replaced by an
explicit valid-clean-state guard without waivers.

Independent source review of the earlier policy caught a fabricated
failure-clean state that could skip a staged-result wipe. The revised policy
rejects that state and the reviewer found no further concrete issue under the
trusted-host premise. Reviewed source SHA-256
`e5f098f786a72f1dd91a1ae882db2cbc40efc55b7049c20cc907767645d72108`.
Checked generated C SHA-256 `b6365b628b6c58928d2f3f43171891845e2b2b05fcd811c24415e6921952ef23`,
Wasm `4a107b64e5975d7aea0fef04a4e966bb0d68295c95d9e64024d2e99f3fd47663`;
release C `c525806a8daab6198b2b70c2430b1a5961e649972193518be0525799280a19c6`,
Wasm `d4f6d2982dbaf22513e4b302df8508d1669999d2929b627c02d5df16ea68e419`.
Compiler: coax0 0.22.0 SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.

```sh
coax/bin/coax test coax/lib/crypto/aead_cleanup.coax
coax/bin/coax mutate coax/lib/crypto/aead_cleanup.coax --mutation-stats
python3 coax/tests/nostr/aead_cleanup.py
```
