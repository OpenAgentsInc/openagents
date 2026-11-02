# Historical tool-source receipts

The source copies in this dated evidence directory are immutable observations,
not maintained runnable tools. They are now packaged in
[historical-tool-sources.tar.gz](historical-tool-sources.tar.gz), with their
original repository-relative paths. The archive contains **106 regular files**;
each extracted file was read back and matched against its original complete
byte count and SHA256 before the loose copy was retired. No source byte or
historical hash was rewritten.

- Archive: 285,843 bytes, SHA256
  `7c2805d78f8fd6dc7d151a81b584f69031ec83726b40bd02e9d046e509881ffd`.
- [Source index](historical-tool-sources.tar.gz.sources.tsv): original path,
  byte count, SHA256, separated by tabs.
- [Archive digest](historical-tool-sources.tar.gz.sha256).

Paths in older receipts continue to identify their historical source bytes.
Resolve a retired path through the index and archive, rather than interpreting
it as a maintained file in the current checkout. The previous handoff commit
`6e54d4065de33215323a1ca3a85a50d2b87bc129` also retains every loose source.
Other evidence payloads and their dated receipts remain in place.

The Coax tool verifies the exact archive member list, refuses nonregular
members, extracts into an owned temporary directory, and reads back every
file's complete size and digest. A restore destination must not already exist:

```shell
COAX=/qualified/coax coax/web/tool.sh aethyr/render/tools/evidence-sources.coax verify docs/verification/data/aethyr-ch11-2026-11-01/historical-tool-sources.tar.gz --restore /tmp/aethyr-historical-sources
```

The returned directory contains the original full paths beneath it. Restoration
does not execute the historical scripts or change the working checkout.
The pack operation uses the same complete readback before `--retire` can remove
the loose copies, and rechecks every live source immediately before retirement.

The path/digest policy's three authored groups pass, including all 256 filename
bytes; **85/85 root mutants are killed**, with no waivers. The explicit compiler
is `1fac3750bcffbb735ef26f4a065d16414b911068fd7d68bc3ca112565d2432aa`;
it is the older qualified compiler, not a matching-main compiler. The initial
pack experiment before strict member inspection passed; the first strict
inspection then refused because `proc_run` strips the final stdout newline.
That failed attempt retired no files. The corrected inspection accepts a final
complete member without requiring a trailing newline.
