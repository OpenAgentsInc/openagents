# 46. Resident compilation and state migration

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This is roadmap R27.

`coax0` checks and emits a whole program on every run. Its resident
compiler, `coax serve`, keeps only the per-file work between runs ("What
exists" below). R27 adds a bounded cache built from content and declaration
dependencies, and a resident Mac host that can change a running program's
code and, when a layout changes, migrate its state. A migration either
publishes atomically or rolls back and keeps the old state. This follows the
body-replacement experiment in [chapter 45](ch45-body-replacement.md), and
only if that experiment showed value.

## What this chapter will cover

- A resident cache and how it is invalidated.
- The resident host's versioned C interface.
- Quiescing workers and callbacks, and replacing procedure tables by stable
  ID.
- Versioned migrations that publish or roll back.
- When old code and GPU resources are retired.

## Outline

1. **The resident cache.** Built from content and declaration dependencies.
   Invalidation is tested across targets, modules, descriptors, plugins,
   generics and build options, with the full build's output as the oracle.
   Safe analysis mode stays available.
2. **The host interface.** A versioned C interface. Persistent data, platform
   services and GPU resources stay owned by the host.
3. **Quiescing.** R15 workers and callbacks stop; no old frame runs.
   Procedure tables are replaced by stable ID, never by stored code address.
4. **Migration.** A layout change needs an explicit versioned migration:
   1. Allocate the new state.
   2. Migrate and validate it, reusing R16's snapshot inventory where it
      applies ([chapter 33](ch33-replay-and-snapshots.md)).
   3. Publish atomically, or roll back and keep the old state.
   Compatibility is never inferred from matching field names.
5. **Retiring resources.** Old code and GPU resources go only after their
   users and queues finish.
6. **Limits.** This is Mac development reload. Signed iOS device updates keep
   their signing and deployment rules. No stack migration or device
   code-loading bypass is implied.

## What exists: `coax serve`

`coax0` reuses work between compiles in two places. This section describes
them and what is still missing.

**The check cache** (#12647). A compile reuses each `#authority` check whose
inputs didn't change, keyed by the tokens of the declarations the check
reaches by name, the procedures it ran, the compiler, the options and the
storage it starts with (`ReuseIndex` in `coax/src/verify.cpp`; INVARIANTS.md,
"A reused check is one whose every input is unchanged").

**The resident compiler** (#13138, `coax/src/serve.cpp`). One long-lived
process for each compiler binary and user. You don't start it: the first
`coax c`, `coax check`, `coax test`, `coax lib`, `coax header`, `coax build`
or `coax deps` starts one and runs as before, and later commands run in it.

```console
$ coax check apps/openagents-web/fivelab.coax     # starts a resident, compiles cold
$ coax check apps/openagents-web/fivelab.coax     # runs in the resident, fills its memo
$ coax check apps/openagents-web/fivelab.coax     # reuses every unit's lex
$ coax serve status
coax serve: pid 40243, up 50 s, 3 requests (0 running), 294 unit lexes kept in 159 MiB ...
$ coax serve stop
```

How it works:

1. **A protocol.** A command connects to a local socket and sends its
   arguments, directory, environment and its three standard streams. The
   resident forks a child, and the child runs the command exactly as a new
   process would: the same code, writing to the caller's own streams, with
   the same exit status. The resident's directory is named by the compiler
   binary's content, and it compares a client's binary with its own bytes, so
   a resident never serves another compiler. A rebuilt compiler gets a new
   resident and the old one leaves.
2. **File identity.** Nothing is watched and nothing is trusted from a
   previous run. Each command reads every unit again and looks its bytes up
   in the memo. An entry is used only when the whole text is equal, so a hit
   is the value a cold compile would compute.
3. **What is kept warm.** For each unit, by its bytes: its tokens (the
   lexer's exact conversion of number literals is most of a large program's
   lexing), that it passed the unit syntax check, and the SHA-256 of its
   bytes and of its tokens, which the cache keys need. Also the compiler
   binary's SHA-256. The child that computed a value sends it to the
   resident when its command ends. The resident never runs a compiler phase,
   so a compile error or a crash in a request can't damage it.
4. **Bounded memory and a way back.** The memo holds at most 4 GiB
   (`COAX_SERVE_MEMORY_MB`), least recently used out first, and the resident
   leaves after 15 idle minutes (`COAX_SERVE_IDLE`). A command with no
   resident, a refusing one, a silent one or a dead one runs cold.
   `COAX_SERVE=0` turns residents off. `coax/tests/serve.coax` compares
   commands run both ways (stdout, stderr, exit status, C and header) on the
   corpora and on real units before and after edits.

Measured on fivelab (294 units, 9.7 MB of source, 1,314 checks) after a
comment appended to one zone file, as the first compile after the edit:
11.5 s before and 4.5 s in a resident on a 30-core build host, and 9.1 to
9.9 s before and 1.6 to 2.8 s on an 18-core Mac. A syntax error in the edited
file is reported in 0.17 s on the Mac, and a type error in 1.05 s.
`docs/verse/remote-builds.md` has the tables.

What a resident does not do yet: the parser's tree, the checker's typed
graph and the emitted C are made again for the whole program on every
compile. The parser already works per unit, so its trees could be kept. The
checker (`lib/compiler/checker/`) resolves names across the whole program in
one pass over one node store; it has to take a previous typed graph and the
set of changed declarations before a one-line edit can skip the unchanged
ones. Emitted C holds each trap's line in the composed program, so an edit
that adds a line changes the C of every later unit; per-declaration C needs
unit-relative lines first. Those two are the rest of R27's cache. On the
build host they are what is left of fivelab's 4.5 seconds: 1.7 s to check,
1.0 s to key its checks, 0.6 s to emit and 0.6 s to parse.

## The consumer and the gate

The consumer is a real world or controller and its shader. The gate:

- An edit preserves state while the code and shader change.
- Incompatible migrations are exercised, and rollback leaves the old state.
- The edit-to-observable-frame latency is measured and published.

## Done when

- R27 has shipped with the gate above.
- The chapter shows a migration and a rollback as exact transcripts.
- The chapter's examples run under `check-examples.sh`.
- Chapter 19's "Hot reload is an engine design" section links here and
  states what shipped.

## Dependencies

- R1 and R8 cache inputs, R3 ownership, R5 stable state identities and R15
  quiescence when workers exist.
- [Chapter 45](ch45-body-replacement.md), [chapter 19](ch19-builds-hot-reload.md)
  and [chapter 33](ch33-replay-and-snapshots.md).
