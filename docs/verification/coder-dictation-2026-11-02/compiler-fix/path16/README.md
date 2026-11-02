# Historical path16 / summary64 qualification

This preserves an intermediate compiler profile: sixteen projection steps, 64 distinct effects per procedure, 64 supplied-store terms and the unchanged 1,048,576 work budget. Later summary128 qualification belongs in a separate record.

Six focused projection groups and 104 retained-borrow groups passed. Four authored fault controls were killed. The native check-body driver passed 33 cases each at O0, O2 and under ASan/UBSan. The normal bootstrap completed with mutation disabled (`COAX_MUTATION=0`); this is not a full mutation claim.

The full compiler completed with 1,039,065 work steps, leaving 9,511 (0.907%) of the unchanged budget. Its LLDB phase log includes the successful process exit and emitted C; the matching output header is retained.

The desktop did **not** pass this profile. After the real generated-header ABI was corrected, `lab_pump` reached 64 exact distinct retained relations and refused the next relation. `effects64.json.gz` preserves those identities and the incoming pair: work was 372,508, so the refusal was the per-procedure capacity. The debugger was stopped after capturing it. This record makes no desktop launch, microphone or composer-delivery claim.

`source/` contains the historical checker closure. The checker was recovered from the archived `append-limit15` sensitivity copy by replacing its unique `path.count>=15` with `path.count>=BS_MAX_PATH`; the other modules were copied verbatim. `record.json` pins both source identities and explains the recovery. The native probe's absolute load path refers to that same historical checkout source, not the current moving file. Generated C/H are retained to preserve the actual tested outputs. See the adjacent [foundation proof](../foundation/README.md) for the independent path-kernel C oracle.

Large sources, headers and stdout use deterministic gzip with mtime zero. `manifest.json` pins encoded bytes and, for gzip files, decompressed bytes. Existing logs were packaged without rerunning checks. No binaries are included.
