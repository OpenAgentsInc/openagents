# Recover unchanged public files outside the workspace

The sealed fresh experiment is complete. Its 16 outcomes are opened. This change
uses those rows only for development and cannot improve the published untouched
confirmation result retroactively.

Six candidates were unavailable to the executable reviewer. Two math-grader
snapshots already contain `/paper` input files, but the restoration code supports
only `/app`. Add an explicit `--allow-public-files` mode that restores only the
outside regular files enumerated in the snapshot manifest. Reject unsafe paths,
links, special files, undeclared files, and missing declared files as before.

Do not mount or substitute those files. Before any review request, run
`sha256sum` on the same paths inside the pinned, networkless, read-only public
task image. Every hash and path must match the retained candidate snapshot.
Otherwise record unknown. This proves these outside inputs are unchanged public
image contents; it does not support modified outside artifacts or missing state.
Keep the one read-only `/app` mount and all existing container bounds.

Run only the two previously unavailable math candidates as development, under a
new record name. Retain input and image digests, outside-file hash comparisons,
every command and response, and the original unknown records. Keep the existing
review prompt, model, bounds, semantic questions, and 0.8 cutoff. Citation recovery
remains a separate recorded invocation, using the exact original observations.

Fixtures must reject undeclared paths, traversal, links, substituted file hashes,
and duplicate hash-output lines, and must admit the exact declared regular files.
No production policy changes, no new candidate generation, and no benchmark
outcomes used as assertions. Shadow-relay patches and changed installed VPP
sources still need separate attribution work; this mode cannot admit them.
