# `@openagentsinc/fast-follow-spec`

Effect-native parser and deterministic identity tools for FastFollowSpec 0.1.
The package validates the ordered Markdown/JSON format, resolves stable source
and directive references, rejects repository path escape, and exposes separate
exact-document and canonical-intent SHA-256 digests.

```sh
fast-follow-spec validate FASTFOLLOW.md
fast-follow-spec digest FASTFOLLOW.md
fast-follow-spec projection FASTFOLLOW.md
fast-follow-spec init path/to/FASTFOLLOW.md --title "Learning program" --id project.fast_follow
```

`discoverFastFollow(start, repositoryRoot)` searches upward for the nearest
directory that contains `AGENTS.md` and resolves only the `FASTFOLLOW.md`
beside it. A nested scope replaces its parent; discovery never merges specs.

The frozen corpus under `fixtures/conformance/0.1` records one case for each
stable diagnostic. Corpus mutations are applied to `valid/minimal.md` by the
test suite so every invalid fixture remains small and reviewable.
