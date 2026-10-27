# 18. Built for agents

> **Partly runs today.** Stable error codes, fix lines, `coax check` and
> readable C output work now. The formatter, query server and expansion
> traces are designed (spec §17.3).

Most of the code in the OpenAgents repositories is written by agents and
reviewed by people. A language designed in 2026 should assume that. It
changes what "good ergonomics" means.

## 18.1 What agents need from a language

- **One way to write things.** Agents copy patterns. When a language has
  five spellings for one idea, agents produce all five, and reviewers
  have to read all five. Coax keeps one declaration form, named struct
  fields, explicit conversions, and (soon) one canonical format printed
  by the compiler.
- **No hidden behavior.** No implicit conversions, no shadowing, no
  constructors, no exceptions, no operator overloads that allocate. What
  a line does is on the line.
- **Errors you can act on.** Stable codes to match on, a precise location,
  and a fix written as an edit (Chapter 4).
- **Fast, cheap checking.** `coax check` answers "is this right?" without
  linking or running anything. An agent can call it after every edit.
- **Sealed compile time.** `#run` code can't touch files, the network or
  the clock, so an agent asking the compiler a question can't
  accidentally trigger a build side effect (Chapter 9).
- **Pinned truth.** Agents learn a language from what they've read, and
  for a young language that's mostly outdated text. This book's examples
  are compiled by the compiler in the same repository, and when the book
  and the compiler disagree, the test fails.

## 18.2 The compiler as a service (design)

The compiler will run as a server and answer questions over a local
socket:

- What's the type of this expression?
- Where is this symbol used?
- What are this job's effects?
- What's this struct's layout on iOS?
- What did this macro expand to, and from which generator?
- What changed since the last build?

Agents should ask the compiler these questions instead of reading text
and guessing. An editor's analysis mode has to be safe: it never runs
privileged build plugins on its own, and when generated inputs are
missing, it says so instead of guessing symbols. Jai's experimental
language server shows why: when the build is programmable, "analyze this
file" can mean running parts of the build.

## 18.3 Provenance

Every generated declaration keeps the chain that produced it: the source
call, the generator, the generated node, the checked declaration and the
emitted C range. An error in generated code points at the call that
caused it first. Agents especially need this, because the alternative is
"fixing" generated code that will be regenerated over the fix.

## 18.4 Writing Coax well, as an agent

The rules, short enough to keep in context:

1. Run `coax check` after every edit, and fix the first error first.
2. Match on error codes, not messages.
3. Don't write a feature `coax0` refuses as "not supported yet". Restructure
   instead.
4. Sequence effectful calls into locals. `coax0` doesn't yet guarantee
   left-to-right evaluation of runtime operands.
5. Put computations that only need constants behind `#run`, and assert
   what they must produce with `#assert`.
6. Read the generated C (`coax c`) when performance or behavior matters.
7. Add a test: a `tests/pass` program with exact output, or a
   `tests/fail` program with the expected error code.
