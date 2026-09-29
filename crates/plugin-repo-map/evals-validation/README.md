# Project map: a second test set, by another key

This is the second, independent test set for Project map
(`crates/plugin-repo-map`), written for the first external validation of
a capability claim ([#9961](https://github.com/OpenAgentsInc/openagents/issues/9961);
[evaluation.md, "Checks, adoption, and credit"](../../../docs/extensions/evaluation.md#checks-adoption-and-credit)).
The starter test set (`evals/`) was released by the hosted runner's key,
which also released the tool, so a check on it proves reproducibility and
nothing about fit. This set is released under a different key
(`openagents ext eval release crates/plugin-repo-map --eval-dir
evals-validation --as validator`), after the tool's release, and names no
distribution of its own, so its claim is about the same distribution as
the tool's: the tasks a map of the project helps with.

It lives beside the starter set only because `openagents ext eval` reads
a test set from a plain directory under the extension's root; the
release, not the directory, says who wrote it. Nothing here was written
by the tool's author's key, and none of the tool's code was changed for
it. The tool's release was locked before these cases existed; the
chronology is on the relay.

## Sampling story

- **Frame**: questions a newcomer asks about a project's shape on their
  first day, answerable from a listing of its files and their sizes and
  not from any file's contents: how much is here, where the bulk of it
  is, how it is built, and where its tests are.
- **Inclusion**: one case per question shape the starter set does not
  already cover (the starter set asks for the largest file, the
  languages, the build files, and the test locations of one project
  each); each should-fire case has its own small fixture project with a
  fact only a listing turns up (a total, a directory's file count, a
  manifest's path, a test directory's path).
- **Exclusions**: questions the starter set already asks, questions that
  need a file's contents (the guest reads sizes and names, not text), and
  projects with more files than the hosted grant lets a run read.
- **Strata**: four should-fire cases (`file-count`, `busiest-directory`,
  `manifests`, `test-files`) and two should-not-fire cases
  (`rename-advice`, `commit-message`), the second stratum being requests
  where a map has no place: naming a function and writing a commit line.
- **Method**: constructed, by hand, from the guest's output fields
  (`files`, `bytes`, `dirs`, `manifests`, `tests`); the byte totals in
  the graders were computed from the fixture files as committed.

Every case asks for `read` and `write`, as the hosted grant allows, and
each should-fire grader reads the trajectory, where a program turn's run
summary lands, because a hosted run has no shell and the baseline arm
can't look at the files any other way.
