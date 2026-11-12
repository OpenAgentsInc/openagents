# Coax in the API service

## Where API rules go (#13321)

A new API rule is `#authority` Coax in a `*-policy.coax` file (`seats.c` + `seat-policy.coax` is the pattern). The policy module takes plain facts and returns the decision, verified with `#test`, `#forall`, and `coax mutate` with zero survivors.

SQL stores rows, guards versions and uniqueness, and writes the outbox; it does not decide policy. The C route file (`coax/web/api/*.c`) is a thin adapter that moves facts in and answers out, and performs host I/O.

A route whose rules live in `plpgsql` moves to a `*-policy.coax` file when it is next changed for another reason (and cites this file). Do not do a standalone sweep. `sql-rules-routes` in `coax/remote/counts.sh` counts `coax/web/api/*.c` files calling `LANGUAGE plpgsql` functions in `coax/web/api/schema.sql` so the count can only go down.


## How Coax links into the API

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
