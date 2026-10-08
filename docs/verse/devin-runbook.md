# Devin runbook

This runbook shows you how to hand coding work to the Devin CLI on your own
computer through Coder, and how to supervise it. Coder talks to Devin over the
Agent Client Protocol (ACP) as `devin acp`, using the CLI's own sign-in. Coder
never calls Devin's cloud API and needs no Devin API key. For how the route
works inside, see [The Devin route](../coder/runtime/devin.md). For the
workshop agent, see the [Alice runbook](alice-runbook.md).

## Check that Devin is ready

1. Install the Devin CLI and sign in once, as you would to use it yourself:

   ```sh
   devin --version        # 3000.11.3 was verified on 2026-10-07
   devin auth login
   ```

   Coder finds `devin` through `DEVIN_BIN`, then `PATH`, then
   `~/.local/bin/devin`. It checks that the stored sign-in,
   `~/.local/share/devin/credentials.toml`, exists and isn't empty; it never
   reads it.

2. Check that Coder lists Devin as an enabled agent:

   ```sh
   openagents coder agents list        # look for "id": "devin-cli", "enabled": true
   openagents coder agents enable devin-cli   # if it's disabled
   ```

3. Run a read-only test:

   ```sh
   openagents coder delegate devin-cli \
     --task "Read-only: print the package name and version from crates/coder-new/Cargo.toml. Do not modify any files."
   ```

   On 2026-10-07 this answered `coder-new 1.0.0-rc.5` in about 8 seconds.

## Delegate work to Devin

### From the command line

```sh
openagents coder delegate devin-cli --task "TEXT" [--session ID]
```

- `--task` is the whole instruction. Write it like an issue: what to change,
  which files, the acceptance criteria, and the check to run.
- `--session ID` attaches the delegation to an existing Coder chat, so the
  work appears there as a child chat that you can follow.
- Devin runs in the current directory. Run the command from the checkout you
  want it to work in, ideally a worktree of its own.

### From a Coder chat

In Coder V1 (`coder`, or `coderdev` from this checkout), ask Coder to
delegate to Devin, for example: "Delegate this to Devin: …". Coder starts a
child chat for the delegation and shows Devin's steps, tool calls, and
answer as they stream.

### As the engine for queued tasks

To let the host start queued Coder tasks on Devin automatically, admit a
Devin route in the host's auto-start policy:

```sh
coder host autostart on --workspace openagents --full-access \
  --route devin:default --route codex:gpt-6-luna --probe-usage
coder host autostart show    # reports whether Devin is connected
```

- `devin:default` uses the model the Devin CLI chooses. `devin:MODEL` pins an
  exact model, such as `devin:swe-2-high`; `devin models` lists yours. The
  host refuses a turn whose session reports a different model.
- Routes run in order. If Devin refuses for capacity, the run moves to the
  next admitted route.

A follow-up on the same task reattaches Devin's earlier session, so Devin
keeps its own context.

## Choose how much Devin may do

Devin runs its own tools, so the grant's access chooses Devin's own mode:

| Access | Devin mode | What Devin can do |
| --- | --- | --- |
| Full (`--full-access`) | `bypass` | Every tool runs without asking, with your login shell's environment. Use it only in a worktree you're ready to throw away. |
| Boundary (default) | `accept-edits`, with `devin --sandbox acp` | Reads files and edits the workspace. Any command that needs permission is refused. |

In every mode the host removes environment variables named `*_API_KEY`,
`*_TOKEN`, and `*_SECRET` before it starts Devin. Devin still reaches its own
service over the network with its sign-in.

### As Alice's engine

```sh
openagents agent engine alice devin            # or devin:MODEL
openagents agent show alice                    # prints "Devin connected"
```

With `engine` set to `devin`, Alice's task-mode work is delegated by Coder
to the `devin-cli` subagent the same way `codex` delegates to Codex: Coder
plans and checks, and Devin edits in her worktree. Questions and read-only
lookups stay with Coder. The chat's access picks Devin's mode the same way
the route's grant does: `bypass` under full access, `accept-edits` under
`devin --sandbox` on the boundary. A Devin refusal is booked under `devin`
in the capacity book and she falls back to Codex, then to Coder's own
model. See [The Devin engine](alice-runbook.md#the-devin-engine).

## Supervise and review

- **Follow it.** A delegation from a chat shows as a child chat in Coder. A
  queued task's transcript records every Devin reply, thought, tool call,
  plan step, and permission answer.
- **Stop it.** Cancel the Coder task or the chat turn. The host stops Devin's
  whole process group and records the cleanup.
- **Review before it lands.** Devin edits files but doesn't push. Review its
  diff in the worktree (`git diff`), run the tests the task named, then commit
  and push yourself.
- **Usage.** Devin reports tokens; the task's result records them. The Devin
  CLI reports no typed usage limit, so a capacity refusal is held for 30
  minutes in the capacity book (`capacity.json`, provider `devin`).

## Troubleshoot

| Symptom | Cause and fix |
| --- | --- |
| `session/new` refused | The Devin CLI isn't signed in. Run `devin auth login`. |
| Devin isn't in `coder agents list`, or is disabled | Install the CLI so `devin` is on `PATH`, then `openagents coder agents enable devin-cli`. |
| The turn ends `no_capacity` | Devin refused for a limit. It's held for 30 minutes; the run uses the next route, if any. |
| A queued task never starts on Devin | Auto-start is off, or no `devin:` route is admitted. Check `coder host autostart show`. |
| Devin can't run tests or builds | Boundary access refuses commands that need permission. Use full access in a disposable worktree, or run the checks yourself. |
| The turn is refused before the prompt | A pinned `devin:MODEL` doesn't match the session's model. Use `devin:default` or a model from `devin models`. |

## Quick reference

| Task | Command |
| --- | --- |
| Check the CLI | `devin --version`, `devin auth login` |
| List or enable agents | `openagents coder agents list`, `openagents coder agents enable devin-cli` |
| Delegate once | `openagents coder delegate devin-cli --task "TEXT"` |
| Attach to a chat | `openagents coder delegate devin-cli --task "TEXT" --session ID` |
| Auto-start on Devin | `coder host autostart on --workspace NAME --route devin:default` |
| Check auto-start | `coder host autostart show` |
| List Devin's models | `devin models` |
