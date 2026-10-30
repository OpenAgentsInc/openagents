# Project details line endings

A body without an Acceptance section used its original bytes after the parser
had normalized line endings for section detection. As a result, a lone CR was
lost by the page's printable-text conversion and joined neighboring lines.
The Coax page and the TypeScript snapshot now both preserve it as a newline.
This is a display fix, not verified event admission or relay authentication.

`coax/tests/nostr/project_details.py` exercises the actual Coax split and
prose output against a separately normalized TypeScript oracle. Before the
change, the checked native run failed on `first\rsecond`: it produced
`firstsecond` instead of `first\\nsecond`. Afterward, all cases passed on
checked and release native ASan/UBSan and Wasm. The fixture includes CR, CRLF,
LF, mixed line endings, an Acceptance section and escaped display characters.
`bun test coax/web/project/details.test.ts` passed all four tests.

The Coax tests used the provenance-verified 0.24.0 compiler with input
fingerprint `446e95f12113b11cdcd4c8c75e3561e01bfde3f5cd610aa5fa29ae25c1dcc366`
and binary SHA-256
`58d4b5585f3e1a7f819c7972d87828317723d5d4bdbec6133e002c2b093c5cf8`.
