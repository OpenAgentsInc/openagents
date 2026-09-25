# Reserved task selection for the next truthful-checks cohort

Status: environment feasibility only. No candidate generation is authorized by
this document alone; the full rule and population must be frozen first.

The completed 72-candidate archive study missed its declared joint improvement
bar. Its outcomes are now development data. The next study must retain that
negative result and cannot reuse its task groups as confirmation.

The [15 candidates](selection.txt) are unused archive tasks at upstream commit
`3b5caaa4863d64dda7f0957bf4fc2d4f019202d4`. A coderos job-configuration inventory
found no prior attempts for them. Selection used public instructions, environment
declarations, and Dockerfiles; no official graders, solutions, or outcomes were
read. This is a selected CPU artifact family, not a random sample of Terminal-Bench
or a TB4/Fable comparison. Related programming patterns remain possible across
tasks even when task identities differ.

These tasks have files under `/app` and public evidence for checking at least
some requirements. They include database recovery, source translation, HTML
sanitization, numeric code, file transformations, and interpretation. Some need
packages the initial image lacks. An unavailable test remains unknown; neither
package absence in a restored image nor a restricted review environment proves
a candidate defect.

Preflight builds the exact public Dockerfile, inventories initial files and
tools, and records image identities. It runs no agent or grader. Retain every
preflight failure. Before freezing the final population, exclude environments
that cannot build, require multiple services or a GPU, or whose initial `/app`
files already exceed the unchanged 128 MiB restoration bound. State each reason
and do not substitute tasks after observing agent outcomes.

Other inspected instructions were excluded for these public reasons:

- `chess-best-move`: the deciding input is an image.
- `custom-memory-heap-crash` and `fix-ocaml-gc`: large compiler builds and source
  trees make the current bounded snapshot/review pipeline unsuitable.
- `erp-procurement-planning`, `git-multibranch`, `nginx-request-logging`, and
  `pypi-server`: service state or artifacts outside `/app` are part of completion.
- `dna-assembly` and `protein-assembly`: specialized biological design, tools,
  or live external reference services extend beyond this artifact family.
- `polyglot-rust-c`: the just-opened cohort includes another Fibonacci polyglot.
- `sqlite-with-gcov`: the installed executable and toolchain are outside `/app`.
- `gpt2-codegolf`: model weights exceed the restoration bound.
- `mcmc-sampling-stan`: a long installation/sampling workflow and installed
  runtime exceed the current review capability.
- `winning-avg-corewars`: repeated stochastic simulation adds a distinct
  reliability and runtime problem that this component does not address.

The planned rule is literal artifact checks followed by the unchanged reproduced
defect detector, with the independently tested `owner-exec` environment profile.
No threshold search or generated expected constants are planned. The five CLI
controls and all opened-candidate replays must finish before protocol freeze.
The intended minimum is three attempts per task per executor, with Luna and
Astra reported separately and both existing checks retained as comparators.
Official outcomes stay unopened until all predictions are sealed on `main`.
Exact counts, bounds, costs, stopping rules, and uncertainty belong in the final
protocol, not an outcome-dependent amendment.
