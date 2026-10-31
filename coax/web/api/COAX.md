# Coax in the API service

The API (`api.c` and its handlers) is C, and Coax emits C, so Coax modules
link straight into it:

- `nip98.coax` is the API's NIP-98 check. `nostr.c` keeps the old
  `nip98_check` signature, so every route answers as it did, and calls
  `nip98_check_coax`. The decisions and refusal words are the former C check's
  (`nip98_reference.c`, kept for comparison only). `nip98_diff.c` compares the
  two on the signed corpus (`nip98_corpus.tsv`, from `nip98_fixtures.ts`) and
  on seeded random mutations. `test.sh` runs it on every API test.
- `mesh-policy.coax` is the mesh route policy (`mesh.c`).

The image build (`Dockerfile`) compiles these from source. Its first stage
bootstraps the Coax compiler from its own sources with the unchanged
`coax/build.sh`, checks included. It takes about six minutes on the 32-core
builder that `deploy.sh` asks for. Then it emits each module's C, and the
service is built from that C. The committed `*.generated.c` and `*.generated.h`
files are checked caches (`nip98.sh`, `mesh-policy.sh`) for local builds
without a compiler.

`coax_probe.coax` is an earlier parser probe and is not linked.
