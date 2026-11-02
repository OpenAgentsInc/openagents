# Policy diagnostic snapshot v3

This is an intermediate diagnostic, not the final policy qualification.
All eight sequential shards cover 437 root-source mutants: 424 were killed
and 13 survived. `diagnostic-merged.log` is the compiler's merged report.
No survivor was waived.

The exact entry and check source snapshots have SHA-256 identities:

- `anime-rules.coax.txt`: `b45e3aee858e459ba4c2dbb1b9344fcceed103b6a297b3e4aaec532be418a955`
- `anime-checks.coax.txt`: `260a60bdb88b0adf4affe735a0b7dd14879a141507a46deabcf6ed7c1b4cc845`

These snapshots were reconstructed only inside this evidence directory by
reversing the saved, exact `cleanup-boundary.patch` against copies of the
post-cleanup sources. Both reconstructed hashes match the identities captured
before shard 1 and verified again after shard 8. Active source was not reverted.
The `.txt` extension prevents the historical snapshots from joining a build.

Producer: immutable Coax 0.48 cache entry
`6bd41d0f4b36ab8973042457b5f4a7c565c730fc88c1ab7ab6ac97bf6f814e9e`,
compiler artifact SHA-256
`711959a2bf3104d860825f015c0630aa3f9a9e0701605d3f845b978e423a44b8`.
The shard records bind the imported source closure under their compiler
fingerprint `3bf4e6c9b5692344`; they run all 203 checks before mutation.
Historical dependency files are not reproduced here; these two snapshots and
the compiler records establish the entry/check boundary rather than claiming
that a checkout of this subdirectory is a standalone build.

Each original shard used the following command from the repository root,
substituting `I` with 1 through 8. Only one single-worker job ran at a time;
the timeout bounded each command at 15 minutes.

```sh
/opt/homebrew/bin/timeout 900 nice -n 15 \
  /Users/christopherdavid/.cache/coax-release/6bd41d0f4b36ab8973042457b5f4a7c565c730fc88c1ab7ab6ac97bf6f814e9e/coax \
  mutate apps/openagents-cli/anime-rules.coax --library --root-only \
  --jobs 1 --shard I/8 --run-steps 250000000 \
  > /tmp/openagents-anime-cli.tOrdsu/final-mutation-I-of-8.rec 2>&1
```

The same pinned producer merged all eight records with `mutate
apps/openagents-cli/anime-rules.coax --library --root-only --jobs 1
--run-steps 250000000`, followed by one `--merge RECORD` for each original
record in ascending shard order. Merge exited 1 because of the 13 survivors.

The misleading `final-mutation-*` original filenames are retained byte for
byte for traceability. They describe the pre-cleanup diagnostic only.
Subsequent source changes remove duplicated guards, express absent places as
sentinels, derive the padded encoder capacity, and add public-boundary checks.
Their final qualification must be recorded separately.
