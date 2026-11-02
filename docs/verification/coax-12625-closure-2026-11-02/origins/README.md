# Origins mutation closure for #12625

This record covers `CB_variant`, `CB_leaf_count`, `CB_discover`, `CB_expr`,
and `CB_query` in the actual Coax borrow checker. It is a focused evaluator
policy result, not a native compiler, full desktop application, or whole
checker mutation claim. The parent closure record owns those other gates.

The initial origins checks and all six query-gap oracles were written before
the only production edit. Production then stayed frozen while the projection
cost fixture was corrected into a distinguishing two-block case. The new
`coax/tests/results-origin-policy.coax` has 12 groups and 42 cases. They
exercise admitted and refused payload metadata, exact sum depth, unit and
borrowed payloads, selected Result/optional origins, optional propagation,
write-only formal storage, direct owned storage, and the bounded work cost
of publishing reusable raw-to-class projections.

| Exact production procedure | Previous surviving sites | Final root sites | Killed | Survived |
| --- | ---: | ---: | ---: | ---: |
| `CB_variant` | 8 | 46 | 46 | 0 |
| `CB_leaf_count` | 5 | 74 | 74 | 0 |
| `CB_discover` | 3 | 132 | 132 | 0 |
| `CB_expr` | 12 | 226 | 226 | 0 |
| `CB_query` | Previously unrun; first complete inventory had 7 | 371 | 371 | 0 |

All five focused procedures have 849 sites killed and zero survivors. The final query result is a complete stock four-shard merge; every shard used one source identity and all source/compiler pins remained unchanged.

The first four bodies are unchanged. Their passing records retain their
original frozen dependency identities and were not rerun after the sibling
cache work. The final query scope binds the three final cache modules and
loads both sibling cache/projection suites, so its caller checks exercise
the coordinated helper contracts. In particular its strict decreasing
`q.next` validation remains unchanged; the query-index helper relies on it.

The complete pre-cleanup query inventory is retained as `CB_query-v3`:
372 mutants, 365 killed, 7 survived, stock merged from four shards. Six
survivors exposed missing semantic checks. The seventh mutated the dead
initial value `bucket:=0`: with query indexing disabled the bucket is never
read; with indexing enabled `CB_index_bucket` unconditionally overwrites it
before use. The approved one-line change to `bucket:int` preserves Coax's
zero default and removes that dead explicit initializer. No guard, helper,
quota, path bound, depth bound, work bound, or incomplete/known-local rule
was changed. `CB_query-before.coax` and `CB_query-final.coax` show the whole
procedure on each side.

The six semantic gaps are covered by independent endpoint behavior:
optional `none` propagation has no backing origin for either selected edge;
write-only queries require an indirect formal and otherwise refuse at the
correct source location; an address of owned local storage remains that
local despite descriptor assignments; and repeated accepted class lookups
fit a declared public fuel allowance after projection publication. These
assertions do not mirror branch text. The refusal helper zero/false values
are consumed by existing callers before a fatal barrier and therefore have
a compositional sentinel contract. No unspecified failed private cache
output or private workspace is asserted after rejection.

Every mutation command used stock `mutate --root-only`, moving exactly one
unchanged production procedure to the entry file and retaining every
baseline check. There are no equivalent markings, mutation filters,
mutator changes, or evaluator-budget changes. The current query initializer
is copied from actual production after the approved edit. The immutable
scope source archives contain every loaded file; `setup.json` and
`loaded-source.json` preserve original source and archive identities.
The producer is the existing qualified image at
`/tmp/coax-12625-native/final-v4/coax`, SHA-256
`710c4f5708d4c7ccbbe3207f89b9e9bb141860988ec9d98e71eca7a14712ac4b`.
The parent's rebuilt final image is a separate gate, not relabeled here.

The failed and canceled work is retained honestly: `CB_query-v1` hit its
840-second owned-process limit; `CB_query-v2` was canceled to bind the final
sibling modules; `CB_query-v3` is the complete failing inventory;
`CB_query-v4` is the checks-first snapshot, including its malformed unused
path-tail fixture. That fixture was corrected before the production edit.
The first leaf-depth attempt hit evaluator call-depth E0403 with a deep
borrowed payload; the final oracle uses a 256-aggregate owned payload and
proves admitted zero borrowed leaves without recursive leaf expansion.
`CB_query-v5` is the complete 371-site inventory that killed 370 and left the projection-publication call alive: its first fuel fixture had only one field block per layout and could not distinguish warm and cold routes. The corrected final fixture admits two real blocks in each layout, then exercises repeated accepted class lookups within the same public fuel allowance. Its original selected check passes, while removing only the publication call fails E0406 at the no-fatal/accepted-result assertion. Both controls are retained; the negative root uses the final scope's exact loaded dependency archive. No failed or canceled run counts as passing evidence.

To reproduce one archived scope, extract its `loaded-source.tar.gz` into a
fresh directory and run the stock compiler there. For each non-query scope:

```sh
coax mutate scope.coax --root-only --jobs 2 --evidence evidence.json
```

For the final query, use the same extracted entry file for all four shards:

```sh
coax mutate scope.coax --root-only --jobs 2 --shard 1/4 --evidence 1.json > 1.rec
coax mutate scope.coax --root-only --jobs 2 --shard 2/4 --evidence 2.json > 2.rec
coax mutate scope.coax --root-only --jobs 2 --shard 3/4 --evidence 3.json > 3.rec
coax mutate scope.coax --root-only --jobs 2 --shard 4/4 --evidence 4.json > 4.rec
coax mutate scope.coax --root-only --merge 1.rec --merge 2.rec --merge 3.rec --merge 4.rec --evidence merged.json
```

Each recorded command was bounded below the owner's 15-minute limit.
Raw stdout/stderr, launch/process receipts, source archives, evidence and
checks-first snapshots are retained under `scopes/` and `checks-first/`.
