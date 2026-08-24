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

## `delegate --child-model devin`, for a Devin fan-out

If `devin` is on `PATH`, `delegate` can run its children on it instead of on
opencode: pass `--child-model devin`, or `devin:<mode>` for a permission mode
other than the default `dangerous`.

Prefer this over running `devin` yourself through `shell`. A child started this
way is a fleet child like any other — it reports through the registry the
interface renders, it can be stopped with `ctrl+x`, it does not block the turn
that started it, and several run at once. Run through `shell`, the same work is
one call that freezes the session and shows nothing until it ends.

A Devin child brings its own credentials and its own model rather than spending
this session's grant, which is a different trust and billing boundary from an
opencode child. That is why the fleet names the agent.

Its print mode has no structured output, so a Devin child reports its answer
once at the end rather than streaming its tool calls the way an opencode child
does.

## The Devin CLI directly, for one task you will wait on

If `devin` is on `PATH`, it is another coding agent on this machine, and it can
take a task end to end rather than one prompt in parallel. Check with
`command -v devin`.

Run it non-interactively through `shell`:

```sh
devin -p "<a complete, self-contained task>"                        # read-only
devin -p "<a complete, self-contained task>" --permission-mode dangerous  # unattended
```

`-p` means one prompt, print the answer, exit. What it may do is the permission
mode, and the default is already the read-only one:

| `--permission-mode` | Auto-approves |
| --- | --- |
| `auto` (default, no flag needed) | read-only tools only |
| `accept-edits` | and edits inside the workspace |
| `smart` | and anything a fast model judges safe |
| `dangerous` | everything, including writes and shell |

For a read-only run, pass no flag. In `-p` mode there is nobody to answer a
prompt, so anything not auto-approved simply does not happen — the mode is the
whole of the boundary. Run `devin --help` if this disagrees with the build in
front of you; the values it lists are the values it takes.

Three things are worth knowing before an unattended run.

**`dangerous` is the mode name on this build.** The published documentation
calls the equivalent mode "bypass". Passing `--permission-mode bypass` is not
rejected — it is accepted and ignored, so the session silently falls back to
prompting, and a prompt nobody can answer is a task that does nothing.

**It refuses a workspace nobody has opened it in**, exiting at once with
`Refusing to run in an untrusted workspace`. Print mode cannot show the trust
prompt, so it fails rather than asking. Pass `--respect-workspace-trust false`
to skip the check, which is what print mode is for; the alternative is someone
starting `devin` interactively in that directory once. Only that exact message
means trust — do not read other failures as a trust problem.

**`dangerous` auto-approves every tool it has.** That is the point of using it
unattended, and it is the reason to say what you are handing over before you
hand it over. Give it a bounded task in this repository, not an open-ended one.

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
