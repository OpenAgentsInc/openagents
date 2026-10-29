# Secret-source audit checked-cast correction

The legacy aggregate, public-view and larger-closure aggregate profiles
accepted a secret-dependent checked narrowing cast. Coax emits a runtime
range check for that cast in both checked and release native C: a secret
outside the target range traps, so this is secret-dependent control flow.
The same behavior must not be treated as a secret-safe source audit result.
The scalar-only profile already refused this case; no broader assurance of
any profile follows from that observation.

The reproducer `candidate :: (x:u64)->u32 #export_c {return cast(u32)x;}`
had SHA-256 `b387a1826d1c8f0f8ae06015daa7cd0917b3aef74d23fbec6e058ab76a735ddb`.
All three aggregate profiles returned acceptance and a secret return mask.
The generated checked and release C shared SHA-256
`1221d97d7d05a188bdd90ee177d1ca0a44ddeb78a8783f3bed42a6a310f66536`
and invoked a range-checking helper. The captured legacy, public-view and
large reports had SHA-256 `b843b435bd1cc9d5c5b0311047ffdf1e29f7568a3b68e731e944146c6b0a0ec0`,
`8a42f67f6b60765bbc680ee7219a5c5d47e4916e60a65644b6e0ef26a81365f9` and
`606ed29238687dd1e32f2917072650f0ccf632cd0d79ec860d8090c11a63bbb5`,
respectively. Their aggregate source hashes were `11e2000298d16bf4ec5af507bfe3276a50d05a485aec2fc2da237cfaa625c73d`,
`a01cf12169831c2892d1dbaf0d36c0e408b71da388e2b0d18704c656c8535794` and
`ac29ff216df396078f995d62f9dab9bf360f238e66efe2342ea855ebad1a5018`.
Those hashes and prior reports are historical evidence, not approvals.
The [exact reproducer](data/secret-checked-cast-2026-10-29/reproducer.coax)
and the captured [legacy](data/secret-checked-cast-2026-10-29/legacy.json),
[public-view](data/secret-checked-cast-2026-10-29/public-view.json), and
[large](data/secret-checked-cast-2026-10-29/large.json) reports are retained.

The correction computes the full operand origin, including aliased memory,
before dropping its region and refuses any checked cast with a secret origin.
This is deliberately conservative: even secret checked widening or identity
casts refuse. Explicit truncating casts retain their original handling;
checked casts of public values may still trap publicly. The corrected legacy,
public-view and large aggregate source hashes are
`60bac74d7d1ad97304827b15681daad41d1335c9bf6ff44d4937f516018022a8`,
`de5a4c08961d2364f61d01151aa2ad33874549ef1102411dbfa72c899fa16225` and
`3b53041cbced584cdfcf0a3578943621bb7ef9da816b0aa418e1cfe6400fb73d`.
An independent source review found no concrete bypass of this narrow rule;
it did not review other effects, run target tests, or approve the analyzer.

The regression failed before the correction and the three corrected Coax
suites passed 192, 196 and 199 checks. Their ordinary mutation runs killed
2,789/2,789, 2,850/2,850 and 2,941/2,941 mutants, respectively. These are
the ordinary scheduler runs; full reference-mode mutation was still running
at this partial publication and is not claimed here. Separate tests cover
signed and unsigned runtime boundaries on checked/release native and WASI,
ordinary public and truncating-cast controls, secret views and aliased
regions, and explicit refusal of float and enum cast types. A secret signed
parameter refuses at type admission; it is not evidence that evaluation
reached that cast. Independently reviewed fixture SHA-256 values are
`b804f55faff6a8607e39e1de181f27eaa79e0bf31dd4afcb2c44762fe323d19a`
for the integer driver and
`5c0c12653c9a920a570dab1c5ba5848e122b4828c921ace6c99a7262f01944a7`
for the float/enum driver. These tests depend on the stated source, compiler,
bridge, loader and stable-filesystem premises; they are not target timing or
general auditor proof.
The [target results](data/secret-checked-cast-2026-10-29/corrected-target-results.json)
have SHA-256 `7e786f0e9493da2932f29036336302f06c1be4879c3854714c9988f740858cef`;
the [type refusals](data/secret-checked-cast-2026-10-29/type-refusals.log)
and ordinary mutation summaries are retained alongside them.

The earlier #4828 signer audit refused rather than accepted the signer. Its
work remains incomplete. Historical #71e7 private-root acceptance and
conditional whole-entry guard decisions are superseded and untrusted until
their exact source graphs, classifications, proof premises and corrected
tools are independently revalidated. On the reviewed current `ecdh_stage`
graph, both corrected public-view and large profiles now refuse at
`bytes.coax:108:28`, where a secret byte is widened by a checked cast. This
is a conservative analysis refusal, not evidence of an ECDH arithmetic or
cryptographic defect; the two earlier public guard records occur before the
refusal and are not a completed audit. Functional ECDH vectors and independent
integer observations retain their own stated scope; they do not repair the
source-audit defect. The historical machine-readable evidence remains intact.
