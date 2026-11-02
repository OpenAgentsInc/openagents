# Anime CLI pure policy final qualification

The exact entry and check snapshots in this directory passed all 207 baseline
checks. Eight sequential root-only mutation shards and the pinned compiler's
merge report establish **420 mutants, 420 killed, zero survivors**. No mutant
was waived. Active entry/check hashes were verified before shard 1 and again
after shard 8; neither changed during this qualification.

`qualification.json` records source, compiler, scope and per-shard results.
The native CLI/API/media journey has its own producer profile; this directory
qualifies the pure catalogue, request/context, reply holding, append and URL
policy. Dependency checks run in the baseline, while mutation is explicitly
scoped to the entry source. The two source snapshots are exact byte copies,
not a standalone dependency closure. The compiler records bind that closure
under fingerprint `28f0165cf6d2557b`.
`source-closure.sha256` identifies all 18 transitive entry/check/dependency
files. `compiler-artifacts.sha256` and `compiler-provenance.record` retain the
immutable producer's binary identity and existing public consistency record;
the compiler was neither refreshed nor rebuilt for this work.

Each shard ran from the repository root, with `I` set to 1 through 8:

```sh
/opt/homebrew/bin/timeout 900 nice -n 15 \
  /Users/christopherdavid/.cache/coax-release/6bd41d0f4b36ab8973042457b5f4a7c565c730fc88c1ab7ab6ac97bf6f814e9e/coax \
  mutate apps/openagents-cli/anime-rules.coax --library --root-only \
  --jobs 1 --shard I/8 --run-steps 250000000 \
  > /tmp/openagents-anime-cli.tOrdsu/zero-mutation-I-of-8.rec 2>&1
```

No extra baseline was rerun during the shards. Every mutation command retains
its required full baseline. Only one low-priority worker ran at a time, with
no concurrent team builds or media work. Every command finished within its
15-minute timeout; the longest shard took 241 seconds.

The pinned compiler then merged the eight records, exiting zero:

```sh
/opt/homebrew/bin/timeout 900 nice -n 15 \
  /Users/christopherdavid/.cache/coax-release/6bd41d0f4b36ab8973042457b5f4a7c565c730fc88c1ab7ab6ac97bf6f814e9e/coax \
  mutate apps/openagents-cli/anime-rules.coax --library --root-only \
  --jobs 1 --run-steps 250000000 \
  --merge /tmp/openagents-anime-cli.tOrdsu/zero-mutation-1-of-8.rec \
  --merge /tmp/openagents-anime-cli.tOrdsu/zero-mutation-2-of-8.rec \
  --merge /tmp/openagents-anime-cli.tOrdsu/zero-mutation-3-of-8.rec \
  --merge /tmp/openagents-anime-cli.tOrdsu/zero-mutation-4-of-8.rec \
  --merge /tmp/openagents-anime-cli.tOrdsu/zero-mutation-5-of-8.rec \
  --merge /tmp/openagents-anime-cli.tOrdsu/zero-mutation-6-of-8.rec \
  --merge /tmp/openagents-anime-cli.tOrdsu/zero-mutation-7-of-8.rec \
  --merge /tmp/openagents-anime-cli.tOrdsu/zero-mutation-8-of-8.rec \
  > /tmp/openagents-anime-cli.tOrdsu/zero-mutation-merged.log 2>&1
```

The initial stub checks failed before implementation; their receipt is retained
as `initial-stubs-red.log`. Those early stub source bytes were not retained,
so that receipt does not claim an exact historical source closure. The later
large opaque-confidence context regression has separate red and green receipts.
The exact pre-cleanup diagnostic source and its 13 survivors are preserved
separately in `../policy-diagnostic-v3/`, along with the cleanup boundary diff.
Those diagnostic records are not included in the final merge.
