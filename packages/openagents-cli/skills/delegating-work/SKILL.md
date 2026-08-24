---
name: delegating-work
description: Choose between running a command yourself, fanning work out to child coding agents with delegate, and handing a whole task to another agent such as the Devin CLI. Use it before reaching for delegate, and whenever work looks like it needs more than one worker.
---

# Handing work to something else

Three ways, in order of what they cost. Pick the cheapest that does the job.

## Do it yourself

The `shell` tool runs a command here. Reading a file, listing a directory,
searching, `git`, a build, a test run: all of it is one call and one process.

This is almost always the right answer for a single command. Starting an agent
to run `pwd` costs minutes and real money and hands back an answer nobody
watched being produced.

## `delegate`, for work that splits

The `delegate` tool starts child coding agents that all run the same prompt in
parallel, in this repository, each with its own file and shell tools. It earns
its cost when the work genuinely splits into parts that do not depend on each
other: several files to change the same way, several hypotheses to check at
once, several test suites to run down.

Each child starts with no context from this conversation and cannot ask
questions, so the prompt has to carry everything. Every child gets the same
prompt and is told its own number separately — write for whichever child is
reading, rather than naming one.

## The Devin CLI, for a whole task

If `devin` is on `PATH`, it is another coding agent on this machine, and it can
take a task end to end rather than one prompt in parallel. Check with
`command -v devin`.

Run it non-interactively through `shell`:

```sh
devin -p "<a complete, self-contained task>" --permission-mode dangerous
```

Three things about that command are worth knowing before you run it.

**`dangerous` is the mode name on this build.** The published documentation
calls the equivalent mode "bypass". Passing `--permission-mode bypass` is not
rejected — it is accepted and ignored, so the session silently falls back to
prompting, and in `-p` mode a prompt nobody can answer is a task that does
nothing. Read `devin --help` if unsure; the values it lists are the values it
takes.

**It refuses a workspace it does not trust.** In an untrusted directory it
exits at once with `Refusing to run in an untrusted workspace`. Trust is
granted by starting `devin` interactively there once, which is something only
the person at the keyboard can do. If you hit that, say so and name the
directory rather than retrying.

**`dangerous` auto-approves every tool it has, including writes and shell.**
That is the point of using it unattended, and it is also the reason to say what
you are handing over before you hand it over. Give it a bounded task in this
repository, not an open-ended one.

There is also `devin acp`, an Agent Client Protocol server over stdio, for a
caller that speaks ACP. `-p` is the simpler route from here and needs no
protocol on this side.

## Which one

| The work | Use |
| --- | --- |
| One command, one answer | `shell` |
| The same thing to N independent parts, at once | `delegate` |
| A whole task you would otherwise do yourself, run by another agent | `devin -p` |

Whatever runs it, the result is yours to check. An agent reporting that it
finished is not evidence that it did; read the diff, run the test, look at the
output.
