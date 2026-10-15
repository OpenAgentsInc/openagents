# Fix a failing test

A test fails and you want it fixed, with the cause explained and nothing
else changed. This guide takes you from the red test to a change you've
read and committed.

You need Coder installed and something that can answer (see
[Start here](/docs/start-here)).

## 1. See the failure yourself

Run the test once, so you know the exact command and what it says. For
example:

```sh
npm test -- login.test.ts
```

Or `cargo test login`, `pytest tests/test_login.py`, or whatever your
project uses. Commit or put aside any work in progress first, so the fix
is the only change in `git status`.

## 2. Ask Coder

Open Coder in the project and give it the command and what you know:

```sh
cd ~/code/my-project
coder
```

```text
npm test -- login.test.ts fails with "expected 200, got 401".
Run it, find out why, and fix the cause, not the test. Run it again
to show it passes, then run the whole suite.
```

The more exact you are, the less Coder has to guess: name the test, paste
the error, and say whether the test or the code is wrong if you know.

## 3. Follow along

Each step shows as a line: the files it reads, the searches it makes, its
edits with their diff, and the test runs with their output. Long test
output is cut down to its errors so the agent reads less; the full output
is saved to a file the step names.

- Press `Ctrl+O` to see every step in full.
- Press `Esc` to stop if it heads the wrong way. What it did so far stays,
  and you can type a correction and send it.
- To add something while it works, press `Ctrl+Enter` to queue it for
  after this turn (see [Queue messages while Coder works](/docs/queue-messages)).

## 4. Check the change

When Coder says the test passes:

```sh
git diff
npm test
```

Read the diff the way you'd read a teammate's. If it changed the test
instead of the code, or touched files it didn't need to, say so in the
chat ("put the test back and fix the handler instead").

To undo a turn's changes, type `/rewind` and pick the message. Coder puts
back the files that turn changed, cuts the chat to just before it, and
puts your message back in the input to edit and send again.

## 5. Commit

Commit as usual:

```sh
git commit -am "Fix login returning 401 for valid sessions"
```

Or ask Coder: "commit this with a message that says what was wrong."

## Tips

- **Give it the rules.** Coder reads your project's `AGENTS.md` and
  `CLAUDE.md` every turn. Put your test command and conventions there once
  ("run `pnpm test` before saying done").
- **Hand off a review.** Ask "have Codex review this change" and another
  agent reads it in its own conversation; its report comes back to your
  chat. See [Choose who answers](/docs/coder-agents).
- **A test that fails on GitHub?** [Solve an issue](/docs/coder-solver)
  takes a GitHub issue to a pull request.

Next: [Use your Claude Code or Codex](/docs/use-claude-code-or-codex).
