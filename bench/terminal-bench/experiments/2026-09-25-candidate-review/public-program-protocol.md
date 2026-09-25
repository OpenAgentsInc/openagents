# Public behavior program experiment

The first frozen review union failed comparison: 4/6 failure calls were correct,
and it caught 4/60 failures against the scenario checks' 6/60. Preserve that
negative result. Neither its thresholds nor its runtime status change.

This next development arm generates executable checks **before reading any
candidate for a task**. The generator sees the public instruction and bounded
files from the task's `environment/` directory. It never reads root-level
`tests/`, `solution/`, `cheat/`, the task README, verifier results, or agent traces.
Public SQLite inputs are exported read-only as SQL. Initial implementation files
are identified as initial state, never presumed correct reference code.

A single Astra high request writes a bounded Python standard-library program.
The program accepts the retained final text files and original public inputs as
separate dictionaries, and returns observations for at most six cited public
requirements. A missing retained output, missing dependency, unsupported format,
timeout, crash, or invalid output is unknown. A passed partial check never means
the whole task passed. Each generated requirement must quote supplied public
text. Jev judges whether each check's failure condition follows from that text.

Execute programs in disposable Docker containers with no network, no host home,
read-only inputs and root filesystem, a writable temporary directory, one CPU,
512 MiB memory, a process limit, and a 20-second execution limit. Preserve the
program, request, reply, decision answers, exact inputs, stdout, stderr, exit
status, and elapsed time. Never import generated Python on the host. Test errors
do not establish candidate failures.

Generate and run the original 26 calibration tasks first, retaining all 132 rows
in recall denominators. For the combined failure signal, require an executed
failed check whose semantic-admission score reaches a cutoff selected from 0.5,
0.7, 0.8, 0.9, and 0.95. Choose maximum true failures subject to at least five
failure calls and 90% empirical precision; ties use the higher threshold. Report
the number of distinct detecting task groups. Do not use a per-task exception or
select individual programs by their correctness labels.

Freeze any qualifying rule before applying it to the 32 historical comparison
tasks. That comparison is now explicitly reused development validation after two
prior measurements. It is not pristine evidence. The eight fresh Microluna grades
remain unopened, and their predictions must be retained before joining grades.
Report every attempted arm and its costs; a negative outcome remains negative.
