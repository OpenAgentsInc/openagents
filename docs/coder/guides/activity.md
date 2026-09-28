# Check whether Coder has work in flight

`coder activity` tells you whether a running `coder` is in the middle of
work, so a script can avoid closing a window that would end it. On a
CoderOS desktop, the close key runs [`os/bin/coder-close`](../../../os/bin/coder-close),
which asks this command before it closes the focused window.

[Coder guides](README.md)

## What counts as work in flight

Two kinds of work count:

- A turn that is running, from the moment you submit a prompt until the reply
  finishes. The terminal and `coder -p` run the same turn, so both count.
- A delegation that has not reported: a task that Coder handed to an executor,
  locally or through a relay worker, that has not returned a result.

## Run the command

```sh
coder activity                        # every running coder on this computer
coder activity --pid 4121 --pid 4188  # only these processes
coder activity --json                 # one JSON object instead of a sentence
```

The command prints one sentence, for example:

```text
A turn is streaming, and 2 delegations have not reported.
```

When nothing is running, it prints `Nothing is running.`

With `--pid`, only the named processes count. Repeat `--pid` for each process.
A window's close key names the window's process and every descendant of it,
because the window is usually a terminal emulator with `coder` as its child.

With `--json`, the command prints an object with the fields `schema`
(`openagents.coder.activity.v1`), `active`, `turns`, `delegations`, and
`sentence`.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Work is in flight. |
| `1` | Nothing is running. |
| `64` | The command line was wrong. |

## Where the records live

While a conversation has work in flight, its process writes
`~/.openagents/activity/<pid>.json` and removes the file when the work ends and
when the process exits. To use a different directory, set
`CODER_ACTIVITY_DIR`. The directory has `0700` permissions and each file has
`0600` permissions. Each write goes to a temporary file first and then replaces
the record, so a reader never sees a partial record.

Only the terminal and `coder -p` write records. Other `coder` subcommands, such
as `coder host serve`, and other programs that use the `coder` crate write
none.

## Limitations

- A process that is killed cannot remove its own record. `coder activity`
  ignores and deletes a record whose process is no longer running. If the
  operating system reuses that process ID before the record is swept, the
  record can count as work in flight until the new process exits.
- A record that cannot be read counts as nothing running. The close key then
  closes the window, which is the behavior from before this command existed.

## The close key

`os/bin/coder-close` reads the focused window from `coder-desk focused`,
collects the window's process and its descendants, and runs
`coder activity --pid ...`. When the command exits `0`, the first press shows a
notice with the sentence, and a second press on the same window within five
seconds closes it. Every other result closes the window on the first press: an
idle window, a window without a process, a missing `coder`, or a `coder` that
fails. To change the confirmation time, set `CODER_CLOSE_SECONDS`.

Run the script's tests with `os/tests/coder-close.sh`.
