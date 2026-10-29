# Experimental signing address admission

This Coax policy checks numeric 32/64-bit ranges and all pairwise intersections
of six signing regions: host/session, workspace, key, auxiliary bytes, message
and output. It bounds every region to 1 MiB, requires 32-byte key and aux and
at least 64 bytes of output, and treats an empty message as no access. It is
not a public pointer adapter: the host must establish exact live allocation
sizes, alignment, actual linear-memory extents, selected pointer width,
stable input identity and exclusive ownership. Numeric range arithmetic alone
cannot establish those facts or truthful wiping.

All 8 loaded Coax checks and 149/149 mutations passed without waivers. The
checked/release native and actual Wasm profiles each passed 12,000 cases
against an independent integer-range model covering aliases, widths and bounds;
the iOS Simulator target was compiled, not executed. Independent source review
of `4c39d59e2c0412dd961396379c9ff5170286c6027dbee37b12cee0259f48d477`
found no concrete numeric width/overlap blocker under those premises; it did not
rerun tests or approve a signer or production target.

Checked generated C SHA-256
`dcffe49afafccef758c482857cea99acf4b1464af8b900d4fb24a67357950814`,
Wasm `305ff2b3b9282139a8403d7871a33192cec265eb0961faaf9ff6e865a2914061`;
release C `4899b026d55ffc75863a163a01d1072f4283dee78ebf23e6585013fd77cc52aa`,
Wasm `305ff2b3b9282139a8403d7871a33192cec265eb0961faaf9ff6e865a2914061`.
Compiler: coax0 0.22.0, SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.
