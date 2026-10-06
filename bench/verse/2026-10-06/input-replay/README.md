# Operational input replay evidence

Issue [#10746](https://github.com/OpenAgentsInc/openagents/issues/10746)
implements V27 of the [Verse audit](../../../../docs/audits/2026-10-04-verse-engine-audit.md).
`checks.json` binds the source, executable, environment, and package results.
`sha256.json` binds every retained artifact. `input-replay.json.gz` contains the
complete canonical segment; compression changes no replay bytes.

The fixture owns two players in instance 700 and uses the actual world admission
and physics methods. It records 68 authority ticks, three logical commits,
pending and completed spellcasts, movement, jumps, controller handoffs, refused
commands, and shutdown. An admitted command's rule refusal still consumes its
sequence. Restoring a committed checkpoint discards its uncommitted future and
resets the host elapsed clock; the ordered history retains that discarded work.
A one-second catch-up batch runs three 30 Hz ticks and records 0.9 seconds of
dropped elapsed time. The new process reads the retained file, checks its
execution identity, and compares every observed checkpoint against a fresh run.

The execution profile records the native executable digest, pinned compiler,
target, Cargo build settings and features, rules, content, configuration,
operator-declared runtime, and RNG algorithm. The initial checkpoint and RNG
state are retained in the segment. This measures Linux x86-64 on one machine and
one exact executable. It does not establish cross-build or cross-platform bit
equality. `portable-check.log` checks compilation of the portable SDK for Wasm;
it does not measure browser replay.

`package-tests.log` records 613 passing world tests and three ignored benchmarks. Additional checks cover
primary and per-caster dice draws, movement intervals, admission refusals,
recording-budget rollback, a full segment's reserved shutdown, unsupported
profiles, altered chains, partial files, and a catch-up batch's first divergent
tick and field. `fixture.log` records a fresh-process replay of the retained
segment. The fixture and its assertions live in `replay/tests.rs`.

To generate a fresh accepted segment for the executable you build, run the
package check, then run its reported test executable with
`VERSE_INPUT_REPLAY_RECEIPT` set to an empty scratch directory and the exact test
`replay::tests::multiplayer_inputs_replay_across_commits_restore_and_process_shutdown`.
The fixture writes the segment, execution profile, and child report. A new build
has its own executable identity; it cannot claim to be the historical binary.
Use `gzip -dc input-replay.json.gz` to recover the retained canonical bytes.

This is a bounded local world diagnostic profile. Its checkpoint commits are
in-memory world boundaries, separate from realm durable revisions. The format
does not capture authentication, realm services, Studio, or asynchronous storage
workers automatically. Presentation trajectories and client prediction logs use
other contracts. Replay files can contain world state and inputs; the API creates
private files and a trusted operator controls retention. No load, frame-time,
or production crash-recovery claim follows from this correctness fixture.
