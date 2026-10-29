# Experimental NFKC-to-scrypt host bridge

This internal bridge connects the pinned Unicode 18.0.0 Coax NFKC normalizer to
the experimental scrypt host. Numeric address admission checks that the full
host, workspace, KDF scratch, original input and salt are disjoint. A 1,024-byte
normalized-password output bounds the scrypt input; a valid raw input whose NFKC
expansion exceeds that capacity refuses. Malformed UTF-8 refuses. Once
normalization is attempted, its caller-owned workspace is erased after success
or refusal; if that erasure fails, status 2 requires quarantining the workspace
and any started job is cancelled for subsequent cleanup. Early null, active or
address-admission refusal occurs before touching or erasing pre-existing
workspace bytes. A failed start can still leave host cleanup pending.

The caller supplies stable, aligned, live allocations with truthful extents,
serializes all calls, prevents other writers, owns the original password and
must erase it separately. Address-range checks do not prove allocation/liveness
or actual Wasm linear-memory bounds. A dishonest or failed eraser is not proof
of erasure; status 2 explicitly reports an unresolved workspace. Stack/register
copies, snapshots, process crashes, secret-key custody and physical-device
behavior remain outside this test. This is not an app integration or approval to
use production secrets.

Checked/release native ASan/UBSan, actual Wasm and iOS 26.5 Simulator run the
normal and injected-failure fixtures. Compatibility and canonical spellings
of Å derive the same independently computed scrypt key; malformed and
oversized-normalized passwords refuse; workspace, scratch and guard bytes are
checked. Early alias and active-job refusals preserve pre-existing workspace
bytes. Injected workspace-wipe failure tests both a started job and a
normalization refusal, requiring quarantine and host cleanup as applicable.
These are controlled test effects, not claims of naturally occurring OS errors.

An additional checked/release iOS Simulator fixture runs N=65536, r=8, p=1
with the compatibility spelling against a separate hashlib.scrypt oracle,
wipes all 64 MiB, checks the entire workspace and scratch and preserves guards.
The fixture uses public values and returns early on test failure; it is not a
crash-cleanup guarantee. Native and Wasm do not run that 64 MiB combined fixture
here; their smaller full bridge and the separately tested large KDF remain
recorded in the earlier job/host evidence.

Independent source review covered bridge C SHA-256
`0329df944d47113ca71cd5a36c541c60c774eb4a9343152499a7ed00a85b47ed` and
header `83f1b5c74a85594e82325d91173a51fbeba10568b757806dad949d16936bb4a5`,
identified the early-refusal distinction, and independently recomputed the
large fixture's oracle. A follow-up fixture review prompted the explicit early-alias byte check; the final small fixture SHA-256 is `2ac304187cdc770e85ec2112c3b18fe3caafdacc41d3aedc457104efb8b2efee`. No production or target-security approval follows.

The normal/fault checked Wasm hashes were
`3fcbcb0391aed487976b4026e5defe46ce26adea1bba09b9c6db148858e42692` and
`8a7d95ab90bafb8511b3aee02be8ec702ff45b06acf65005b7ff58cd1780f611`;
release `d58a418d1243c75cd5030d3461e2175c52177c206c0c35e3472e71ff3cea3cc4`
and `86591f066e3fba5846e10169336ba46559f897ced5e807a3dfed08a43462e4ca`.
Checked/release full-size simulator binaries were
`b2f6b74f8f8c8db61becb29eec4429af8a8db4ed2721c25e10e632fe7f4daf42` and
`02dfce7e2771159b68a6add2af43bc929c54a6df10c14e9f3e77c00d511e6fb6`.

```sh
COAX_IOS_SIMULATOR=<booted-simulator-udid> COAX_IOS_LARGE=1 python3 coax/tests/nostr/scrypt_text_host.py
```

Python/Node are local test tools; normal runtime integration uses Coax and the
host C compiler without Python or host Unicode normalization.
